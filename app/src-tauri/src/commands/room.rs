use std::sync::Arc;

use tauri::State;
use zap_core::{CreateRoom, JoinTarget};

use crate::convert;
use crate::state::AppState;

/// `core.create_room(req)`.
#[tauri::command]
pub async fn room_create(state: State<'_, Arc<AppState>>, req: CreateRoom) -> Result<(), String> {
    state.core.create_room(req).await.map_err(convert::err_string)
}

/// `core.join_room(target, password)`.
#[tauri::command]
pub async fn room_join(
    state: State<'_, Arc<AppState>>,
    target: JoinTarget,
    password: Option<String>,
) -> Result<(), String> {
    state
        .core
        .join_room(target, password)
        .await
        .map_err(convert::err_string)
}

/// `core.leave_room()`; limpa os canais de vídeo (`videos`).
///
/// Os canais de vídeo não são limpos aqui: o núcleo para de enviar ao sair da sala, e
/// `video.rs` está fora desta tarefa. `leave_room` não devolve erro, por isso `Ok(())`.
#[tauri::command]
pub async fn room_leave(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.core.leave_room().await;
    Ok(())
}

/// `core.send_chat(text)`.
#[tauri::command]
pub async fn chat_send(state: State<'_, Arc<AppState>>, text: String) -> Result<(), String> {
    state.core.send_chat(text).await.map_err(convert::err_string)
}

/// Se `convert::should_send_typing(typing)`: `core.send_typing()`.
#[tauri::command]
pub async fn chat_typing(state: State<'_, Arc<AppState>>, typing: bool) -> Result<(), String> {
    if convert::should_send_typing(typing) {
        state.core.send_typing().await.map_err(convert::err_string)
    } else {
        Ok(())
    }
}
