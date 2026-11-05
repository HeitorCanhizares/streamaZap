//! Mede o conversor de CPU (`cargo run --release -p zap-gpu --example cpu_nv12`).
//!
//! Gera um padrão de teste BGRA 1920x1080 (blocos coloridos, bordas brancas e tabuleiros de 1 px)
//! e cronometra cada conversão com 1 thread e com `cpu_threads()`. Sem argumentos; portátil.
use std::time::Instant;

use zap_gpu::{convert_batch, cpu_threads, BatchOptions, CpuScaler, GpuError, Nv12Mut, PixelFormat, SrcImage};

const W: u32 = 1920;
const H: u32 = 1080;
const BLOCO_W: usize = 37;
const BLOCO_H: usize = 23;
/// Aquecimentos descartados antes das medidas.
const AQUECIMENTOS: usize = 3;
const MEDIDAS: usize = 40;

/// Gerador congruencial linear: barato, determinístico e sem dependências.
struct Lcg(u32);

impl Lcg {
    fn proximo(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 16) as u8
    }
}

/// Padrão "tela" BGRA 1920x1080 com stride de 7680 bytes e alfa 255.
///
/// Cada bloco de 37x23 recebe uma cor do LCG (semente 12345, em ordem de bloco linha a linha).
/// A primeira coluna e a primeira linha de cada bloco são brancas. Blocos com `(bx + by) % 3 == 0`
/// ganham um tabuleiro preto/branco de 1 px, com preto quando `(x + y) % 2 == 0` (coordenadas da
/// imagem). Blocos parciais na borda direita e inferior são cortados.
fn padrao_tela() -> Vec<u8> {
    let w = W as usize;
    let h = H as usize;
    let mut px = vec![0u8; w * h * 4];
    let mut lcg = Lcg(12345);
    for by in 0..h.div_ceil(BLOCO_H) {
        for bx in 0..w.div_ceil(BLOCO_W) {
            let (x0, y0) = (bx * BLOCO_W, by * BLOCO_H);
            let (x1, y1) = ((x0 + BLOCO_W).min(w), (y0 + BLOCO_H).min(h));
            // A cor é sorteada sempre, mesmo em bloco de tabuleiro, para a sequência ficar fixa.
            let cor = [lcg.proximo(), lcg.proximo(), lcg.proximo()];
            let tabuleiro = (bx + by) % 3 == 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (b, g, r) = if x == x0 || y == y0 {
                        (255, 255, 255)
                    } else if tabuleiro {
                        if (x + y) % 2 == 0 {
                            (0, 0, 0)
                        } else {
                            (255, 255, 255)
                        }
                    } else {
                        (cor[0], cor[1], cor[2])
                    };
                    let i = (y * w + x) * 4;
                    px[i] = b;
                    px[i + 1] = g;
                    px[i + 2] = r;
                    px[i + 3] = 255;
                }
            }
        }
    }
    px
}

/// Destino NV12 contíguo sem preenchimento (`w * h * 3 / 2` bytes), com stride igual à largura.
fn nv12_vazio(w: u32, h: u32) -> Vec<u8> {
    vec![0u8; w as usize * h as usize * 3 / 2]
}

/// Médias, mediana e p95 (ordem do posto) de uma lista de tempos em ms.
fn estatisticas(mut v: Vec<f64>) -> (f64, f64, f64) {
    let n = v.len();
    let media = v.iter().sum::<f64>() / n as f64;
    v.sort_by(f64::total_cmp);
    let mediana = if n % 2 == 0 {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    } else {
        v[n / 2]
    };
    let p95 = v[((n as f64 * 0.95).ceil() as usize).saturating_sub(1).min(n - 1)];
    (media, mediana, p95)
}

