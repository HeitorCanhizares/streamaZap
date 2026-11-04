use crate::camera_enum::{native_modes, MfGuard};
use crate::{pick_camera_mode, CameraMode, CaptureError, CaptureHandle, CapturedFrame, FrameSink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use windows::core::{Error, Interface, HRESULT, PWSTR};
use windows::Win32::Graphics::Direct3D11::{ID3D11Texture2D, D3D11_BOX};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFDXGIBuffer, IMFDXGIDeviceManager, IMFMediaSource, IMFSample, IMFSourceReader,
    MFCreateAttributes, MFCreateDXGIDeviceManager, MFCreateMediaType, MFCreateSourceReaderFromMediaSource,
    MFEnumDeviceSources, MFMediaType_Video, MFVideoFormat_NV12, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
    MF_E_HW_MFT_FAILED_START_STREAMING, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR,
    MF_SOURCE_READER_D3D_MANAGER, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};
use zap_gpu::{now_us, upload, GpuDevice, PixelFormat, Texture, TextureRing};

/// Texturas NV12 do anel de entrega.
const TAMANHO_ANEL: usize = 4;
/// HRESULTs de "dispositivo em uso / acesso negado".
const E_ACCESSDENIED: HRESULT = HRESULT(0x8007_0005u32 as i32);
const HR_SHARING_VIOLATION: HRESULT = HRESULT(0x8007_0020u32 as i32);

/// Leva o ponteiro do dispositivo para a thread de captura.
struct SendPtr<T>(*const T);

// SAFETY: `GpuDevice` é Send + Sync (multithread protegido). O contrato de `capture_camera` exige
// que o chamador mantenha o dispositivo vivo enquanto o handle existir.
unsafe impl<T> Send for SendPtr<T> {}

impl<T> SendPtr<T> {
    /// Método (e não acesso ao campo) para a closure capturar o wrapper inteiro, que é Send.
    fn ptr(&self) -> *const T {
        self.0
    }
}

fn os(context: &'static str, e: &Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.message(),
    }
}

/// Mapeia falhas de "em uso" para `CameraBusy`.
fn os_ou_ocupada(context: &'static str, e: &Error) -> CaptureError {
    let hr = e.code();
    if hr == MF_E_HW_MFT_FAILED_START_STREAMING || hr == E_ACCESSDENIED || hr == HR_SHARING_VIOLATION {
        CaptureError::CameraBusy
    } else {
        os(context, e)
    }
}

/// Symbolic link de uma ativação (libera o buffer do MF).
fn symbolic_link(a: &IMFActivate) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: ponteiros de saída válidos; o buffer devolvido é liberado logo abaixo.
    unsafe {
        a.GetAllocatedString(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            &mut p,
            &mut len,
        )
    }
    .ok()?;
    // SAFETY: `p` aponta para `len` caracteres alocados pelo MF.
    let s = unsafe { p.to_string() }.ok();
    // SAFETY: o buffer veio de GetAllocatedString (CoTaskMemAlloc).
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// Abre a fonte de mídia da câmera `id` (enumeração local; o helper de camera_enum é privado).
fn abrir_fonte(id: &str) -> Result<IMFMediaSource, CaptureError> {
    let mut attrs: Option<IMFAttributes> = None;
    // SAFETY: ponteiro de saída válido.
    unsafe { MFCreateAttributes(&mut attrs, 1) }.map_err(|e| os("MFCreateAttributes", &e))?;
    let attrs = attrs.ok_or(CaptureError::Os {
        context: "MFCreateAttributes",
        message: "atributos nulos".into(),
    })?;
    // SAFETY: GUIDs constantes e atributos válidos.
    unsafe {
        attrs.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }
    .map_err(|e| os("SetGUID", &e))?;

    let mut arr: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    // SAFETY: ponteiros de saída válidos; o array é liberado abaixo.
    unsafe { MFEnumDeviceSources(&attrs, &mut arr, &mut count) }.map_err(|e| os("MFEnumDeviceSources", &e))?;
    let mut ativacoes = Vec::with_capacity(count as usize);
    if !arr.is_null() {
        for i in 0..count as usize {
            // SAFETY: índice dentro de `count`; `read` toma a posse da referência COM.
            if let Some(a) = unsafe { arr.add(i).read() } {
                ativacoes.push(a);
            }
        }
        // SAFETY: o array foi alocado por MFEnumDeviceSources (CoTaskMemAlloc).
        unsafe { CoTaskMemFree(Some(arr as *const _)) };
    }
    let ativacao = ativacoes
        .iter()
        .find(|a| symbolic_link(a).as_deref() == Some(id))
        .ok_or(CaptureError::CameraNotFound)?;
    // SAFETY: ativação válida; o tipo pedido é a fonte de mídia.
    unsafe { ativacao.ActivateObject::<IMFMediaSource>() }.map_err(|e| os_ou_ocupada("ActivateObject", &e))
}

