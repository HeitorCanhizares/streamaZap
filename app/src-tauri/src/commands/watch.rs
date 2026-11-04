use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::state::AppState;

/// `videos.register(tile, channel)` ANTES de `core.watch(&track_id, tile,
/// convert::tile_size_from_px(size), max_layer)`; se o núcleo falhar, `videos.remove(tile)`.
/// Argumentos como no `api.ts`: `trackId, tile, size: [w, h], maxLayer: Option<u8>, channel`.
#[tauri::command]
pub async fn watch(
    state: State<'_, Arc<AppState>>,
    track_id: String,
    tile: u32,
    size: [u32; 2],
    max_layer: Option<u8>,
    channel: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    todo!("tarefa tauri-11")
}

/// `core.unwatch(tile)` e `videos.remove(tile)`.
#[tauri::command]
pub async fn unwatch(state: State<'_, Arc<AppState>>, tile: u32) -> Result<(), String> {
    todo!("tarefa tauri-11")
}

/// `core.set_volume(&participant_id, volume, muted)`.
#[tauri::command]
pub async fn set_volume(
    state: State<'_, Arc<AppState>>,
    participant_id: String,
    volume: f32,
    muted: bool,
) -> Result<(), String> {
    todo!("tarefa tauri-11")
}

/// `core.set_smoothing(auto, ms)`.
#[tauri::command]
pub async fn set_smoothing(state: State<'_, Arc<AppState>>, auto: bool, ms: u32) -> Result<(), String> {
    todo!("tarefa tauri-11")
}
