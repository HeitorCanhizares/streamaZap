//! Simulador de enlace determinístico (fila FIFO, taxa variável, jitter, perda).
//!
//! Tudo é função do estado interno (seed) e das entradas (`now`, `bytes`): mesmo seed e mesma
//! sequência de chamadas produzem exatamente as mesmas saídas.

use crate::Us;

#[derive(Debug, Clone, Copy)]
pub struct LinkConfig {
    pub base_delay_ms: f64,
    /// Desvio padrão (normal truncada em >= 0).
    pub jitter_ms: f64,
    pub loss: f64,
    /// Fila drop-tail.
    pub queue_bytes: usize,
}

/// Estado do enlace. Um `Link` modela uma única direção (fila FIFO de um gargalo).
#[derive(Debug, Clone)]
pub struct Link {
    cfg: LinkConfig,
    rng: u64,
    /// Capacidade em bits/s, usada só por pacotes enviados depois de `set_capacity_kbps`.
    capacidade_bps: u64,
    /// Instante em que a fila esvazia (fim da serialização do último pacote aceito).
    busy_until_us: Us,
}

/// Constante usada quando o seed é 0: xorshift não pode ter estado zero.
const SEED_ZERO: u64 = 0x9E37_79B9_7F4A_7C15;
/// Taxa inicial do enlace: 10_000 kbps.
const CAPACIDADE_INICIAL_KBPS: u32 = 10_000;
/// Piso para u1 do Box-Muller, evitando ln(0).
const U1_MIN: f64 = 1e-12;

impl Link {
    pub fn new(cfg: LinkConfig, seed: u64) -> Self {
        let rng = if seed == 0 { SEED_ZERO } else { seed };
        Self {
            cfg,
            rng,
            capacidade_bps: u64::from(CAPACIDADE_INICIAL_KBPS) * 1000,
            busy_until_us: 0,
        }
    }

    /// Altera a taxa. Só afeta pacotes enviados depois; a serialização já agendada não muda.
    pub fn set_capacity_kbps(&mut self, now: Us, kbps: u32) {
        // `now` não entra no cálculo: a taxa vale para pacotes futuros, a fila atual é mantida.
        let _ = now;
        self.capacidade_bps = u64::from(kbps.max(1)) * 1000;
    }

    /// Envia `bytes` no instante `now`. Devolve o instante de chegada, ou `None` se o pacote
    /// foi perdido (perda aleatória ou fila cheia).
    ///
    /// Nota: o jitter pode reordenar pacotes. Por isso a chegada não é monotônica em relação às
    /// chegadas anteriores; quem precisar de ordem deve reordenar na recepção.
    pub fn send(&mut self, now: Us, bytes: u32) -> Option<Us> {
        // (1) Perda aleatória. O sorteio acontece sempre, para o consumo do PRNG não depender
        // de a perda ter ocorrido.
        if self.uniforme() < self.cfg.loss {
            return None;
        }

        // (2) Fila drop-tail: bytes ainda não serializados no instante `now`.
        let backlog_us = self.busy_until_us.saturating_sub(now);
        let backlog_bytes = backlog_us as f64 * self.capacidade_bps as f64 / 8.0 / 1e6;
        if backlog_bytes + f64::from(bytes) > self.cfg.queue_bytes as f64 {
            return None;
        }

        // (3) Serialização: começa quando o enlace fica livre.
        let inicio = now.max(self.busy_until_us);
        let serializacao_us = (f64::from(bytes) * 8.0 * 1e6 / self.capacidade_bps as f64).round();
        self.busy_until_us = inicio + serializacao_us as Us;

        // (4) Atraso fixo mais jitter truncado em >= 0 (normal com média 0).
        let base_us = self.cfg.base_delay_ms.max(0.0) * 1000.0;
        let jitter_us = self.normal(0.0, self.cfg.jitter_ms).max(0.0) * 1000.0;
        let atraso_us = (base_us + jitter_us).round() as Us;

        Some(self.busy_until_us + atraso_us)
    }

    /// Tempo de espera na fila (em us) para um pacote enviado agora.
    pub fn queue_delay_us(&self, now: Us) -> Us {
        self.busy_until_us.saturating_sub(now)
    }

