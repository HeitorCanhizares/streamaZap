use std::sync::Arc;

use tauri::State;
use zap_core::{CreateRoom, JoinTarget};

use crate::state::AppState;

/// `core.create_room(req)`.
#[tauri::command]
pub async fn room_create(state: State<'_, Arc<AppState>>, req: CreateRoom) -> Result<(), String> {
    todo!("tarefa tauri-09")
}

/// `core.join_room(target, password)`.
#[tauri::command]
pub async fn room_join(
    state: State<'_, Arc<AppState>>,
    target: JoinTarget,
    password: Option<String>,
) -> Result<(), String> {
    todo!("tarefa tauri-09")
}

/// `core.leave_room()`; limpa os canais de vídeo (`videos`).
#[tauri::command]
pub async fn room_leave(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    todo!("tarefa tauri-09")
}

/// `core.send_chat(text)`.
#[tauri::command]
pub async fn chat_send(state: State<'_, Arc<AppState>>, text: String) -> Result<(), String> {
    todo!("tarefa tauri-09")
}

/// Se `convert::should_send_typing(typing)`: `core.send_typing()`.
#[tauri::command]
pub async fn chat_typing(state: State<'_, Arc<AppState>>, typing: bool) -> Result<(), String> {
    todo!("tarefa tauri-09")
}
