use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub os: String,
}

/// Função pura usada por `app_info`, para testar sem subir o Tauri.
fn build_info() -> AppInfo {
    AppInfo {
        name: "streamaZap".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
    }
}

/// `{ name: "streamaZap", version: env!("CARGO_PKG_VERSION"), os: std::env::consts::OS }`.
#[tauri::command]
pub async fn app_info() -> Result<AppInfo, String> {
    Ok(build_info())
}

/// Estado atual do núcleo. A interface pede ao abrir: o primeiro `zap://state` pode ter saído
/// antes de a página começar a ouvir.
#[tauri::command]
pub async fn snapshot_get(state: State<'_, Arc<AppState>>) -> Result<zap_core::Snapshot, String> {
    Ok((*state.core.snapshot()).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_campos() {
        let info = tauri::async_runtime::block_on(app_info()).expect("app_info deve ser Ok");
        assert_eq!(info.name, "streamaZap");
        assert!(!info.version.is_empty());
        assert!(!info.os.is_empty());
    }
}
