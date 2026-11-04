//! Janelas deslizantes: mínimo, máximo, quantil, taxa e perda.
#![allow(unused_variables, dead_code)]

use crate::Us;
use std::collections::VecDeque;

/// Mínimo numa janela de tempo (deque monotônica).
#[derive(Debug, Clone)]
pub struct SlidingMin<T: PartialOrd + Copy> {
    window_us: Us,
    deque: VecDeque<(Us, T)>,
}
impl<T: PartialOrd + Copy> SlidingMin<T> {
    pub fn new(window_us: Us) -> Self {
        Self {
            window_us,
            deque: VecDeque::new(),
        }
    }
    pub fn push(&mut self, now: Us, value: T) -> T {
        // Remove do fim os valores que não são melhores que o novo (>= value):
        // nunca vão ser o mínimo enquanto o novo estiver na janela.
        while let Some(&(_, back)) = self.deque.back() {
            if back >= value {
                self.deque.pop_back();
            } else {
                break;
            }
        }
        self.deque.push_back((now, value));
        // Expira pela frente; mantém sempre a amostra nova, mesmo com janela 0.
        while self.deque.len() > 1 {
            match self.deque.front() {
                Some(&(t, _)) if t.saturating_add(self.window_us) <= now => {
                    self.deque.pop_front();
                }
                _ => break,
            }
        }
        match self.deque.front() {
            Some(&(_, v)) => v,
            None => value,
        }
    }
    pub fn get(&self) -> Option<T> {
        self.deque.front().map(|&(_, v)| v)
    }
    pub fn reset(&mut self) {
        self.deque.clear();
    }
}

