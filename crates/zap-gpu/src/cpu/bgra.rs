//! BGRA -> NV12 com escala bilinear (f32, passada vertical plana + 2 taps horizontais).
use super::eixo::{Eixos, FaixaDst, Scratch, Tap};

const KYR: f32 = 0.2126 * 219.0 / 255.0;
const KYG: f32 = 0.7152 * 219.0 / 255.0;
const KYB: f32 = 0.0722 * 219.0 / 255.0;
const KR: f32 = 0.2126;
const KG: f32 = 0.7152;
const KB: f32 = 0.0722;
const ESC_CROMA: f32 = 224.0 / 255.0;

/// Mistura duas linhas da origem com o peso vertical do tap, byte a byte (alfa incluso), num laço
/// plano sobre f32 para o compilador vetorizar.
fn mistura_vertical(src: &[u8], src_stride: usize, in_w: usize, t: Tap, col: &mut [f32]) {
    let n = in_w * 4;
    let a = &src[t.i0 as usize * src_stride..][..n];
    let col = &mut col[..n];
    if t.f == 0.0 {
        for (c, &x) in col.iter_mut().zip(a) {
            *c = x as f32;
        }
    } else {
        let b = &src[t.i1 as usize * src_stride..][..n];
        let f = t.f;
        let g = 1.0 - f;
        for ((c, &x), &y) in col.iter_mut().zip(a).zip(b) {
            *c = g * x as f32 + f * y as f32;
        }
    }
}

/// Passada horizontal: dois taps por pixel de saída, componentes B, G, R, A.
fn horizontal(col: &[f32], taps: &[Tap], saida: &mut [[f32; 4]]) {
    for (t, o) in taps.iter().zip(saida.iter_mut()) {
        let g = 1.0 - t.f;
        let i0 = t.i0 as usize * 4;
        let i1 = t.i1 as usize * 4;
        let p0 = &col[i0..i0 + 4];
        let p1 = &col[i1..i1 + 4];
        for k in 0..4 {
            o[k] = g * p0[k] + t.f * p1[k];
        }
    }
}

