use crate::com::ComGuard;
use crate::{AudioApp, AudioError};
use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, S_OK};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

fn backend<E: std::fmt::Display>(contexto: &str, e: E) -> AudioError {
    AudioError::Backend(format!("{contexto}: {e}"))
}

/// Pega só o nome do arquivo (após a última '\\') de um caminho do Windows.
pub fn file_name_from_path(path: &str) -> String {
    path.rsplit('\\').next().unwrap_or(path).to_string()
}

/// Nome do executável do processo; se não der para abrir, "pid <N>".
fn process_name(pid: u32) -> String {
    let fallback = || format!("pid {pid}");
    // SAFETY: OpenProcess com direito mínimo; o handle é fechado abaixo.
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return fallback(),
    };
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` tem `len` elementos e vive durante a chamada; handle válido.
    let res = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) };
    // SAFETY: handle obtido por OpenProcess acima, fechado uma única vez.
    unsafe {
        let _ = CloseHandle(handle);
    }
    match res {
        Ok(()) => {
            let caminho = String::from_utf16_lossy(&buf[..len as usize]);
            let nome = file_name_from_path(&caminho);
            if nome.is_empty() {
                fallback()
            } else {
                nome
            }
        }
        Err(_) => fallback(),
    }
}

/// Sessões de áudio ativas no dispositivo de renderização padrão
/// (IMMDeviceEnumerator -> IAudioSessionManager2 -> sessões com estado Active e pid != 0),
/// sem o próprio processo, sem repetir pid, ordenadas por nome.
pub fn list_audio_apps() -> Result<Vec<AudioApp>, AudioError> {
    let _com = ComGuard::new();
    // SAFETY: COM inicializado na thread pelo ComGuard; interfaces gerenciadas pelo crate windows.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| backend("CoCreateInstance(MMDeviceEnumerator)", e))?;
        // Sem dispositivo de saída: lista vazia em vez de erro.
        let device = match enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let manager: IAudioSessionManager2 = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| backend("Activate(IAudioSessionManager2)", e))?;
        let sessions = manager
            .GetSessionEnumerator()
            .map_err(|e| backend("GetSessionEnumerator", e))?;
        let count = sessions
            .GetCount()
            .map_err(|e| backend("IAudioSessionEnumerator::GetCount", e))?;

        let meu_pid = std::process::id();
        let mut apps: Vec<AudioApp> = Vec::new();
        for i in 0..count {
            let Ok(ctrl) = sessions.GetSession(i) else {
                continue;
            };
            let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() else {
                continue;
            };
            if ctrl2.IsSystemSoundsSession() == S_OK {
                continue;
            }
            let Ok(pid) = ctrl2.GetProcessId() else {
                continue;
            };
            if pid == 0 || pid == meu_pid {
                continue;
            }
            match ctrl2.GetState() {
                Ok(s) if s == AudioSessionStateActive => {}
                _ => continue,
            }
            if apps.iter().any(|a| a.pid == pid) {
                continue;
            }
            apps.push(AudioApp {
                pid,
                name: process_name(pid),
            });
        }
        apps.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.pid.cmp(&b.pid))
        });
        Ok(apps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_com_separador() {
        assert_eq!(file_name_from_path(r"C:\Games\cs2\cs2.exe"), "cs2.exe");
    }

    #[test]
    fn file_name_sem_separador() {
        assert_eq!(file_name_from_path("app.exe"), "app.exe");
    }

    #[test]
    fn file_name_vazio() {
        assert_eq!(file_name_from_path(""), "");
    }

    #[test]
    fn list_audio_apps_sem_erro() {
        let apps = list_audio_apps().expect("deve listar sem erro");
        let me = std::process::id();
        assert!(apps.iter().all(|a| a.pid != me));
        let mut pids: Vec<u32> = apps.iter().map(|a| a.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(pids.len(), apps.len());
        let ordenado = apps
            .windows(2)
            .all(|w| (w[0].name.to_lowercase(), w[0].pid) <= (w[1].name.to_lowercase(), w[1].pid));
        assert!(ordenado);
    }
}