    /// Próximo valor do xorshift64* (estado não-zero garantido pelo `new`).
    fn proximo(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniforme em [0, 1) com 53 bits.
    fn uniforme(&mut self) -> f64 {
        (self.proximo() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Normal por Box-Muller. Usa só um dos dois valores gerados, para que cada chamada consuma
    /// um número fixo de sorteios.
    fn normal(&mut self, media: f64, desvio: f64) -> f64 {
        let u1 = self.uniforme().max(U1_MIN);
        let u2 = self.uniforme();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        media + desvio * z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(base: f64, jitter: f64, loss: f64, queue: usize) -> LinkConfig {
        LinkConfig {
            base_delay_ms: base,
            jitter_ms: jitter,
            loss,
            queue_bytes: queue,
        }
    }

    #[test]
    fn deterministico() {
        let c = cfg(20.0, 5.0, 0.05, 100_000);
        let mut a = Link::new(c, 42);
        let mut b = Link::new(c, 42);
        for i in 0..1000u64 {
            let now = i * 700;
            assert_eq!(a.send(now, 1200), b.send(now, 1200));
        }
    }

    #[test]
    fn seed_zero_nao_trava() {
        let mut l = Link::new(cfg(0.0, 1.0, 0.0, 1_000_000), 0);
        let s: Vec<_> = (0..10).map(|i| l.send(i * 10_000, 100)).collect();
        assert!(s.iter().all(|x| x.is_some()));
        assert_ne!(l.rng, 0);
    }

    #[test]
    fn serializacao() {
        let mut l = Link::new(cfg(0.0, 0.0, 0.0, 1_000_000), 1);
        l.set_capacity_kbps(0, 1000);
        assert_eq!(l.send(0, 1250), Some(10_000));
    }

    #[test]
    fn fila_acumula() {
        let mut l = Link::new(cfg(0.0, 0.0, 0.0, 1_000_000), 1);
        l.set_capacity_kbps(0, 1000);
        for i in 1..=10u64 {
            assert_eq!(l.send(0, 1250), Some(i * 10_000));
        }
        assert_eq!(l.queue_delay_us(0), 100_000);
    }

    #[test]
    fn drop_tail() {
        let mut l = Link::new(cfg(0.0, 0.0, 0.0, 2000), 1);
        l.set_capacity_kbps(0, 1000);
        let a = l.send(0, 1250);
        let b = l.send(0, 1250);
        let c = l.send(0, 1250);
        assert!(a.is_some());
        // Backlog após o primeiro pacote: 1250 bytes. 1250 + 1250 > 2000, então o segundo cai.
        assert!(b.is_none());
        assert!(c.is_none());
    }

    #[test]
    fn perda_1pc() {
        let mut l = Link::new(cfg(0.0, 0.0, 0.01, 1_000_000), 7);
        let mut perdas = 0u32;
        for i in 0..100_000u64 {
            // Espaçamento de 2 ms: a fila (1250 bytes em 10 Mbps = 1 ms) esvazia entre envios.
            if l.send(i * 2_000, 1250).is_none() {
                perdas += 1;
            }
        }
        assert!((700..=1300).contains(&perdas), "perdas = {perdas}");
    }

    #[test]
    fn jitter_nao_negativo() {
        let mut l = Link::new(cfg(15.0, 8.0, 0.0, 10_000_000), 3);
        for i in 0..10_000u64 {
            let now = i * 5_000;
            let chegada = l.send(now, 200).expect("sem perda");
            assert!(chegada >= now + 15_000);
        }
    }

    #[test]
    fn capacidade_variavel() {
        let mut l = Link::new(cfg(0.0, 0.0, 0.0, 10_000_000), 9);
        l.set_capacity_kbps(0, 1000);
        // 1250 bytes a 1 Mbps: 10 ms.
        assert_eq!(l.send(0, 1250), Some(10_000));
        // Troca para 2 Mbps. O pacote já agendado mantém a fila; o novo usa a taxa nova:
        // busy = 10_000 + 5_000.
        l.set_capacity_kbps(0, 2000);
        assert_eq!(l.send(0, 1250), Some(15_000));
        // Taxa mínima: kbps 0 vira 1 kbps, então 125 bytes levam 1 s.
        l.set_capacity_kbps(0, 0);
        assert_eq!(l.send(20_000_000, 125), Some(21_000_000));
    }
}