/// Converte a faixa `dst`. `src` é a imagem BGRA inteira (`src_stride` bytes por linha, `in_w` x
/// `in_h` pixels). Se `eixos.identidade`, chama `identidade::faixa_identidade`.
pub(crate) fn faixa_bgra(
    src: &[u8],
    src_stride: usize,
    in_w: u32,
    in_h: u32,
    eixos: &Eixos,
    dst: FaixaDst<'_>,
    scratch: &mut Scratch,
) {
    if eixos.identidade {
        return super::identidade::faixa_identidade(src, src_stride, dst);
    }
    let _ = in_h;
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
    let in_w = in_w as usize;
    let Scratch { col, a, .. } = scratch;

    // Luma: uma linha de saída por vez.
    for r in 0..linhas {
        mistura_vertical(src, src_stride, in_w, eixos.ly[y0 + r], col);
        horizontal(col, &eixos.lx, &mut a[..w]);
        let saida = &mut y[r * y_stride..r * y_stride + w];
        for (p, o) in a[..w].iter().zip(saida.iter_mut()) {
            let v = 16.0 + KYR * p[2] + KYG * p[1] + KYB * p[0];
            // `as u8` satura em 0 e 255.
            *o = (v + 0.5) as u8;
        }
    }

    // Croma: grade de croma sobre a origem BGRA (cx/cy já apontam para pixels da origem).
    let wc = w / 2;
    for c in 0..linhas / 2 {
        mistura_vertical(src, src_stride, in_w, eixos.cy[y0 / 2 + c], col);
        horizontal(col, &eixos.cx[..wc], &mut a[..wc]);
        let saida = &mut uv[c * uv_stride..c * uv_stride + w];
        for (p, o) in a[..wc].iter().zip(saida.chunks_exact_mut(2)) {
            let (b, r) = (p[0], p[2]);
            let yl = KR * r + KG * p[1] + KB * b;
            let u = 128.0 + ESC_CROMA * (b - yl) / 1.8556;
            let v = 128.0 + ESC_CROMA * (r - yl) / 1.5748;
            o[0] = (u + 0.5) as u8;
            o[1] = (v + 0.5) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    fn rodar(src: &[u8], in_w: u32, in_h: u32, out_w: u32, out_h: u32) -> (Vec<u8>, Vec<u8>) {
        let eixos = Eixos::new(in_w, in_h, PixelFormat::Bgra8, out_w, out_h);
        let mut scratch = Scratch::new(in_w, out_w);
        let (w, h) = (out_w as usize, out_h as usize);
        let mut y = vec![0u8; w * h];
        let mut uv = vec![0u8; w * h / 2];
        faixa_bgra(
            src,
            in_w as usize * 4,
            in_w,
            in_h,
            &eixos,
            FaixaDst {
                y: &mut y,
                y_stride: w,
                uv: &mut uv,
                uv_stride: w,
                y0: 0,
                linhas: h,
                out_w,
            },
            &mut scratch,
        );
        (y, uv)
    }

    fn cor(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        (0..w * h).flat_map(|_| [b, g, r, 255]).collect()
    }

    fn ruido(n: u32, semente: u32) -> Vec<u8> {
        let mut x = semente;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                (x >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn cores_puras() {
        let cores = [
            (255u8, 0u8, 0u8),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 255),
            (0, 0, 0),
            (128, 128, 128),
            (10, 200, 90),
        ];
        let tamanhos = [
            (64, 64, 64, 64),
            (1920, 1080, 1280, 720),
            (1920, 1080, 640, 360),
            (1001, 777, 1002, 778),
            (641, 361, 642, 360),
        ];
        for &(iw, ih, ow, oh) in &tamanhos {
            for &(r, g, b) in &cores {
                let (yy, uv) = rodar(&cor(iw, ih, r, g, b), iw, ih, ow, oh);
                let (rf, gf, bf) = (r as f64, g as f64, b as f64);
                let yl = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
                let ey = (16.0 + 219.0 * yl / 255.0).round();
                let eu = (128.0 + 224.0 / 255.0 * (bf - yl) / 1.8556).round();
                let ev = (128.0 + 224.0 / 255.0 * (rf - yl) / 1.5748).round();
                let ok = |v: u8, e: f64| (v as f64 - e).abs() <= 1.0;
                assert!(yy.iter().all(|&v| ok(v, ey)), "Y {iw}x{ih}->{ow}x{oh} {r},{g},{b}");
                assert!(
                    uv.chunks_exact(2).all(|p| ok(p[0], eu) && ok(p[1], ev)),
                    "UV {iw}x{ih}->{ow}x{oh} {r},{g},{b}"
                );
            }
        }
    }

    fn grade(w: u32, h: u32, resto: u32) -> Vec<u8> {
        let mut v = vec![0u8; (w * h * 4) as usize];
        for yy in 0..h {
            for x in 0..w {
                let o = ((yy * w + x) * 4) as usize;
                let val = if x % 3 == resto && yy % 3 == resto { 255 } else { 0 };
                v[o..o + 4].copy_from_slice(&[val, val, val, 255]);
            }
        }
        v
    }

    #[test]
    fn escala_3_1_pega_o_pixel_do_meio() {
        let (y, _) = rodar(&grade(1920, 1080, 1), 1920, 1080, 640, 360);
        assert!(y.iter().all(|&v| v == 235));
        let (y, _) = rodar(&grade(1920, 1080, 0), 1920, 1080, 640, 360);
        assert!(y.iter().all(|&v| v == 16));
    }

    #[test]
    fn gradiente_monotono() {
        let (w, h) = (1920usize, 1080usize);
        let mut src = vec![0u8; w * h * 4];
        for l in 0..h {
            for x in 0..w {
                let r = (x * 255 / (w - 1)) as u8;
                let o = (l * w + x) * 4;
                src[o..o + 4].copy_from_slice(&[0, 0, r, 255]);
            }
        }
        let (y, _) = rodar(&src, 1920, 1080, 1280, 720);
        for linha in y.chunks(1280) {
            assert!(linha.windows(2).all(|p| p[0] <= p[1]));
        }
    }

    #[test]
    fn faixas_iguais_ao_todo() {
        let (iw, ih, ow, oh) = (1920u32, 1080u32, 1280u32, 720u32);
        let src = ruido(iw * ih * 4, 7);
        let (ty, tuv) = rodar(&src, iw, ih, ow, oh);
        let eixos = Eixos::new(iw, ih, PixelFormat::Bgra8, ow, oh);
        let mut scratch = Scratch::new(iw, ow);
        let w = ow as usize;
        let mut y = vec![0u8; w * oh as usize];
        let mut uv = vec![0u8; w * oh as usize / 2];
        for (i, (yc, uvc)) in y.chunks_mut(64 * w).zip(uv.chunks_mut(32 * w)).enumerate() {
            let linhas = yc.len() / w;
            faixa_bgra(
                &src,
                iw as usize * 4,
                iw,
                ih,
                &eixos,
                FaixaDst {
                    y: yc,
                    y_stride: w,
                    uv: uvc,
                    uv_stride: w,
                    y0: i * 64,
                    linhas,
                    out_w: ow,
                },
                &mut scratch,
            );
        }
        assert!(y == ty);
        assert!(uv == tuv);
    }

    #[test]
    fn identidade_despacha() {
        let (w, h) = (64u32, 32u32);
        let src = ruido(w * h * 4, 99);
        let (y, uv) = rodar(&src, w, h, w, h);
        let mut ey = vec![0u8; (w * h) as usize];
        let mut euv = vec![0u8; (w * h / 2) as usize];
        super::super::identidade::faixa_identidade(
            &src,
            w as usize * 4,
            FaixaDst {
                y: &mut ey,
                y_stride: w as usize,
                uv: &mut euv,
                uv_stride: w as usize,
                y0: 0,
                linhas: h as usize,
                out_w: w,
            },
        );
        assert!(y == ey);
        assert!(uv == euv);
    }
}
