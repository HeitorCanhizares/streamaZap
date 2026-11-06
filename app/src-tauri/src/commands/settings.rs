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
    use zap_core::settings::{apply_patch, load, UpdateSettings};
    use zap_core::{InstallMode, RoomMode, Settings, UpdateChannel};

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

    /// Liga o Online por patch parcial: o resto de `server` continua como estava.
    #[test]
    fn patch_liga_o_online() {
        let padrao = Settings::default();
        let s = apply_patch(&padrao, &json!({"server": {"enabled": true}})).expect("patch válido");
        assert!(s.server.enabled);
        assert_eq!(s.server.address, padrao.server.address);
        assert_eq!(s.server.password, padrao.server.password);
        assert_eq!(s.server.use_relay, padrao.server.use_relay);
    }

    /// Desligar o Online leva `open_in` de volta para `Local` (regra do `normalize`).
    #[test]
    fn patch_desliga_o_online_e_volta_para_local() {
        let mut s = Settings::default();
        s.server.enabled = true;
        s.open_in = RoomMode::Online;
        let novo = apply_patch(&s, &json!({"server": {"enabled": false}})).expect("patch válido");
        assert!(!novo.server.enabled);
        assert_eq!(novo.open_in, RoomMode::Local);
    }

    /// Cada modo de instalação do JSON vira o enum certo; valor desconhecido é erro.
    #[test]
    fn patch_do_modo_de_instalacao() {
        let padrao = Settings::default();
        let casos = [
            ("ask", InstallMode::Ask),
            ("on_exit", InstallMode::OnExit),
            ("auto", InstallMode::Auto),
        ];
        for (texto, esperado) in casos {
            let novo = apply_patch(&padrao, &json!({"update": {"install": texto}})).expect("patch válido");
            assert_eq!(novo.update.install, esperado, "modo {texto}");
            assert_eq!(novo.update.check, padrao.update.check);
            assert_eq!(novo.update.download, padrao.update.download);
            assert_eq!(novo.update.channel, padrao.update.channel);
        }
        assert!(apply_patch(&padrao, &json!({"update": {"install": "qualquer"}})).is_err());
    }

    /// Desligar procurar e baixar atualização por patch.
    #[test]
    fn patch_de_baixar_e_procurar() {
        let novo = apply_patch(
            &Settings::default(),
            &json!({"update": {"check": false, "download": false}}),
        )
        .expect("patch válido");
        assert!(!novo.update.check);
        assert!(!novo.update.download);
    }

    /// Grava `conteudo` como `settings.json` num dir temporário único, carrega com `load` e
    /// apaga o dir.
    fn carregar_arquivo(conteudo: &str) -> Settings {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("streamazap-settings-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("cria dir temporário");
        std::fs::write(dir.join("settings.json"), conteudo).expect("grava settings.json de teste");
        let s = load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        s
    }

    /// `update.auto` antigo vira `check`/`download`/`install` (migração do core-01).
    #[test]
    fn migracao_update_auto_true_e_false() {
        let ligado = carregar_arquivo(r#"{"update":{"auto":true}}"#);
        assert_eq!(
            ligado.update,
            UpdateSettings {
                check: true,
                download: true,
                install: InstallMode::Auto,
                channel: UpdateChannel::Estavel,
            }
        );

        let desligado = carregar_arquivo(r#"{"update":{"auto":false}}"#);
        assert_eq!(
            desligado.update,
            UpdateSettings {
                check: false,
                download: false,
                install: InstallMode::Ask,
                channel: UpdateChannel::Estavel,
            }
        );
    }

    /// Arquivo antigo sem `server.enabled`: quem tinha senha continua com o Online ligado.
    #[test]
    fn migracao_server_enabled_pela_senha() {
        assert!(carregar_arquivo(r#"{"server":{"password":"abc"}}"#).server.enabled);
        assert!(!carregar_arquivo(r#"{"server":{"password":""}}"#).server.enabled);
    }
}
