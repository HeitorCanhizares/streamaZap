//! Captura do microfone em tempo real: escolha do modo e reserva (plano §4.2).

use crate::{AudioError, FRAME};
use crossbeam_channel::Sender;
#[cfg(windows)]
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MicOptions {
    /// Id do endpoint (`MicDevice::id`); `None` = padrão de comunicação.
    pub device_id: Option<String>,
    /// "Cancelar eco e ruído": tenta o Voice Capture DSP.
    pub processing: bool,
}

/// 20 ms mono 48 kHz.
pub struct MicBlock {
    pub pcm: Box<[f32; FRAME]>,
    /// `clock::now_us()` no fim do bloco.
    pub capture_us: u64,
    /// Pico do bloco, 0.0..=1.0.
    pub level: f32,
}

/// Aviso gravado quando o DSP de voz não abre e a captura cai para o som direto.
#[cfg(windows)]
const AVISO_RESERVA: &str = "Cancelamento de eco indisponível neste microfone: usando o som direto";

/// Thread "zap-mic" (MMCSS "Pro Audio").
pub struct MicCapture {
    #[cfg(windows)]
    inner: Option<std::thread::JoinHandle<()>>,
    #[cfg(windows)]
    stop: Arc<AtomicBool>,
    #[cfg(windows)]
    dsp_ativo: Arc<AtomicBool>,
    #[cfg(windows)]
    erros: Arc<Mutex<Vec<String>>>,
}

impl MicCapture {
    /// `processing` tenta o DSP e cai para a captura direta se ele falhar. A abertura acontece
    /// dentro da thread (dona do COM); `start` espera o resultado dela.
    ///
    /// Canal cheio: a thread descarta o bloco novo e conta; a regra "descarta o mais velho" fica
    /// com o consumidor, que drena o canal.
    #[cfg(windows)]
    pub fn start(opts: MicOptions, out: Sender<MicBlock>) -> Result<Self, AudioError> {
        let stop = Arc::new(AtomicBool::new(false));
        let dsp_ativo = Arc::new(AtomicBool::new(false));
        let erros = Arc::new(Mutex::new(Vec::new()));
        let (pronto_tx, pronto_rx) = crossbeam_channel::bounded::<Result<(), AudioError>>(1);

        let ctx = win::Ctx {
            opts,
            out,
            stop: stop.clone(),
            dsp_ativo: dsp_ativo.clone(),
            erros: erros.clone(),
        };
        let handle = std::thread::Builder::new()
            .name("zap-mic".into())
            .spawn(move || win::laco(ctx, pronto_tx))
            .map_err(|e| AudioError::Backend(format!("thread zap-mic: {e}")))?;

        match pronto_rx.recv() {
            Ok(Ok(())) => Ok(MicCapture {
                inner: Some(handle),
                stop,
                dsp_ativo,
                erros,
            }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                let _ = handle.join();
                Err(AudioError::Backend("thread zap-mic terminou na abertura".into()))
            }
        }
    }

    #[cfg(not(windows))]
    pub fn start(_opts: MicOptions, _out: Sender<MicBlock>) -> Result<Self, AudioError> {
        Err(AudioError::Unsupported)
    }

    /// O DSP está ativo de verdade.
    pub fn processing_active(&self) -> bool {
        #[cfg(windows)]
        return self.dsp_ativo.load(Ordering::Relaxed);
        #[cfg(not(windows))]
        false
    }

    /// Avisos e erros acumulados (reserva do DSP, microfone bloqueado...).
    pub fn errors(&self) -> Vec<String> {
        #[cfg(windows)]
        return self.erros.lock().map(|e| e.clone()).unwrap_or_default();
        #[cfg(not(windows))]
        Vec::new()
    }
}

