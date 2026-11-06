use std::sync::Arc;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use zap_core::ServerTestResult;

use crate::convert;
use crate::state::AppState;
use crate::update_state::UpdateState;

/// Marca a prévia do tile como aberta, para a instalação sozinha saber que há prévia na tela.
fn registrar_previa(upd: &UpdateState, tile: u32) {
    upd.previas.lock().insert(tile);
}

/// Tira a marca da prévia do tile. Tile que não existe não faz nada.
fn remover_previa(upd: &UpdateState, tile: u32) {
    upd.previas.lock().remove(&tile);
}

/// "Testar conexão": `core.server_test(address, password)`. Nunca falha: o resultado leva
/// `ok`, `code` e `message` (tarefa tauri-online-01).
#[tauri::command]
pub async fn server_test(
    state: State<'_, Arc<AppState>>,
    address: String,
    password: String,
) -> Result<ServerTestResult, String> {
    Ok(state.core.server_test(address, password).await)
}

/// `online_set_account(token)`: entrega ao núcleo o token de acesso da conta (`None` = saiu da
/// conta) com `core.online_set_account(token)`. Não bloqueia. Nunca logar o token.
#[tauri::command]
pub async fn online_set_account(state: State<'_, Arc<AppState>>, token: Option<String>) -> Result<(), String> {
    state.core.online_set_account(normalizar_token(token));
    Ok(())
}

/// Apara o token; vazio ou só espaços vira `None` (saiu da conta).
fn normalizar_token(token: Option<String>) -> Option<String> {
    token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// Prévia na lista de salas: `videos.register(tile, channel)` ANTES de
/// `core.room_preview(code, tile)`; se o núcleo falhar, `videos.remove(tile)` (tarefa
/// tauri-online-01).
#[tauri::command]
pub async fn room_preview(
    state: State<'_, Arc<AppState>>,
    upd: State<'_, UpdateState>,
    code: String,
    tile: u32,
    channel: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    // O canal precisa existir antes do primeiro quadro da prévia chegar.
    state.videos.register(tile, channel);
    match state.core.room_preview(code, tile).await {
        Ok(()) => {
            registrar_previa(&upd, tile);
            Ok(())
        }
        Err(e) => {
            state.videos.remove(tile);
            Err(convert::err_string(e))
        }
    }
}

/// `core.room_preview_stop(tile)` e `videos.remove(tile)` (tarefa tauri-online-01).
#[tauri::command]
pub async fn room_preview_stop(
    state: State<'_, Arc<AppState>>,
    upd: State<'_, UpdateState>,
    tile: u32,
) -> Result<(), String> {
    state.core.room_preview_stop(tile);
    state.videos.remove(tile);
    remover_previa(&upd, tile);
    Ok(())
}

#[cfg(test)]
mod tests {
    use zap_core::ServerTestResult;

    use super::{normalizar_token, registrar_previa, remover_previa};
    use crate::update_state::UpdateState;

    #[test]
    fn registrar_e_remover_previa() {
        let upd = UpdateState::default();
        registrar_previa(&upd, 1_000_001);
        assert_eq!(upd.previas.lock().len(), 1);
        registrar_previa(&upd, 1_000_001);
        assert_eq!(upd.previas.lock().len(), 1);
        registrar_previa(&upd, 1_000_002);
        assert_eq!(upd.previas.lock().len(), 2);
        remover_previa(&upd, 1_000_001);
        assert_eq!(upd.previas.lock().len(), 1);
        remover_previa(&upd, 9_999_999);
        assert_eq!(upd.previas.lock().len(), 1);
    }

    #[test]
    fn server_test_result_serializa_chaves_snake_case() {
        let valor = ServerTestResult {
            ok: true,
            name: Some("Servidor".into()),
            ping_ms: Some(12),
            code: None,
            message: "Conectado".into(),
            ..Default::default()
        };
        let json = serde_json::to_value(&valor).expect("serializa");
        let mut chaves: Vec<&str> = json.as_object().expect("objeto").keys().map(String::as_str).collect();
        chaves.sort_unstable();
        assert_eq!(
            chaves,
            vec!["code", "message", "name", "ok", "ping_ms", "requires_account"]
        );
        assert_eq!(json["ping_ms"], 12);
    }

    #[test]
    fn comandos_registrados_no_generate_handler() {
        let lib = include_str!("../lib.rs");
        assert!(lib.contains("commands::online::server_test,"));
        assert!(lib.contains("commands::online::room_preview,"));
        assert!(lib.contains("commands::online::room_preview_stop,"));
        assert!(lib.contains("commands::online::online_set_account,"));
    }

    #[test]
    fn normalizar_token_none_continua_none() {
        assert_eq!(normalizar_token(None), None);
    }

    #[test]
    fn normalizar_token_vazio_e_espacos_viram_none() {
        assert_eq!(normalizar_token(Some(String::new())), None);
        assert_eq!(normalizar_token(Some("   ".into())), None);
    }

    #[test]
    fn normalizar_token_apara() {
        assert_eq!(
            normalizar_token(Some(
                "  abc.def.ghi 
"
                .into()
            )),
            Some("abc.def.ghi".into())
        );
    }

    #[test]
    fn online_set_account_nao_loga_o_token() {
        let src = include_str!("online.rs");
        let ini = src.find("pub async fn online_set_account").expect("fn existe");
        let resto = &src[ini..];
        let fim = resto.find("#[tauri::command]").unwrap_or(resto.len());
        let trecho = &resto[..fim];
        assert!(!trecho.contains("tracing"));
        assert!(!trecho.contains("println"));
    }
}
