/// Resultado de `escolher_conversor`.
#[allow(dead_code)] // usado só no Windows (win_video) e nos testes
pub(crate) enum Escolhido<T> {
    Gpu(T),
    /// `motivo`: texto do erro que levou à CPU (`None` = a CPU foi pedida).
    Cpu {
        motivo: Option<String>,
    },
}

/// `Auto`: chama `tentar_gpu`; `Ok` -> `Gpu`; qualquer erro -> `Cpu` com o texto do erro.
/// `Gpu`: `Ok` -> `Gpu`; erro -> `Err` (sobe). `Cpu`: não chama `tentar_gpu`; `Cpu { motivo: None }`.
#[allow(dead_code)] // usado só no Windows (win_video) e nos testes
pub(crate) fn escolher_conversor<T>(
    modo: crate::ConversionSpec,
    tentar_gpu: impl FnOnce() -> Result<T, zap_gpu::GpuError>,
) -> Result<Escolhido<T>, zap_gpu::GpuError> {
    match modo {
        // Auto: qualquer falha do Video Processor (Unsupported, Os, ...) vira fallback para a CPU.
        crate::ConversionSpec::Auto => match tentar_gpu() {
            Ok(t) => Ok(Escolhido::Gpu(t)),
            Err(e) => Ok(Escolhido::Cpu {
                motivo: Some(e.to_string()),
            }),
        },
        // Gpu: nunca cai para a CPU; o erro sobe para quem chamou.
        crate::ConversionSpec::Gpu => tentar_gpu().map(Escolhido::Gpu),
        // Cpu: a GPU nem é tentada.
        crate::ConversionSpec::Cpu => Ok(Escolhido::Cpu { motivo: None }),
    }
}

/// Tamanho em bytes de um quadro sintético. Quadro não-chave: `kbps * 1000 / 8 / fps` bytes;
/// keyframe: 8x isso. Mínimo 16 bytes. `fps == 0` conta como 1.
pub fn synthetic_frame_size(kbps: u32, fps: u32, keyframe: bool) -> usize {
    // fps 0 não faz sentido; tratamos como 1 para não dividir por zero.
    let fps = u64::from(fps.max(1));
    let base = u64::from(kbps) * 1000 / 8 / fps;
    let size = if keyframe { base * 8 } else { base };
    // Mínimo de 16 bytes: quadro vazio ainda precisa de cabeçalho.
    usize::try_from(size.max(16)).unwrap_or(usize::MAX)
}

/// Decide quando uma camada deve receber o próximo quadro (intervalo de 1/fps em µs).
#[derive(Clone, Debug)]
pub struct LayerGate {
    interval_us: u64,
    next_us: Option<u64>,
}

impl LayerGate {
    pub fn new(fps: u32) -> Self {
        Self {
            interval_us: interval_for(fps),
            next_us: None,
        }
    }
    pub fn set_fps(&mut self, fps: u32) {
        // Só o intervalo muda; o próximo horário segue o relógio atual da camada.
        self.interval_us = interval_for(fps);
    }
    /// `true` se o quadro de `now_us` deve ser usado (e avança o relógio da camada).
    pub fn ready(&mut self, now_us: u64) -> bool {
        let interval = self.interval_us;
        let next = match self.next_us {
            // Primeiro quadro sempre passa.
            None => {
                self.next_us = Some(now_us.saturating_add(interval));
                return true;
            }
            Some(next) => next,
        };
        // Tolerância de 10% do intervalo: evita perder um quadro por jitter de tick.
        let tolerance = interval / 10;
        if now_us < next.saturating_sub(tolerance) {
            return false;
        }
        let mut next = next.saturating_add(interval);
        // Atrasou mais de um intervalo: re-ancora em vez de rajadas de quadros atrasados.
        if next.saturating_add(interval) <= now_us {
            next = now_us.saturating_add(interval);
        }
        self.next_us = Some(next);
        true
    }
    /// Esquece o próximo horário (camada acabou de ser ligada: o próximo quadro passa).
    pub fn reset(&mut self) {
        self.next_us = None;
    }
}

/// Intervalo em µs para `fps` quadros por segundo (fps 0 conta como 1).
fn interval_for(fps: u32) -> u64 {
    1_000_000 / u64::from(fps.max(1))
}

