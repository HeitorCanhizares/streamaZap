use crate::{AdapterInfo, GpuError, Vendor};
use windows::core::Interface;
use windows::Win32::Foundation::{E_INVALIDARG, HMODULE};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11VideoContext, ID3D11VideoDevice,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
};

/// Converte um erro do Windows no erro do crate, guardando o contexto.
fn os(ctx: &'static str, e: windows::core::Error) -> GpuError {
    GpuError::Os {
        context: ctx,
        message: e.message(),
    }
}

/// LUID do adaptador no formato `HighPart << 32 | LowPart`.
fn luid_de(desc: &windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1) -> i64 {
    ((desc.AdapterLuid.HighPart as i64) << 32) | (desc.AdapterLuid.LowPart as i64)
}

/// Percorre os adaptadores e devolve o primeiro que satisfaz `aceita`.
fn achar_adaptador(
    aceita: impl Fn(&windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1) -> bool,
) -> Result<IDXGIAdapter1, GpuError> {
    // SAFETY: chamada COM sem ponteiros de entrada; o resultado é tipado pelo crate windows.
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }.map_err(|e| os("CreateDXGIFactory1", e))?;
    let mut i = 0u32;
    loop {
        // SAFETY: `factory` é válido; o índice só é lido pelo DXGI.
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => return Err(GpuError::AdapterNotFound),
            Err(e) => return Err(os("EnumAdapters1", e)),
        };
        // SAFETY: `adapter` é um objeto COM válido recém-obtido.
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| os("GetDesc1", e))?;
        if aceita(&desc) {
            return Ok(adapter);
        }
        i += 1;
    }
}

/// Chama `D3D11CreateDevice` com a lista de níveis de recurso dada.
fn criar_d3d(
    adapter: &IDXGIAdapter,
    niveis: &[D3D_FEATURE_LEVEL],
    com_video: bool,
) -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut dev: Option<ID3D11Device> = None;
    let mut ctx: Option<ID3D11DeviceContext> = None;
    // SAFETY: os ponteiros de saída apontam para variáveis locais válidas; com adaptador
    // explícito o tipo de driver tem de ser UNKNOWN.
    unsafe {
        D3D11CreateDevice(
            Some(adapter),
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            if com_video {
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT
            } else {
                D3D11_CREATE_DEVICE_BGRA_SUPPORT
            },
            Some(niveis),
            D3D11_SDK_VERSION,
            Some(&mut dev),
            None,
            Some(&mut ctx),
        )?;
    }
    match (dev, ctx) {
        (Some(d), Some(c)) => Ok((d, c)),
        _ => Err(windows::core::Error::from(E_INVALIDARG)),
    }
}

/// Dispositivo D3D11 com multithread protegido (`ID3D11Multithread::SetMultithreadProtected(TRUE)`).
///
/// Normalmente `BGRA_SUPPORT | VIDEO_SUPPORT` numa GPU de verdade. Sem suporte a vídeo (driver
/// básico, máquina virtual, runner do CI) o dispositivo ainda serve para captura, texturas e leitura;
/// só o `Converter` (Video Processor) fica indisponível (`video()` devolve `Unsupported`).
///
/// `Clone` = mesmo dispositivo e mesmo contexto imediato (`AddRef` dos COM), já protegido para
/// multithread.
#[derive(Clone)]
pub struct GpuDevice {
    pub(crate) device: ID3D11Device,
    pub(crate) context: ID3D11DeviceContext,
    pub(crate) video: Option<(ID3D11VideoDevice, ID3D11VideoContext)>,
    pub(crate) adapter: AdapterInfo,
}

// SAFETY: o dispositivo é criado com SetMultithreadProtected(TRUE); o D3D11 serializa as
// chamadas ao dispositivo e ao contexto imediato entre threads.
unsafe impl Send for GpuDevice {}
// SAFETY: idem acima.
unsafe impl Sync for GpuDevice {}

impl GpuDevice {
    /// Adaptador padrão (primeiro de `IDXGIFactory1::EnumAdapters1`), driver HARDWARE.
    pub fn create_default() -> Result<Self, GpuError> {
        // 1) GPU de verdade com vídeo; 2) a mesma sem vídeo; 3) qualquer adaptador (inclusive o
        // "Microsoft Basic Render Driver" de máquinas virtuais e do CI), sem vídeo.
        let real = achar_adaptador(|d| (d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) == 0);
        let mut ultimo = None;
        if let Ok(adapter) = real {
            match Self::create(adapter.clone(), true) {
                Ok(dev) => return Ok(dev),
                Err(e) => {
                    tracing::warn!("GPU sem suporte a vídeo do D3D11 ({e}); tentando sem vídeo");
                    match Self::create(adapter, false) {
                        Ok(dev) => return Ok(dev),
                        Err(e) => ultimo = Some(e),
                    }
                }
            }
        }
        match achar_adaptador(|_| true) {
            Ok(adapter) => Self::create(adapter, false).map_err(|e| ultimo.unwrap_or(e)),
            Err(e) => Err(ultimo.unwrap_or(e)),
        }
    }
    /// Adaptador com o LUID dado (`AdapterInfo::luid`, = HighPart<<32 | LowPart).
    /// LUID desconhecido: `GpuError::AdapterNotFound`.
    pub fn create_for_adapter(luid: i64) -> Result<Self, GpuError> {
        let adapter = achar_adaptador(|d| luid_de(d) == luid)?;
        Self::create(adapter.clone(), true).or_else(|_| Self::create(adapter, false))
    }

