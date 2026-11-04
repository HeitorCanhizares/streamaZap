use crate::{GpuDevice, GpuError, PixelFormat, Texture};
use std::mem::ManuallyDrop;
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11VideoContext1, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView,
    ID3D11VideoProcessorOutputView, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT,
    D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
    D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709, DXGI_FORMAT,
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL,
};

fn os(ctx: &'static str, e: windows::core::Error) -> GpuError {
    GpuError::Os {
        context: ctx,
        message: e.message(),
    }
}

fn dxgi_de(f: PixelFormat) -> DXGI_FORMAT {
    match f {
        PixelFormat::Bgra8 => DXGI_FORMAT_B8G8R8A8_UNORM,
        PixelFormat::Nv12 => DXGI_FORMAT_NV12,
    }
}

/// Monta o bitfield de `D3D11_VIDEO_PROCESSOR_COLOR_SPACE`: Usage bit0, RGB_Range bit1,
/// YCbCr_Matrix bit2, xvYCC bit3, Nominal_Range bits 4-5.
fn color_space(rgb_range: u32, matrix: u32, nominal: u32) -> D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
        _bitfield: (rgb_range << 1) | (matrix << 2) | (nominal << 4),
    }
}

/// Conversor/escalador por `ID3D11VideoProcessor`: BGRA ou NV12 -> NV12 BT.709 faixa limitada.
pub struct Converter {
    pub(crate) enumerator: ID3D11VideoProcessorEnumerator,
    pub(crate) processor: ID3D11VideoProcessor,
    pub(crate) in_w: u32,
    pub(crate) in_h: u32,
    pub(crate) in_fmt: PixelFormat,
    pub(crate) out_w: u32,
    pub(crate) out_h: u32,
}

// SAFETY: criado a partir de um dispositivo com multithread protegido.
unsafe impl Send for Converter {}

