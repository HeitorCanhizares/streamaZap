use crate::{CaptureError, CaptureHandle, CapturedFrame, FrameSink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::core::{factory, IInspectable, Interface};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{ID3D11Texture2D, D3D11_BOX, D3D11_TEXTURE2D_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::IsWindow;
use zap_gpu::{now_us, pin_mta, ComGuard, GpuDevice, PixelFormat, Texture, TextureRing};

/// Texturas do anel de entrega (o consumidor tem folga de 2 quadros).
const TAMANHO_ANEL: usize = 3;
/// Buffers do pool de quadros do WGC.
const BUFFERS_POOL: i32 = 2;
const FORMATO: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;

/// `true` se `GraphicsCaptureSession::IsSupported()`.
pub fn wgc_supported() -> bool {
    pin_mta();
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

fn os_err(context: &'static str, e: windows::core::Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.to_string(),
    }
}

/// Leva um valor para outra thread (a do pool de quadros ou a de captura).
struct SendWrap<T>(T);

// SAFETY: usado só com objetos WinRT agile (o IDirect3DDevice do handler).
unsafe impl<T> Send for SendWrap<T> {}

impl<T> SendWrap<T> {
    /// Método (e não acesso ao campo) para a closure capturar o wrapper inteiro, que é Send.
    fn get(&self) -> &T {
        &self.0
    }
}

/// Estado compartilhado entre os handlers de evento (rodam em threads do pool).
struct Estado<S: FrameSink> {
    sink: S,
    ring: TextureRing,
    /// Tamanho atual do conteúdo (e do anel).
    tamanho: SizeInt32,
}

/// Captura de janela por Windows.Graphics.Capture.
///
/// A captura guarda um clone do `GpuDevice`, então o `dev` do chamador pode ser solto antes do handle.
pub fn capture_window(dev: &GpuDevice, hwnd: isize, sink: impl FrameSink) -> Result<CaptureHandle, CaptureError> {
    pin_mta();
    // SAFETY: IsWindow aceita qualquer valor de HWND.
    if !unsafe { IsWindow(Some(HWND(hwnd as _))) }.as_bool() {
        return Err(CaptureError::WindowNotFound);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let dev_owned = dev.clone();
    let (tx, rx) = mpsc::sync_channel::<Result<(), CaptureError>>(1);

    let join = std::thread::Builder::new()
        .name("zap-capture-wgc".to_string())
        .spawn(move || {
            let _com = match ComGuard::new() {
                Ok(g) => g,
                Err(e) => {
                    let _ = tx.send(Err(e.into()));
                    return;
                }
            };
            let sessao = match iniciar(&dev_owned, hwnd, sink) {
                Ok(s) => {
                    let _ = tx.send(Ok(()));
                    s
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            while !stop_thread.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(20));
            }
            sessao.encerrar();
        })
        .map_err(|e| CaptureError::Os {
            context: "thread zap-capture-wgc",
            message: e.to_string(),
        })?;

    match rx.recv() {
        Ok(Ok(())) => Ok(CaptureHandle::new(stop, join)),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => {
            let _ = join.join();
            Err(CaptureError::Os {
                context: "thread zap-capture-wgc",
                message: "a thread terminou antes de iniciar a captura".to_string(),
            })
        }
    }
}

/// Objetos vivos da captura, para encerrar na ordem certa.
struct Sessao {
    session: windows::Graphics::Capture::GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
    item: GraphicsCaptureItem,
    token_quadro: i64,
    token_fechou: i64,
}

impl Sessao {
    fn encerrar(self) {
        let _ = self.pool.RemoveFrameArrived(self.token_quadro);
        let _ = self.item.RemoveClosed(self.token_fechou);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

fn iniciar<S: FrameSink>(dev: &GpuDevice, hwnd: isize, sink: S) -> Result<Sessao, CaptureError> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|e| os_err("IGraphicsCaptureItemInterop", e))?;
    // SAFETY: o HWND foi validado por IsWindow; a chamada devolve um item novo.
    let item: GraphicsCaptureItem =
        unsafe { interop.CreateForWindow(HWND(hwnd as _)) }.map_err(|e| os_err("CreateForWindow", e))?;
    let tamanho = item.Size().map_err(|e| os_err("GraphicsCaptureItem::Size", e))?;
    if tamanho.Width <= 0 || tamanho.Height <= 0 {
        return Err(CaptureError::InvalidArgument("janela com tamanho zero".to_string()));
    }

    let dxgi = dev
        .d3d_device()
        .cast::<IDXGIDevice>()
        .map_err(|e| os_err("IDXGIDevice", e))?;
    // SAFETY: `dxgi` é um dispositivo DXGI válido.
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .map_err(|e| os_err("CreateDirect3D11DeviceFromDXGIDevice", e))?;
    let device: IDirect3DDevice = inspectable.cast().map_err(|e| os_err("IDirect3DDevice", e))?;

    let ring = TextureRing::new(
        dev,
        tamanho.Width as u32,
        tamanho.Height as u32,
        PixelFormat::Bgra8,
        TAMANHO_ANEL,
        false,
    )?;
    let estado = Arc::new(Mutex::new(Estado { sink, ring, tamanho }));

    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&device, FORMATO, BUFFERS_POOL, tamanho)
        .map_err(|e| os_err("CreateFreeThreaded", e))?;
    let session = pool
        .CreateCaptureSession(&item)
        .map_err(|e| os_err("CreateCaptureSession", e))?;
    let _ = session.SetIsCursorCaptureEnabled(true);
    // Windows 10 antigo não tem a borda opcional: ignora o erro.
    let _ = session.SetIsBorderRequired(false);

    // Clone do dispositivo e dispositivo WinRT para o handler (roda em thread do pool).
    let dev_arc = Arc::new(dev.clone());
    let ctx = SendWrap((dev_arc, device));
    let estado_quadro = Arc::clone(&estado);
    let handler = TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |pool, _| {
        let Some(pool) = pool.as_ref() else { return Ok(()) };
        let (dev_arc, device) = ctx.get();
        let dev: &GpuDevice = dev_arc;
        let Ok(mut est) = estado_quadro.lock() else {
            return Ok(());
        };
        if let Err(e) = tratar_quadro(dev, device, pool, &mut est) {
            tracing::debug!("quadro WGC descartado: {e}");
        }
        Ok(())
    });
    let token_quadro = pool.FrameArrived(&handler).map_err(|e| os_err("FrameArrived", e))?;

    let estado_fechou = Arc::clone(&estado);
    let fechou = TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
        if let Ok(est) = estado_fechou.lock() {
            est.sink.on_error(CaptureError::WindowClosed);
        }
        Ok(())
    });
    let token_fechou = match item.Closed(&fechou) {
        Ok(t) => t,
        Err(e) => {
            let _ = pool.RemoveFrameArrived(token_quadro);
            return Err(os_err("Closed", e));
        }
    };

    if let Err(e) = session.StartCapture() {
        let _ = pool.RemoveFrameArrived(token_quadro);
        let _ = item.RemoveClosed(token_fechou);
        return Err(os_err("StartCapture", e));
    }
    Ok(Sessao {
        session,
        pool,
        item,
        token_quadro,
        token_fechou,
    })
}

