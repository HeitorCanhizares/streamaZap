//! Reamostragem 16 kHz → 48 kHz (×3), FIR polifásico (plano do modo palco, §4.3).
//!
//! Passa-baixa sinc com janela de Kaiser (β = 8), corte em 7,6 kHz, 96 coeficientes
//! (32 por fase), atraso de 1 ms. Puro: sem I/O.

use std::sync::OnceLock;

use crate::FRAME;

/// Amostras de 16 kHz em 20 ms.
pub const IN_SAMPLES: usize = 320;

/// Fator de interpolação.
const FATOR: usize = 3;
/// Coeficientes por fase.
const TAPS: usize = 32;
/// Total de coeficientes do protótipo.
const TOTAL: usize = FATOR * TAPS;
/// Parâmetro β da janela de Kaiser.
const BETA: f64 = 8.0;
/// Corte relativo à taxa de saída (7,6 kHz / 48 kHz), em ciclos por amostra.
const FC: f64 = 7600.0 / 48000.0;

/// Bessel modificada de ordem 0, por série de potências.
fn bessel_i0(x: f64) -> f64 {
    let q = x * x / 4.0;
    let mut termo = 1.0;
    let mut soma = 1.0;
    for k in 1..60 {
        termo *= q / (k as f64 * k as f64);
        soma += termo;
        if termo < soma * 1e-17 {
            break;
        }
    }
    soma
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let a = std::f64::consts::PI * x;
        a.sin() / a
    }
}

/// Coeficientes por fase, calculados uma vez. Cada fase soma 1.0 (ganho DC unitário).
fn coeffs() -> &'static [[f32; TAPS]; FATOR] {
    static COEFFS: OnceLock<[[f32; TAPS]; FATOR]> = OnceLock::new();
    COEFFS.get_or_init(|| {
        let centro = (TOTAL as f64 - 1.0) / 2.0;
        let i0_beta = bessel_i0(BETA);
        let mut h = [0.0f64; TOTAL];
        for (n, v) in h.iter_mut().enumerate() {
            let t = n as f64 - centro;
            let r = 2.0 * n as f64 / (TOTAL as f64 - 1.0) - 1.0;
            let kaiser = bessel_i0(BETA * (1.0 - r * r).max(0.0).sqrt()) / i0_beta;
            *v = sinc(2.0 * FC * t) * kaiser * 2.0 * FC;
        }
        let mut fases = [[0.0f32; TAPS]; FATOR];
        for (p, fase) in fases.iter_mut().enumerate() {
            let soma: f64 = (0..TAPS).map(|j| h[FATOR * j + p]).sum();
            for (j, c) in fase.iter_mut().enumerate() {
                *c = (h[FATOR * j + p] / soma) as f32;
            }
        }
        fases
    })
}

pub struct Upsampler3 {
    /// Últimas 32 amostras de entrada (histórico do FIR), da mais antiga à mais nova.
    history: [f32; TAPS],
}

impl Upsampler3 {
    pub fn new() -> Self {
        // Garante que os coeficientes já existem antes do primeiro `process`.
        let _ = coeffs();
        Self { history: [0.0; TAPS] }
    }

    /// 320 amostras de 16 kHz → 960 de 48 kHz.
    pub fn process(&mut self, input: &[f32; IN_SAMPLES], out: &mut [f32; FRAME]) {
        let fases = coeffs();
        // Histórico seguido da entrada atual, num buffer de pilha (sem alocação).
        let mut buf = [0.0f32; TAPS + IN_SAMPLES];
        buf[..TAPS].copy_from_slice(&self.history);
        buf[TAPS..].copy_from_slice(input);

        for k in 0..IN_SAMPLES {
            // x[k - j] está em buf[TAPS + k - j].
            let base = TAPS + k;
            for (p, fase) in fases.iter().enumerate() {
                let mut acc = 0.0f32;
                for (j, c) in fase.iter().enumerate() {
                    acc += c * buf[base - j];
                }
                out[FATOR * k + p] = acc;
            }
        }

        self.history.copy_from_slice(&buf[IN_SAMPLES..]);
    }
}