/// Contador de `seq` por camada.
#[derive(Clone, Debug, Default)]
pub struct SeqCounter {
    next: u32,
}

impl SeqCounter {
    pub fn new() -> Self {
        Self { next: 0 }
    }
    /// Devolve o atual e avança (com wrap).
    pub fn take(&mut self) -> u32 {
        let current = self.next;
        self.next = self.next.wrapping_add(1);
        current
    }
}

/// Janela das últimas N amostras (µs) para média e p95.
#[derive(Clone, Debug)]
pub struct Samples {
    buf: Vec<f64>,
    cap: usize,
    pos: usize,
}

impl Samples {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Vec::new(),
            cap: cap.max(1),
            pos: 0,
        }
    }
    pub fn push(&mut self, v: f64) {
        if self.buf.len() < self.cap {
            self.buf.push(v);
        } else {
            // Cheia: sobrescreve o mais antigo. `pos` só anda depois de encher, então
            // aponta sempre para a amostra mais velha.
            self.buf[self.pos] = v;
            self.pos = (self.pos + 1) % self.cap;
        }
    }
    pub fn avg(&self) -> f64 {
        if self.buf.is_empty() {
            return 0.0;
        }
        self.buf.iter().sum::<f64>() / self.buf.len() as f64
    }
    /// Percentil 95 (posição `ceil(0,95 * n) - 1` do vetor ordenado); 0 se vazio.
    pub fn p95(&self) -> f64 {
        if self.buf.is_empty() {
            return 0.0;
        }
        let mut sorted = self.buf.clone();
        sorted.sort_by(f64::total_cmp);
        let n = sorted.len();
        // ceil(0,95 * n) em inteiro, sem erro de arredondamento de ponto flutuante.
        let rank = (n * 95).div_ceil(100);
        sorted[rank.saturating_sub(1).min(n - 1)]
    }
    pub fn stage(&self) -> crate::StageStats {
        crate::StageStats {
            avg_us: self.avg(),
            p95_us: self.p95(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_nao_chave_segue_bitrate() {
        // 4000 kbps a 50 fps: 4_000_000 bits/s / 8 / 50 = 10_000 bytes.
        assert_eq!(synthetic_frame_size(4000, 50, false), 10_000);
    }

    #[test]
    fn synthetic_chave_8x() {
        assert_eq!(synthetic_frame_size(4000, 50, true), 80_000);
    }

    #[test]
    fn synthetic_minimo_16_e_fps_zero() {
        // Taxa ínfima cai no mínimo de 16 bytes.
        assert_eq!(synthetic_frame_size(0, 30, false), 16);
        assert_eq!(synthetic_frame_size(1, 1000, false), 16);
        // fps 0 conta como 1: 4000 kbps -> 500_000 bytes por quadro.
        assert_eq!(synthetic_frame_size(4000, 0, false), 500_000);
        assert_eq!(
            synthetic_frame_size(4000, 0, false),
            synthetic_frame_size(4000, 1, false)
        );
    }

    #[test]
    fn gate_60fps_em_tick_de_120hz_passa_um_a_cada_dois() {
        let mut gate = LayerGate::new(60);
        // Ticks de 120 Hz: 8333 µs entre eles, 10 ticks.
        let passes = (0..10u64).filter(|i| gate.ready(i * 8333)).count();
        assert_eq!(passes, 5);
        // Os que passam são os pares (0, 2, 4, ...).
        let mut gate = LayerGate::new(60);
        for i in 0..10u64 {
            assert_eq!(gate.ready(i * 8333), i % 2 == 0, "tick {i}");
        }
    }

    #[test]
    fn gate_primeiro_quadro_passa() {
        let mut gate = LayerGate::new(30);
        assert!(gate.ready(123_456));
        // Logo depois, sem esperar o intervalo, não passa.
        assert!(!gate.ready(123_457));
    }

    #[test]
    fn gate_atraso_grande_reancora() {
        let mut gate = LayerGate::new(60);
        assert!(gate.ready(0));
        // Muito atrasado (mais de um intervalo): passa e re-ancora no presente.
        let now = 1_000_000;
        assert!(gate.ready(now));
        // Um quadro logo depois não recupera o atraso em rajada.
        assert!(!gate.ready(now + 1000));
        // E o próximo passa no tempo certo a partir do presente.
        assert!(gate.ready(now + 16_666));
    }

    #[test]
    fn gate_reset_deixa_passar() {
        let mut gate = LayerGate::new(30);
        assert!(gate.ready(0));
        assert!(!gate.ready(1));
        gate.reset();
        assert!(gate.ready(2));
    }

    #[test]
    fn seq_incrementa_e_da_wrap() {
        let mut seq = SeqCounter::new();
        assert_eq!(seq.take(), 0);
        assert_eq!(seq.take(), 1);
        let mut seq = SeqCounter { next: u32::MAX };
        assert_eq!(seq.take(), u32::MAX);
        assert_eq!(seq.take(), 0);
    }

    #[test]
    fn samples_media_e_p95() {
        let mut s = Samples::new(100);
        for v in 1..=100 {
            s.push(f64::from(v));
        }
        assert!((s.avg() - 50.5).abs() < 1e-9);
        assert_eq!(s.p95(), 95.0);
        let st = s.stage();
        assert!((st.avg_us - 50.5).abs() < 1e-9);
        assert_eq!(st.p95_us, 95.0);
    }

    #[test]
    fn samples_janela_circular_descarta_antigos() {
        let mut s = Samples::new(3);
        for v in [1.0, 2.0, 3.0, 10.0, 20.0] {
            s.push(v);
        }
        // Janela deve ter só [3, 10, 20] (os dois primeiros foram descartados).
        assert!((s.avg() - 11.0).abs() < 1e-9);
        assert_eq!(s.p95(), 20.0);
    }

    #[test]
    fn samples_vazio_zero() {
        let s = Samples::new(10);
        assert_eq!(s.avg(), 0.0);
        assert_eq!(s.p95(), 0.0);
        assert_eq!(s.stage(), crate::StageStats::default());
    }
}

#[cfg(test)]
mod escolha {
    use super::*;
    use crate::ConversionSpec;
    use zap_gpu::GpuError;

    #[test]
    fn auto_com_ok_fica_na_gpu() {
        let r = escolher_conversor(ConversionSpec::Auto, || Ok::<u32, GpuError>(7));
        assert!(matches!(r, Ok(Escolhido::Gpu(7))));
    }

    #[test]
    fn auto_com_unsupported_cai_para_cpu() {
        let r = escolher_conversor(ConversionSpec::Auto, || Err::<u32, GpuError>(GpuError::Unsupported));
        match r {
            Ok(Escolhido::Cpu { motivo: Some(m) }) => assert!(!m.is_empty()),
            _ => panic!("esperava Cpu com motivo"),
        }
    }

    #[test]
    fn auto_com_os_cai_para_cpu() {
        let r = escolher_conversor(ConversionSpec::Auto, || {
            Err::<u32, GpuError>(GpuError::Os {
                context: "x",
                message: "y".into(),
            })
        });
        match r {
            Ok(Escolhido::Cpu { motivo: Some(m) }) => assert!(m.contains("y")),
            _ => panic!("esperava Cpu com motivo contendo o erro"),
        }
    }

    #[test]
    fn gpu_com_ok() {
        let r = escolher_conversor(ConversionSpec::Gpu, || Ok::<u32, GpuError>(3));
        assert!(matches!(r, Ok(Escolhido::Gpu(3))));
    }

    #[test]
    fn gpu_com_unsupported_sobe_erro() {
        let r = escolher_conversor(ConversionSpec::Gpu, || Err::<u32, GpuError>(GpuError::Unsupported));
        assert!(matches!(r, Err(GpuError::Unsupported)));
    }

    #[test]
    fn cpu_nao_chama_a_closure() {
        let chamou = std::cell::Cell::new(false);
        let r = escolher_conversor(ConversionSpec::Cpu, || {
            chamou.set(true);
            Ok::<u32, GpuError>(1)
        });
        assert!(!chamou.get());
        assert!(matches!(r, Ok(Escolhido::Cpu { motivo: None })));
    }
}
