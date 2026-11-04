use tauri::{AppHandle, Manager, WebviewWindow};

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "Janela principal não encontrada".to_string())
}

/// Mostra, tira do minimizado e foca a janela `main` (segunda instância do app).
pub fn focus_main(app: &AppHandle) {
    // Sem janela não há o que focar; erros de foco não devem derrubar a instância.
    if let Ok(w) = main_window(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Minimiza a janela `main`.
#[tauri::command]
pub async fn window_minimize(app: AppHandle) -> Result<(), String> {
    let w = main_window(&app)?;
    w.minimize().map_err(|e| e.to_string())
}

/// Alterna maximizar/restaurar.
#[tauri::command]
pub async fn window_toggle_maximize(app: AppHandle) -> Result<(), String> {
    let w = main_window(&app)?;
    if w.is_maximized().map_err(|e| e.to_string())? {
        w.unmaximize().map_err(|e| e.to_string())
    } else {
        w.maximize().map_err(|e| e.to_string())
    }
}

/// Fecha a janela `main` (encerra o app).
#[tauri::command]
pub async fn window_close(app: AppHandle) -> Result<(), String> {
    let w = main_window(&app)?;
    w.close().map_err(|e| e.to_string())
}

/// Tela cheia ligada/desligada (F11 da sala).
#[tauri::command]
pub async fn window_set_fullscreen(app: AppHandle, on: bool) -> Result<(), String> {
    let w = main_window(&app)?;
    w.set_fullscreen(on).map_err(|e| e.to_string())
}
