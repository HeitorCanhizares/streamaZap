use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use zap_core::ServerTestResult;

use crate::convert;
use crate::state::AppState;

/// "Testar conexão": `core.server_test(address, password)`. Nunca falha: o resultado leva
/// `ok`, `code` e `message` (tarefa tauri-online-01).
#[tauri::command]
pub async fn server_test(
    state: State<'_, Arc<AppState>>,
    address: String,
    password: String,
) -> Result<ServerTestResult, String> {
    Ok(state.core.server_test(address, password).await)
}

/// Prévia na lista de salas: `videos.register(tile, channel)` ANTES de
/// `core.room_preview(code, tile)`; se o núcleo falhar, `videos.remove(tile)` (tarefa
/// tauri-online-01).
#[tauri::command]
pub async fn room_preview(
    state: State<'_, Arc<AppState>>,
    code: String,
    tile: u32,
    channel: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    // O canal precisa existir antes do primeiro quadro da prévia chegar.
    state.videos.register(tile, channel);
    match state.core.room_preview(code, tile).await {
        Ok(()) => Ok(()),
        Err(e) => {
            state.videos.remove(tile);
            Err(convert::err_string(e))
        }
    }
}

/// `core.room_preview_stop(tile)` e `videos.remove(tile)` (tarefa tauri-online-01).
#[tauri::command]
pub async fn room_preview_stop(state: State<'_, Arc<AppState>>, tile: u32) -> Result<(), String> {
    state.core.room_preview_stop(tile);
    state.videos.remove(tile);
    Ok(())
}

#[cfg(test)]
mod tests {
    use zap_core::ServerTestResult;

    #[test]
    fn server_test_result_serializa_chaves_snake_case() {
        let valor = ServerTestResult {
            ok: true,
            name: Some("Servidor".into()),
            ping_ms: Some(12),
            code: None,
            message: "Conectado".into(),
        };
        let json = serde_json::to_value(&valor).expect("serializa");
        let mut chaves: Vec<&str> = json.as_object().expect("objeto").keys().map(String::as_str).collect();
        chaves.sort_unstable();
        assert_eq!(chaves, vec!["code", "message", "name", "ok", "ping_ms"]);
        assert_eq!(json["ping_ms"], 12);
    }

    #[test]
    fn comandos_registrados_no_generate_handler() {
        let lib = include_str!("../lib.rs");
        assert!(lib.contains("commands::online::server_test,"));
        assert!(lib.contains("commands::online::room_preview,"));
        assert!(lib.contains("commands::online::room_preview_stop,"));
    }
}
