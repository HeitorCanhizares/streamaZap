//! Relógio monotônico em microssegundos.

use std::time::Instant;

/// Relógio com origem fixada na criação. Barato de clonar.
#[derive(Clone, Debug)]
pub struct Clock {
    origin: Instant,
}

impl Clock {
    pub fn new() -> Self {
        Self { origin: Instant::now() }
    }
    /// Microssegundos desde a criação.
    pub fn now_us(&self) -> u64 {
        // u128 -> u64: sobra para ~584 mil anos de execução, então o corte não ocorre na prática
        self.origin.elapsed().as_micros() as u64
    }
    /// `now_us` truncado para u32 (dá a volta).
    pub fn send_us(&self) -> u32 {
        // Truncar de propósito: o campo de 32 bits no protocolo dá a volta a cada ~71 min
        self.now_us() as u32
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn monotonico() {
        let c = Clock::new();
        let a = c.now_us();
        let b = c.now_us();
        assert!(b >= a);
    }

    #[test]
    fn avanca() {
        let c = Clock::new();
        let antes = c.now_us();
        std::thread::sleep(Duration::from_millis(5));
        let depois = c.now_us();
        assert!(depois - antes >= 4000, "avançou só {} us", depois - antes);
    }

    #[test]
    fn clone_compartilha_origem() {
        let c = Clock::new();
        let c2 = c.clone();
        let diff = c.now_us().abs_diff(c2.now_us());
        assert!(diff < 5000, "diferença {} us", diff);
    }

    #[test]
    fn send_us_trunca() {
        let c = Clock::new();
        let s = c.send_us();
        assert!(u64::from(s) <= u64::from(u32::MAX));
        // Lê o `send_us` antes do `now_us`: na ordem inversa, um tique entre as duas leituras dá
        // -1 us, que no `wrapping_sub` vira 4294967295 (visto no CI Windows).
        let envio = c.send_us();
        let agora = c.now_us() as u32;
        let diff = agora.wrapping_sub(envio);
        assert!(diff < 1000, "diferença {} us", diff);
    }
}
