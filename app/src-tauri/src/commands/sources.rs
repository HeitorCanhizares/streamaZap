use std::sync::Arc;

use tauri::ipc::Response;
use tauri::State;
use zap_core::Sources;

use crate::state::AppState;

/// `core.sources()` (pode demorar: roda em `spawn_blocking`).
#[tauri::command]
pub async fn sources_list(state: State<'_, Arc<AppState>>) -> Result<Sources, String> {
    todo!("tarefa tauri-08")
}

/// `zap_core::sources::parse_source(&id)` → `core.thumbnail(&ref, w, h)` (em `spawn_blocking`)
/// → `Response::new(convert::encode_thumbnail(&img))` (bytes crus).
#[tauri::command]
pub async fn source_thumbnail(state: State<'_, Arc<AppState>>, id: String, w: u32, h: u32) -> Result<Response, String> {
    todo!("tarefa tauri-08")
}
