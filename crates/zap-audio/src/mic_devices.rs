//! Lista de microfones e forma da saída padrão (fone ou caixa).

use crate::AudioError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicDevice {
    /// Id do endpoint do Windows (`IMMDevice::GetId`).
    pub id: String,
    /// Nome amigável.
    pub name: String,
    /// É o padrão de comunicação.
    pub is_default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormFactor {
    Headphones,
    Speakers,
    Unknown,
}

/// Microfones ativos; `is_default` = o padrão de comunicação.
#[cfg(windows)]
pub fn list_microphones() -> Result<Vec<MicDevice>, AudioError> {
    win::list_microphones()
}

/// Fora do Windows: lista vazia.
#[cfg(not(windows))]
pub fn list_microphones() -> Result<Vec<MicDevice>, AudioError> {
    Ok(Vec::new())
}

/// Forma da saída padrão de reprodução; `Unknown` em qualquer falha.
#[cfg(windows)]
pub fn default_output_form_factor() -> OutputFormFactor {
    win::default_output_form_factor()
}

#[cfg(not(windows))]
pub fn default_output_form_factor() -> OutputFormFactor {
    OutputFormFactor::Unknown
}

#[cfg(windows)]
#[allow(unused_imports)]
pub(crate) use win::{capture_device_index, find_capture_device};

#[cfg(windows)]
mod win {
    use super::{MicDevice, OutputFormFactor};
    use crate::com::ComGuard;
    use crate::AudioError;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::{
        eCapture, eCommunications, eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::{PropVariantToStringAlloc, PropVariantToUInt32};
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};

    // PKEY_AudioEndpoint_FormFactor: {1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E}, pid 0.
    const PKEY_FORM_FACTOR: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e),
        pid: 0,
    };

    fn backend<E: std::fmt::Display>(contexto: &str, e: E) -> AudioError {
        AudioError::Backend(format!("{contexto}: {e}"))
    }

    fn enumerador() -> Result<IMMDeviceEnumerator, AudioError> {
        // SAFETY: COM já inicializado na thread pelo chamador; a interface é gerenciada pelo crate windows.
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| backend("CoCreateInstance(MMDeviceEnumerator)", e))
    }

    /// Converte um PWSTR devolvido pelo COM em `String` e libera a memória com `CoTaskMemFree`.
    fn id_para_string(pwstr: PWSTR) -> String {
        if pwstr.is_null() {
            return String::new();
        }
        // SAFETY: o ponteiro veio de uma API COM que aloca com CoTaskMemAlloc e termina em nulo;
        // é lido uma vez e liberado logo em seguida, sem uso posterior.
        unsafe {
            let texto = pwstr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(pwstr.0 as *const _));
            texto
        }
    }

    fn device_id(device: &IMMDevice) -> Option<String> {
        // SAFETY: chamada COM simples sobre interface válida.
        let pwstr = unsafe { device.GetId() }.ok()?;
        Some(id_para_string(pwstr))
    }

    fn friendly_name(device: &IMMDevice) -> Option<String> {
        // SAFETY: COM sobre interfaces válidas; o PROPVARIANT é limpo com PropVariantClear e a
        // string alocada por PropVariantToStringAlloc é liberada em `id_para_string`.
        unsafe {
            let store = device.OpenPropertyStore(STGM_READ).ok()?;
            let pv = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
            let texto = PropVariantToStringAlloc(&pv).ok().map(id_para_string);
            texto.filter(|t| !t.is_empty())
        }
    }

    fn default_capture_id(enumerator: &IMMDeviceEnumerator) -> Option<String> {
        // SAFETY: chamada COM simples sobre interface válida.
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications) }.ok()?;
        device_id(&device)
    }

    pub fn list_microphones() -> Result<Vec<MicDevice>, AudioError> {
        let _com = ComGuard::new();
        let enumerator = enumerador()?;
        let padrao = default_capture_id(&enumerator);
        // SAFETY: COM inicializado acima; interfaces gerenciadas pelo crate windows.
        let colecao = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }
            .map_err(|e| backend("EnumAudioEndpoints", e))?;
        // SAFETY: chamada COM simples sobre interface válida.
        let n = unsafe { colecao.GetCount() }.unwrap_or(0);
        let mut lista = Vec::new();
        for i in 0..n {
            // SAFETY: índice dentro de GetCount.
            let Ok(device) = (unsafe { colecao.Item(i) }) else {
                continue;
            };
            let Some(id) = device_id(&device).filter(|id| !id.is_empty()) else {
                continue;
            };
            let name = friendly_name(&device).unwrap_or_else(|| id.clone());
            let is_default = padrao.as_deref() == Some(id.as_str());
            lista.push(MicDevice { id, name, is_default });
        }
        lista.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(lista)
    }

    pub fn default_output_form_factor() -> OutputFormFactor {
        let _com = ComGuard::new();
        let Ok(enumerator) = enumerador() else {
            return OutputFormFactor::Unknown;
        };
        // SAFETY: COM sobre interfaces válidas; o PROPVARIANT se limpa sozinho no Drop.
        unsafe {
            let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else {
                return OutputFormFactor::Unknown;
            };
            let Ok(store) = device.OpenPropertyStore(STGM_READ) else {
                return OutputFormFactor::Unknown;
            };
            let Ok(pv) = store.GetValue(&PKEY_FORM_FACTOR) else {
                return OutputFormFactor::Unknown;
            };
            let valor = PropVariantToUInt32(&pv);
            match valor {
                // Headphones = 3, Headset = 5.
                Ok(3) | Ok(5) => OutputFormFactor::Headphones,
                // Speakers = 1.
                Ok(1) => OutputFormFactor::Speakers,
                _ => OutputFormFactor::Unknown,
            }
        }
    }

    /// Dispositivo de captura: `None` = padrão de comunicação; `Some(id)` = o endpoint com esse id.
    /// O chamador precisa ter COM inicializado na thread.
    #[allow(dead_code)] // usada pelas tarefas audio-06 e audio-07
    pub(crate) fn find_capture_device(id: Option<&str>) -> Result<IMMDevice, AudioError> {
        let enumerator = enumerador()?;
        match id {
            // SAFETY: chamada COM simples sobre interface válida.
            None => unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications) }
                .map_err(|_| AudioError::NoDevice),
            Some(id) => {
                let mut wide: Vec<u16> = id.encode_utf16().collect();
                wide.push(0);
                // SAFETY: `wide` termina em nulo e vive até o fim da chamada.
                unsafe { enumerator.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(|_| AudioError::NoDevice)
            }
        }
    }

    /// Índice do dispositivo na coleção de captura ativa; -1 para o padrão (`None`).
    #[allow(dead_code)] // usada pelas tarefas audio-06 e audio-07
    pub(crate) fn capture_device_index(id: Option<&str>) -> Result<i32, AudioError> {
        let Some(alvo) = id else {
            return Ok(-1);
        };
        let _com = ComGuard::new();
        let enumerator = enumerador()?;
        // SAFETY: COM inicializado acima; interfaces gerenciadas pelo crate windows.
        let colecao = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }
            .map_err(|e| backend("EnumAudioEndpoints", e))?;
        // SAFETY: chamada COM simples sobre interface válida.
        let n = unsafe { colecao.GetCount() }.unwrap_or(0);
        for i in 0..n {
            // SAFETY: índice dentro de GetCount.
            let Ok(device) = (unsafe { colecao.Item(i) }) else {
                continue;
            };
            if device_id(&device).as_deref() == Some(alvo) {
                return i32::try_from(i).map_err(|_| AudioError::NoDevice);
            }
        }
        Err(AudioError::NoDevice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn lista_nao_falha() {
        let lista = list_microphones().expect("lista deve ser Ok");
        if lista.is_empty() {
            eprintln!("aviso: nenhum microfone ativo neste PC");
            return;
        }
        assert!(lista.iter().filter(|m| m.is_default).count() <= 1);
        assert!(lista.iter().all(|m| !m.id.is_empty() && !m.name.is_empty()));
    }

    #[cfg(windows)]
    #[test]
    fn padrao_vem_primeiro() {
        let lista = list_microphones().expect("lista");
        if let Some(pos) = lista.iter().position(|m| m.is_default) {
            assert_eq!(pos, 0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn form_factor_nao_entra_em_panico() {
        let _ = default_output_form_factor();
    }

    #[cfg(windows)]
    #[test]
    fn indice_do_padrao_e_menos_um() {
        assert!(matches!(capture_device_index(None), Ok(-1)));
    }

    #[cfg(windows)]
    #[test]
    fn indice_de_id_listado() {
        for m in list_microphones().expect("lista") {
            let i = capture_device_index(Some(&m.id)).expect("índice");
            assert!(i >= 0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn id_inexistente() {
        let _com = crate::com::ComGuard::new();
        assert!(find_capture_device(Some("zzz-nao-existe")).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_fora_do_windows() {
        assert!(matches!(list_microphones(), Ok(v) if v.is_empty()));
        assert_eq!(default_output_form_factor(), OutputFormFactor::Unknown);
    }
}
