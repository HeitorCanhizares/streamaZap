//! Conversor de CPU portável: BGRA/NV12 para NV12 (BT.709 faixa limitada), no mesmo filtro do
//! Video Processor da NVIDIA (bilinear com centros alinhados). Rust seguro, sem I/O.
//! Detalhes do algoritmo: `docs/planos/m2b-conversao-cpu.md` §3 e §5.1 a.

mod bgra;
mod eixo;
mod identidade;
mod lote;
mod nv12;

pub use lote::convert_batch;

use crate::{GpuError, PixelFormat};
use eixo::Eixos;

/// Imagem de origem na memória (pode ser uma staging mapeada).
#[derive(Clone, Copy, Debug)]
pub enum SrcImage<'a> {
    /// B, G, R, A por pixel (alfa ignorado). `stride` em bytes, >= width * 4.
    Bgra {
        data: &'a [u8],
        stride: usize,
        width: u32,
        height: u32,
    },
    /// Plano Y e plano UV intercalado (Cb, Cr), BT.709 faixa limitada. `width` e `height` pares;
    /// strides >= width.
    Nv12 {
        y: &'a [u8],
        y_stride: usize,
        uv: &'a [u8],
        uv_stride: usize,
        width: u32,
        height: u32,
    },
}

impl SrcImage<'_> {
    pub fn width(&self) -> u32 {
        match self {
            Self::Bgra { width, .. } | Self::Nv12 { width, .. } => *width,
        }
    }
    pub fn height(&self) -> u32 {
        match self {
            Self::Bgra { height, .. } | Self::Nv12 { height, .. } => *height,
        }
    }
    pub fn format(&self) -> PixelFormat {
        match self {
            Self::Bgra { .. } => PixelFormat::Bgra8,
            Self::Nv12 { .. } => PixelFormat::Nv12,
        }
    }
}

/// Destino NV12 na memória (pode ser uma staging mapeada).
#[derive(Debug)]
pub struct Nv12Mut<'a> {
    pub y: &'a mut [u8],
    pub y_stride: usize,
    pub uv: &'a mut [u8],
    pub uv_stride: usize,
    pub width: u32,
    pub height: u32,
}

impl<'a> Nv12Mut<'a> {
    /// Buffer único no formato do mapeamento de uma textura NV12 e do `read_back`: `height` linhas
    /// de Y e `height / 2` de UV, todas com `stride` bytes, UV começando em `stride * height`.
    /// Precisa de ao menos `stride * (height + height / 2 - 1) + width` bytes. Buffer curto,
    /// `stride < width`, tamanho zero ou ímpar: `InvalidArgument`.
    pub fn from_contiguous(buf: &'a mut [u8], stride: usize, width: u32, height: u32) -> Result<Self, GpuError> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(GpuError::InvalidArgument(format!(
                "NV12 exige largura e altura pares e não nulas, veio {width}x{height}"
            )));
        }
        let w = width as usize;
        let h = height as usize;
        if stride < w {
            return Err(GpuError::InvalidArgument(format!(
                "stride ({stride}) menor que a largura ({width})"
            )));
        }
        // Bytes necessários: `h` linhas de Y e `h / 2 - 1` linhas de UV antes da última, mais a
        // última linha de UV, que ocupa só `width` bytes. Soma com checagem para não estourar.
        let necessario = h
            .checked_add(h / 2 - 1)
            .and_then(|linhas| stride.checked_mul(linhas))
            .and_then(|bytes| bytes.checked_add(w));
        let Some(necessario) = necessario else {
            return Err(GpuError::InvalidArgument(format!(
                "dimensões de buffer NV12 fora do intervalo: stride {stride}, {width}x{height}"
            )));
        };
        if buf.len() < necessario {
            return Err(GpuError::InvalidArgument(format!(
                "buffer NV12 curto: {} bytes, precisa de {necessario}",
                buf.len()
            )));
        }
        // Com o buffer validado, `stride * h` cabe dentro de `necessario` (h >= 2).
        let (y, uv) = buf.split_at_mut(stride * h);
        Ok(Self {
            y,
            y_stride: stride,
            uv,
            uv_stride: stride,
            width,
            height,
        })
    }
}

/// Conversor/escalador de CPU para um par (entrada, saída), com os pesos já calculados.
#[derive(Debug, Clone)]
pub struct CpuScaler {
    pub(crate) in_w: u32,
    pub(crate) in_h: u32,
    pub(crate) in_fmt: PixelFormat,
    pub(crate) out_w: u32,
    pub(crate) out_h: u32,
    pub(crate) eixos: Eixos,
}

