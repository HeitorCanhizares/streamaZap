use std::sync::Arc;

use tauri::State;
use zap_core::ShareRequest;

use crate::state::AppState;

/// `core.start_share(req)`.
#[tauri::command]
pub async fn share_start(state: State<'_, Arc<AppState>>, req: ShareRequest) -> Result<(), String> {
    todo!("tarefa tauri-10")
}

/// `core.update_share(req)`.
#[tauri::command]
pub async fn share_update(state: State<'_, Arc<AppState>>, req: ShareRequest) -> Result<(), String> {
    todo!("tarefa tauri-10")
}

/// `core.stop_share()`.
#[tauri::command]
pub async fn share_stop(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    todo!("tarefa tauri-10")
}

/// `core.set_camera(on)`.
#[tauri::command]
pub async fn camera_set(state: State<'_, Arc<AppState>>, on: bool) -> Result<(), String> {
    todo!("tarefa tauri-10")
}
