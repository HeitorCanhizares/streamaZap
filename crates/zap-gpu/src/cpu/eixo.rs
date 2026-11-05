//! Pesos bilineares por eixo e tipos compartilhados pelos laços de pixel.
use crate::PixelFormat;

/// Dois taps de um pixel de saída: peso `1 - f` em `i0` e `f` em `i1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tap {
    pub i0: u32,
    pub i1: u32,
    pub f: f32,
}

/// Para cada `o` em `0..out`, em f64: `s = ((o + 0,5) * inp / out - 0,5).clamp(0, inp - 1)`,
/// `i0 = floor(s)`, `i1 = min(i0 + 1, inp - 1)`, `f = s - i0` (guardado em f32).
pub(crate) fn bilinear(inp: u32, out: u32) -> Vec<Tap> {
    // Cálculo em f64: com f32 o erro de arredondamento desloca os taps em imagens grandes.
    let ultimo = inp - 1;
    (0..out)
        .map(|o| {
            let s = ((o as f64 + 0.5) * inp as f64 / out as f64 - 0.5).clamp(0.0, ultimo as f64);
            let i0 = s.floor() as u32;
            let i1 = (i0 + 1).min(ultimo);
            // A fração é guardada em f32: o peso final cabe nisso sem perda perceptível.
            let f = (s - i0 as f64) as f32;
            Tap { i0, i1, f }
        })
        .collect()
}

/// Os 4 eixos de um par (entrada, saída).
#[derive(Debug, Clone)]
pub(crate) struct Eixos {
    /// Luma horizontal: `bilinear(in_w, out_w)`.
    pub lx: Vec<Tap>,
    /// Luma vertical: `bilinear(in_h, out_h)`.
    pub ly: Vec<Tap>,
    /// Croma horizontal: BGRA `bilinear(in_w, out_w/2)`; NV12 `bilinear(in_w/2, out_w/2)`.
    pub cx: Vec<Tap>,
    /// Croma vertical: BGRA `bilinear(in_h, out_h/2)`; NV12 `bilinear(in_h/2, out_h/2)`.
    pub cy: Vec<Tap>,
    /// Entrada e saída com o mesmo tamanho (atalho inteiro no BGRA, cópia no NV12).
    pub identidade: bool,
}

impl Eixos {
    /// Os tamanhos já foram validados por `CpuScaler::new`.
    pub(crate) fn new(in_w: u32, in_h: u32, in_fmt: PixelFormat, out_w: u32, out_h: u32) -> Self {
        let (cx, cy) = match in_fmt {
            // BGRA: croma com a mesma resolução da entrada, amostrada na metade da saída.
            PixelFormat::Bgra8 => (bilinear(in_w, out_w / 2), bilinear(in_h, out_h / 2)),
            // NV12: a croma já é metade da entrada (CbCr intercalado, 1 amostra por 2x2).
            PixelFormat::Nv12 => (bilinear(in_w / 2, out_w / 2), bilinear(in_h / 2, out_h / 2)),
        };
        Self {
            lx: bilinear(in_w, out_w),
            ly: bilinear(in_h, out_h),
            cx,
            cy,
            identidade: in_w == out_w && in_h == out_h,
        }
    }
}

/// Rascunho de uma thread, alocado uma vez por chamada de `convert_batch`.
#[derive(Debug)]
pub(crate) struct Scratch {
    /// Mistura vertical de uma linha da origem: `max_in_w * 4` f32 (BGRA; o NV12 usa o que precisar).
    pub col: Vec<f32>,
    /// Uma linha de pixels horizontais (`[f32; 4]`), `max_out_w` elementos. Basta uma: cada linha
    /// de saída é misturada na vertical antes (`col`) e convertida logo depois.
    pub a: Vec<[f32; 4]>,
}

