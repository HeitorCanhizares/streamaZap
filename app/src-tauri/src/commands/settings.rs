use std::sync::Arc;

use tauri::State;
use zap_core::Settings;

use crate::convert;
use crate::state::AppState;

/// `state.core.settings()`.
#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.core.settings())
}

/// Aplica o patch parcial (`zap_core::settings::apply_patch(&atual, &patch)`; erro →
/// `err_string`), grava via `core.update_settings(|s| *s = novo)` e devolve `core.settings()`.
#[tauri::command]
pub async fn settings_set(state: State<'_, Arc<AppState>>, patch: serde_json::Value) -> Result<Settings, String> {
    let atual = state.core.settings();
    let novo = zap_core::settings::apply_patch(&atual, &patch).map_err(convert::err_string)?;
    state.core.update_settings(|s| *s = novo);
    Ok(state.core.settings())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use zap_core::Settings;

    /// Sem `Core` real no teste: exercita o caminho de erro do `apply_patch` usado por
    /// `settings_set`. Preset inválido precisa virar `Err` com mensagem não vazia.
    #[test]
    fn patch_invalido() {
        let resultado =
            zap_core::settings::apply_patch(&Settings::default(), &json!({"video": {"default_preset": "xx"}}));
        assert!(resultado.is_err());
        let msg = resultado.map_err(crate::convert::err_string).err().unwrap_or_default();
        assert!(!msg.is_empty());
    }
}
