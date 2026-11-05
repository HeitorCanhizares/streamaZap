//! Stub para plataformas sem D3D11: tudo devolve `GpuError::Unsupported`.
use crate::{AdapterInfo, GpuError, PixelFormat};

#[derive(Clone)]
pub struct GpuDevice {
    adapter: AdapterInfo,
}

impl GpuDevice {
    // Sem D3D11 não há dispositivo: a construção sempre falha, por isso `adapter` nunca é
    // preenchido de verdade aqui.
    pub fn create_default() -> Result<Self, GpuError> {
        Err(GpuError::Unsupported)
    }
    pub fn create_for_adapter(_luid: i64) -> Result<Self, GpuError> {
        Err(GpuError::Unsupported)
    }
    pub fn adapter(&self) -> &AdapterInfo {
        &self.adapter
    }
}

pub struct Texture {
    format: PixelFormat,
    width: u32,
    height: u32,
}

impl Texture {
    pub fn new(
        _dev: &GpuDevice,
        _width: u32,
        _height: u32,
        _format: PixelFormat,
        _gdi_compatible: bool,
    ) -> Result<Self, GpuError> {
        Err(GpuError::Unsupported)
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

pub struct TextureRing {
    textures: Vec<Texture>,
}

impl TextureRing {
    pub fn new(
        _dev: &GpuDevice,
        _w: u32,
        _h: u32,
        _fmt: PixelFormat,
        _n: usize,
        _gdi_compatible: bool,
    ) -> Result<Self, GpuError> {
        Err(GpuError::Unsupported)
    }
    // Mesmo nome da API do Windows (não é `Iterator`): o lint é falso positivo aqui.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> &Texture {
        // Nenhum TextureRing pode ser construído fora do Windows (`new` sempre falha),
        // então este caminho é inalcançável.
        unreachable!("stub: TextureRing nunca é criado")
    }
    pub fn len(&self) -> usize {
        self.textures.len()
    }
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

pub struct Converter;

impl Converter {
    pub fn new(
        _dev: &GpuDevice,
        _in_w: u32,
        _in_h: u32,
        _in_fmt: PixelFormat,
        _out_w: u32,
        _out_h: u32,
    ) -> Result<Self, GpuError> {
        Err(GpuError::Unsupported)
    }
    pub fn convert(&mut self, _dev: &GpuDevice, _src: &Texture, _dst: &Texture) -> Result<(), GpuError> {
        Err(GpuError::Unsupported)
    }
}

pub fn read_back(_dev: &GpuDevice, _tex: &Texture) -> Result<Vec<u8>, GpuError> {
    Err(GpuError::Unsupported)
}

pub fn upload(_dev: &GpuDevice, _tex: &Texture, _data: &[u8], _stride: u32) -> Result<(), GpuError> {
    Err(GpuError::Unsupported)
}

/// No stub não faz nada.
pub fn pin_mta() {}

/// No stub não faz nada.
pub struct ComGuard;

impl ComGuard {
    pub fn new() -> Result<Self, GpuError> {
        Ok(ComGuard)
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::Vendor;

    /// Dispositivo de mentira para exercitar as funções que pedem `&GpuDevice`.
    fn dispositivo_falso() -> GpuDevice {
        GpuDevice {
            adapter: AdapterInfo {
                name: String::from("stub"),
                vendor: Vendor::Other(0),
                luid: 0,
                dedicated_vram_mb: 0,
            },
        }
    }

    #[test]
    fn create_default_unsupported() {
        assert!(matches!(GpuDevice::create_default(), Err(GpuError::Unsupported)));
    }

    #[test]
    fn create_for_adapter_unsupported() {
        assert!(matches!(
            GpuDevice::create_for_adapter(0x1234),
            Err(GpuError::Unsupported)
        ));
    }

    #[test]
    fn texture_new_unsupported() {
        let dev = dispositivo_falso();
        let r = Texture::new(&dev, 1920, 1080, PixelFormat::Bgra8, false);
        assert!(matches!(r, Err(GpuError::Unsupported)));
    }

    #[test]
    fn texture_ring_new_unsupported() {
        let dev = dispositivo_falso();
        let r = TextureRing::new(&dev, 1920, 1080, PixelFormat::Nv12, 3, true);
        assert!(matches!(r, Err(GpuError::Unsupported)));
    }

    #[test]
    fn converter_new_unsupported() {
        let dev = dispositivo_falso();
        let r = Converter::new(&dev, 1920, 1080, PixelFormat::Bgra8, 1280, 720);
        assert!(matches!(r, Err(GpuError::Unsupported)));
    }

    #[test]
    fn read_back_e_upload_unsupported() {
        let dev = dispositivo_falso();
        let tex = Texture {
            format: PixelFormat::Bgra8,
            width: 4,
            height: 4,
        };
        assert!(matches!(read_back(&dev, &tex), Err(GpuError::Unsupported)));
        assert!(matches!(upload(&dev, &tex, &[0u8; 64], 16), Err(GpuError::Unsupported)));
        assert_eq!(tex.width(), 4);
        assert_eq!(tex.height(), 4);
        assert_eq!(tex.format(), PixelFormat::Bgra8);
    }

    #[test]
    fn com_guard_new_ok() {
        assert!(ComGuard::new().is_ok());
    }

    #[test]
    fn clone_do_dispositivo_falso() {
        let dev = dispositivo_falso();
        let copia = dev.clone();
        assert_eq!(copia.adapter().name, "stub");
    }

    #[test]
    fn pin_mta_vazio() {
        // No stub não faz nada: só garante que chamar várias vezes não entra em pânico.
        pin_mta();
        pin_mta();
        pin_mta();
    }
}
