use crate::{keep_window, CaptureError, WindowCandidate, WindowInfo};
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

/// Só o nome do arquivo de um caminho (separador `\` ou `/`).
fn file_name_of(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or("").to_string()
}

/// Nome do executável do processo; vazio se não for possível abrir o processo.
fn process_name(pid: u32) -> String {
    // SAFETY: o HANDLE é usado só aqui e fechado antes de sair; o buffer é local e `len` informa seu tamanho.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        if r.is_err() {
            return String::new();
        }
        file_name_of(&String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Lê os dados crus de uma janela.
fn candidate_of(hwnd: HWND) -> WindowCandidate {
    // SAFETY: `hwnd` vem do EnumWindows; todos os ponteiros passados apontam para variáveis locais válidas.
    unsafe {
        let visible = IsWindowVisible(hwnd).as_bool();
        let mut cloaked_val: u32 = 0;
        let cloaked = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked_val as *mut u32 as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
        .map(|_| cloaked_val != 0)
        .unwrap_or(false);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let tool_window = ex & WS_EX_TOOLWINDOW.0 != 0;

        let n = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let got = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
        let title = String::from_utf16_lossy(&buf[..got.min(buf.len())]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut r = RECT::default();
        let _ = GetWindowRect(hwnd, &mut r);
        WindowCandidate {
            visible,
            cloaked,
            tool_window,
            title,
            pid,
            width: (r.right - r.left).max(0) as u32,
            height: (r.bottom - r.top).max(0) as u32,
        }
    }
}

/// Callback do `EnumWindows`: acumula os HWND no `Vec` passado em `data`.
unsafe extern "system" fn coleta_janela(hwnd: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` é o ponteiro para o `Vec<HWND>` de `list_windows`, vivo durante a enumeração.
    let lista = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
    lista.push(hwnd);
    TRUE
}

/// Janelas capturáveis (filtradas por `logic::keep_window`).
pub fn list_windows() -> Result<Vec<WindowInfo>, CaptureError> {
    zap_gpu::pin_mta();
    let mut hwnds: Vec<HWND> = Vec::new();
    // SAFETY: o ponteiro para `hwnds` fica válido até o fim do EnumWindows (síncrono).
    unsafe {
        EnumWindows(Some(coleta_janela), LPARAM(&mut hwnds as *mut Vec<HWND> as isize)).map_err(|e| {
            CaptureError::Os {
                context: "EnumWindows",
                message: e.to_string(),
            }
        })?;
    }
    let own_pid = std::process::id();
    let mut out = Vec::new();
    for hwnd in hwnds {
        let c = candidate_of(hwnd);
        if !keep_window(&c, own_pid) {
            continue;
        }
        out.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            process_name: process_name(c.pid),
            title: c.title,
            pid: c.pid,
            width: c.width,
            height: c.height,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_of_extrai_nome() {
        assert_eq!(file_name_of("C:\\Jogos\\cs2.exe"), "cs2.exe");
        assert_eq!(file_name_of("C:/Jogos/cs2.exe"), "cs2.exe");
        assert_eq!(file_name_of("cs2.exe"), "cs2.exe");
        assert_eq!(file_name_of(""), "");
    }

    #[test]
    fn list_windows_nao_falha_e_nao_inclui_proprio_pid() {
        let v = list_windows().expect("EnumWindows não deveria falhar");
        let own = std::process::id();
        assert!(v.iter().all(|w| w.pid != own));
    }
}
