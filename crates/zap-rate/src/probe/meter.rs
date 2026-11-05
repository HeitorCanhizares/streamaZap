//! Medidor de grupos de sondas (tarefa rate-04).
#![allow(dead_code)]

use super::*;

/// Mede a taxa de chegada de cada grupo de sondas (lado de quem recebe).
#[derive(Debug, Clone, Default)]
pub struct ProbeMeter {
    cluster: u16,
    aberto: bool,
    primeira: Us,
    ultima: Us,
    n: u32,
    bytes: u64,
    bytes_pendentes: u64,
}
impl ProbeMeter {
    pub fn new() -> Self {
        Self::default()
    }
    /// Sonda chegou. `cluster` diferente do grupo aberto fecha o anterior e devolve o resultado dele.
    pub fn on_probe(&mut self, cluster: u16, arrival_us: Us, bytes: u32) -> Option<ProbeResult> {
        if self.aberto && self.cluster == cluster {
            self.ultima = self.ultima.max(arrival_us);
            self.n += 1;
            // Os bytes de mídia que vieram antes desta sonda entram junto com ela.
            self.bytes += self.bytes_pendentes + u64::from(bytes);
            self.bytes_pendentes = 0;
            return None;
        }
        let resultado = if self.aberto { self.fechar() } else { None };
        // Os bytes da primeira sonda só marcam o início do grupo, não contam.
        *self = Self {
            cluster,
            aberto: true,
            primeira: arrival_us,
            ultima: arrival_us,
            n: 1,
            bytes: 0,
            bytes_pendentes: 0,
        };
        resultado
    }
    /// Vídeo/áudio chegou: conta no grupo aberto (só entra no total quando chega uma sonda depois dele).
    pub fn on_media(&mut self, arrival_us: Us, bytes: u32) {
        if self.aberto && arrival_us >= self.primeira {
            self.bytes_pendentes += u64::from(bytes);
        }
    }
    /// Fecha o grupo se a última sonda foi há ≥ `FIM_GRUPO_US`.
    pub fn poll(&mut self, now: Us) -> Option<ProbeResult> {
        if self.aberto && now >= self.ultima.saturating_add(FIM_GRUPO_US) {
            return self.fechar();
        }
        None
    }
    fn fechar(&mut self) -> Option<ProbeResult> {
        self.aberto = false;
        // Mídia que chegou depois da última sonda nunca conta.
        self.bytes_pendentes = 0;
        let span = self.ultima.saturating_sub(self.primeira);
        if self.n < MIN_SONDAS || span < MIN_DURACAO_US {
            return None;
        }
        let kbps = (self.bytes as f64 * 8000.0 / span as f64).round() as u32;
        Some(ProbeResult {
            cluster: self.cluster,
            kbps,
            probes: self.n,
            span_us: span,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eventos (instante, é_sonda) de 200 ms: sondas a cada 2866 us e vídeo a cada 24 ms.
    fn gera(arredonda: Us) -> Vec<(Us, bool)> {
        let mut ev = Vec::new();
        let mut t = 0;
        while t <= 200_000 {
            ev.push((t, true));
            t += 2866;
        }
        let mut t = 1_000;
        while t <= 200_000 {
            ev.push((t, false));
            t += 24_000;
        }
        if arredonda > 1 {
            for e in ev.iter_mut() {
                e.0 = e.0 / arredonda * arredonda;
            }
        }
        ev.sort_by_key(|e| e.0);
        ev
    }

    fn mede(ev: &[(Us, bool)]) -> (ProbeMeter, Option<ProbeResult>) {
        let mut m = ProbeMeter::new();
        for &(t, sonda) in ev {
            if sonda {
                assert!(m.on_probe(1, t, 1200).is_none());
            } else {
                m.on_media(t, 1200);
            }
        }
        let fim = m.ultima + FIM_GRUPO_US;
        let r = m.poll(fim);
        (m, r)
    }

    #[test]
    fn meter_mede_rajada() {
        let (_, r) = mede(&gera(1));
        let r = r.expect("resultado");
        assert!((r.kbps as f64 - 3750.0).abs() <= 3750.0 * 0.05, "kbps {}", r.kbps);
        assert!(r.probes >= 60);
        assert!((r.span_us as i64 - 200_000).abs() < 5_000);
    }

    #[test]
    fn meter_chegada_em_degraus_de_15ms() {
        let (_, r) = mede(&gera(15_625));
        let r = r.expect("resultado");
        assert!((r.kbps as f64 - 3750.0).abs() <= 3750.0 * 0.10, "kbps {}", r.kbps);
    }

    #[test]
    fn meter_descarta_grupo_curto() {
        let mut m = ProbeMeter::new();
        for i in 0..4 {
            m.on_probe(1, i * 40_000, 1200);
        }
        assert!(m.poll(10_000_000).is_none());
        for i in 0..10 {
            m.on_probe(1, i * 8_000, 1200);
        }
        assert!(m.poll(10_000_000).is_none());
    }

    #[test]
    fn meter_fecha_por_cluster_novo() {
        let mut m = ProbeMeter::new();
        for i in 0..70 {
            m.on_probe(1, i * 2866, 1200);
        }
        let r = m.on_probe(2, 300_000, 1200).expect("fecha o cluster 1");
        assert_eq!(r.cluster, 1);
        assert_eq!(r.probes, 70);
        // O cluster 2 ficou aberto com uma sonda; nada a devolver ao fechar.
        assert!(m.poll(300_000 + FIM_GRUPO_US).is_none());
    }

    #[test]
    fn meter_fecha_por_silencio() {
        let mut m = ProbeMeter::new();
        for i in 0..70 {
            m.on_probe(1, i * 2866, 1200);
        }
        let ultima = 69 * 2866;
        assert!(m.poll(ultima + 49_999).is_none());
        assert!(m.poll(ultima + 50_000).is_some());
        assert!(m.poll(ultima + 100_000).is_none());
    }

    #[test]
    fn meter_midia_depois_da_ultima_nao_conta() {
        let mut a = ProbeMeter::new();
        let mut b = ProbeMeter::new();
        for i in 0..70 {
            a.on_probe(1, i * 2866, 1200);
            b.on_probe(1, i * 2866, 1200);
        }
        b.on_media(69 * 2866 + 10, 100_000);
        let ra = a.poll(1_000_000).expect("a");
        let rb = b.poll(1_000_000).expect("b");
        assert_eq!(ra.kbps, rb.kbps);
    }
}
