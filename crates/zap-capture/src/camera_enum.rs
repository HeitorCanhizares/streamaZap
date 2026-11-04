use crate::camera_mode::CameraMode;
use crate::{CameraInfo, CaptureError};
use windows::core::{Error, HRESULT, PWSTR};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFMediaSource, MFCreateAttributes, MFCreateSourceReaderFromMediaSource,
    MFEnumDeviceSources, MFShutdown, MFStartup, MFSTARTUP_FULL, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_E_HW_MFT_FAILED_START_STREAMING, MF_E_NO_MORE_TYPES,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
};
use windows::Win32::System::Com::CoTaskMemFree;
use zap_gpu::ComGuard;

/// HRESULTs de "dispositivo em uso / acesso negado" além do MF_E_HW_MFT_FAILED_START_STREAMING.
const E_ACCESSDENIED: HRESULT = HRESULT(0x8007_0005u32 as i32);
const HR_SHARING_VIOLATION: HRESULT = HRESULT(0x8007_0020u32 as i32);

/// Guardião de COM + Media Foundation: `MFStartup` na criação e `MFShutdown` ao dropar.
/// Campo `_com` é dropado depois do `MFShutdown` (que roda no `Drop` antes dos campos).
#[allow(dead_code)]
pub(crate) struct MfGuard {
    _com: ComGuard,
}

impl MfGuard {
    pub(crate) fn new() -> Result<Self, CaptureError> {
        let com = ComGuard::new()?;
        // SAFETY: chamada FFI sem ponteiros; o par com MFShutdown fica no Drop.
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|e| os("MFStartup", &e))?;
        Ok(Self { _com: com })
    }
}

impl Drop for MfGuard {
    fn drop(&mut self) {
        // SAFETY: par com o MFStartup bem-sucedido de `new`.
        let _ = unsafe { MFShutdown() };
    }
}

fn os(context: &'static str, e: &Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.message(),
    }
}

/// Converte razão de quadros em fps inteiro (arredondado ao mais próximo); `den == 0` dá 0.
fn fps_from_ratio(num: u32, den: u32) -> u32 {
    if den == 0 {
        return 0;
    }
    let (n, d) = (u64::from(num), u64::from(den));
    ((n * 2 + d) / (d * 2)) as u32
}

/// Lê uma string alocada do atributo e libera o buffer com `CoTaskMemFree`.
fn alloc_string(a: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: ponteiros de saída válidos; o buffer devolvido é liberado logo abaixo.
    unsafe { a.GetAllocatedString(key, &mut p, &mut len) }.ok()?;
    // SAFETY: `p` aponta para `len` caracteres alocados pelo MF.
    let s = unsafe { p.to_string() }.ok();
    // SAFETY: o buffer veio de GetAllocatedString (CoTaskMemAlloc).
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// Enumera as ativações de câmeras de vídeo (a ordem é a do MF).
fn enum_activates() -> Result<Vec<IMFActivate>, CaptureError> {
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
    let mut out = Vec::with_capacity(count as usize);
    if !arr.is_null() {
        for i in 0..count as usize {
            // SAFETY: o índice está dentro de `count`; `read` toma a posse da referência COM
            // (o array bruto é liberado depois sem dropar os elementos).
            if let Some(a) = unsafe { arr.add(i).read() } {
                out.push(a);
            }
        }
        // SAFETY: o array foi alocado por MFEnumDeviceSources (CoTaskMemAlloc).
        unsafe { CoTaskMemFree(Some(arr as *const _)) };
    }
    Ok(out)
}

/// Câmeras de vídeo (`MFEnumDeviceSources`, `MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID`).
/// Lista vazia não é erro.
pub fn list_cameras() -> Result<Vec<CameraInfo>, CaptureError> {
    let _mf = MfGuard::new()?;
    let ativacoes = enum_activates()?;
    let mut out = Vec::with_capacity(ativacoes.len());
    for a in &ativacoes {
        let Some(id) = alloc_string(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK) else {
            continue;
        };
        let name = alloc_string(a, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME).unwrap_or_default();
        out.push(CameraInfo { id, name });
    }
    // As ativações são liberadas antes do MfGuard (declarado primeiro, dropado por último).
    drop(ativacoes);
    Ok(out)
}

/// Modos nativos da câmera `id` (só tipos de vídeo com tamanho e fps legíveis).
/// `CameraNotFound` se o id não existir; `CameraBusy` se não abrir por estar em uso.
// Usada por camera.rs (capture-11); até lá fica sem chamador.
#[allow(dead_code)]
pub(crate) fn native_modes(id: &str) -> Result<Vec<CameraMode>, CaptureError> {
    let _mf = MfGuard::new()?;
    let ativacoes = enum_activates()?;
    let ativacao = ativacoes
        .iter()
        .find(|a| alloc_string(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).as_deref() == Some(id))
        .ok_or(CaptureError::CameraNotFound)?;

    // SAFETY: a ativação é válida e o tipo pedido é a fonte de mídia.
    let source: IMFMediaSource = unsafe { ativacao.ActivateObject::<IMFMediaSource>() }.map_err(|e| {
        let hr = e.code();
        if hr == MF_E_HW_MFT_FAILED_START_STREAMING || hr == E_ACCESSDENIED || hr == HR_SHARING_VIOLATION {
            CaptureError::CameraBusy
        } else {
            os("ActivateObject", &e)
        }
    })?;

    let modos = ler_modos(&source);
    // SAFETY: encerra a fonte ativada acima, uma única vez.
    let _ = unsafe { source.Shutdown() };
    modos
}

fn ler_modos(source: &IMFMediaSource) -> Result<Vec<CameraMode>, CaptureError> {
    // SAFETY: fonte válida; sem atributos extras.
    let reader = unsafe { MFCreateSourceReaderFromMediaSource(source, None) }
        .map_err(|e| os("MFCreateSourceReaderFromMediaSource", &e))?;
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    let mut modos = Vec::new();
    for i in 0u32.. {
        // SAFETY: leitor válido.
        let tipo = match unsafe { reader.GetNativeMediaType(stream, i) } {
            Ok(t) => t,
            Err(e) if e.code() == MF_E_NO_MORE_TYPES => break,
            Err(e) => return Err(os("GetNativeMediaType", &e)),
        };
        // SAFETY: tipo válido; atributos empacotados em 2 x u32 (alto = largura/num).
        let (size, rate) = unsafe { (tipo.GetUINT64(&MF_MT_FRAME_SIZE), tipo.GetUINT64(&MF_MT_FRAME_RATE)) };
        let Ok(size) = size else { continue };
        let (width, height) = ((size >> 32) as u32, size as u32);
        if width == 0 || height == 0 {
            continue;
        }
        let fps = rate.map_or(0, |r| fps_from_ratio((r >> 32) as u32, r as u32));
        modos.push(CameraMode {
            width,
            height,
            fps,
            type_index: i,
        });
    }
    Ok(modos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_from_ratio_casos() {
        assert_eq!(fps_from_ratio(30, 1), 30);
        assert_eq!(fps_from_ratio(30000, 1001), 30);
        assert_eq!(fps_from_ratio(24000, 1001), 24);
        assert_eq!(fps_from_ratio(60, 1), 60);
        assert_eq!(fps_from_ratio(7, 0), 0);
    }

    #[test]
    fn list_cameras_nao_falha() {
        let cams = list_cameras().expect("list_cameras");
        println!("câmeras: {}", cams.len());
        if let Some(c) = cams.first() {
            let modos = native_modes(&c.id).expect("native_modes");
            assert!(modos.iter().all(|m| m.width > 0 && m.height > 0));
        }
    }
}