impl Converter {
    pub fn new(
        dev: &GpuDevice,
        in_w: u32,
        in_h: u32,
        in_fmt: PixelFormat,
        out_w: u32,
        out_h: u32,
    ) -> Result<Self, GpuError> {
        for (w, h) in [(in_w, in_h), (out_w, out_h)] {
            if w == 0 || h == 0 || w % 2 != 0 || h % 2 != 0 {
                return Err(GpuError::InvalidArgument(format!(
                    "tamanho inválido para o conversor (precisa ser par e maior que 0): {w}x{h}"
                )));
            }
        }
        let taxa = DXGI_RATIONAL {
            Numerator: 60,
            Denominator: 1,
        };
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: taxa,
            InputWidth: in_w,
            InputHeight: in_h,
            OutputFrameRate: taxa,
            OutputWidth: out_w,
            OutputHeight: out_h,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        // SAFETY: `desc` é válido durante a chamada; o dispositivo de vídeo é válido.
        let enumerator = unsafe { dev.video()?.0.CreateVideoProcessorEnumerator(&desc) }
            .map_err(|e| os("CreateVideoProcessorEnumerator", e))?;

        let suporta = |fmt: DXGI_FORMAT, flag: i32| -> Result<bool, GpuError> {
            // SAFETY: `enumerator` é um objeto COM válido.
            let bits =
                unsafe { enumerator.CheckVideoProcessorFormat(fmt) }.map_err(|e| os("CheckVideoProcessorFormat", e))?;
            Ok(bits & (flag as u32) != 0)
        };
        if !suporta(dxgi_de(in_fmt), D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT.0)?
            || !suporta(DXGI_FORMAT_NV12, D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT.0)?
        {
            return Err(GpuError::Unsupported);
        }

        // SAFETY: `enumerator` é válido; índice 0 de conversão de taxa sempre existe.
        let processor = unsafe { dev.video()?.0.CreateVideoProcessor(&enumerator, 0) }
            .map_err(|e| os("CreateVideoProcessor", e))?;

        // Espaço de cor (versão bitfield). Entrada RGB: 0-255; entrada NV12: 16-235. Saída: BT.709 16-235.
        let entrada = match in_fmt {
            PixelFormat::Bgra8 => color_space(0, 1, 1),
            PixelFormat::Nv12 => color_space(1, 1, 2),
        };
        let saida = color_space(0, 1, 2);
        let src_rect = RECT {
            left: 0,
            top: 0,
            right: in_w as i32,
            bottom: in_h as i32,
        };
        let dst_rect = RECT {
            left: 0,
            top: 0,
            right: out_w as i32,
            bottom: out_h as i32,
        };
        let ctx = dev.video()?.1;
        // SAFETY: `processor` e `ctx` são válidos; os ponteiros apontam para locais vivos na chamada.
        unsafe {
            ctx.VideoProcessorSetStreamColorSpace(&processor, 0, &entrada);
            ctx.VideoProcessorSetOutputColorSpace(&processor, &saida);
            ctx.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&src_rect));
            ctx.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&dst_rect));
            ctx.VideoProcessorSetOutputTargetRect(&processor, true, Some(&dst_rect));
        }
        // Versão com tipos DXGI, mais explícita; se o driver não expõe a interface, fica o bitfield.
        if let Ok(ctx1) = ctx.cast::<ID3D11VideoContext1>() {
            let cs_in = match in_fmt {
                PixelFormat::Bgra8 => DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
                PixelFormat::Nv12 => DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            };
            // SAFETY: `ctx1` e `processor` são válidos.
            unsafe {
                ctx1.VideoProcessorSetStreamColorSpace1(&processor, 0, cs_in);
                ctx1.VideoProcessorSetOutputColorSpace1(&processor, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709);
            }
        }
        Ok(Self {
            enumerator,
            processor,
            in_w,
            in_h,
            in_fmt,
            out_w,
            out_h,
        })
    }
    /// Converte/escala `src` para NV12 em `dst`. `src` e `dst` devem ter exatamente os tamanhos e
    /// o formato de entrada dados em `new` (senão `InvalidArgument`); `dst` deve ser Nv12.
    /// Espaço de cor: entrada RGB 0-255 (ou YCbCr BT.709 limitado se NV12) -> saída YCbCr BT.709
    /// faixa limitada. Usa `VideoProcessorBlt`.
    pub fn convert(&mut self, dev: &GpuDevice, src: &Texture, dst: &Texture) -> Result<(), GpuError> {
        if src.width() != self.in_w || src.height() != self.in_h || src.format() != self.in_fmt {
            return Err(GpuError::InvalidArgument(format!(
                "origem {}x{} {:?} não bate com o conversor ({}x{} {:?})",
                src.width(),
                src.height(),
                src.format(),
                self.in_w,
                self.in_h,
                self.in_fmt
            )));
        }
        if dst.format() != PixelFormat::Nv12 || dst.width() != self.out_w || dst.height() != self.out_h {
            return Err(GpuError::InvalidArgument(format!(
                "destino {}x{} {:?} deve ser Nv12 {}x{}",
                dst.width(),
                dst.height(),
                dst.format(),
                self.out_w,
                self.out_h
            )));
        }
        let in_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let out_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let mut in_view: Option<ID3D11VideoProcessorInputView> = None;
        // SAFETY: textura, enumerador e descritor são válidos; `in_view` é saída local.
        unsafe {
            dev.video()?
                .0
                .CreateVideoProcessorInputView(src.raw(), &self.enumerator, &in_desc, Some(&mut in_view))
        }
        .map_err(|e| os("CreateVideoProcessorInputView", e))?;
        let mut out_view: Option<ID3D11VideoProcessorOutputView> = None;
        // SAFETY: idem acima, para a visão de saída.
        unsafe {
            dev.video()?
                .0
                .CreateVideoProcessorOutputView(dst.raw(), &self.enumerator, &out_desc, Some(&mut out_view))
        }
        .map_err(|e| os("CreateVideoProcessorOutputView", e))?;
        let (Some(in_view), Some(out_view)) = (in_view, out_view) else {
            return Err(GpuError::Os {
                context: "CreateVideoProcessor*View",
                message: "visão nula após sucesso".to_string(),
            });
        };

        let stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            OutputIndex: 0,
            InputFrameOrField: 0,
            PastFrames: 0,
            FutureFrames: 0,
            ppPastSurfaces: std::ptr::null_mut(),
            pInputSurface: ManuallyDrop::new(Some(in_view)),
            ppFutureSurfaces: std::ptr::null_mut(),
            ppPastSurfacesRight: std::ptr::null_mut(),
            pInputSurfaceRight: ManuallyDrop::new(None),
            ppFutureSurfacesRight: std::ptr::null_mut(),
        };
        let streams = [stream];
        // SAFETY: processador, visão de saída e o stream (com a visão de entrada viva) são válidos.
        let r = unsafe {
            dev.video()?
                .1
                .VideoProcessorBlt(&self.processor, &out_view, 0, &streams)
        };
        // Libera a visão de entrada: o ManuallyDrop não faz isso sozinho.
        let [mut stream] = streams;
        // SAFETY: `pInputSurface` foi inicializado acima e não é usado depois.
        unsafe { ManuallyDrop::drop(&mut stream.pInputSurface) };
        r.map_err(|e| os("VideoProcessorBlt", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tamanho_impar_recusa() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        if !dev.has_video() {
            eprintln!("pulando: GPU sem Video Processor");
            return;
        }
        let r = Converter::new(&dev, 641, 360, PixelFormat::Bgra8, 640, 360);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }

    #[test]
    fn new_ok_1080_para_720() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        match Converter::new(&dev, 1920, 1080, PixelFormat::Bgra8, 1280, 720) {
            Ok(_) => {}
            Err(GpuError::Unsupported) => eprintln!("pulando: formato sem suporte no driver"),
            Err(e) => panic!("falhou: {e}"),
        }
    }

    #[test]
    fn convert_tamanho_errado_recusa() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let Ok(mut c) = Converter::new(&dev, 1280, 720, PixelFormat::Bgra8, 1280, 720) else {
            eprintln!("pulando: sem conversor");
            return;
        };
        let (Ok(src), Ok(dst)) = (
            Texture::new(&dev, 640, 360, PixelFormat::Bgra8, false),
            Texture::new(&dev, 1280, 720, PixelFormat::Nv12, false),
        ) else {
            eprintln!("pulando: sem texturas");
            return;
        };
        assert!(matches!(c.convert(&dev, &src, &dst), Err(GpuError::InvalidArgument(_))));
    }
}