/// Aquece, mede `MEDIDAS` rodadas e imprime uma linha `caso | threads | media | mediana | p95`.
fn medir(caso: &str, threads: usize, mut rodada: impl FnMut() -> Result<(), GpuError>) -> Result<(), GpuError> {
    for _ in 0..AQUECIMENTOS {
        rodada()?;
    }
    let mut tempos = Vec::with_capacity(MEDIDAS);
    for _ in 0..MEDIDAS {
        let inicio = Instant::now();
        rodada()?;
        tempos.push(inicio.elapsed().as_secs_f64() * 1000.0);
    }
    let (media, mediana, p95) = estatisticas(tempos);
    println!("{caso} | {threads} | {media:.3} | {mediana:.3} | {p95:.3}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let n = cpu_threads();
    println!("cpu_threads() = {n}");
    println!("caso | threads | media | mediana | p95 (ms)");
    // Com 1 núcleo útil, as duas rodadas seriam iguais: mede só uma.
    let mut contagens = vec![1usize];
    if n > 1 {
        contagens.push(n);
    }

    let pad = padrao_tela();
    let src = SrcImage::Bgra {
        data: &pad,
        stride: W as usize * 4,
        width: W,
        height: H,
    };
    let s1080 = CpuScaler::new(W, H, PixelFormat::Bgra8, 1920, 1080)?;
    let s720 = CpuScaler::new(W, H, PixelFormat::Bgra8, 1280, 720)?;
    let s360 = CpuScaler::new(W, H, PixelFormat::Bgra8, 640, 360)?;

    // Um destino por caso e contagem de threads, alocado fora da medição.
    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut buf = nv12_vazio(1920, 1080);
        let mut dst = Nv12Mut::from_contiguous(&mut buf, 1920, 1920, 1080)?;
        medir("BGRA 1080p -> 1080p", t, || s1080.convert(&src, &mut dst, opts))?;
    }

    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut buf = nv12_vazio(1280, 720);
        let mut dst = Nv12Mut::from_contiguous(&mut buf, 1280, 1280, 720)?;
        medir("BGRA 1080p -> 720p", t, || s720.convert(&src, &mut dst, opts))?;
    }

    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut buf = nv12_vazio(640, 360);
        let mut dst = Nv12Mut::from_contiguous(&mut buf, 640, 640, 360)?;
        medir("BGRA 1080p -> 360p", t, || s360.convert(&src, &mut dst, opts))?;
    }

    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut b720 = nv12_vazio(1280, 720);
        let mut b360 = nv12_vazio(640, 360);
        let mut jobs = vec![
            (&s720, Nv12Mut::from_contiguous(&mut b720, 1280, 1280, 720)?),
            (&s360, Nv12Mut::from_contiguous(&mut b360, 640, 640, 360)?),
        ];
        medir("BGRA 1080p -> 720p + 360p (lote 2)", t, || {
            convert_batch(&src, &mut jobs, opts)
        })?;
    }

    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut b1080 = nv12_vazio(1920, 1080);
        let mut b720 = nv12_vazio(1280, 720);
        let mut b360 = nv12_vazio(640, 360);
        let mut jobs = vec![
            (&s1080, Nv12Mut::from_contiguous(&mut b1080, 1920, 1920, 1080)?),
            (&s720, Nv12Mut::from_contiguous(&mut b720, 1280, 1280, 720)?),
            (&s360, Nv12Mut::from_contiguous(&mut b360, 640, 640, 360)?),
        ];
        medir("BGRA 1080p -> 1080p + 720p + 360p (lote 3)", t, || {
            convert_batch(&src, &mut jobs, opts)
        })?;
    }

    // Origem NV12 720p, obtida do padrão com o próprio conversor de CPU (fora da medição).
    let mut nv_buf = nv12_vazio(1280, 720);
    {
        let mut nv_dst = Nv12Mut::from_contiguous(&mut nv_buf, 1280, 1280, 720)?;
        s720.convert(&src, &mut nv_dst, BatchOptions::default())?;
    }
    let (y, uv) = nv_buf.split_at(1280 * 720);
    let src_nv = SrcImage::Nv12 {
        y,
        y_stride: 1280,
        uv,
        uv_stride: 1280,
        width: 1280,
        height: 720,
    };
    let s_nv = CpuScaler::new(1280, 720, PixelFormat::Nv12, 640, 360)?;

    for &t in &contagens {
        let opts = BatchOptions {
            threads: t,
            on_thread_start: None,
        };
        let mut buf = nv12_vazio(640, 360);
        let mut dst = Nv12Mut::from_contiguous(&mut buf, 640, 640, 360)?;
        medir("NV12 720p -> 360p", t, || s_nv.convert(&src_nv, &mut dst, opts))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Soma e hash simples (multiplicador 31) de um buffer, para comparar duas gerações.
    fn assinatura(buf: &[u8]) -> (u64, u64) {
        buf.iter().fold((0u64, 0u64), |(hash, soma), &b| {
            (hash.wrapping_mul(31).wrapping_add(b as u64), soma + b as u64)
        })
    }

    #[test]
    fn padrao_tela_tem_tamanho_e_e_deterministico() {
        let a = padrao_tela();
        let b = padrao_tela();
        assert_eq!(a.len(), 1920 * 1080 * 4);
        assert_eq!(b.len(), 1920 * 1080 * 4);
        assert_eq!(assinatura(&a), assinatura(&b));
    }
}
