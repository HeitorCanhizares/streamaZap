use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::convert;
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
    // O canal precisa existir antes do primeiro quadro chegar.
    state.videos.register(tile, channel);
    match state
        .core
        .watch(&track_id, tile, convert::tile_size_from_px(size), max_layer)
    {
        Ok(()) => Ok(()),
        Err(e) => {
            state.videos.remove(tile);
            Err(convert::err_string(e))
        }
    }
}

/// `core.unwatch(tile)` e `videos.remove(tile)`.
#[tauri::command]
pub async fn unwatch(state: State<'_, Arc<AppState>>, tile: u32) -> Result<(), String> {
    state.core.unwatch(tile);
    state.videos.remove(tile);
    Ok(())
}

/// `core.set_volume(&participant_id, volume, muted)`.
#[tauri::command]
pub async fn set_volume(
    state: State<'_, Arc<AppState>>,
    participant_id: String,
    volume: f32,
    muted: bool,
) -> Result<(), String> {
    state.core.set_volume(&participant_id, clamp_volume(volume), muted);
    Ok(())
}

/// Volume geral da sala: `core.set_master_volume(volume, muted)`.
#[tauri::command]
pub async fn set_master_volume(state: State<'_, Arc<AppState>>, volume: f32, muted: bool) -> Result<(), String> {
    state.core.set_master_volume(clamp_volume(volume), muted);
    Ok(())
}

/// `core.set_smoothing(auto, ms)`.
#[tauri::command]
pub async fn set_smoothing(state: State<'_, Arc<AppState>>, auto: bool, ms: u32) -> Result<(), String> {
    state.core.set_smoothing(auto, ms.min(2000));
    Ok(())
}

/// Limita o volume a 0..=1; NaN vira 0.0 (silêncio é o valor seguro).
fn clamp_volume(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_volume;

    #[test]
    fn volume_limitado() {
        assert_eq!(clamp_volume(1.5), 1.0);
        assert_eq!(clamp_volume(-1.0), 0.0);
        assert_eq!(clamp_volume(f32::NAN), 0.0);
        assert_eq!(clamp_volume(0.4), 0.4);
    }
}