impl Scratch {
    /// `col` com `max_in_w * 4` f32 e `a` com `max_out_w` elementos, zerados.
    pub(crate) fn new(max_in_w: u32, max_out_w: u32) -> Self {
        Self {
            col: vec![0.0; max_in_w as usize * 4],
            a: vec![[0.0; 4]; max_out_w as usize],
        }
    }
}

/// Faixa de destino entregue a um laço de pixel: `y` e `uv` são os pedaços (`chunks_mut`) que
/// começam na linha de luma `y0` (par) e na linha de croma `y0 / 2`.
#[derive(Debug)]
pub(crate) struct FaixaDst<'a> {
    pub y: &'a mut [u8],
    pub y_stride: usize,
    pub uv: &'a mut [u8],
    pub uv_stride: usize,
    /// Primeira linha de luma da faixa na imagem de saída (par).
    pub y0: usize,
    /// Quantidade de linhas de luma da faixa (par).
    pub linhas: usize,
    pub out_w: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn eixo_casos_conhecidos() {
        // Mesmo tamanho: cada saída cai exatamente numa entrada.
        for (o, t) in bilinear(8, 8).iter().enumerate() {
            assert_eq!(t.i0, o as u32);
            assert_eq!(t.f, 0.0);
        }

        // Metade do tamanho: cada saída mistura duas entradas no meio.
        for (o, t) in bilinear(16, 8).iter().enumerate() {
            assert_eq!(t.i0, 2 * o as u32);
            assert_eq!(t.i1, 2 * o as u32 + 1);
            assert!(perto(t.f, 0.5), "o={o} f={}", t.f);
        }

        // Razão 3 exata: a posição cai sempre num pixel inteiro.
        for (o, t) in bilinear(1080, 360).iter().enumerate() {
            assert_eq!(t.i0, 3 * o as u32 + 1);
            assert_eq!(t.f, 0.0);
        }

        // Razão 1,5: o=0 cai em s=0,25 (fórmula da spec, sem arredondar para 0).
        // o=1 cai em s=1,75.
        let taps = bilinear(1920, 1280);
        assert_eq!(taps[0].i0, 0);
        assert_eq!(taps[0].i1, 1);
        assert!(perto(taps[0].f, 0.25), "f={}", taps[0].f);
        assert_eq!(taps[1].i0, 1);
        assert_eq!(taps[1].i1, 2);
        assert!(perto(taps[1].f, 0.75), "f={}", taps[1].f);

        // Entrada de 1 pixel: tudo aponta para o pixel 0.
        for t in bilinear(1, 4) {
            assert_eq!((t.i0, t.i1, t.f), (0, 0, 0.0));
        }
    }

    #[test]
    fn eixos_bgra_e_nv12_tamanhos() {
        let bgra = Eixos::new(1001, 777, PixelFormat::Bgra8, 1002, 778);
        assert_eq!(bgra.lx.len(), 1002);
        assert_eq!(bgra.ly.len(), 778);
        assert_eq!(bgra.cx.len(), 501);
        assert_eq!(bgra.cy.len(), 389);
        assert!(!bgra.identidade);

        let nv12 = Eixos::new(1280, 720, PixelFormat::Nv12, 640, 360);
        assert_eq!(nv12.lx.len(), 640);
        assert_eq!(nv12.ly.len(), 360);
        assert_eq!(nv12.cx.len(), 320);
        assert_eq!(nv12.cy.len(), 180);
        assert!(!nv12.identidade);

        let igual = Eixos::new(64, 64, PixelFormat::Bgra8, 64, 64);
        assert!(igual.identidade);
    }

    #[test]
    fn scratch_tamanhos() {
        let s = Scratch::new(1920, 1280);
        assert_eq!(s.col.len(), 1920 * 4);
        assert_eq!(s.a.len(), 1280);
        assert!(s.col.iter().all(|v| *v == 0.0));
        assert!(s.a.iter().all(|p| *p == [0.0; 4]));
    }
}