/// Pega o próximo quadro do pool, copia para o anel e entrega ao sink.
fn tratar_quadro<S: FrameSink>(
    dev: &GpuDevice,
    device: &IDirect3DDevice,
    pool: &Direct3D11CaptureFramePool,
    est: &mut Estado<S>,
) -> Result<(), CaptureError> {
    let frame = pool.TryGetNextFrame().map_err(|e| os_err("TryGetNextFrame", e))?;
    let resultado = copiar_e_entregar(dev, device, pool, est, &frame);
    // Devolve o buffer ao pool o quanto antes.
    let _ = frame.Close();
    resultado
}

fn copiar_e_entregar<S: FrameSink>(
    dev: &GpuDevice,
    device: &IDirect3DDevice,
    pool: &Direct3D11CaptureFramePool,
    est: &mut Estado<S>,
    frame: &windows::Graphics::Capture::Direct3D11CaptureFrame,
) -> Result<(), CaptureError> {
    let conteudo = frame.ContentSize().map_err(|e| os_err("ContentSize", e))?;
    if conteudo.Width <= 0 || conteudo.Height <= 0 {
        return Ok(());
    }
    let surface = frame.Surface().map_err(|e| os_err("Surface", e))?;
    let acesso: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|e| os_err("IDirect3DDxgiInterfaceAccess", e))?;
    // SAFETY: a superfície do quadro é uma textura D3D11 do mesmo dispositivo.
    let origem: ID3D11Texture2D = unsafe { acesso.GetInterface() }.map_err(|e| os_err("GetInterface", e))?;

    // Janela redimensionada: recria o pool (para os próximos quadros) e o anel.
    if conteudo.Width != est.tamanho.Width || conteudo.Height != est.tamanho.Height {
        pool.Recreate(device, FORMATO, BUFFERS_POOL, conteudo)
            .map_err(|e| os_err("Recreate", e))?;
        est.ring = TextureRing::new(
            dev,
            conteudo.Width as u32,
            conteudo.Height as u32,
            PixelFormat::Bgra8,
            TAMANHO_ANEL,
            false,
        )?;
        est.tamanho = conteudo;
    }
    let (w, h) = (conteudo.Width as u32, conteudo.Height as u32);

    let destino = est.ring.next().raw().clone();
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: `origem` é válida e `desc` é local.
    unsafe { origem.GetDesc(&mut desc) };
    let ctx = dev.context();
    if desc.Width == w && desc.Height == h {
        // SAFETY: texturas do mesmo dispositivo, mesmo tamanho e formato.
        unsafe { ctx.CopyResource(&destino, &origem) };
    } else {
        // A textura do pool é maior que o conteúdo: copia só a região útil.
        let caixa = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: w.min(desc.Width),
            bottom: h.min(desc.Height),
            back: 1,
        };
        // SAFETY: texturas do mesmo dispositivo; a caixa cabe na origem e no destino.
        unsafe { ctx.CopySubresourceRegion(&destino, 0, 0, 0, 0, &origem, 0, Some(&caixa)) };
    }

    est.sink.on_frame(CapturedFrame {
        texture: Texture::from_raw(destino, PixelFormat::Bgra8, w, h),
        width: w,
        height: h,
        capture_us: now_us(),
        new_content: true,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SinkNulo;
    impl FrameSink for SinkNulo {
        fn on_frame(&self, _frame: CapturedFrame) {}
        fn on_error(&self, _err: CaptureError) {}
    }

    #[test]
    fn wgc_supported_nao_falha() {
        // Qualquer resposta serve; só não pode dar pânico.
        let _ = wgc_supported();
    }

    #[test]
    fn capture_window_hwnd_invalido_erro() {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pulando: sem GPU ({e})");
                return;
            }
        };
        match capture_window(&dev, 0x1, SinkNulo) {
            Err(CaptureError::WindowNotFound) | Err(CaptureError::Os { .. }) => {}
            Err(e) => panic!("erro inesperado: {e}"),
            Ok(_) => panic!("hwnd inválido não deveria capturar"),
        }
    }
}
