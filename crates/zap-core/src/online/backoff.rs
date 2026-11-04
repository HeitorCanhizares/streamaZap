//! Atrasos de reconexão.

use std::time::Duration;

/// 1 s, 2 s, 4 s, 8 s e depois 15 s para sempre (`reset` volta ao começo).
#[derive(Debug, Default, Clone)]
pub struct Backoff {
    pub(crate) attempt: u32,
}

impl Backoff {
    pub fn new() -> Self {
        Self::default()
    }
    /// Atraso da tentativa atual e avança.
    pub fn next_delay(&mut self) -> Duration {
        // Tabela fixa: depois da 4a tentativa o teto de 15 s não cresce mais.
        let secs = match self.attempt {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 15,
        };
        // saturating_add evita overflow em tentativas muito longas.
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_secs(secs)
    }
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequencia_1_2_4_8_15() {
        let mut b = Backoff::new();
        let esperado = [1, 2, 4, 8, 15, 15];
        for s in esperado {
            assert_eq!(b.next_delay(), Duration::from_secs(s));
        }
    }

    #[test]
    fn satura_em_15s() {
        let mut b = Backoff::new();
        let mut ultimo = Duration::ZERO;
        for _ in 0..100 {
            ultimo = b.next_delay();
        }
        assert_eq!(ultimo, Duration::from_secs(15));

        b.attempt = u32::MAX - 1;
        assert_eq!(b.next_delay(), Duration::from_secs(15));
        assert_eq!(b.next_delay(), Duration::from_secs(15));
        assert_eq!(b.attempt, u32::MAX);
    }

    #[test]
    fn reset_volta_ao_inicio() {
        let mut b = Backoff::new();
        b.next_delay();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.attempt, 0);
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }
}
