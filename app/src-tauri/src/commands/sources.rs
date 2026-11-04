use std::sync::Arc;

use tauri::ipc::Response;
use tauri::State;
use zap_core::Sources;

use crate::convert;
use crate::state::AppState;

/// Limite de uma dimensão de miniatura: evita pedidos absurdos ao sistema de captura.
const DIM_MIN: u32 = 1;
const DIM_MAX: u32 = 1024;

/// Restringe uma dimensão pedida pela interface ao intervalo `1..=1024`.
fn clamp_dim(v: u32) -> u32 {
    v.clamp(DIM_MIN, DIM_MAX)
}

/// `core.sources()` (pode demorar: roda em `spawn_blocking`).
#[tauri::command]
pub async fn sources_list(state: State<'_, Arc<AppState>>) -> Result<Sources, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || core.sources())
        .await
        .map_err(|e| e.to_string())
}

/// `zap_core::sources::parse_source(&id)` → `core.thumbnail(&ref, w, h)` (em `spawn_blocking`)
/// → `Response::new(convert::encode_thumbnail(&img))` (bytes crus).
#[tauri::command]
pub async fn source_thumbnail(state: State<'_, Arc<AppState>>, id: String, w: u32, h: u32) -> Result<Response, String> {
    // Id inválido falha antes de qualquer captura.
    let fonte = zap_core::sources::parse_source(&id).map_err(convert::err_string)?;
    let (w, h) = (clamp_dim(w), clamp_dim(h));
    let core = state.core.clone();
    let img = tauri::async_runtime::spawn_blocking(move || core.thumbnail(&fonte, w, h))
        .await
        .map_err(|e| e.to_string())?
        .map_err(convert::err_string)?;
    Ok(Response::new(convert::encode_thumbnail(&img)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_dim_limites() {
        assert_eq!(clamp_dim(0), 1);
        assert_eq!(clamp_dim(5000), 1024);
        assert_eq!(clamp_dim(300), 300);
    }
}
