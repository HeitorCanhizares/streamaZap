//! Desembrulha contadores `u32` que dão a volta em `u64` crescente.
#![allow(dead_code)]

/// Avança o contador a partir do último valor conhecido.
///
/// A diferença é calculada em modulo 2^32 e lida como `i32`, então um salto
/// pequeno para trás (pacote reordenado) ou para frente (volta ao redor) sai certo.
/// O estado só anda para frente: reordenação não faz o relógio voltar.
fn desembrulha(last: &mut Option<u64>, value: u32) -> u64 {
    let Some(prev) = *last else {
        // Primeira amostra: não há referência, então vale o próprio valor.
        *last = Some(u64::from(value));
        return u64::from(value);
    };

    let diff = value.wrapping_sub(prev as u32) as i32;
    // Saturar em 0 evita pânico quando a diferença negativa passaria de zero.
    let novo = if diff >= 0 {
        prev.saturating_add(diff as u64)
    } else {
        prev.saturating_sub(diff.unsigned_abs() as u64)
    };

    if novo > prev {
        *last = Some(novo);
    }
    novo
}

#[derive(Debug, Clone, Default)]
pub struct SeqUnwrapper {
    last: Option<u64>,
}
impl SeqUnwrapper {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn unwrap(&mut self, value: u32) -> u64 {
        desembrulha(&mut self.last, value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TimeUnwrapper {
    last: Option<u64>,
}
impl TimeUnwrapper {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn unwrap(&mut self, value: u32) -> u64 {
        desembrulha(&mut self.last, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cresce_simples() {
        let mut u = SeqUnwrapper::new();
        assert_eq!(u.unwrap(1), 1);
        assert_eq!(u.unwrap(2), 2);
        assert_eq!(u.unwrap(3), 3);
    }

    #[test]
    fn volta_ao_redor() {
        let m: u64 = 1 << 32;
        let mut u = TimeUnwrapper::new();
        assert_eq!(u.unwrap(u32::MAX - 1), m - 2);
        assert_eq!(u.unwrap(u32::MAX), m - 1);
        assert_eq!(u.unwrap(0), m);
        assert_eq!(u.unwrap(1), m + 1);
    }

    #[test]
    fn reordenado() {
        let mut u = SeqUnwrapper::new();
        assert_eq!(u.unwrap(10), 10);
        assert_eq!(u.unwrap(12), 12);
        // Pacote atrasado: devolve 11, mas o estado continua em 12.
        assert_eq!(u.unwrap(11), 11);
        assert_eq!(u.unwrap(13), 13);
    }

    #[test]
    fn nao_negativo() {
        let mut u = SeqUnwrapper::new();
        assert_eq!(u.unwrap(5), 5);
        // Diferença negativa grande: satura em 0, sem pânico.
        assert_eq!(u.unwrap(4_000_000_000), 0);
    }
}
