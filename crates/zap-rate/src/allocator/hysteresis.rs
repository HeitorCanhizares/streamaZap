//! Passo 3 do algoritmo: histerese por espectador.
#![allow(unused_variables, dead_code)]

use crate::Us;

/// Tempo que o alvo de subida precisa se sustentar antes de aplicar (3 s).
const ESPERA_SUBIDA: Us = 3_000_000;
/// Intervalo mínimo entre duas subidas efetivadas (5 s).
const INTERVALO_SUBIDA: Us = 5_000_000;
/// Tempo de banda baixa antes de descer sem sobrecarga (500 ms).
const ESPERA_DESCIDA: Us = 500_000;

#[derive(Debug, Clone, Default)]
pub struct Hysteresis {
    pub(crate) up_since: Option<Us>,
    pub(crate) last_up: Option<Us>,
    pub(crate) low_since: Option<Us>,
}

impl Hysteresis {
    pub fn new() -> Self {
        Self::default()
    }
    /// Decide a camada desejada final. `target` = alvo do `fill`, `current` = camada atual,
    /// `available_kbps` = banda disponível do espectador, `current_kbps` = kbps da camada atual,
    /// `overuse` = relatório em sobrecarga.
    pub fn decide(
        &mut self,
        now: Us,
        target: u8,
        current: u8,
        available_kbps: f64,
        current_kbps: f64,
        overuse: bool,
    ) -> u8 {
        // (a) Já está no alvo: nada pendente, zera os contadores.
        if target == current {
            self.up_since = None;
            self.low_since = None;
            return current;
        }

        // (b) Subir qualidade (número de camada menor). Exige o alvo sustentado por 3 s e
        // intervalo de 5 s desde a última subida efetivada. Sem descida pendente, então
        // `low_since` também é zerado.
        if target < current {
            self.low_since = None;
            let inicio = *self.up_since.get_or_insert(now);
            let sustentado = now.saturating_sub(inicio) >= ESPERA_SUBIDA;
            let espacada = self
                .last_up
                .is_none_or(|ultima| now.saturating_sub(ultima) >= INTERVALO_SUBIDA);
            if sustentado && espacada {
                self.last_up = Some(now);
                self.up_since = None;
                return target;
            }
            return current;
        }

        // (c) Descer qualidade (target > current). A subida pendente deixa de valer.
        self.up_since = None;

        // Sobrecarga reportada: desce na hora, sem esperar.
        if overuse {
            self.low_since = None;
            return target;
        }

        // Banda abaixo de 95% da camada atual: precisa persistir por 500 ms.
        if available_kbps < 0.95 * current_kbps {
            let inicio = *self.low_since.get_or_insert(now);
            if now.saturating_sub(inicio) >= ESPERA_DESCIDA {
                self.low_since = None;
                self.up_since = None;
                return target;
            }
            return current;
        }

        // Banda voltou: cancela a descida pendente.
        self.low_since = None;
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Us = 1_000;
    const S: Us = 1_000_000;

    /// Roda `decide` a cada 100 ms de `inicio` até `fim` (inclusive) e devolve o primeiro
    /// instante em que a camada mudou, se houver.
    #[allow(clippy::too_many_arguments)]
    fn primeira_mudanca(
        h: &mut Hysteresis,
        inicio: Us,
        fim: Us,
        target: u8,
        current: u8,
        available: f64,
        current_kbps: f64,
        overuse: bool,
    ) -> Option<Us> {
        let mut t = inicio;
        while t <= fim {
            if h.decide(t, target, current, available, current_kbps, overuse) != current {
                return Some(t);
            }
            t += 100 * MS;
        }
        None
    }

    #[test]
    fn nao_sobe_antes_de_3s() {
        let mut h = Hysteresis::new();
        let mut t = 0;
        while t < 3 * S {
            assert_eq!(h.decide(t, 0, 1, 10_000.0, 1_000.0, false), 1, "t={t}");
            t += 100 * MS;
        }
        assert_eq!(h.decide(3 * S, 0, 1, 10_000.0, 1_000.0, false), 0);
    }

    #[test]
    fn nao_sobe_duas_vezes_em_5s() {
        let mut h = Hysteresis::new();
        // Primeira subida efetivada em t=3s.
        assert_eq!(
            primeira_mudanca(&mut h, 0, 3 * S, 0, 1, 10_000.0, 1_000.0, false),
            Some(3 * S)
        );
        // Novo alvo melhor logo depois: só aplica em t>=8s (5s desde a última subida).
        let mudanca = primeira_mudanca(&mut h, 3 * S + 100 * MS, 10 * S, 0, 1, 10_000.0, 1_000.0, false);
        assert_eq!(mudanca, Some(8 * S));
    }

    #[test]
    fn desce_na_hora_com_overuse() {
        let mut h = Hysteresis::new();
        assert_eq!(h.decide(0, 2, 1, 0.0, 1_000.0, true), 2);
        assert_eq!(h.decide(1, 2, 1, 0.0, 1_000.0, true), 2);
    }

    #[test]
    fn desce_apos_500ms_de_banda_baixa() {
        let mut h = Hysteresis::new();
        // Banda 500 kbps < 0.95 * 1000: conta desde t=0.
        assert_eq!(h.decide(0, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(400 * MS, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(500 * MS, 2, 1, 500.0, 1_000.0, false), 2);
    }

    #[test]
    fn banda_volta_cancela_descida() {
        let mut h = Hysteresis::new();
        assert_eq!(h.decide(0, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(300 * MS, 2, 1, 999.0, 1_000.0, false), 1);
        // Nova queda: o relógio recomeça do zero.
        assert_eq!(h.decide(400 * MS, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(850 * MS, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(900 * MS, 2, 1, 500.0, 1_000.0, false), 2);
    }

    #[test]
    fn igual_zera_contadores() {
        let mut h = Hysteresis::new();
        // Deixa uma subida e uma descida pendentes.
        assert_eq!(h.decide(0, 0, 1, 10_000.0, 1_000.0, false), 1);
        assert_eq!(h.decide(0, 2, 1, 500.0, 1_000.0, false), 1);
        assert!(h.up_since.is_some() || h.low_since.is_some());
        // Alvo igual à atual limpa tudo.
        assert_eq!(h.decide(200 * MS, 1, 1, 500.0, 1_000.0, false), 1);
        assert!(h.up_since.is_none());
        assert!(h.low_since.is_none());
        // Depois disso a descida conta de novo a partir de t=1s, não de t=0.
        assert_eq!(h.decide(S, 2, 1, 500.0, 1_000.0, false), 1);
        assert_eq!(h.decide(S + 500 * MS, 2, 1, 500.0, 1_000.0, false), 2);
    }
}