impl Default for Upsampler3 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Gera `blocos` blocos de um seno de `freq` Hz a 16 kHz e devolve a saída contínua a 48 kHz.
    fn seno_saida(freq: f64, amp: f64, blocos: usize) -> Vec<f32> {
        let mut up = Upsampler3::new();
        let mut saida = Vec::with_capacity(blocos * FRAME);
        let mut n = 0usize;
        for _ in 0..blocos {
            let mut entrada = [0.0f32; IN_SAMPLES];
            for v in entrada.iter_mut() {
                *v = (amp * (2.0 * PI * freq * n as f64 / 16000.0).sin()) as f32;
                n += 1;
            }
            let mut out = [0.0f32; FRAME];
            up.process(&entrada, &mut out);
            saida.extend_from_slice(&out);
        }
        saida
    }

    /// Projeção em cos/sin de `freq` (taxa 48 kHz): retorna (a, b) de y ≈ a cos + b sin.
    fn projeta(y: &[f32], freq: f64) -> (f64, f64) {
        let (mut a, mut b) = (0.0, 0.0);
        for (i, &v) in y.iter().enumerate() {
            let w = 2.0 * PI * freq * i as f64 / 48000.0;
            a += v as f64 * w.cos();
            b += v as f64 * w.sin();
        }
        let n = y.len() as f64;
        (2.0 * a / n, 2.0 * b / n)
    }

    #[test]
    fn ganho_dc() {
        let mut up = Upsampler3::new();
        let entrada = [1.0f32; IN_SAMPLES];
        let mut out = [0.0f32; FRAME];
        for _ in 0..10 {
            up.process(&entrada, &mut out);
        }
        for (i, v) in out.iter().enumerate() {
            assert!((v - 1.0).abs() < 0.01, "amostra {i}: {v}");
        }
    }

    #[test]
    fn seno_1khz_mantem_frequencia() {
        let y = seno_saida(1000.0, 0.5, 60);
        let janela = &y[5 * FRAME..5 * FRAME + 48000];
        let cruzamentos = janela.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        assert!((1998..=2002).contains(&cruzamentos), "cruzamentos: {cruzamentos}");
    }

    #[test]
    fn distorcao_menor_que_50db() {
        let y = seno_saida(1000.0, 0.5, 60);
        let janela = &y[5 * FRAME..5 * FRAME + 48000];
        let (a, b) = projeta(janela, 1000.0);
        let (mut sinal, mut residuo) = (0.0f64, 0.0f64);
        for (i, &v) in janela.iter().enumerate() {
            let w = 2.0 * PI * 1000.0 * i as f64 / 48000.0;
            let est = a * w.cos() + b * w.sin();
            sinal += est * est;
            let r = v as f64 - est;
            residuo += r * r;
        }
        let db = 10.0 * (sinal / residuo.max(1e-30)).log10();
        assert!(db > 50.0, "relação sinal/resíduo: {db:.1} dB");
    }

    #[test]
    fn imagem_atenuada() {
        let y = seno_saida(1000.0, 0.5, 60);
        let janela = &y[5 * FRAME..5 * FRAME + 48000];
        let (a1, b1) = projeta(janela, 1000.0);
        let (a15, b15) = projeta(janela, 15000.0);
        let e1 = a1 * a1 + b1 * b1;
        let e15 = a15 * a15 + b15 * b15;
        let db = 10.0 * (e1 / e15.max(1e-30)).log10();
        assert!(db > 40.0, "atenuação da imagem: {db:.1} dB");
    }

    #[test]
    fn blocos_iguais_a_continuo() {
        // 4 blocos em chamadas separadas contra a convolução direta do sinal inteiro.
        let total = 4 * IN_SAMPLES;
        let x: Vec<f32> = (0..total)
            .map(|n| (0.5 * (2.0 * PI * 1300.0 * n as f64 / 16000.0).sin()) as f32)
            .collect();
        let fases = coeffs();
        let mut up = Upsampler3::new();
        for b in 0..4 {
            let mut entrada = [0.0f32; IN_SAMPLES];
            entrada.copy_from_slice(&x[b * IN_SAMPLES..(b + 1) * IN_SAMPLES]);
            let mut out = [0.0f32; FRAME];
            up.process(&entrada, &mut out);
            for (i, &got) in out.iter().enumerate() {
                let k = b * IN_SAMPLES + i / FATOR;
                let p = i % FATOR;
                let mut esp = 0.0f32;
                for j in 0..TAPS {
                    if k >= j {
                        esp += fases[p][j] * x[k - j];
                    }
                }
                assert!((got - esp).abs() < 1e-6, "bloco {b} amostra {i}");
            }
        }
    }

    #[test]
    fn silencio_da_silencio() {
        let mut up = Upsampler3::new();
        let entrada = [0.0f32; IN_SAMPLES];
        let mut out = [1.0f32; FRAME];
        up.process(&entrada, &mut out);
        assert!(out.iter().all(|&v| v == 0.0));
    }
}
