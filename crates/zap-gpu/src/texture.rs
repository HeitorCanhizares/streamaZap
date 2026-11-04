use crate::{GpuDevice, GpuError, PixelFormat};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_GDI_COMPATIBLE,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};

/// Textura 2D na GPU com formato e tamanho conhecidos.
pub struct Texture {
    pub(crate) inner: ID3D11Texture2D,
    pub(crate) format: PixelFormat,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

// SAFETY: a textura pertence a um dispositivo criado com SetMultithreadProtected(TRUE).
unsafe impl Send for Texture {}

impl Texture {
    /// Cria uma textura DEFAULT. Bgra8 -> `DXGI_FORMAT_B8G8R8A8_UNORM` com bind
    /// `SHADER_RESOURCE | RENDER_TARGET`; Nv12 -> `DXGI_FORMAT_NV12` com `RENDER_TARGET | SHADER_RESOURCE`.
    /// `gdi_compatible` (só Bgra8) liga `D3D11_RESOURCE_MISC_GDI_COMPATIBLE`.
    /// Largura/altura 0 (ou ímpares em Nv12): `GpuError::InvalidArgument`.
    pub fn new(
        dev: &GpuDevice,
        width: u32,
        height: u32,
        format: PixelFormat,
        gdi_compatible: bool,
    ) -> Result<Self, GpuError> {
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidArgument(format!(
                "tamanho de textura inválido: {width}x{height}"
            )));
        }
        if format == PixelFormat::Nv12 && (width % 2 != 0 || height % 2 != 0) {
            return Err(GpuError::InvalidArgument(format!(
                "NV12 exige largura e altura pares: {width}x{height}"
            )));
        }
        let (dxgi, misc) = match format {
            PixelFormat::Bgra8 => (
                DXGI_FORMAT_B8G8R8A8_UNORM,
                if gdi_compatible {
                    D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32
                } else {
                    0
                },
            ),
            PixelFormat::Nv12 => {
                if gdi_compatible {
                    tracing::debug!("gdi_compatible ignorado em texturas NV12");
                }
                (DXGI_FORMAT_NV12, 0)
            }
        };
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: dxgi,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: misc,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        // SAFETY: `desc` e `tex` são válidos durante a chamada; o dispositivo é válido.
        unsafe { dev.d3d_device().CreateTexture2D(&desc, None, Some(&mut tex)) }.map_err(|e| GpuError::Os {
            context: "CreateTexture2D",
            message: e.message(),
        })?;
        let inner = tex.ok_or_else(|| GpuError::Os {
            context: "CreateTexture2D",
            message: "textura nula após sucesso".to_string(),
        })?;
        Ok(Self {
            inner,
            format,
            width,
            height,
        })
    }
    /// Embrulha uma textura criada por outro código (captura WGC/câmera).
    pub fn from_raw(inner: ID3D11Texture2D, format: PixelFormat, width: u32, height: u32) -> Self {
        Self {
            inner,
            format,
            width,
            height,
        }
    }
    pub fn raw(&self) -> &ID3D11Texture2D {
        &self.inner
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn format(&self) -> PixelFormat {
        self.format
    }
}

/// N texturas iguais devolvidas em rodízio.
pub struct TextureRing {
    pub(crate) textures: Vec<Texture>,
    pub(crate) next: usize,
}

impl TextureRing {
    /// `n == 0` -> `InvalidArgument`. Cria `n` texturas com `Texture::new`.
    pub fn new(
        dev: &GpuDevice,
        w: u32,
        h: u32,
        fmt: PixelFormat,
        n: usize,
        gdi_compatible: bool,
    ) -> Result<Self, GpuError> {
        if n == 0 {
            return Err(GpuError::InvalidArgument(
                "o anel precisa de ao menos 1 textura".to_string(),
            ));
        }
        let textures = (0..n)
            .map(|_| Texture::new(dev, w, h, fmt, gdi_compatible))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { textures, next: 0 })
    }
    /// Próxima textura em rodízio (0, 1, ..., n-1, 0, ...).
    #[allow(clippy::should_implement_trait)] // assinatura fixada pela spec
    pub fn next(&mut self) -> &Texture {
        let i = self.next;
        self.next = (i + 1) % self.textures.len();
        &self.textures[i]
    }
    pub fn len(&self) -> usize {
        self.textures.len()
    }
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("sem GPU, pulando: {e}");
                None
            }
        }
    }

    #[test]
    fn texture_bgra_tamanho() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 640, 360, PixelFormat::Bgra8, false).unwrap();
        assert_eq!(t.width(), 640);
        assert_eq!(t.height(), 360);
        assert_eq!(t.format(), PixelFormat::Bgra8);
    }

    #[test]
    fn texture_nv12_impar_recusa() {
        let Some(dev) = dispositivo() else { return };
        let r = Texture::new(&dev, 641, 360, PixelFormat::Nv12, false);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }

    #[test]
    fn texture_zero_recusa() {
        let Some(dev) = dispositivo() else { return };
        let r = Texture::new(&dev, 0, 360, PixelFormat::Bgra8, false);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
        let r = Texture::new(&dev, 640, 0, PixelFormat::Bgra8, false);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }

    #[test]
    fn ring_rodizio() {
        use windows::core::Interface;
        let Some(dev) = dispositivo() else { return };
        let mut ring = TextureRing::new(&dev, 64, 64, PixelFormat::Bgra8, 3, false).unwrap();
        let ptrs: Vec<_> = (0..6).map(|_| ring.next().raw().as_raw()).collect();
        assert_eq!(ptrs[0], ptrs[3]);
        assert_eq!(ptrs[1], ptrs[4]);
        assert_eq!(ptrs[2], ptrs[5]);
        assert_ne!(ptrs[0], ptrs[1]);
        assert_ne!(ptrs[1], ptrs[2]);
    }

    #[test]
    fn ring_n_zero_recusa() {
        let Some(dev) = dispositivo() else { return };
        let r = TextureRing::new(&dev, 64, 64, PixelFormat::Bgra8, 0, false);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }
}
