//! Funções puras de mistura usadas pela captura e pela saída.

/// Soma `src` em `dst` (mesmo tamanho), sem limitar.
///
/// Se os tamanhos diferirem, soma só até `min(len)`: no callback de áudio
/// não podemos entrar em pânico.
#[inline]
pub fn add_into(dst: &mut [f32], src: &[f32]) {
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d += *s;
    }
}

/// Limita cada amostra a [-1.0, 1.0] (NaN vira 0.0).
#[inline]
pub fn limit(buf: &mut [f32]) {
    for x in buf.iter_mut() {
        // `clamp` propagaria o NaN; tratamos ele antes.
        *x = if x.is_nan() { 0.0 } else { x.clamp(-1.0, 1.0) };
    }
}

/// Pico absoluto do bloco (0.0 se vazio).
///
/// `f32::max` ignora NaN, então amostras NaN não contam como pico.
#[inline]
pub fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0_f32, |acc, x| acc.max(x.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_into_soma() {
        let mut dst = [1.0_f32, 2.0, 3.0];
        add_into(&mut dst, &[0.5, -1.0, 2.0]);
        assert_eq!(dst, [1.5, 1.0, 5.0]);
    }

    #[test]
    fn add_into_tamanhos_diferentes_nao_panica() {
        let mut dst = [1.0_f32, 1.0];
        add_into(&mut dst, &[1.0, 2.0, 3.0]);
        assert_eq!(dst, [2.0, 3.0]);

        let mut dst = [1.0_f32, 1.0, 1.0];
        add_into(&mut dst, &[5.0]);
        assert_eq!(dst, [6.0, 1.0, 1.0]);
    }

    #[test]
    fn limit_clampa_e_zera_nan() {
        let mut buf = [1.5_f32, -2.0, 0.25, f32::NAN, -1.0, 1.0];
        limit(&mut buf);
        assert_eq!(buf, [1.0, -1.0, 0.25, 0.0, -1.0, 1.0]);
    }

    #[test]
    fn peak_vazio_e_zero() {
        assert_eq!(peak(&[]), 0.0);
    }

    #[test]
    fn peak_pega_negativo() {
        assert_eq!(peak(&[0.1, -0.7, 0.3]), 0.7);
        assert_eq!(peak(&[f32::NAN, -0.2, 0.1]), 0.2);
    }
}
