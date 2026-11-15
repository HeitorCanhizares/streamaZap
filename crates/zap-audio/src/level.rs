//! Medidor de nível: pico dos últimos `hold_us` microssegundos. Puro: o tempo vem de quem chama.

use std::collections::VecDeque;

pub struct PeakHold {
    hold_us: u64,
    /// Pares (instante em µs, pico), em ordem de chegada.
    window: VecDeque<(u64, f32)>,
}

impl PeakHold {
    pub fn new(hold_us: u64) -> Self {
        Self {
            hold_us,
            window: VecDeque::new(),
        }
    }

    /// Registra o pico de um bloco medido em `now_us`.
    pub fn push(&mut self, peak: f32, now_us: u64) {
        let peak = if !peak.is_finite() || peak < 0.0 { 0.0 } else { peak };
        // Descarta da frente o que já saiu da janela.
        while let Some(&(t, _)) = self.window.front() {
            if now_us.saturating_sub(t) >= self.hold_us {
                self.window.pop_front();
            } else {
                break;
            }
        }
        // Janela monotônica decrescente: picos menores ou iguais nunca mais serão o maior.
        while let Some(&(_, p)) = self.window.back() {
            if p <= peak {
                self.window.pop_back();
            } else {
                break;
            }
        }
        self.window.push_back((now_us, peak));
    }

    /// Maior pico dentro da janela que termina em `now_us`; 0.0 se não há nenhum.
    pub fn value(&self, now_us: u64) -> f32 {
        // `&self` não remove: o primeiro par ainda dentro da janela é o maior.
        self.window
            .iter()
            .find(|&&(t, _)| now_us.saturating_sub(t) < self.hold_us)
            .map_or(0.0, |&(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segura_o_pico_pela_janela() {
        let mut h = PeakHold::new(250_000);
        h.push(0.8, 0);
        assert_eq!(h.value(0), 0.8);
        assert_eq!(h.value(249_999), 0.8);
        assert_eq!(h.value(250_000), 0.0);
    }

    #[test]
    fn cai_para_o_proximo_maior() {
        let mut h = PeakHold::new(250_000);
        h.push(0.8, 0);
        h.push(0.3, 100_000);
        assert_eq!(h.value(100_000), 0.8);
        assert_eq!(h.value(260_000), 0.3);
        assert_eq!(h.value(360_000), 0.0);
    }

    #[test]
    fn pico_novo_maior_substitui() {
        let mut h = PeakHold::new(250_000);
        h.push(0.2, 0);
        h.push(0.9, 10_000);
        assert_eq!(h.value(10_000), 0.9);
    }

    #[test]
    fn vazio_da_zero() {
        let h = PeakHold::new(250_000);
        assert_eq!(h.value(0), 0.0);
        assert_eq!(h.value(1_000_000), 0.0);
    }

    #[test]
    fn nan_e_negativo_viram_zero() {
        let mut h = PeakHold::new(250_000);
        h.push(f32::NAN, 0);
        assert_eq!(h.value(0), 0.0);
        h.push(-0.5, 1);
        assert_eq!(h.value(1), 0.0);
        h.push(1.5, 2);
        assert_eq!(h.value(2), 1.5);
    }

    #[test]
    fn tempo_regredindo_nao_entra_em_panico() {
        let mut h = PeakHold::new(250_000);
        h.push(0.5, 1_000_000);
        h.push(0.4, 10);
        let _ = h.value(5);
        let _ = h.value(1_000_000);
    }

    #[test]
    fn hold_zero() {
        let mut h = PeakHold::new(0);
        h.push(0.7, 100);
        assert_eq!(h.value(100), 0.0);
        assert_eq!(h.value(0), 0.0);
    }
}
