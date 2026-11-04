use std::sync::Arc;

use tauri::State;
use zap_core::Settings;

use crate::state::AppState;

/// `state.core.settings()`.
#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    todo!("tarefa tauri-07")
}

/// Aplica o patch parcial (`zap_core::settings::apply_patch(&atual, &patch)`; erro →
/// `err_string`), grava via `core.update_settings(|s| *s = novo)` e devolve `core.settings()`.
#[tauri::command]
pub async fn settings_set(state: State<'_, Arc<AppState>>, patch: serde_json::Value) -> Result<Settings, String> {
    todo!("tarefa tauri-07")
}
