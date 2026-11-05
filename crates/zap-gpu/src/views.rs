//! Cache das visões do Video Processor (evita criar duas por quadro dentro do lock).
use crate::GpuError;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, ID3D11VideoDevice, ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView,
    ID3D11VideoProcessorOutputView, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D,
};

/// Máximo de visões de entrada e, separadamente, de saída.
pub const MAX_VIEWS: usize = 8;

fn os(ctx: &'static str, e: windows::core::Error) -> GpuError {
    GpuError::Os {
        context: ctx,
        message: e.message(),
    }
}

/// Cache: chave = ponteiro cru da textura; guarda um clone COM da textura para o ponteiro não ser
/// reaproveitado por outra textura. Cheio -> esvazia e recomeça.
#[derive(Default)]
pub(crate) struct ViewCache {
    pub(crate) inputs: Vec<(usize, ID3D11Texture2D, ID3D11VideoProcessorInputView)>,
    pub(crate) outputs: Vec<(usize, ID3D11Texture2D, ID3D11VideoProcessorOutputView)>,
}

impl ViewCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    /// Visão de entrada da textura (cria e guarda se não houver).
    pub(crate) fn input(
        &mut self,
        vd: &ID3D11VideoDevice,
        enumerator: &ID3D11VideoProcessorEnumerator,
        tex: &ID3D11Texture2D,
    ) -> Result<ID3D11VideoProcessorInputView, GpuError> {
        let chave = tex.as_raw() as usize;
        if let Some((_, _, v)) = self.inputs.iter().find(|(k, _, _)| *k == chave) {
            return Ok(v.clone());
        }
        if self.inputs.len() >= MAX_VIEWS {
            self.inputs.clear();
        }
        let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut out: Option<ID3D11VideoProcessorInputView> = None;
        // SAFETY: textura, enumerador e descritor são válidos; `out` é saída local.
        unsafe { vd.CreateVideoProcessorInputView(tex, enumerator, &desc, Some(&mut out)) }
            .map_err(|e| os("CreateVideoProcessorInputView", e))?;
        let Some(visao) = out else {
            return Err(GpuError::Os {
                context: "CreateVideoProcessor*View",
                message: "visão nula após sucesso".to_string(),
            });
        };
        self.inputs.push((chave, tex.clone(), visao.clone()));
        Ok(visao)
    }
    /// Visão de saída da textura (cria e guarda se não houver).
    pub(crate) fn output(
        &mut self,
        vd: &ID3D11VideoDevice,
        enumerator: &ID3D11VideoProcessorEnumerator,
        tex: &ID3D11Texture2D,
    ) -> Result<ID3D11VideoProcessorOutputView, GpuError> {
        let chave = tex.as_raw() as usize;
        if let Some((_, _, v)) = self.outputs.iter().find(|(k, _, _)| *k == chave) {
            return Ok(v.clone());
        }
        if self.outputs.len() >= MAX_VIEWS {
            self.outputs.clear();
        }
        let desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let mut out: Option<ID3D11VideoProcessorOutputView> = None;
        // SAFETY: idem acima, para a visão de saída.
        unsafe { vd.CreateVideoProcessorOutputView(tex, enumerator, &desc, Some(&mut out)) }
            .map_err(|e| os("CreateVideoProcessorOutputView", e))?;
        let Some(visao) = out else {
            return Err(GpuError::Os {
                context: "CreateVideoProcessor*View",
                message: "visão nula após sucesso".to_string(),
            });
        };
        self.outputs.push((chave, tex.clone(), visao.clone()));
        Ok(visao)
    }
    /// (visões de entrada, visões de saída) guardadas.
    pub(crate) fn len(&self) -> (usize, usize) {
        (self.inputs.len(), self.outputs.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GpuDevice, PixelFormat, Texture};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_RATIONAL;

    fn preparo() -> Option<(GpuDevice, ID3D11VideoProcessorEnumerator)> {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("sem GPU, pulando: {e}");
                return None;
            }
        };
        if !dev.has_video() {
            eprintln!("sem Video Processor, pulando");
            return None;
        }
        let taxa = DXGI_RATIONAL {
            Numerator: 60,
            Denominator: 1,
        };
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: taxa,
            InputWidth: 1280,
            InputHeight: 720,
            OutputFrameRate: taxa,
            OutputWidth: 1280,
            OutputHeight: 720,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        let en = {
            let (vd, _) = dev.video().ok()?;
            // SAFETY: `desc` é válido durante a chamada.
            unsafe { vd.CreateVideoProcessorEnumerator(&desc) }.ok()?
        };
        Some((dev, en))
    }

    #[test]
    fn mesma_textura_reusa_visao() {
        let Some((dev, en)) = preparo() else { return };
        let tex = Texture::new(&dev, 1280, 720, PixelFormat::Bgra8, false).unwrap();
        let (vd, _) = dev.video().unwrap();
        let mut c = ViewCache::new();
        c.input(vd, &en, tex.raw()).unwrap();
        c.input(vd, &en, tex.raw()).unwrap();
        assert_eq!(c.len().0, 1);
    }

    #[test]
    fn cache_cheio_esvazia() {
        let Some((dev, en)) = preparo() else { return };
        let (vd, _) = dev.video().unwrap();
        let mut c = ViewCache::new();
        let texs: Vec<Texture> = (0..9)
            .map(|_| Texture::new(&dev, 1280, 720, PixelFormat::Bgra8, false).unwrap())
            .collect();
        for t in &texs {
            c.input(vd, &en, t.raw()).unwrap();
            assert!(c.len().0 <= MAX_VIEWS);
        }
        assert_eq!(c.len().0, 1);
    }

    #[test]
    fn entrada_e_saida_contadas_separadas() {
        let Some((dev, en)) = preparo() else { return };
        let (vd, _) = dev.video().unwrap();
        let a = Texture::new(&dev, 1280, 720, PixelFormat::Bgra8, false).unwrap();
        let b = Texture::new(&dev, 1280, 720, PixelFormat::Nv12, false).unwrap();
        let mut c = ViewCache::new();
        c.input(vd, &en, a.raw()).unwrap();
        c.output(vd, &en, b.raw()).unwrap();
        assert_eq!(c.len(), (1, 1));
    }
}