/// Cria o reader com D3D, transformações de hardware e vídeo avançado (conversão de cor), já com
/// saída NV12 no tamanho e fps do modo.
fn criar_reader(
    dev: &GpuDevice,
    source: &IMFMediaSource,
    modo: &CameraMode,
) -> Result<(IMFSourceReader, IMFDXGIDeviceManager), CaptureError> {
    let mut token = 0u32;
    let mut gerente: Option<IMFDXGIDeviceManager> = None;
    // SAFETY: ponteiros de saída válidos.
    unsafe { MFCreateDXGIDeviceManager(&mut token, &mut gerente) }.map_err(|e| os("MFCreateDXGIDeviceManager", &e))?;
    let gerente = gerente.ok_or(CaptureError::Os {
        context: "MFCreateDXGIDeviceManager",
        message: "gerente nulo".into(),
    })?;
    // SAFETY: o dispositivo D3D11 é válido; o token veio da criação do gerente.
    unsafe { gerente.ResetDevice(dev.d3d_device(), token) }.map_err(|e| os("ResetDevice", &e))?;

    let mut attrs: Option<IMFAttributes> = None;
    // SAFETY: ponteiro de saída válido.
    unsafe { MFCreateAttributes(&mut attrs, 3) }.map_err(|e| os("MFCreateAttributes", &e))?;
    let attrs = attrs.ok_or(CaptureError::Os {
        context: "MFCreateAttributes",
        message: "atributos nulos".into(),
    })?;
    // SAFETY: chaves constantes; o gerente é válido e fica vivo junto do reader.
    unsafe {
        attrs
            .SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &gerente)
            .map_err(|e| os("SetUnknown", &e))?;
        attrs
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
            .map_err(|e| os("SetUINT32", &e))?;
        attrs
            .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
            .map_err(|e| os("SetUINT32", &e))?;
    }
    // SAFETY: fonte e atributos válidos.
    let reader = unsafe { MFCreateSourceReaderFromMediaSource(source, &attrs) }
        .map_err(|e| os_ou_ocupada("MFCreateSourceReaderFromMediaSource", &e))?;

    // SAFETY: tipo novo e atributos constantes; o reader converte para NV12.
    unsafe {
        let tipo = MFCreateMediaType().map_err(|e| os("MFCreateMediaType", &e))?;
        tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| os("SetGUID", &e))?;
        tipo.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
            .map_err(|e| os("SetGUID", &e))?;
        tipo.SetUINT64(
            &MF_MT_FRAME_SIZE,
            (u64::from(modo.width) << 32) | u64::from(modo.height),
        )
        .map_err(|e| os("SetUINT64", &e))?;
        tipo.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(modo.fps.max(1)) << 32) | 1)
            .map_err(|e| os("SetUINT64", &e))?;
        reader
            .SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &tipo)
            .map_err(|e| os_ou_ocupada("SetCurrentMediaType", &e))?;
    }
    Ok((reader, gerente))
}

/// Stride (bytes por linha) do tipo de saída atual; cai na largura se ausente ou inválido.
fn stride_atual(reader: &IMFSourceReader, largura: u32) -> u32 {
    // SAFETY: reader válido; só leitura de atributo.
    let s = unsafe {
        reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .and_then(|t| t.GetUINT32(&MF_MT_DEFAULT_STRIDE))
    };
    // O atributo é um i32 com sinal guardado em u32 (negativo = imagem de cabeça para baixo).
    match s {
        Ok(v) => (v as i32).unsigned_abs().max(largura),
        Err(_) => largura,
    }
}

