//! Agenda de checagem de atualização, com relógio injetável (testável com relógio falso).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::updater::{ESPERA_INICIAL, INTERVALO_CHECAGEM, INTERVALO_FALHA, SORTEIO_MAX};

/// Relógio monotônico: tempo decorrido desde a abertura do app.
pub trait Relogio: Send + Sync {
    fn agora(&self) -> Duration;
}

/// Relógio de verdade (`Instant`).
pub struct RelogioSistema {
    inicio: Instant,
}

impl RelogioSistema {
    pub fn novo() -> Self {
        Self { inicio: Instant::now() }
    }
}

impl Default for RelogioSistema {
    fn default() -> Self {
        Self::novo()
    }
}

impl Relogio for RelogioSistema {
    fn agora(&self) -> Duration {
        self.inicio.elapsed()
    }
}

/// Quando é a próxima checagem. Puro: o tempo entra como parâmetro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Agenda {
    proxima: Duration,
}

impl Agenda {
    /// Primeira checagem em `agora + ESPERA_INICIAL` (10 s).
    pub fn nova(agora: Duration) -> Self {
        Self {
            proxima: agora + ESPERA_INICIAL,
        }
    }

    /// Instante agendado da próxima checagem.
    pub fn proxima(&self) -> Duration {
        self.proxima
    }

    /// `true` quando `agora >= proxima`.
    pub fn vencida(&self, agora: Duration) -> bool {
        agora >= self.proxima
    }

    /// Depois de uma checagem concluída: `proxima = agora + INTERVALO_CHECAGEM + min(sorteio, SORTEIO_MAX)`.
    pub fn reagendar(&mut self, agora: Duration, sorteio: Duration) {
        self.proxima = agora + INTERVALO_CHECAGEM + sorteio.min(SORTEIO_MAX);
    }

    /// Depois de uma checagem que falhou: `proxima = agora + INTERVALO_FALHA`.
    pub fn reagendar_falha(&mut self, agora: Duration) {
        self.proxima = agora + INTERVALO_FALHA;
    }
}

/// Sorteio de 0 a `SORTEIO_MAX` (sem dependência de `rand`: usa os nanossegundos do relógio do
/// sistema misturados). Nunca passa de `SORTEIO_MAX`.
pub fn sorteio_aleatorio() -> Duration {
    // Relógio do sistema antes de 1970 (ou erro) cai em zero: o sorteio só perde entropia.
    let (secs, nanos) = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), u64::from(d.subsec_nanos())),
        Err(_) => (0, 0),
    };
    let mut x = nanos ^ (secs << 17);
    x ^= x >> 13;
    x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    Duration::from_secs(x % (SORTEIO_MAX.as_secs() + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const SEIS_HORAS: Duration = Duration::from_secs(6 * 3600);

    /// Relógio controlado pelo teste (em milissegundos).
    struct RelogioFalso {
        t: AtomicU64,
    }

    impl RelogioFalso {
        fn novo() -> Self {
            Self { t: AtomicU64::new(0) }
        }

        fn avancar(&self, d: Duration) {
            self.t.fetch_add(d.as_millis() as u64, Ordering::SeqCst);
        }
    }

    impl Relogio for RelogioFalso {
        fn agora(&self) -> Duration {
            Duration::from_millis(self.t.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn nova_agenda_vence_em_10s() {
        let agenda = Agenda::nova(Duration::ZERO);
        assert!(!agenda.vencida(Duration::from_secs(9)));
        assert!(agenda.vencida(Duration::from_secs(10)));
    }

    #[test]
    fn agenda_de_6h_com_relogio_falso() {
        let relogio = RelogioFalso::novo();
        let mut agenda = Agenda::nova(relogio.agora());

        relogio.avancar(Duration::from_secs(10));
        assert!(agenda.vencida(relogio.agora()));

        let agora = relogio.agora();
        agenda.reagendar(agora, Duration::ZERO);
        assert!(!agenda.vencida(relogio.agora()));

        relogio.avancar(Duration::from_secs(5 * 3600 + 59 * 60));
        assert!(!agenda.vencida(relogio.agora()));

        relogio.avancar(Duration::from_secs(60));
        assert!(agenda.vencida(relogio.agora()));
    }

    #[test]
    fn sorteio_soma_ao_intervalo() {
        let mut agenda = Agenda::nova(Duration::ZERO);
        agenda.reagendar(Duration::ZERO, Duration::from_secs(120));
        assert_eq!(agenda.proxima(), SEIS_HORAS + Duration::from_secs(120));
    }

    #[test]
    fn sorteio_limitado_ao_teto() {
        let mut agenda = Agenda::nova(Duration::ZERO);
        agenda.reagendar(Duration::ZERO, Duration::from_secs(3600));
        assert_eq!(agenda.proxima(), SEIS_HORAS + Duration::from_secs(5 * 60));
    }

    #[test]
    fn falha_tenta_em_30_min() {
        let mut agenda = Agenda::nova(Duration::ZERO);
        let agora = Duration::from_secs(100);
        agenda.reagendar_falha(agora);
        assert_eq!(agenda.proxima(), agora + Duration::from_secs(30 * 60));
        assert!(!agenda.vencida(agora + Duration::from_secs(29 * 60)));
        assert!(agenda.vencida(agora + Duration::from_secs(30 * 60)));
    }

    #[test]
    fn sorteio_aleatorio_dentro_do_teto() {
        for _ in 0..200 {
            assert!(sorteio_aleatorio() <= SORTEIO_MAX);
        }
    }

    #[test]
    fn relogio_sistema_cresce() {
        let relogio = RelogioSistema::novo();
        let primeira = relogio.agora();
        let segunda = relogio.agora();
        assert!(segunda >= primeira);
    }
}
