use crate::{AdapterInfo, FormatSupport, GpuError, PixelFormat, Vendor};
use windows::core::Interface;
use windows::Win32::Foundation::{E_INVALIDARG, HMODULE};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11VideoContext, ID3D11VideoDevice,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_FORMAT_SUPPORT,
    D3D11_FORMAT_SUPPORT_CPU_LOCKABLE, D3D11_FORMAT_SUPPORT_RENDER_TARGET, D3D11_FORMAT_SUPPORT_TEXTURE2D,
    D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_INPUT, D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_OUTPUT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12};
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

/// Adaptador de software (`DXGI_ADAPTER_FLAG_SOFTWARE`: "Microsoft Basic Render Driver", WARP).
fn eh_software(desc: &windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1) -> bool {
    (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0
}

/// Ordem da cadeia do [`GpuDevice::create_default`], separada da criação para ser testável.
/// Primeiro a GPU de verdade com vídeo, depois a mesma sem vídeo e, por fim, o adaptador de
/// software sem vídeo (máquina virtual, runner do CI, driver quebrado). Se tudo falha, devolve o
/// erro da GPU de verdade, que explica mais que o do adaptador básico.
fn em_cadeia<A: Clone, T>(
    real: Result<A, GpuError>,
    software: impl FnOnce() -> Result<A, GpuError>,
    criar: impl Fn(A, bool) -> Result<T, GpuError>,
) -> Result<T, GpuError> {
    let mut ultimo = None;
    if let Ok(adapter) = real {
        match criar(adapter.clone(), true) {
            Ok(dev) => return Ok(dev),
            Err(e) => {
                tracing::warn!("GPU sem suporte a vídeo do D3D11 ({e}); tentando sem vídeo");
                match criar(adapter, false) {
                    Ok(dev) => return Ok(dev),
                    Err(e) => {
                        tracing::warn!("a GPU não abriu nem sem vídeo ({e}); tentando o adaptador básico");
                        ultimo = Some(e);
                    }
                }
            }
        }
    }
    match software() {
        Ok(adapter) => criar(adapter, false).map_err(|e| ultimo.unwrap_or(e)),
        Err(e) => Err(ultimo.unwrap_or(e)),
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

/// Guarda do lock do dispositivo (`ID3D11Multithread::Enter`/`Leave`). Solta no `Drop`.
pub struct DeviceLock<'a> {
    pub(crate) mt: &'a ID3D11Multithread,
}

impl Drop for DeviceLock<'_> {
    fn drop(&mut self) {
        // SAFETY: o guarda só existe depois de um `Enter` bem-sucedido, e não é `Send`,
        // então o `Leave` roda na mesma thread que fez o `Enter`.
        unsafe { self.mt.Leave() };
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
    /// Interface de multithread do mesmo dispositivo (para `lock`).
    pub(crate) multithread: ID3D11Multithread,
}

// SAFETY: o dispositivo é criado com SetMultithreadProtected(TRUE); o D3D11 serializa as
// chamadas ao dispositivo e ao contexto imediato entre threads.
unsafe impl Send for GpuDevice {}
// SAFETY: idem acima.
unsafe impl Sync for GpuDevice {}

impl GpuDevice {
    /// Adaptador padrão: a primeira GPU de verdade de `IDXGIFactory1::EnumAdapters1` (com vídeo
    /// e, se não der, sem vídeo); se ela não abrir, o adaptador de software, sem vídeo.
    pub fn create_default() -> Result<Self, GpuError> {
        em_cadeia(
            achar_adaptador(|d| !eh_software(d)),
            || achar_adaptador(eh_software),
            Self::create,
        )
    }
    /// Adaptador com o LUID dado (`AdapterInfo::luid`, = HighPart<<32 | LowPart).
    /// LUID desconhecido: `GpuError::AdapterNotFound`.
    pub fn create_for_adapter(luid: i64) -> Result<Self, GpuError> {
        let adapter = achar_adaptador(|d| luid_de(d) == luid)?;
        Self::create(adapter.clone(), true).or_else(|_| Self::create(adapter, false))
    }

    /// Primeiro adaptador com `DXGI_ADAPTER_FLAG_SOFTWARE` ("Microsoft Basic Render Driver", WARP),
    /// criado sem vídeo. Existe em todo Windows 10/11; serve para testar o caminho sem Video
    /// Processor num PC que tem um.
    pub fn create_basic() -> Result<Self, GpuError> {
        let adapter = achar_adaptador(eh_software)?;
        // Nunca com vídeo: o objetivo é exercitar o caminho sem Video Processor.
        Self::create(adapter, false)
    }

    /// `ID3D11Device::CheckFormatSupport`; em erro, tudo `false`.
    pub fn format_support(&self, format: PixelFormat) -> FormatSupport {
        let dxgi = match format {
            PixelFormat::Bgra8 => DXGI_FORMAT_B8G8R8A8_UNORM,
            PixelFormat::Nv12 => DXGI_FORMAT_NV12,
        };
        // SAFETY: `device` é um objeto COM válido; a chamada só consulta o driver.
        let bits = match unsafe { self.device.CheckFormatSupport(dxgi) } {
            Ok(b) => b,
            Err(_) => return FormatSupport::default(),
        };
        let tem = |f: D3D11_FORMAT_SUPPORT| (bits & f.0 as u32) != 0;
        FormatSupport {
            texture2d: tem(D3D11_FORMAT_SUPPORT_TEXTURE2D),
            render_target: tem(D3D11_FORMAT_SUPPORT_RENDER_TARGET),
            cpu_lockable: tem(D3D11_FORMAT_SUPPORT_CPU_LOCKABLE),
            video_processor_input: tem(D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_INPUT),
            video_processor_output: tem(D3D11_FORMAT_SUPPORT_VIDEO_PROCESSOR_OUTPUT),
        }
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
            multithread: mt,
        })
    }
    /// Lock explícito do dispositivo; devolve o guarda e quanto se esperou por ele.
    /// Reentrante: chamadas D3D11 feitas com o guarda vivo não travam.
    ///
    /// Serve para medir a espera pelo lock do dispositivo (decisão em
    /// docs/arquitetura-v3.md §2): a `Duration` devolvida é o tempo bloqueado no `Enter`.
    pub fn lock(&self) -> (DeviceLock<'_>, std::time::Duration) {
        let t0 = std::time::Instant::now();
        // SAFETY: `multithread` é um objeto COM válido; o `Leave` correspondente
        // acontece no `Drop` do guarda, na mesma thread.
        unsafe { self.multithread.Enter() };
        let espera = t0.elapsed();
        (DeviceLock { mt: &self.multithread }, espera)
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

    /// Criação de mentira para a cadeia: abre só os pares (adaptador, vídeo) listados.
    fn criar_so(
        abre: &'static [(&'static str, bool)],
    ) -> impl Fn(&'static str, bool) -> Result<(&'static str, bool), GpuError> {
        move |a, video| {
            if abre.contains(&(a, video)) {
                Ok((a, video))
            } else {
                Err(GpuError::Os {
                    context: "teste",
                    message: format!("{a} não abre (vídeo: {video})"),
                })
            }
        }
    }

    #[test]
    fn cadeia_prefere_gpu_com_video() {
        let r = em_cadeia(
            Ok("gpu"),
            || Ok("basico"),
            criar_so(&[("gpu", true), ("basico", false)]),
        );
        assert_eq!(r.ok(), Some(("gpu", true)));
    }

    #[test]
    fn cadeia_gpu_sem_video() {
        let r = em_cadeia(
            Ok("gpu"),
            || Ok("basico"),
            criar_so(&[("gpu", false), ("basico", false)]),
        );
        assert_eq!(r.ok(), Some(("gpu", false)));
    }

    #[test]
    fn cadeia_gpu_que_nao_abre_cai_no_basico() {
        // Antes o último passo pegava o primeiro adaptador de qualquer tipo: a mesma GPU que
        // acabara de falhar.
        let r = em_cadeia(Ok("gpu"), || Ok("basico"), criar_so(&[("basico", false)]));
        assert_eq!(r.ok(), Some(("basico", false)));
    }

    #[test]
    fn cadeia_sem_gpu_usa_o_basico_sem_video() {
        let r = em_cadeia(
            Err(GpuError::AdapterNotFound),
            || Ok("basico"),
            criar_so(&[("basico", true), ("basico", false)]),
        );
        assert_eq!(r.ok(), Some(("basico", false)));
    }

    #[test]
    fn cadeia_tudo_falha_devolve_o_erro_da_gpu() {
        let r = em_cadeia(Ok("gpu"), || Err(GpuError::AdapterNotFound), criar_so(&[]));
        match r {
            Err(GpuError::Os { message, .. }) => assert!(message.starts_with("gpu"), "{message}"),
            outro => panic!("esperava o erro da GPU, veio {outro:?}"),
        }
        let r = em_cadeia::<&str, (&str, bool)>(
            Err(GpuError::AdapterNotFound),
            || Err(GpuError::AdapterNotFound),
            criar_so(&[]),
        );
        assert!(matches!(r, Err(GpuError::AdapterNotFound)));
    }

    /// No Windows de verdade (este PC ou o runner do CI): se o adaptador básico abre, o padrão
    /// também tem de abrir. Mostra qual placa a cadeia escolheu (o CI resume esta linha).
    #[test]
    fn cadeia_real_nunca_falha_com_adaptador_basico() {
        let basico = GpuDevice::create_basic();
        match GpuDevice::create_default() {
            Ok(d) => eprintln!(
                "cadeia de dispositivos: {} (vídeo: {})",
                d.adapter().name,
                if d.has_video() { "sim" } else { "não" }
            ),
            Err(e) => assert!(basico.is_err(), "o padrão falhou ({e}), mas o adaptador básico abre"),
        }
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

    #[test]
    fn lock_devolve_espera_pequena_sem_concorrencia() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let (guarda, espera) = dev.lock();
        drop(guarda);
        assert!(espera < std::time::Duration::from_millis(50), "espera = {espera:?}");
    }

    #[test]
    fn lock_e_reentrante() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let (_a, _) = dev.lock();
        let (_b, _) = dev.lock();
        let tex = crate::Texture::new(&dev, 64, 64, crate::PixelFormat::Bgra8, false);
        assert!(tex.is_ok());
    }

    #[test]
    fn lock_bloqueia_outra_thread() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let dev_a = dev.clone();
        let a = std::thread::spawn(move || {
            let (guarda, _) = dev_a.lock();
            let _ = tx.send(());
            std::thread::sleep(std::time::Duration::from_millis(100));
            drop(guarda);
        });
        // Só tenta depois de A já ter pegado o lock.
        assert!(rx.recv().is_ok());
        let (guarda, espera) = dev.lock();
        drop(guarda);
        let _ = a.join();
        assert!(espera >= std::time::Duration::from_millis(60), "espera = {espera:?}");
    }

    #[test]
    fn format_support_bgra_basico() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let s = dev.format_support(PixelFormat::Bgra8);
        assert!(s.texture2d && s.render_target, "{s:?}");
    }

    #[test]
    fn format_support_nv12_texture() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let s = dev.format_support(PixelFormat::Nv12);
        assert!(s.texture2d, "{s:?}");
        if dev.has_video() {
            assert!(s.video_processor_output, "{s:?}");
        }
    }

    #[test]
    fn create_basic_sem_video() {
        match GpuDevice::create_basic() {
            Ok(d) => {
                assert!(!d.has_video());
                let nome = d.adapter().name.to_lowercase();
                assert!(
                    d.adapter().vendor == Vendor::Microsoft || nome.contains("basic") || nome.contains("warp"),
                    "adaptador = {:?}",
                    d.adapter()
                );
            }
            Err(GpuError::AdapterNotFound) => eprintln!("PULANDO: sem adaptador de software"),
            Err(e) => panic!("erro inesperado: {e}"),
        }
    }

    #[test]
    fn format_support_basico_nv12_cpu_lockable() {
        let Ok(dev) = GpuDevice::create_basic() else {
            eprintln!("PULANDO: sem adaptador de software");
            return;
        };
        let s = dev.format_support(PixelFormat::Nv12);
        assert!(s.texture2d && s.cpu_lockable, "{s:?}");
        assert!(!s.video_processor_input && !s.video_processor_output, "{s:?}");
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