    fn create(adapter: IDXGIAdapter1, com_video: bool) -> Result<Self, GpuError> {
        // SAFETY: `adapter` é um objeto COM válido.
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| os("GetDesc1", e))?;
        let base: IDXGIAdapter = adapter.cast().map_err(|e| os("IDXGIAdapter", e))?;

        // Tenta 11_1 e, se o sistema não conhece esse nível (E_INVALIDARG), cai para 11_0.
        let (device, context) = match criar_d3d(&base, &[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0], com_video) {
            Ok(par) => par,
            Err(e) if e.code() == E_INVALIDARG => {
                criar_d3d(&base, &[D3D_FEATURE_LEVEL_11_0], com_video).map_err(|e| os("D3D11CreateDevice", e))?
            }
            Err(e) => return Err(os("D3D11CreateDevice", e)),
        };

        let mt: ID3D11Multithread = device.cast().map_err(|e| os("ID3D11Multithread", e))?;
        // SAFETY: `mt` é válido; a chamada só liga a proteção de threads.
        // O retorno é o estado anterior, que não interessa.
        let _ = unsafe { mt.SetMultithreadProtected(true) };

        let video = if com_video {
            let vd: ID3D11VideoDevice = device.cast().map_err(|e| os("video device", e))?;
            let vc: ID3D11VideoContext = context.cast().map_err(|e| os("video context", e))?;
            Some((vd, vc))
        } else {
            None
        };

        let fim = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        let adapter_info = AdapterInfo {
            name: String::from_utf16_lossy(&desc.Description[..fim]),
            vendor: Vendor::from_pci_id(desc.VendorId),
            luid: luid_de(&desc),
            dedicated_vram_mb: (desc.DedicatedVideoMemory / (1024 * 1024)) as u32,
        };
        Ok(Self {
            device,
            context,
            video,
            adapter: adapter_info,
        })
    }
    pub fn adapter(&self) -> &AdapterInfo {
        &self.adapter
    }
    pub fn d3d_device(&self) -> &ID3D11Device {
        &self.device
    }
    pub fn context(&self) -> &ID3D11DeviceContext {
        &self.context
    }
    /// O dispositivo tem o Video Processor (conversão/escala para NV12 na GPU)?
    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }
    /// Dispositivo e contexto de vídeo, ou `Unsupported` se a GPU/driver não oferece.
    pub(crate) fn video(&self) -> Result<(&ID3D11VideoDevice, &ID3D11VideoContext), GpuError> {
        self.video.as_ref().map(|(d, c)| (d, c)).ok_or(GpuError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn create_default_ou_pula() {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pulando: sem GPU: {e}");
                return;
            }
        };
        assert!(!dev.adapter().name.is_empty());
        assert_send_sync::<GpuDevice>();
    }

    #[test]
    fn create_for_adapter_luid_invalido() {
        let r = GpuDevice::create_for_adapter(0x7777_7777_7777);
        assert!(matches!(r, Err(GpuError::AdapterNotFound)));
    }

    #[test]
    fn create_for_adapter_mesmo_luid() {
        let Ok(a) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let b = GpuDevice::create_for_adapter(a.adapter().luid);
        assert!(matches!(&b, Ok(d) if d.adapter().name == a.adapter().name));
    }

    fn assert_clone<T: Clone>() {}

    #[test]
    fn clone_compartilha_dispositivo() {
        assert_clone::<GpuDevice>();
        let Ok(a) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let b = a.clone();
        // O clone só incrementa a contagem de referências COM: mesmos ponteiros.
        assert_eq!(a.d3d_device().as_raw(), b.d3d_device().as_raw());
        assert_eq!(a.context().as_raw(), b.context().as_raw());
        assert_eq!(b.adapter().luid, a.adapter().luid);
        // O clone continua válido depois que o original some.
        drop(a);
        let tex = crate::Texture::new(&b, 64, 64, crate::PixelFormat::Bgra8, false);
        assert!(tex.is_ok());
    }
}
