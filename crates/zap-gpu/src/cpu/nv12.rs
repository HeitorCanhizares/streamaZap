//! NV12 -> NV12 (cópia na identidade; bilinear por plano nos outros tamanhos).
use super::eixo::{Eixos, FaixaDst, Scratch};

/// Converte a faixa `dst` de uma origem NV12 (`y`/`uv` inteiros, `in_w` x `in_h` de luma).
/// Identidade: copia as linhas. Com escala: plano Y (1 canal) com `lx`/`ly`; plano UV (2 canais
/// intercalados) com `cx`/`cy`; arredonda com `(x + 0.5) as u8`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn faixa_nv12(
    y: &[u8],
    y_stride: usize,
    uv: &[u8],
    uv_stride: usize,
    in_w: u32,
    in_h: u32,
    eixos: &Eixos,
    mut dst: FaixaDst<'_>,
    scratch: &mut Scratch,
) {
    // Só conferência em debug: o plano Y precisa ter `in_h` linhas de `in_w` bytes.
    debug_assert!(in_h >= 1 && y.len() >= y_stride * (in_h as usize - 1) + in_w as usize);
    let w = in_w as usize;
    if eixos.identidade {
        for r in 0..dst.linhas {
            let origem = (dst.y0 + r) * y_stride;
            let destino = r * dst.y_stride;
            dst.y[destino..destino + w].copy_from_slice(&y[origem..origem + w]);
        }
        for c in 0..dst.linhas / 2 {
            let origem = (dst.y0 / 2 + c) * uv_stride;
            let destino = c * dst.uv_stride;
            dst.uv[destino..destino + w].copy_from_slice(&uv[origem..origem + w]);
        }
        return;
    }
    escala_y(y, y_stride, w, eixos, &mut dst, scratch);
    escala_uv(uv, uv_stride, w, eixos, &mut dst, scratch);
}

/// Plano Y: mistura vertical das duas linhas de origem e depois o filtro horizontal.
fn escala_y(y: &[u8], y_stride: usize, w: usize, eixos: &Eixos, dst: &mut FaixaDst<'_>, scratch: &mut Scratch) {
    let out_w = dst.out_w as usize;
    for r in 0..dst.linhas {
        let t = eixos.ly[dst.y0 + r];
        let a = (t.i0 as usize) * y_stride;
        let b = (t.i1 as usize) * y_stride;
        mistura_vertical(&mut scratch.col[..w], &y[a..a + w], &y[b..b + w], t.f);
        let destino = r * dst.y_stride;
        let linha = &mut dst.y[destino..destino + out_w];
        for (o, h) in linha.iter_mut().zip(&eixos.lx) {
            *o = amostra(&scratch.col, h.i0 as usize, h.i1 as usize, h.f);
        }
    }
}

/// Plano UV: as linhas de croma são tratadas como `w` bytes (pares Cb, Cr). Os taps horizontais
/// de `cx` estão na grade de pares, então o índice do byte é `2 * i` + canal.
fn escala_uv(uv: &[u8], uv_stride: usize, w: usize, eixos: &Eixos, dst: &mut FaixaDst<'_>, scratch: &mut Scratch) {
    let out_w = dst.out_w as usize;
    for c in 0..dst.linhas / 2 {
        let t = eixos.cy[dst.y0 / 2 + c];
        let a = (t.i0 as usize) * uv_stride;
        let b = (t.i1 as usize) * uv_stride;
        mistura_vertical(&mut scratch.col[..w], &uv[a..a + w], &uv[b..b + w], t.f);
        let destino = c * dst.uv_stride;
        let linha = &mut dst.uv[destino..destino + out_w];
        for (par, h) in linha.chunks_exact_mut(2).zip(&eixos.cx) {
            let (i0, i1) = (2 * h.i0 as usize, 2 * h.i1 as usize);
            par[0] = amostra(&scratch.col, i0, i1, h.f);
            par[1] = amostra(&scratch.col, i0 + 1, i1 + 1, h.f);
        }
    }
}

/// `col[k] = (1 - f) * a[k] + f * b[k]`. Com `f == 0` usa só `a` (sem ler `b` na conta).
/// `a` e `b` têm o mesmo comprimento que `col`; a forma com zip deixa o laço vetorizar.
fn mistura_vertical(col: &mut [f32], a: &[u8], b: &[u8], f: f32) {
    if f == 0.0 {
        for (c, &p) in col.iter_mut().zip(a) {
            *c = f32::from(p);
        }
    } else {
        let g = 1.0 - f;
        for ((c, &p), &q) in col.iter_mut().zip(a).zip(b) {
            *c = g * f32::from(p) + f * f32::from(q);
        }
    }
}

