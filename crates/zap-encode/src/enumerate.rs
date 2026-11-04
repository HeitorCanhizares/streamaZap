//! Enumeração dos MFT codificadores H.264.
use crate::logic::vendor_from_mft_string;
use crate::mf::MfGuard;
use crate::{EncodeError, EncoderInfo};
use windows::core::{GUID, PWSTR};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFMediaType_Video, MFTEnumEx, MFT_ENUM_HARDWARE_VENDOR_ID_Attribute, MFT_FRIENDLY_NAME_Attribute,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_REGISTER_TYPE_INFO,
};
use windows::Win32::System::Com::CoTaskMemFree;
use zap_gpu::Vendor;

/// Um MFT encontrado, ainda não ativado.
pub(crate) struct Candidate {
    pub info: EncoderInfo,
    pub activate: IMFActivate,
}

/// `MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, flags, entrada NV12, saída H264)`. `hardware = true`:
/// `MFT_ENUM_FLAG_HARDWARE | SORTANDFILTER`; `false`: `MFT_ENUM_FLAG_SYNCMFT | SORTANDFILTER`.
/// Exige `MfGuard` vivo na thread chamadora.
pub(crate) fn enumerate(hardware: bool) -> Result<Vec<Candidate>, EncodeError> {
    let entrada = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let saida = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let base: MFT_ENUM_FLAG = if hardware {
        MFT_ENUM_FLAG_HARDWARE
    } else {
        MFT_ENUM_FLAG_SYNCMFT
    };
    let flags = MFT_ENUM_FLAG(base.0 | MFT_ENUM_FLAG_SORTANDFILTER.0);

    let mut ativadores: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut quantidade: u32 = 0;
    // SAFETY: os ponteiros de tipo apontam para locais vivos; os de saída são locais válidos.
    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            flags,
            Some(&entrada),
            Some(&saida),
            &mut ativadores,
            &mut quantidade,
        )
    }
    .map_err(|e| EncodeError::os("MFTEnumEx", &e))?;

    let mut encontrados = Vec::new();
    if !ativadores.is_null() {
        for i in 0..quantidade as usize {
            // SAFETY: o array tem `quantidade` elementos; `take` move a referência COM para fora
            // e deixa None, assim o array pode ser liberado sem Release duplicado.
            let ativador = unsafe { (*ativadores.add(i)).take() };
            if let Some(activate) = ativador {
                let info = montar_info(&activate, hardware);
                encontrados.push(Candidate { info, activate });
            }
        }
        // SAFETY: o array foi alocado com CoTaskMemAlloc pelo MFTEnumEx e não é mais usado.
        unsafe { CoTaskMemFree(Some(ativadores as *const _)) };
    }
    Ok(encontrados)
}

/// Lê um atributo string do ativador; `None` se faltar ou falhar.
fn ler_string(activate: &IMFActivate, chave: &GUID) -> Option<String> {
    let mut ptr = PWSTR::null();
    let mut len: u32 = 0;
    // SAFETY: `ptr` e `len` são locais válidos; em sucesso o MF aloca a string com CoTaskMemAlloc.
    unsafe { activate.GetAllocatedString(chave, &mut ptr, &mut len) }.ok()?;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: em sucesso `ptr` aponta para `len` u16 válidos, terminados em zero.
    let texto = unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len as usize)) };
    // SAFETY: a string foi alocada com CoTaskMemAlloc e não é usada depois.
    unsafe { CoTaskMemFree(Some(ptr.0 as *const _)) };
    Some(texto)
}

fn montar_info(activate: &IMFActivate, hardware: bool) -> EncoderInfo {
    let nome = ler_string(activate, &MFT_FRIENDLY_NAME_Attribute)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "H.264 encoder".to_string());
    let vendor = if hardware {
        match ler_string(activate, &MFT_ENUM_HARDWARE_VENDOR_ID_Attribute) {
            Some(v) => vendor_from_mft_string(&v),
            None => vendor_from_mft_string(&nome),
        }
    } else {
        Vendor::Microsoft
    };
    EncoderInfo {
        name: nome,
        vendor,
        hardware,
    }
}

/// Todos os codificadores (hardware primeiro, depois software). Faz o próprio `MfGuard::acquire`
/// e `zap_gpu::ComGuard::new()`. Lista vazia não é erro.
pub fn list_encoders() -> Result<Vec<EncoderInfo>, EncodeError> {
    let _com = zap_gpu::ComGuard::new()?;
    let _mf = MfGuard::acquire()?;
    let mut todos = Vec::new();
    for hardware in [true, false] {
        match enumerate(hardware) {
            Ok(lista) => todos.extend(lista.into_iter().map(|c| c.info)),
            Err(e) => tracing::warn!(hardware, "falha ao enumerar codificadores: {e}"),
        }
    }
    Ok(todos)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn list_encoders_nao_da_erro() {
        let lista = list_encoders().expect("list_encoders");
        if lista.is_empty() {
            eprintln!("sem codificadores H.264 neste runner: pulando");
            return;
        }
        assert!(lista.iter().all(|i| !i.name.is_empty()));
        // Hardware antes de software: depois do primeiro software não pode haver hardware.
        let primeiro_sw = lista.iter().position(|i| !i.hardware).unwrap_or(lista.len());
        assert!(lista[primeiro_sw..].iter().all(|i| !i.hardware));
    }

    #[test]
    fn enumerate_software_ou_vazio() {
        let _com = zap_gpu::ComGuard::new().expect("com");
        let _mf = MfGuard::acquire().expect("mf");
        let lista = enumerate(false).expect("enumerate");
        assert!(lista.iter().all(|c| !c.info.hardware && !c.info.name.is_empty()));
    }
}