/// Máximo numa janela de tempo (deque monotônica).
#[derive(Debug, Clone)]
pub struct SlidingMax<T: PartialOrd + Copy> {
    window_us: Us,
    deque: VecDeque<(Us, T)>,
}
impl<T: PartialOrd + Copy> SlidingMax<T> {
    pub fn new(window_us: Us) -> Self {
        Self {
            window_us,
            deque: VecDeque::new(),
        }
    }
    pub fn push(&mut self, now: Us, value: T) -> T {
        // Simétrico ao mínimo: descarta do fim os valores <= value.
        while let Some(&(_, back)) = self.deque.back() {
            if back <= value {
                self.deque.pop_back();
            } else {
                break;
            }
        }
        self.deque.push_back((now, value));
        // Expira pela frente; mantém sempre a amostra nova, mesmo com janela 0.
        while self.deque.len() > 1 {
            match self.deque.front() {
                Some(&(t, _)) if t.saturating_add(self.window_us) <= now => {
                    self.deque.pop_front();
                }
                _ => break,
            }
        }
        match self.deque.front() {
            Some(&(_, v)) => v,
            None => value,
        }
    }
    pub fn get(&self) -> Option<T> {
        self.deque.front().map(|&(_, v)| v)
    }
    pub fn reset(&mut self) {
        self.deque.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn sliding_min_basico() {
        let mut m = SlidingMin::new(1_000_000);
        assert_eq!(m.push(0, 5), 5);
        assert_eq!(m.push(1, 3), 3);
        assert_eq!(m.push(2, 4), 3);
    }

    #[test]
    fn sliding_min_expira() {
        let mut m = SlidingMin::new(1000);
        assert_eq!(m.push(0, 1), 1);
        assert_eq!(m.push(2000, 5), 5);
    }

    #[test]
    fn sliding_max_basico() {
        let mut m = SlidingMax::new(1_000_000);
        assert_eq!(m.push(0, 3), 3);
        assert_eq!(m.push(1, 5), 5);
        assert_eq!(m.push(2, 4), 5);
    }

    #[test]
    fn sliding_max_expira() {
        let mut m = SlidingMax::new(1000);
        assert_eq!(m.push(0, 9), 9);
        assert_eq!(m.push(2000, 2), 2);
    }

    #[test]
    fn sliding_m_reset_limpa() {
        let mut min = SlidingMin::new(1000);
        let mut max = SlidingMax::new(1000);
        min.push(0, 1);
        max.push(0, 1);
        min.reset();
        max.reset();
        assert_eq!(min.get(), None);
        assert_eq!(max.get(), None);
    }

    proptest! {
        #[test]
        fn sliding_m_compara_ingenuo(
            steps in proptest::collection::vec((0u64..500, -1000i32..1000), 1..200),
            window in 1u64..2000,
        ) {
            let mut min = SlidingMin::new(window);
            let mut max = SlidingMax::new(window);
            let mut hist: Vec<(u64, i32)> = Vec::new();
            let mut now = 0u64;
            for (dt, v) in steps {
                now += dt;
                hist.push((now, v));
                let in_win: Vec<i32> = hist
                    .iter()
                    .filter(|(t, _)| t.saturating_add(window) > now)
                    .map(|&(_, v)| v)
                    .collect();
                let esperado_min = in_win.iter().copied().min();
                let esperado_max = in_win.iter().copied().max();
                prop_assert_eq!(min.push(now, v), esperado_min.unwrap_or(v));
                prop_assert_eq!(max.push(now, v), esperado_max.unwrap_or(v));
            }
        }
    }

    #[test]
    fn quantile_vazio_none() {
        let q = SlidingQuantile::new(1_000_000);
        assert_eq!(q.quantile(0.5), None);
        assert!(q.is_empty());
    }

    #[test]
    fn quantile_mediana_de_1_a_100() {
        let mut q = SlidingQuantile::new(u64::MAX / 2);
        for v in 1..=100 {
            q.push(0, f64::from(v));
        }
        let m = q.quantile(0.5).unwrap_or(f64::NAN);
        assert!((49.0..=52.0).contains(&m), "mediana = {m}");
    }

    #[test]
    fn quantile_p95_de_1_a_100() {
        let mut q = SlidingQuantile::new(u64::MAX / 2);
        for v in 1..=100 {
            q.push(0, f64::from(v));
        }
        let p = q.quantile(0.95).unwrap_or(f64::NAN);
        assert!((94.0..=97.0).contains(&p), "p95 = {p}");
    }

    #[test]
    fn quantile_expira() {
        let mut q = SlidingQuantile::new(1_000_000);
        q.push(0, 50.0);
        q.push(2_000_000, 10.0);
        assert_eq!(q.len(), 1);
        let m = q.quantile(0.5).unwrap_or(f64::NAN);
        assert!((10.0..=11.0).contains(&m), "mediana = {m}");
    }

    #[test]
    fn quantile_valor_acima_2000() {
        let mut q = SlidingQuantile::new(u64::MAX / 2);
        q.push(0, 5000.0);
        let m = q.quantile(1.0).unwrap_or(f64::NAN);
        assert!((2000.0..=2001.0).contains(&m), "valor = {m}");
    }

    #[test]
    fn quantile_negativo_e_nan_vao_a_zero() {
        let mut q = SlidingQuantile::new(u64::MAX / 2);
        q.push(0, -3.0);
        q.push(0, f64::NAN);
        let m = q.quantile(1.0).unwrap_or(f64::NAN);
        assert!((0.0..=1.0).contains(&m), "valor = {m}");
    }

    #[test]
    fn quantile_q_fora_de_0_1_e_clampado() {
        let mut q = SlidingQuantile::new(u64::MAX / 2);
        for v in 1..=100 {
            q.push(0, f64::from(v));
        }
        assert_eq!(q.quantile(-5.0), q.quantile(0.0));
        assert_eq!(q.quantile(7.0), q.quantile(1.0));
    }

    #[test]
    fn meter_rate_constante() {
        // 1250 bytes = 10_000 bits a cada 10 ms => 1 Mbit/s na janela cheia.
        let mut m = RateMeter::new(500_000);
        let mut t = 0u64;
        while t <= 1_000_000 {
            m.add(t, 1250);
            t += 10_000;
        }
        let bps = m.rate_bps(1_000_000);
        assert!((0.95e6..=1.05e6).contains(&bps), "bps = {bps}");
    }

    #[test]
    fn meter_rate_expira() {
        let mut m = RateMeter::new(500_000);
        m.add(0, 1250);
        assert_eq!(m.rate_bps(2_000_000), 0.0);
    }

    #[test]
    fn meter_loss_metade() {
        let mut l = LossMeter::new(u64::MAX / 2);
        for i in 0..100 {
            l.record(i, i % 2 == 0);
        }
        let f = l.fraction(100);
        assert!((f - 0.5).abs() < 1e-6, "fração = {f}");
    }

    #[test]
    fn meter_loss_expira() {
        let mut l = LossMeter::new(1_000_000);
        l.record(0, false);
        l.record(0, false);
        l.record(2_000_000, true);
        assert_eq!(l.fraction(2_000_000), 0.0);
    }

    #[test]
    fn meter_loss_vazio_zero() {
        let mut l = LossMeter::new(1_000_000);
        assert_eq!(l.fraction(0), 0.0);
    }
}

/// Número de baldes: 0..=2000 ms, um balde por milissegundo.
const BALDES: usize = 2001;
/// Índice do último balde (valores acima dele caem aqui).
const ULTIMO_BALDE: u16 = 2000;

/// Quantil por histograma de baldes de 1 ms (0..=2000 ms).
#[derive(Debug, Clone)]
pub struct SlidingQuantile {
    window_us: Us,
    buckets: Vec<u32>,
    queue: VecDeque<(Us, u16)>,
}
impl SlidingQuantile {
    pub fn new(window_us: Us) -> Self {
        Self {
            window_us,
            buckets: vec![0; BALDES],
            queue: VecDeque::new(),
        }
    }