/// Captura de câmera por Media Foundation Source Reader, em thread própria.
///
/// O `dev` precisa continuar vivo até o `CaptureHandle` ser descartado.
pub fn capture_camera(
    dev: &GpuDevice,
    id: &str,
    want_w: u32,
    want_h: u32,
    want_fps: u32,
    sink: impl FrameSink,
) -> Result<CaptureHandle, CaptureError> {
    let modos = native_modes(id)?;
    let idx = pick_camera_mode(&modos, want_w, want_h, want_fps).ok_or(CaptureError::CameraNotFound)?;
    let modo = modos[idx];

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let dev_ptr = SendPtr(dev as *const GpuDevice);
    let id = id.to_string();
    // Os objetos COM nascem na thread de captura; o resultado da abertura volta por este canal.
    let (tx, rx) = mpsc::sync_channel::<Result<(), CaptureError>>(1);

    let join = std::thread::Builder::new()
        .name("zap-capture-camera".to_string())
        .spawn(move || {
            // SAFETY: ver `SendPtr`; o chamador mantém o dispositivo vivo durante a captura.
            let dev = unsafe { &*dev_ptr.ptr() };
            let _mf = match MfGuard::new() {
                Ok(g) => g,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            // SAFETY: altera só a prioridade da thread atual.
            if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) } {
                tracing::debug!("SetThreadPriority falhou: {e}");
            }
            let preparo = abrir_fonte(&id).and_then(|source| {
                match (
                    criar_reader(dev, &source, &modo),
                    TextureRing::new(dev, modo.width, modo.height, PixelFormat::Nv12, TAMANHO_ANEL, false),
                ) {
                    (Ok((reader, gerente)), Ok(ring)) => Ok((source, reader, gerente, ring)),
                    (Err(e), _) => {
                        // SAFETY: encerra a fonte aberta acima, uma única vez.
                        let _ = unsafe { source.Shutdown() };
                        Err(e)
                    }
                    (_, Err(e)) => {
                        // SAFETY: idem.
                        let _ = unsafe { source.Shutdown() };
                        Err(e.into())
                    }
                }
            });
            match preparo {
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
                Ok((source, reader, gerente, ring)) => {
                    let _ = tx.send(Ok(()));
                    laco(dev, &modo, &sink, &stop_thread, &reader, ring);
                    // O reader e o gerente morrem antes do Shutdown da fonte e do MfGuard.
                    drop(reader);
                    drop(gerente);
                    // SAFETY: encerra a fonte aberta, uma única vez.
                    let _ = unsafe { source.Shutdown() };
                }
            }
        })
        .map_err(|e| CaptureError::Os {
            context: "thread zap-capture-camera",
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
                context: "thread zap-capture-camera",
                message: "a thread terminou antes de abrir a câmera".into(),
            })
        }
    }
}

fn laco<S: FrameSink>(
    dev: &GpuDevice,
    modo: &CameraMode,
    sink: &S,
    stop: &AtomicBool,
    reader: &IMFSourceReader,
    mut ring: TextureRing,
) {
    let stride = stride_atual(reader, modo.width);
    while !stop.load(Ordering::Acquire) {
        let mut flags = 0u32;
        let mut amostra: Option<IMFSample> = None;
        // SAFETY: ponteiros de saída locais e válidos; leitura síncrona.
        let r = unsafe {
            reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                None,
                Some(&mut amostra),
            )
        };
        let capture_us = now_us();
        if let Err(e) = r {
            sink.on_error(os("ReadSample", &e));
            return;
        }
        if flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
            sink.on_error(CaptureError::Os {
                context: "ReadSample",
                message: "fluxo da câmera terminou ou falhou".into(),
            });
            return;
        }
        // Amostra nula = lacuna ou marca de fluxo; segue lendo.
        let Some(amostra) = amostra else { continue };
        let slot = ring.next();
        match entregar_amostra(dev, &amostra, slot, stride) {
            Ok(()) => {
                let texture = Texture::from_raw(slot.raw().clone(), PixelFormat::Nv12, modo.width, modo.height);
                sink.on_frame(CapturedFrame {
                    texture,
                    width: modo.width,
                    height: modo.height,
                    capture_us,
                    new_content: true,
                });
            }
            Err(e) => {
                sink.on_error(e);
                return;
            }
        }
    }
}