/// Filtro horizontal de 2 taps sobre o rascunho e arredondamento para byte. O `as` satura,
/// então valores fora de 0..=255 (não deveriam ocorrer) viram 0 ou 255 em vez de estourar.
fn amostra(col: &[f32], i0: usize, i1: usize, f: f32) -> u8 {
    let v = (1.0 - f) * col[i0] + f * col[i1];
    (v + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    /// Gerador congruencial de semente fixa: dados pseudoaleatórios e repetíveis, sem dependências.
    struct Lcg(u64);

    impl Lcg {
        fn byte(&mut self) -> u8 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 56) as u8
        }
    }

    /// Converte uma imagem inteira numa faixa única, com strides da saída igual à largura.
    /// Devolve (Y, UV) da saída.
    #[allow(clippy::too_many_arguments)]
    fn converte(
        y: &[u8],
        uv: &[u8],
        in_w: u32,
        in_h: u32,
        out_w: u32,
        out_h: u32,
        ys: usize,
        uvs: usize,
    ) -> (Vec<u8>, Vec<u8>) {
        let eixos = Eixos::new(in_w, in_h, PixelFormat::Nv12, out_w, out_h);
        let (ow, oh) = (out_w as usize, out_h as usize);
        let mut sy = vec![0u8; ow * oh];
        let mut suv = vec![0u8; ow * oh / 2];
        let mut scratch = Scratch::new(in_w, out_w);
        let dst = FaixaDst {
            y: &mut sy,
            y_stride: ow,
            uv: &mut suv,
            uv_stride: ow,
            y0: 0,
            linhas: oh,
            out_w,
        };
        faixa_nv12(y, ys, uv, uvs, in_w, in_h, &eixos, dst, &mut scratch);
        (sy, suv)
    }

    #[test]
    fn nv12_solido() {
        for (ow, oh) in [(640u32, 360u32), (960, 540)] {
            let (w, h) = (1280usize, 720usize);
            let y = vec![63u8; w * h];
            let mut uv = vec![0u8; w * (h / 2)];
            for par in uv.chunks_exact_mut(2) {
                par[0] = 102;
                par[1] = 240;
            }
            let (sy, suv) = converte(&y, &uv, 1280, 720, ow, oh, w, w);
            assert!(sy.iter().all(|&v| v == 63), "Y em {ow}x{oh}");
            for par in suv.chunks_exact(2) {
                assert_eq!(par, &[102u8, 240u8], "UV em {ow}x{oh}");
            }
        }
    }

    #[test]
    fn nv12_identidade_copia() {
        let (w, h, ys) = (1280usize, 720usize, 1300usize);
        let mut rng = Lcg(0x5EED);
        let y: Vec<u8> = (0..ys * h).map(|_| rng.byte()).collect();
        let uv: Vec<u8> = (0..ys * (h / 2)).map(|_| rng.byte()).collect();
        let (sy, suv) = converte(&y, &uv, 1280, 720, 1280, 720, ys, ys);
        for r in 0..h {
            assert_eq!(sy[r * w..(r + 1) * w], y[r * ys..r * ys + w], "Y linha {r}");
        }
        for r in 0..h / 2 {
            assert_eq!(suv[r * w..(r + 1) * w], uv[r * ys..r * ys + w], "UV linha {r}");
        }
    }

    #[test]
    fn nv12_escala_meia_fica_na_faixa() {
        let (w, h) = (1280usize, 720usize);
        // Gradiente horizontal monotono em Y (0..=255) e em Cb; Cr fixo em 128.
        let y: Vec<u8> = (0..w * h).map(|i| ((i % w) * 255 / (w - 1)) as u8).collect();
        let mut uv = vec![0u8; w * (h / 2)];
        for r in 0..h / 2 {
            for p in 0..w / 2 {
                uv[r * w + 2 * p] = (p * 255 / (w / 2 - 1)) as u8;
                uv[r * w + 2 * p + 1] = 128;
            }
        }
        let (sy, suv) = converte(&y, &uv, 1280, 720, 640, 360, w, w);
        for r in 0..360 {
            let linha = &sy[r * 640..(r + 1) * 640];
            assert!(linha.windows(2).all(|p| p[0] <= p[1]), "Y não monotono na linha {r}");
        }
        for r in 0..180 {
            let linha = &suv[r * 640..(r + 1) * 640];
            let cb: Vec<u8> = linha.iter().step_by(2).copied().collect();
            let cr: Vec<u8> = linha.iter().skip(1).step_by(2).copied().collect();
            assert!(cb.windows(2).all(|p| p[0] <= p[1]), "Cb não monotono na linha {r}");
            assert!(cr.iter().all(|&v| v == 128), "Cr fora do fixo na linha {r}");
        }
    }

    #[test]
    fn nv12_faixas_iguais_ao_todo() {
        let (in_w, in_h, ow, oh) = (1280u32, 720u32, 640u32, 384u32);
        let (w, h) = (1280usize, 720usize);
        let mut rng = Lcg(42);
        let y: Vec<u8> = (0..w * h).map(|_| rng.byte()).collect();
        let uv: Vec<u8> = (0..w * (h / 2)).map(|_| rng.byte()).collect();
        let (inteiro_y, inteiro_uv) = converte(&y, &uv, in_w, in_h, ow, oh, w, w);

        // Mesma conversão em faixas de 64 linhas de luma (32 de croma), uma por vez.
        let eixos = Eixos::new(in_w, in_h, PixelFormat::Nv12, ow, oh);
        let (oww, ohh) = (ow as usize, oh as usize);
        let mut sy = vec![0u8; oww * ohh];
        let mut suv = vec![0u8; oww * ohh / 2];
        let mut scratch = Scratch::new(in_w, ow);
        for (k, (fy, fuv)) in sy.chunks_mut(64 * oww).zip(suv.chunks_mut(32 * oww)).enumerate() {
            let dst = FaixaDst {
                y: fy,
                y_stride: oww,
                uv: fuv,
                uv_stride: oww,
                y0: k * 64,
                linhas: 64,
                out_w: ow,
            };
            faixa_nv12(&y, w, &uv, w, in_w, in_h, &eixos, dst, &mut scratch);
        }
        assert_eq!(sy, inteiro_y, "Y das faixas difere da conversão inteira");
        assert_eq!(suv, inteiro_uv, "UV das faixas difere da conversão inteira");
    }
}