    /// Converte o valor em índice de balde. Negativo e NaN viram 0; acima de 2000 vai ao último.
    fn balde(value_ms: f64) -> u16 {
        if value_ms.is_nan() || value_ms <= 0.0 {
            return 0;
        }
        let inteiro = value_ms.floor();
        if inteiro >= f64::from(ULTIMO_BALDE) {
            ULTIMO_BALDE
        } else {
            // Segura: o ramo acima garante 0 < inteiro < 2000.
            inteiro as u16
        }
    }

    pub fn push(&mut self, now: Us, value_ms: f64) {
        // Expira pela frente: a expiração só acontece aqui, nunca em quantile/len.
        while let Some(&(t, antigo)) = self.queue.front() {
            if t.saturating_add(self.window_us) > now {
                break;
            }
            self.queue.pop_front();
            if let Some(c) = self.buckets.get_mut(usize::from(antigo)) {
                *c = c.saturating_sub(1);
            }
        }
        let b = Self::balde(value_ms);
        self.queue.push_back((now, b));
        if let Some(c) = self.buckets.get_mut(usize::from(b)) {
            *c += 1;
        }
    }

    /// Quantil `q` (clampado em [0, 1]) em milissegundos; `None` se a janela estiver vazia.
    pub fn quantile(&self, q: f64) -> Option<f64> {
        let n = self.queue.len();
        if n == 0 {
            return None;
        }
        // NaN não tem posição no ranking: tratamos como o mínimo.
        let q = if q.is_nan() { 0.0 } else { q.clamp(0.0, 1.0) };
        let rank = q * (n - 1) as f64;

        let mut antes: u64 = 0;
        let mut ultimo_ocupado = None;
        for (i, &c) in self.buckets.iter().enumerate() {
            if c == 0 {
                continue;
            }
            ultimo_ocupado = Some((i, c));
            let fim = antes + u64::from(c);
            if rank < fim as f64 {
                return Some(Self::interpola(i, c, rank - antes as f64));
            }
            antes = fim;
        }
        // Só chega aqui por arredondamento: usa o último balde ocupado.
        ultimo_ocupado.map(|(i, c)| Self::interpola(i, c, f64::from(c) - 1.0))
    }

    /// Posição dentro do balde `i` com `c` amostras e `dentro` = posição do rank no balde.
    fn interpola(i: usize, c: u32, dentro: f64) -> f64 {
        let base = i as f64;
        let r = base + (dentro + 0.5) / f64::from(c);
        r.clamp(base, base + 1.0)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Bytes por segundo numa janela.
#[derive(Debug, Clone)]
pub struct RateMeter {
    window_us: Us,
    samples: VecDeque<(Us, u32)>,
    total: u64,
}
impl RateMeter {
    pub fn new(window_us: Us) -> Self {
        Self {
            window_us,
            samples: VecDeque::new(),
            total: 0,
        }
    }
    pub fn add(&mut self, now: Us, bytes: u32) {
        self.samples.push_back((now, bytes));
        self.total = self.total.saturating_add(u64::from(bytes));
    }
    /// Bits por segundo.
    pub fn rate_bps(&mut self, now: Us) -> f64 {
        // Expira pela frente: amostras com instante + janela <= now saem da soma.
        while let Some(&(t, bytes)) = self.samples.front() {
            if t.saturating_add(self.window_us) > now {
                break;
            }
            self.samples.pop_front();
            self.total = self.total.saturating_sub(u64::from(bytes));
        }
        if self.samples.is_empty() || self.window_us == 0 {
            return 0.0;
        }
        // Divisor é sempre a janela inteira (não o tempo decorrido): estimativa estável.
        let segundos = self.window_us as f64 / 1_000_000.0;
        self.total as f64 * 8.0 / segundos
    }
}

/// Fração de perda numa janela.
#[derive(Debug, Clone)]
pub struct LossMeter {
    window_us: Us,
    samples: VecDeque<(Us, bool)>,
}
impl LossMeter {
    pub fn new(window_us: Us) -> Self {
        Self {
            window_us,
            samples: VecDeque::new(),
        }
    }
    pub fn record(&mut self, now: Us, received: bool) {
        self.samples.push_back((now, received));
    }
    /// Fração de pacotes perdidos na janela; 0.0 se a janela estiver vazia.
    pub fn fraction(&mut self, now: Us) -> f32 {
        while let Some(&(t, _)) = self.samples.front() {
            if t.saturating_add(self.window_us) > now {
                break;
            }
            self.samples.pop_front();
        }
        let total = self.samples.len();
        if total == 0 {
            return 0.0;
        }
        let perdidos = self.samples.iter().filter(|&&(_, recebido)| !recebido).count();
        perdidos as f32 / total as f32
    }
}