impl CpuScaler {
    /// Entrada BGRA: qualquer tamanho >= 1x1; NV12: par >= 2x2. Saída: par >= 2x2. Cada lado
    /// <= 16384. Fora disso: `InvalidArgument`.
    pub fn new(in_w: u32, in_h: u32, in_fmt: PixelFormat, out_w: u32, out_h: u32) -> Result<Self, GpuError> {
        const LADO_MAX: u32 = 16384;
        for (nome, lado) in [
            ("largura de entrada", in_w),
            ("altura de entrada", in_h),
            ("largura de saída", out_w),
            ("altura de saída", out_h),
        ] {
            if lado == 0 || lado > LADO_MAX {
                return Err(GpuError::InvalidArgument(format!(
                    "{nome} fora do intervalo 1..={LADO_MAX}: {lado}"
                )));
            }
        }
        if out_w < 2 || out_h < 2 || out_w % 2 != 0 || out_h % 2 != 0 {
            return Err(GpuError::InvalidArgument(format!(
                "saída precisa ser par e de pelo menos 2x2, veio {out_w}x{out_h}"
            )));
        }
        if in_fmt == PixelFormat::Nv12 && (in_w < 2 || in_h < 2 || in_w % 2 != 0 || in_h % 2 != 0) {
            return Err(GpuError::InvalidArgument(format!(
                "entrada NV12 precisa ser par e de pelo menos 2x2, veio {in_w}x{in_h}"
            )));
        }
        Ok(Self {
            in_w,
            in_h,
            in_fmt,
            out_w,
            out_h,
            eixos: Eixos::new(in_w, in_h, in_fmt, out_w, out_h),
        })
    }
    pub fn input(&self) -> (u32, u32, PixelFormat) {
        (self.in_w, self.in_h, self.in_fmt)
    }
    pub fn output(&self) -> (u32, u32) {
        (self.out_w, self.out_h)
    }
    /// Atalho para `convert_batch` com um trabalho só.
    pub fn convert(&self, src: &SrcImage<'_>, dst: &mut Nv12Mut<'_>, opts: BatchOptions) -> Result<(), GpuError> {
        let d = Nv12Mut {
            y: &mut *dst.y,
            y_stride: dst.y_stride,
            uv: &mut *dst.uv,
            uv_stride: dst.uv_stride,
            width: dst.width,
            height: dst.height,
        };
        convert_batch(src, &mut [(self, d)], opts)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BatchOptions {
    /// Threads ao todo, contando a que chama; 0 conta como 1.
    pub threads: usize,
    /// Roda no início de cada thread criada (não na que chama). O `zap-media` usa para a prioridade.
    pub on_thread_start: Option<fn()>,
}

impl Default for BatchOptions {
    /// `threads: cpu_threads()`, `on_thread_start: None`.
    fn default() -> Self {
        Self {
            threads: cpu_threads(),
            on_thread_start: None,
        }
    }
}

/// `clamp(available_parallelism / 2, 1, 4)`; 2 se o sistema não informar.
pub fn cpu_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(2)
        .clamp(1, 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_contiguous_separa() {
        // stride 80, 64x32: Y ocupa 80*32 bytes; UV vem depois, com 80*15 + 64 bytes.
        let total = 80 * (32 + 16 - 1) + 64;
        let mut buf = vec![0u8; total];
        {
            let d = Nv12Mut::from_contiguous(&mut buf, 80, 64, 32).unwrap();
            assert_eq!(d.y.len(), 80 * 32);
            assert_eq!(d.uv.len(), total - 80 * 32);
            assert_eq!(d.y_stride, 80);
            assert_eq!(d.uv_stride, 80);
            assert_eq!((d.width, d.height), (64, 32));
            d.y[0] = 7;
            d.uv[0] = 9;
        }
        assert_eq!(buf[0], 7);
        assert_eq!(buf[80 * 32], 9);
    }

    #[test]
    fn from_contiguous_invalidos() {
        let minimo = 80 * (32 + 16 - 1) + 64;
        let mut curto = vec![0u8; minimo - 1];
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut curto, 80, 64, 32),
            Err(GpuError::InvalidArgument(_))
        ));
        let mut buf = vec![0u8; 4096];
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut buf, 60, 64, 32),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut buf, 80, 0, 32),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut buf, 80, 64, 0),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut buf, 80, 63, 32),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            Nv12Mut::from_contiguous(&mut buf, 80, 64, 31),
            Err(GpuError::InvalidArgument(_))
        ));
        // O mínimo exato é aceito.
        let mut exato = vec![0u8; minimo];
        assert!(Nv12Mut::from_contiguous(&mut exato, 80, 64, 32).is_ok());
    }

    #[test]
    fn new_validacao() {
        use PixelFormat::{Bgra8, Nv12};
        let invalido = |r: Result<CpuScaler, GpuError>| matches!(r, Err(GpuError::InvalidArgument(_)));
        assert!(invalido(CpuScaler::new(640, 360, Bgra8, 641, 360)), "saída ímpar");
        assert!(invalido(CpuScaler::new(640, 360, Bgra8, 0, 360)), "saída zero");
        assert!(
            invalido(CpuScaler::new(641, 360, Nv12, 640, 360)),
            "NV12 de entrada ímpar"
        );
        assert!(invalido(CpuScaler::new(16385, 16, Bgra8, 64, 64)), "lado 16385");
        assert!(
            invalido(CpuScaler::new(64, 64, Bgra8, 16386, 64)),
            "saída acima de 16384"
        );

        assert!(CpuScaler::new(641, 361, Bgra8, 642, 360).is_ok());
        assert!(CpuScaler::new(1, 1, Bgra8, 2, 2).is_ok());
        let s = CpuScaler::new(1280, 720, Nv12, 640, 360).unwrap();
        assert_eq!(s.input(), (1280, 720, Nv12));
        assert_eq!(s.output(), (640, 360));
    }

    #[test]
    fn cpu_threads_entre_1_e_4() {
        assert!((1..=4).contains(&cpu_threads()));
    }

    #[test]
    fn batch_options_default() {
        let o = BatchOptions::default();
        assert_eq!(o.threads, cpu_threads());
        assert!(o.on_thread_start.is_none());
    }
}
