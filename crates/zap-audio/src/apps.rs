//! Lista de apps cujo som dá para capturar (só Windows).

use crate::app_tree::{self, ProcTable};
use crate::com::ComGuard;
use crate::{AppSound, AudioApp, AudioError};
use std::ffi::c_void;
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, STILL_ACTIVE, S_OK};
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, AudioSessionStateInactive, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

fn backend<E: std::fmt::Display>(contexto: &str, e: E) -> AudioError {
    AudioError::Backend(format!("{contexto}: {e}"))
}

/// Pega só o nome do arquivo (após a última '\\') de um caminho do Windows.
pub fn file_name_from_path(path: &str) -> String {
    path.rsplit('\\').next().unwrap_or(path).to_string()
}

/// Todos os processos: pid → (pai, executável). Falha vira tabela vazia.
pub(crate) fn process_table() -> ProcTable {
    let mut tabela = ProcTable::new();
    // SAFETY: snapshot de processos; o handle é fechado no fim.
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return tabela;
    };
    let mut e = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: `e` tem `dwSize` preenchido e vive durante as chamadas; handle válido.
    let mut ok = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
    while ok {
        let fim = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
        let exe = String::from_utf16_lossy(&e.szExeFile[..fim]);
        tabela.insert(e.th32ProcessID, (e.th32ParentProcessID, exe));
        // SAFETY: idem.
        ok = unsafe { Process32NextW(snap, &mut e) }.is_ok();
    }
    // SAFETY: handle do snapshot, fechado uma única vez.
    unsafe {
        let _ = CloseHandle(snap);
    }
    tabela
}

/// O processo ainda está rodando. Sem permissão para abrir: considera vivo.
pub(crate) fn is_alive(pid: u32) -> bool {
    // SAFETY: OpenProcess com direito mínimo; o handle é fechado abaixo.
    let h = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        // Acesso negado: o processo existe (protegido); qualquer outro erro: não existe mais.
        Err(e) => return e.code() == E_ACCESSDENIED,
    };
    let mut codigo = 0u32;
    // SAFETY: handle válido; ponteiro de saída local.
    let r = unsafe { GetExitCodeProcess(h, &mut codigo) };
    // SAFETY: handle obtido acima, fechado uma única vez.
    unsafe {
        let _ = CloseHandle(h);
    }
    r.is_err() || codigo == STILL_ACTIVE.0 as u32
}

/// Caminho completo do executável do processo.
fn image_path(pid: u32) -> Option<String> {
    // SAFETY: OpenProcess com direito mínimo; o handle é fechado abaixo.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` tem `len` elementos e vive durante a chamada; handle válido.
    let res = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) };
    // SAFETY: handle obtido por OpenProcess acima, fechado uma única vez.
    unsafe {
        let _ = CloseHandle(handle);
    }
    res.ok()?;
    let caminho = String::from_utf16_lossy(&buf[..len as usize]);
    (!caminho.is_empty()).then_some(caminho)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Texto de um valor do bloco de versão (`\StringFileInfo\...`).
fn version_string(bloco: &[u8], chave: &str) -> Option<String> {
    let chave = wide(chave);
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `bloco` é o buffer preenchido por GetFileVersionInfoW e vive durante a leitura;
    // o ponteiro devolvido aponta para dentro dele, com `len` caracteres UTF-16.
    let ok = unsafe {
        VerQueryValueW(
            bloco.as_ptr() as *const c_void,
            PCWSTR(chave.as_ptr()),
            &mut ptr,
            &mut len,
        )
    };
    if !ok.as_bool() || ptr.is_null() || len == 0 {
        return None;
    }
    // SAFETY: ver acima.
    let s = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
    let fim = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    let texto = String::from_utf16_lossy(&s[..fim]).trim().to_string();
    (!texto.is_empty()).then_some(texto)
}

