use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use zap_core::ShareRequest;

use crate::convert;
use crate::state::AppState;

/// `core.start_share(req)`.
#[tauri::command]
pub async fn share_start(state: State<'_, Arc<AppState>>, req: ShareRequest) -> Result<(), String> {
    state.core.start_share(req).await.map_err(convert::err_string)
}

/// `core.update_share(req)`.
#[tauri::command]
pub async fn share_update(state: State<'_, Arc<AppState>>, req: ShareRequest) -> Result<(), String> {
    state.core.update_share(req).await.map_err(convert::err_string)
}

/// `core.stop_share()`.
#[tauri::command]
pub async fn share_stop(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // `stop_share` não retorna erro: a parada é sempre "best effort".
    state.core.stop_share().await;
    Ok(())
}

/// `core.set_camera(on)`.
#[tauri::command]
pub async fn camera_set(state: State<'_, Arc<AppState>>, on: bool) -> Result<(), String> {
    state.core.set_camera(on).await.map_err(convert::err_string)
}

/// Prévia da câmera nas configurações: `videos.register(tile, channel)` ANTES de
/// `core.camera_preview(camera_id, tile)`; se o núcleo falhar, `videos.remove(tile)`.
#[tauri::command]
pub async fn camera_preview(
    state: State<'_, Arc<AppState>>,
    camera_id: Option<String>,
    tile: u32,
    channel: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    state.videos.register(tile, channel);
    match state.core.camera_preview(camera_id, tile).await {
        Ok(()) => Ok(()),
        Err(e) => {
            state.videos.remove(tile);
            Err(convert::err_string(e))
        }
    }
}

/// `core.camera_preview_stop(tile)` e `videos.remove(tile)`.
#[tauri::command]
pub async fn camera_preview_stop(state: State<'_, Arc<AppState>>, tile: u32) -> Result<(), String> {
    state.core.camera_preview_stop(tile);
    state.videos.remove(tile);
    Ok(())
}