/// Copia o conteúdo da amostra para `destino` (GPU->GPU se vier em textura, senão upload).
fn entregar_amostra(dev: &GpuDevice, amostra: &IMFSample, destino: &Texture, stride: u32) -> Result<(), CaptureError> {
    // SAFETY: amostra válida.
    let buffer = unsafe { amostra.GetBufferByIndex(0) }.map_err(|e| os("GetBufferByIndex", &e))?;
    if let Ok(dxgi) = buffer.cast::<IMFDXGIBuffer>() {
        let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
        // SAFETY: `p` recebe uma referência COM nova de ID3D11Texture2D (posse passa para `from_raw`).
        unsafe { dxgi.GetResource(&ID3D11Texture2D::IID, &mut p) }.map_err(|e| os("GetResource", &e))?;
        if p.is_null() {
            return Err(CaptureError::Os {
                context: "GetResource",
                message: "textura nula".into(),
            });
        }
        // SAFETY: ponteiro não nulo devolvido por GetResource com o IID pedido.
        let origem = unsafe { ID3D11Texture2D::from_raw(p) };
        // SAFETY: buffer DXGI válido.
        let sub = unsafe { dxgi.GetSubresourceIndex() }.map_err(|e| os("GetSubresourceIndex", &e))?;
        let caixa = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: destino.width(),
            bottom: destino.height(),
            back: 1,
        };
        // SAFETY: as duas texturas são do mesmo dispositivo; a caixa cabe no destino NV12 (dims pares).
        unsafe {
            dev.context()
                .CopySubresourceRegion(destino.raw(), 0, 0, 0, 0, &origem, sub, Some(&caixa));
        }
        return Ok(());
    }

    let mut dados: *mut u8 = std::ptr::null_mut();
    let mut atual = 0u32;
    // SAFETY: ponteiros de saída válidos; o Unlock vem logo depois de usar o ponteiro.
    unsafe { buffer.Lock(&mut dados, None, Some(&mut atual)) }.map_err(|e| os("Lock", &e))?;
    let resultado = if dados.is_null() {
        Err(CaptureError::Os {
            context: "Lock",
            message: "buffer nulo".into(),
        })
    } else {
        // SAFETY: entre Lock e Unlock, `dados` aponta para `atual` bytes legíveis.
        let fatia = unsafe { std::slice::from_raw_parts(dados, atual as usize) };
        upload(dev, destino, fatia, stride).map_err(CaptureError::from)
    };
    // SAFETY: par com o Lock bem-sucedido acima.
    let _ = unsafe { buffer.Unlock() };
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::Sender;
    use std::time::{Duration, Instant};

    struct SinkCanal(Sender<(PixelFormat, u32, u32)>);

    impl FrameSink for SinkCanal {
        fn on_frame(&self, f: CapturedFrame) {
            let _ = self.0.send((f.texture.format(), f.width, f.height));
        }
        fn on_error(&self, e: CaptureError) {
            eprintln!("erro da câmera: {e}");
        }
    }

    fn gpu() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("pulando: sem GPU ({e})");
                None
            }
        }
    }

    #[test]
    fn capture_camera_entrega_quadros_nv12() {
        let Some(dev) = gpu() else { return };
        let cams = crate::list_cameras().unwrap_or_default();
        let Some(cam) = cams.first() else {
            eprintln!("pulando: sem câmeras");
            return;
        };
        let (tx, rx) = mpsc::channel();
        let handle = match capture_camera(&dev, &cam.id, 640, 480, 30, SinkCanal(tx)) {
            Ok(h) => h,
            Err(CaptureError::CameraBusy) => {
                eprintln!("pulando: câmera em uso");
                return;
            }
            Err(e) => panic!("capture_camera falhou: {e}"),
        };
        let fim = Instant::now() + Duration::from_secs(3);
        let mut quadros = 0;
        while let Some(resto) = fim.checked_duration_since(Instant::now()) {
            if let Ok((fmt, w, h)) = rx.recv_timeout(resto) {
                assert_eq!(fmt, PixelFormat::Nv12);
                assert!(w > 0 && h > 0);
                quadros += 1;
                break;
            }
        }
        drop(handle);
        assert!(quadros >= 1, "nenhum quadro em 3 s");
    }

    #[test]
    fn capture_camera_id_inexistente_erro() {
        let Some(dev) = gpu() else { return };
        let (tx, _rx) = mpsc::channel();
        let r = capture_camera(&dev, "nao-existe", 640, 480, 30, SinkCanal(tx));
        assert!(matches!(r, Err(CaptureError::CameraNotFound)));
    }
}
