use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use zap_core::{Preset, ShareRequest};
use zap_proto::CameraLayout;

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

/// `core.set_camera_quality(preset)`: qualidade da câmera à parte da tela, ao vivo se ligada.
#[tauri::command]
pub async fn camera_quality_set(state: State<'_, Arc<AppState>>, preset: Preset) -> Result<(), String> {
    state.core.set_camera_quality(preset).await.map_err(convert::err_string)
}

/// `core.set_camera_layout(layout)`: canto ou solta, ao vivo se a câmera está ligada.
#[tauri::command]
pub async fn camera_layout_set(state: State<'_, Arc<AppState>>, layout: CameraLayout) -> Result<(), String> {
    state.core.set_camera_layout(layout).await.map_err(convert::err_string)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json, to_value};
    use zap_proto::Corner;

    // A interface manda os argumentos em JSON; o Tauri desserializa com serde.
    // Estes testes travam o formato do contrato para os comandos de câmera.

    #[test]
    fn preset_da_camera_aceita_os_quatro_valores() {
        assert_eq!(from_value::<Preset>(json!("720p30")).ok(), Some(Preset::P720p30));
        assert_eq!(from_value::<Preset>(json!("720p60")).ok(), Some(Preset::P720p60));
        assert_eq!(from_value::<Preset>(json!("1080p30")).ok(), Some(Preset::P1080p30));
        assert_eq!(from_value::<Preset>(json!("1080p60")).ok(), Some(Preset::P1080p60));
        assert!(from_value::<Preset>(json!("4k")).is_err());
    }

    #[test]
    fn layout_canto_aceita_os_quatro_cantos() {
        let canto = |c: &str| json!({"modo": "canto", "canto": c});
        assert_eq!(
            from_value::<CameraLayout>(canto("sup_esq")).ok(),
            Some(CameraLayout::Corner {
                corner: Corner::TopLeft
            })
        );
        assert_eq!(
            from_value::<CameraLayout>(canto("sup_dir")).ok(),
            Some(CameraLayout::Corner {
                corner: Corner::TopRight
            })
        );
        assert_eq!(
            from_value::<CameraLayout>(canto("inf_esq")).ok(),
            Some(CameraLayout::Corner {
                corner: Corner::BottomLeft
            })
        );
        assert_eq!(
            from_value::<CameraLayout>(canto("inf_dir")).ok(),
            Some(CameraLayout::Corner {
                corner: Corner::BottomRight
            })
        );
    }

    #[test]
    fn layout_solta() {
        assert_eq!(
            from_value::<CameraLayout>(json!({"modo": "solta"})).ok(),
            Some(CameraLayout::Free)
        );
    }

    #[test]
    fn layout_invalido_falha() {
        assert!(from_value::<CameraLayout>(json!({"modo": "canto", "canto": "meio"})).is_err());
        assert!(from_value::<CameraLayout>(json!({"modo": "flutuante"})).is_err());
    }

    #[test]
    fn layout_serializa_igual_ao_contrato() {
        assert_eq!(to_value(CameraLayout::Free).ok(), Some(json!({"modo": "solta"})));
        assert_eq!(
            to_value(CameraLayout::default()).ok(),
            Some(json!({"modo": "canto", "canto": "inf_dir"}))
        );
    }
}
