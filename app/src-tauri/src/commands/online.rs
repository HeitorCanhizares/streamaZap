use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use zap_core::ServerTestResult;

use crate::state::AppState;

/// "Testar conexão": `core.server_test(address, password)`. Nunca falha: o resultado leva
/// `ok`, `code` e `message` (tarefa tauri-online-01).
#[tauri::command]
pub async fn server_test(
    state: State<'_, Arc<AppState>>,
    address: String,
    password: String,
) -> Result<ServerTestResult, String> {
    let _ = (state, address, password);
    todo!("tarefa tauri-online-01")
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
    let _ = (state, code, tile, channel);
    todo!("tarefa tauri-online-01")
}

/// `core.room_preview_stop(tile)` e `videos.remove(tile)` (tarefa tauri-online-01).
#[tauri::command]
pub async fn room_preview_stop(state: State<'_, Arc<AppState>>, tile: u32) -> Result<(), String> {
    let _ = (state, tile);
    todo!("tarefa tauri-online-01")
}