#[cfg(windows)]
impl Drop for MicCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.inner.take() {
            let _ = h.join();
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::mic_dsp::DspCapture;
    use crate::mic_wasapi::DirectCapture;
    use crossbeam_channel::TrySendError;
    use std::time::Duration;
    use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW};

    pub(super) struct Ctx {
        pub opts: MicOptions,
        pub out: Sender<MicBlock>,
        pub stop: Arc<AtomicBool>,
        pub dsp_ativo: Arc<AtomicBool>,
        pub erros: Arc<Mutex<Vec<String>>>,
    }

    enum Fonte {
        Dsp(Box<DspCapture>),
        Direta(Box<DirectCapture>),
    }

    impl Fonte {
        fn next_block(&mut self, stop: &AtomicBool) -> Result<Option<Box<[f32; FRAME]>>, AudioError> {
            match self {
                Fonte::Dsp(d) => d.next_block(stop),
                Fonte::Direta(d) => d.next_block(stop),
            }
        }
    }

    struct Mmcss(Option<windows::Win32::Foundation::HANDLE>);

    impl Drop for Mmcss {
        fn drop(&mut self) {
            if let Some(h) = self.0 {
                // SAFETY: handle devolvido por AvSetMmThreadCharacteristicsW nesta mesma thread.
                unsafe {
                    let _ = AvRevertMmThreadCharacteristics(h);
                }
            }
        }
    }

    fn registrar(erros: &Mutex<Vec<String>>, texto: String) {
        if let Ok(mut e) = erros.lock() {
            e.push(texto);
        }
    }

    /// Abre a fonte conforme as opções, com reserva do DSP para a captura direta.
    fn abrir(ctx: &Ctx) -> Result<Fonte, AudioError> {
        let id = ctx.opts.device_id.as_deref();
        if ctx.opts.processing {
            match DspCapture::open(id) {
                Ok(d) => {
                    ctx.dsp_ativo.store(true, Ordering::Relaxed);
                    return Ok(Fonte::Dsp(Box::new(d)));
                }
                Err(AudioError::MicBlocked) => return Err(AudioError::MicBlocked),
                Err(e) => {
                    tracing::warn!(erro = %e, "microfone: DSP de voz indisponível, usando captura direta");
                    registrar(&ctx.erros, AVISO_RESERVA.to_string());
                }
            }
        }
        ctx.dsp_ativo.store(false, Ordering::Relaxed);
        DirectCapture::open(id).map(|d| Fonte::Direta(Box::new(d)))
    }

    /// Espera `ms` em fatias curtas, acordando se `stop` ligar.
    fn dormir(stop: &AtomicBool, ms: u64) {
        let mut restante = ms;
        while restante > 0 && !stop.load(Ordering::Relaxed) {
            let f = restante.min(20);
            std::thread::sleep(Duration::from_millis(f));
            restante -= f;
        }
    }

    pub(super) fn laco(ctx: Ctx, pronto: Sender<Result<(), AudioError>>) {
        let mut indice = 0u32;
        // SAFETY: nome constante terminado em nulo; `indice` é um ponteiro de saída válido.
        let mmcss = Mmcss(unsafe { AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut indice).ok() });
        if mmcss.0.is_none() {
            tracing::debug!("microfone: MMCSS indisponível, seguindo sem prioridade elevada");
        }

        let mut fonte = match abrir(&ctx) {
            Ok(f) => {
                let _ = pronto.send(Ok(()));
                f
            }
            Err(e) => {
                let _ = pronto.send(Err(e));
                return;
            }
        };

        let descartados = AtomicU64::new(0);
        let mut reaberto = false;
        loop {
            match fonte.next_block(&ctx.stop) {
                Ok(Some(pcm)) => {
                    let level = pcm.iter().fold(0.0f32, |m, v| m.max(v.abs())).min(1.0);
                    let bloco = MicBlock {
                        pcm,
                        capture_us: crate::clock::now_us(),
                        level,
                    };
                    match ctx.out.try_send(bloco) {
                        Ok(()) => {}
                        Err(TrySendError::Full(_)) => {
                            let n = descartados.fetch_add(1, Ordering::Relaxed) + 1;
                            if n % 100 == 0 {
                                tracing::warn!(total = n, "microfone: consumidor lento, blocos descartados");
                            }
                        }
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
                Ok(None) => return,
                Err(e) => {
                    tracing::warn!(erro = %e, "microfone: erro na captura");
                    registrar(&ctx.erros, e.to_string());
                    if reaberto {
                        return;
                    }
                    reaberto = true;
                    // Solta a fonte (e o COM dela) antes de reabrir.
                    drop(fonte);
                    dormir(&ctx.stop, 500);
                    if ctx.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    match abrir(&ctx) {
                        Ok(f) => fonte = f,
                        Err(e) => {
                            registrar(&ctx.erros, e.to_string());
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn nao_windows_devolve_unsupported() {
        use super::*;
        let (tx, _rx) = crossbeam_channel::bounded(8);
        let r = MicCapture::start(
            MicOptions {
                device_id: None,
                processing: false,
            },
            tx,
        );
        assert!(matches!(r, Err(AudioError::Unsupported)));
    }

    #[cfg(windows)]
    mod win {
        use super::super::*;
        use std::time::{Duration, Instant};

        fn opts(processing: bool) -> MicOptions {
            MicOptions {
                device_id: None,
                processing,
            }
        }

        /// Inicia; `None` (com aviso) se não houver microfone utilizável.
        fn iniciar(processing: bool, cap: usize) -> Option<(MicCapture, crossbeam_channel::Receiver<MicBlock>)> {
            if crate::list_microphones().map(|l| l.is_empty()).unwrap_or(true) {
                eprintln!("aviso: sem microfone, teste pulado");
                return None;
            }
            let (tx, rx) = crossbeam_channel::bounded(cap);
            match MicCapture::start(opts(processing), tx) {
                Ok(m) => Some((m, rx)),
                Err(AudioError::NoDevice | AudioError::MicBlocked) => {
                    eprintln!("aviso: sem microfone utilizável, teste pulado");
                    None
                }
                Err(e) => panic!("falha inesperada ao iniciar: {e}"),
            }
        }

        #[test]
        fn padrao_entrega_90_blocos_em_2s() {
            let Some((_m, rx)) = iniciar(false, 8) else { return };
            let ini = Instant::now();
            let mut instantes = Vec::new();
            let mut ultimo_us = 0u64;
            while ini.elapsed() < Duration::from_secs(2) {
                if let Ok(b) = rx.recv_timeout(Duration::from_millis(100)) {
                    assert!((0.0..=1.0).contains(&b.level));
                    assert!(b.capture_us >= ultimo_us, "capture_us não cresce");
                    ultimo_us = b.capture_us;
                    instantes.push(Instant::now());
                }
            }
            assert!(instantes.len() >= 90, "só {} blocos em 2 s", instantes.len());
            let regime = &instantes[5..];
            let total = regime[regime.len() - 1].duration_since(regime[0]);
            let medio = total.as_secs_f64() * 1000.0 / (regime.len() - 1) as f64;
            assert!((18.0..=22.0).contains(&medio), "período médio {medio:.2} ms");
        }

        #[test]
        fn processing_true_ativa_ou_avisa() {
            let Some((m, rx)) = iniciar(true, 8) else { return };
            let ini = Instant::now();
            while ini.elapsed() < Duration::from_secs(1) {
                let _ = rx.recv_timeout(Duration::from_millis(50));
            }
            assert!(
                m.processing_active() || m.errors().iter().any(|e| e == AVISO_RESERVA),
                "nem DSP ativo nem aviso de reserva: {:?}",
                m.errors()
            );
        }

        #[test]
        fn drop_encerra_thread() {
            let Some((m, rx)) = iniciar(false, 8) else { return };
            let ini = Instant::now();
            drop(m);
            assert!(ini.elapsed() < Duration::from_secs(1));
            while rx.try_recv().is_ok() {}
            assert!(matches!(
                rx.recv_timeout(Duration::from_millis(100)),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected)
            ));
        }

        #[test]
        fn consumidor_lento_nao_trava() {
            let Some((m, _rx)) = iniciar(false, 8) else { return };
            std::thread::sleep(Duration::from_secs(1));
            let ini = Instant::now();
            drop(m);
            assert!(ini.elapsed() < Duration::from_secs(1));
        }

        #[test]
        fn device_id_inexistente() {
            let (tx, _rx) = crossbeam_channel::bounded(8);
            let r = MicCapture::start(
                MicOptions {
                    device_id: Some("zzz".into()),
                    processing: false,
                },
                tx,
            );
            assert!(r.is_err());
        }
    }
}