/// Descrição do executável ("Google Chrome", "Spotify"), lida dos dados de versão do arquivo.
fn file_description(caminho: &str) -> Option<String> {
    let nome = wide(caminho);
    // SAFETY: string terminada em nulo que vive durante a chamada.
    let tam = unsafe { GetFileVersionInfoSizeW(PCWSTR(nome.as_ptr()), None) };
    if tam == 0 {
        return None;
    }
    let mut bloco = vec![0u8; tam as usize];
    // SAFETY: `bloco` tem `tam` bytes.
    unsafe { GetFileVersionInfoW(PCWSTR(nome.as_ptr()), None, tam, bloco.as_mut_ptr() as *mut c_void) }.ok()?;

    let mut idiomas: Vec<String> = Vec::new();
    let traducao = wide("\\VarFileInfo\\Translation");
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: como em `version_string`; a tradução é uma lista de pares u16 (idioma, página).
    let ok = unsafe {
        VerQueryValueW(
            bloco.as_ptr() as *const c_void,
            PCWSTR(traducao.as_ptr()),
            &mut ptr,
            &mut len,
        )
    };
    if ok.as_bool() && !ptr.is_null() && len >= 4 {
        // SAFETY: `len` bytes válidos dentro de `bloco`.
        let pares = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize / 2) };
        for p in pares.chunks_exact(2) {
            idiomas.push(format!("{:04x}{:04x}", p[0], p[1]));
        }
    }
    idiomas.extend(["040904b0", "040904e4", "041604b0", "000004b0"].map(String::from));
    idiomas
        .iter()
        .find_map(|i| version_string(&bloco, &format!("\\StringFileInfo\\{i}\\FileDescription")))
}

/// Pids com sessão de som em qualquer saída ativa (caixas, fone, HDMI), com o estado delas.
fn session_pids() -> Result<Vec<(u32, AppSound)>, AudioError> {
    let mut achados = Vec::new();
    // SAFETY: COM inicializado na thread pelo chamador; interfaces gerenciadas pelo crate windows.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| backend("CoCreateInstance(MMDeviceEnumerator)", e))?;
        // Sem dispositivo de saída: lista vazia em vez de erro.
        let Ok(dispositivos) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
            return Ok(achados);
        };
        let n = dispositivos.GetCount().unwrap_or(0);
        for d in 0..n {
            let Ok(device) = dispositivos.Item(d) else {
                continue;
            };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessoes) = manager.GetSessionEnumerator() else {
                continue;
            };
            let count = sessoes.GetCount().unwrap_or(0);
            for i in 0..count {
                let Ok(ctrl) = sessoes.GetSession(i) else {
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
                let som = match ctrl2.GetState() {
                    Ok(s) if s == AudioSessionStateActive => AppSound::Playing,
                    Ok(s) if s == AudioSessionStateInactive => AppSound::Ready,
                    // Expirada: o app já fechou o áudio.
                    _ => continue,
                };
                achados.push((pid, som));
            }
        }
    }
    Ok(achados)
}

/// Apps cujo som dá para capturar: os que têm sessão de som em qualquer saída (tocando ou
/// quietos) e os processos com janela aberta (`window_pids`), um item por app (raiz da árvore),
/// sem o próprio streamaZap. Ordem: tocando, com som, só com janela; dentro, por nome.
pub fn list_audio_apps(window_pids: &[u32]) -> Result<Vec<AudioApp>, AudioError> {
    let _com = ComGuard::new();
    let procs = process_table();
    let mut achados = session_pids()?;
    achados.extend(window_pids.iter().map(|&p| (p, AppSound::Silent)));
    let mut apps: Vec<AudioApp> = app_tree::group(&achados, &procs, std::process::id())
        .into_iter()
        .map(|(pid, exe, sound)| {
            let descricao = image_path(pid).and_then(|c| file_description(&c));
            AudioApp {
                pid,
                name: app_tree::display_name(descricao.as_deref(), &exe),
                exe,
                sound,
            }
        })
        .collect();
    app_tree::finish(&mut apps);
    Ok(apps)
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
    fn tabela_tem_o_proprio_processo() {
        let t = process_table();
        let (_, exe) = t.get(&std::process::id()).expect("o próprio processo na tabela");
        assert!(exe.to_lowercase().ends_with(".exe"), "{exe}");
        assert!(is_alive(std::process::id()));
        assert!(!is_alive(0xFFFF_FFF0));
    }

    #[test]
    fn descricao_de_um_executavel_do_windows() {
        let caminho = r"C:\Windows\explorer.exe";
        if std::path::Path::new(caminho).exists() {
            let d = file_description(caminho).expect("explorer.exe tem descrição");
            assert!(!d.is_empty());
        }
    }

    #[test]
    fn list_audio_apps_sem_erro() {
        // Inclui o próprio processo como "janela": tem de ficar de fora.
        let me = std::process::id();
        let apps = list_audio_apps(&[me]).expect("deve listar sem erro");
        for a in &apps {
            eprintln!("{:?} {} [{}] {}", a.sound, a.name, a.exe, a.pid);
        }
        assert!(apps.iter().all(|a| a.pid != me));
        let mut pids: Vec<u32> = apps.iter().map(|a| a.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(pids.len(), apps.len());
        let ordenado = apps.windows(2).all(|w| w[0].sound <= w[1].sound);
        assert!(ordenado);
    }
}
