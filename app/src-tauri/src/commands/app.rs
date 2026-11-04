use serde::Serialize;

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

/// Diagnóstico completo é do M5: devolve `Err("Diagnóstico completo chega no M5")`.
#[tauri::command]
pub async fn diagnostics_run() -> Result<(), String> {
    Err("Diagnóstico completo chega no M5".to_string())
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

    #[test]
    fn diagnostico_pendente() {
        let erro =
            tauri::async_runtime::block_on(diagnostics_run()).expect_err("diagnostics_run deve ser Err até o M5");
        assert!(erro.contains("M5"));
    }
}
