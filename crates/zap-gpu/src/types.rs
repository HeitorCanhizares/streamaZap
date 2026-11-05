/// Fabricante da GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    /// WARP / Microsoft Basic Render (0x1414).
    Microsoft,
    Other(u32),
}

impl Vendor {
    /// Converte o `VendorId` do PCI: 0x10DE Nvidia, 0x1002 Amd, 0x8086 Intel, 0x1414 Microsoft,
    /// qualquer outro `Other(id)`.
    pub fn from_pci_id(id: u32) -> Self {
        match id {
            0x10DE => Self::Nvidia,
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            0x1414 => Self::Microsoft,
            outro => Self::Other(outro),
        }
    }
}

/// Informações do adaptador de vídeo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInfo {
    pub name: String,
    pub vendor: Vendor,
    pub luid: i64,
    pub dedicated_vram_mb: u32,
}

/// Formato de pixel das texturas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Bgra8,
    Nv12,
}

impl PixelFormat {
    /// Tamanho em bytes de um quadro sem padding: Bgra8 = w*h*4; Nv12 = w*h*3/2 (w e h pares).
    pub fn frame_bytes(self, width: u32, height: u32) -> usize {
        // Calcula em usize: w*h*4 de 4K (≈33M) já passa de u32 em casos extremos.
        let w = width as usize;
        let h = height as usize;
        match self {
            Self::Bgra8 => w * h * 4,
            // Y (w*h) + UV intercalado a 1/4 de resolução (w*h/2).
            Self::Nv12 => w * h * 3 / 2,
        }
    }
}

/// O que o dispositivo permite fazer com um formato (`ID3D11Device::CheckFormatSupport`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatSupport {
    pub texture2d: bool,
    pub render_target: bool,
    pub cpu_lockable: bool,
    pub video_processor_input: bool,
    pub video_processor_output: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReadbackTiming {
    /// Do `CopyResource` ao `Map` que deu certo (µs): espera pelo quadro + cópia na GPU.
    pub copy_wait_us: f64,
    /// Parte de `copy_wait_us` em que a GPU ainda terminava o trabalho anterior à cópia (µs): na
    /// captura, a imagem do DWM e as cópias da própria captura. O caminho da GPU também espera
    /// isso, só que dentro da fila da GPU. 0 sem consulta de evento no driver.
    pub frame_wait_us: f64,
    /// Soma das esperas pelo lock do dispositivo nas chamadas desta leitura (µs).
    pub lock_wait_us: f64,
    /// Tentativas de `Map`.
    pub map_attempts: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UploadTiming {
    /// Soma das esperas pelo lock (`Map`, `Unmap`, `CopyResource`) (µs).
    pub lock_wait_us: f64,
    pub map_attempts: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_pci_id_conhecidos() {
        assert_eq!(Vendor::from_pci_id(0x10DE), Vendor::Nvidia);
        assert_eq!(Vendor::from_pci_id(0x1002), Vendor::Amd);
        assert_eq!(Vendor::from_pci_id(0x8086), Vendor::Intel);
        assert_eq!(Vendor::from_pci_id(0x1414), Vendor::Microsoft);
    }

    #[test]
    fn from_pci_id_outro() {
        assert_eq!(Vendor::from_pci_id(0x1234), Vendor::Other(0x1234));
    }

    #[test]
    fn frame_bytes_bgra() {
        assert_eq!(PixelFormat::Bgra8.frame_bytes(1920, 1080), 8_294_400);
    }

    #[test]
    fn frame_bytes_nv12() {
        assert_eq!(PixelFormat::Nv12.frame_bytes(1920, 1080), 3_110_400);
    }
}
