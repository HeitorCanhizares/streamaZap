//! BGRA -> NV12 sem escala, em aritmética inteira (3x mais rápido que o caminho em f32).
use super::eixo::FaixaDst;

/// Converte a faixa `dst` de uma origem BGRA do mesmo tamanho da saída. Fórmulas inteiras da
/// seção 5.1 a do plano (`Y`, `U`, `V` com somas dos blocos 2x2). Os laços iteram sobre fatias
/// (sem índices com checagem de limites) para o compilador vetorizar.
pub(crate) fn faixa_identidade(src: &[u8], src_stride: usize, dst: FaixaDst<'_>) {
    let FaixaDst {
        y,
        y_stride,
        uv,
        uv_stride,
        y0,
        linhas,
        out_w,
    } = dst;
    let w = out_w as usize;

    // Luma: uma linha inteira por vez, num laço simples.
    for r in 0..linhas {
        let ini = (y0 + r) * src_stride;
        let linha = &src[ini..ini + w * 4];
        let saida = &mut y[r * y_stride..r * y_stride + w];
        for (px, o) in linha.chunks_exact(4).zip(saida.iter_mut()) {
            let b = px[0] as u32;
            let g = px[1] as u32;
            let r = px[2] as u32;
            let v = (11966 * r + 40255 * g + 4064 * b + (16 << 16) + 32768) >> 16;
            *o = v.min(255) as u8;
        }
    }

    // Croma: um bloco 2x2 por par U,V.
    for c in 0..linhas / 2 {
        let ini0 = (y0 + 2 * c) * src_stride;
        let ini1 = ini0 + src_stride;
        let l0 = &src[ini0..ini0 + w * 4];
        let l1 = &src[ini1..ini1 + w * 4];
        let saida = &mut uv[c * uv_stride..c * uv_stride + w];
        for ((p0, p1), o) in l0
            .chunks_exact(8)
            .zip(l1.chunks_exact(8))
            .zip(saida.chunks_exact_mut(2))
        {
            let sb = p0[0] as i32 + p0[4] as i32 + p1[0] as i32 + p1[4] as i32;
            let sg = p0[1] as i32 + p0[5] as i32 + p1[1] as i32 + p1[5] as i32;
            let sr = p0[2] as i32 + p0[6] as i32 + p1[2] as i32 + p1[6] as i32;
            let u = (1799 * sb - 412 * sr - 1387 * sg + (128 << 14) + 8192) >> 14;
            let v = (1799 * sr - 1634 * sg - 165 * sb + (128 << 14) + 8192) >> 14;
            o[0] = u.clamp(0, 255) as u8;
            o[1] = v.clamp(0, 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imagem(w: usize, h: usize, stride: usize) -> Vec<u8> {
        let mut x: u32 = 12345;
        let mut v = vec![0xEEu8; stride * h];
        for l in 0..h {
            for b in &mut v[l * stride..l * stride + w * 4] {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                *b = (x >> 24) as u8;
            }
        }
        v
    }

    /// Referência pixel a pixel, escrita de forma independente.
    fn referencia(src: &[u8], stride: usize, w: usize, h: usize) -> (Vec<u8>, Vec<u8>) {
        let px = |x: usize, y: usize| {
            let o = y * stride + x * 4;
            (src[o] as i32, src[o + 1] as i32, src[o + 2] as i32)
        };
        let mut yp = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let (b, g, r) = px(x, y);
                let v = (11966 * r + 40255 * g + 4064 * b + (16 << 16) + 32768) >> 16;
                yp[y * w + x] = v.min(255) as u8;
            }
        }
        let mut uvp = vec![0u8; w * h / 2];
        for cy in 0..h / 2 {
            for cx in 0..w / 2 {
                let (mut sb, mut sg, mut sr) = (0, 0, 0);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (b, g, r) = px(2 * cx + dx, 2 * cy + dy);
                    sb += b;
                    sg += g;
                    sr += r;
                }
                let u = (1799 * sb - 412 * sr - 1387 * sg + (128 << 14) + 8192) >> 14;
                let v = (1799 * sr - 1634 * sg - 165 * sb + (128 << 14) + 8192) >> 14;
                uvp[cy * w + 2 * cx] = u.clamp(0, 255) as u8;
                uvp[cy * w + 2 * cx + 1] = v.clamp(0, 255) as u8;
            }
        }
        (yp, uvp)
    }

    #[test]
    fn identidade_igual_formula_inteira() {
        let (w, h) = (64usize, 32usize);
        let src = imagem(w, h, w * 4);
        let mut y = vec![0u8; w * h];
        let mut uv = vec![0u8; w * h / 2];
        faixa_identidade(
            &src,
            w * 4,
            FaixaDst {
                y: &mut y,
                y_stride: w,
                uv: &mut uv,
                uv_stride: w,
                y0: 0,
                linhas: h,
                out_w: w as u32,
            },
        );
        let (ry, ruv) = referencia(&src, w * 4, w, h);
        assert_eq!(y, ry);
        assert_eq!(uv, ruv);
    }

    #[test]
    fn identidade_em_faixas() {
        let (w, h) = (64usize, 32usize);
        let src = imagem(w, h, w * 4);
        let (ry, ruv) = referencia(&src, w * 4, w, h);
        let mut y = vec![0u8; w * h];
        let mut uv = vec![0u8; w * h / 2];
        for (i, (yc, uvc)) in y.chunks_mut(16 * w).zip(uv.chunks_mut(8 * w)).enumerate() {
            faixa_identidade(
                &src,
                w * 4,
                FaixaDst {
                    y: yc,
                    y_stride: w,
                    uv: uvc,
                    uv_stride: w,
                    y0: i * 16,
                    linhas: 16,
                    out_w: w as u32,
                },
            );
        }
        assert_eq!(y, ry);
        assert_eq!(uv, ruv);
    }

    #[test]
    fn identidade_com_stride() {
        let (w, h) = (64usize, 32usize);
        let ss = w * 4 + 64;
        let src = imagem(w, h, ss);
        let (ry, ruv) = referencia(&src, ss, w, h);
        let ds = w + 16;
        let mut y = vec![0xEEu8; ds * h];
        let mut uv = vec![0xEEu8; ds * h / 2];
        faixa_identidade(
            &src,
            ss,
            FaixaDst {
                y: &mut y,
                y_stride: ds,
                uv: &mut uv,
                uv_stride: ds,
                y0: 0,
                linhas: h,
                out_w: w as u32,
            },
        );
        for l in 0..h {
            assert_eq!(&y[l * ds..l * ds + w], &ry[l * w..(l + 1) * w]);
            assert!(y[l * ds + w..(l + 1) * ds].iter().all(|&b| b == 0xEE));
        }
        for l in 0..h / 2 {
            assert_eq!(&uv[l * ds..l * ds + w], &ruv[l * w..(l + 1) * w]);
            assert!(uv[l * ds + w..(l + 1) * ds].iter().all(|&b| b == 0xEE));
        }
    }

    #[test]
    fn identidade_cores_basicas() {
        let (w, h) = (4usize, 4usize);
        for (valor, esperado) in [(255u8, [235i32, 128, 128]), (0u8, [16, 128, 128])] {
            let src = vec![valor; w * h * 4];
            let mut y = vec![0u8; w * h];
            let mut uv = vec![0u8; w * h / 2];
            faixa_identidade(
                &src,
                w * 4,
                FaixaDst {
                    y: &mut y,
                    y_stride: w,
                    uv: &mut uv,
                    uv_stride: w,
                    y0: 0,
                    linhas: h,
                    out_w: w as u32,
                },
            );
            assert!((y[0] as i32 - esperado[0]).abs() <= 1);
            assert!((uv[0] as i32 - esperado[1]).abs() <= 1);
            assert!((uv[1] as i32 - esperado[2]).abs() <= 1);
        }
    }
}
