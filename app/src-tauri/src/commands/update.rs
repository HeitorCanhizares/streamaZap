//! Comandos de atualização: `update_check`, `update_download`, `update_install_now`,
//! `update_install_on_exit`. Emitem `zap://update` (`crate::updater::EstadoUpdate`).

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;
use crate::update_state::UpdateState;
use crate::updater::{self, EstadoUpdate, UpdateInfo};

/// Extrai `plugins.updater.pubkey` do JSON de configuração.
#[cfg(test)]
fn pubkey_do_conf(conf: &serde_json::Value) -> Option<&str> {
    conf.get("plugins")?.get("updater")?.get("pubkey")?.as_str()
}

/// Monta a resposta de `update_check` a partir da versão achada (se houver).
fn info_de(versao: Option<String>, notas: Option<String>) -> UpdateInfo {
    UpdateInfo {
        disponivel: versao.is_some(),
        versao,
        notas,
        tamanho: None,
    }
}

/// Mensagem de erro (pt-BR) quando a busca falha.
fn msg_erro_check(e: &str) -> String {
    format!("Não foi possível procurar atualização: {e}")
}

/// Lê `plugins.updater.pubkey` da configuração do app e diz se é uma chave real.
fn pubkey_ok(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .is_some_and(updater::pubkey_configurada)
}

/// Lógica de `update_check`, reutilizada pela checagem automática.
async fn checar(app: &AppHandle, state: &AppState, upd: &UpdateState) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;

    if !pubkey_ok(app) {
        return Err("atualização não configurada nesta build".into());
    }
    let canal = state.core.settings().update.channel;
    let url = updater::endpoint_do_canal(canal)
        .parse::<tauri::Url>()
        .map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(u)) => {
            let info = info_de(Some(u.version.clone()), u.body.clone());
            *upd.update.lock() = Some(u);
            *upd.bytes.lock() = None;
            Ok(info)
        }
        Ok(None) => {
            *upd.update.lock() = None;
            Ok(info_de(None, None))
        }
        Err(e) => Err(msg_erro_check(&e.to_string())),
    }
}

/// Procura atualização no endpoint do canal de `settings.update.channel`. Pubkey placeholder ->
/// `Err("atualização não configurada nesta build")`.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    upd: State<'_, UpdateState>,
) -> Result<UpdateInfo, String> {
    checar(&app, &state, &upd).await
}

/// Grava a fase em `upd.estado` e emite `zap://update`; falha de emit só vai para o log.
fn publicar(app: &AppHandle, upd: &UpdateState, e: EstadoUpdate) {
    *upd.estado.lock() = e.clone();
    if let Err(err) = app.emit(updater::EVENT_UPDATE, e) {
        tracing::warn!("falha ao emitir zap://update: {err}");
    }
}

/// Só emite quando o percentual mudou desde o último emitido (evita inundar o frontend).
fn deve_emitir(ultimo: Option<u8>, novo: u8) -> bool {
    ultimo != Some(novo)
}

/// Baixa a atualização achada por `update_check`, emitindo `zap://update` com progresso.
#[tauri::command]
pub async fn update_download(app: AppHandle, upd: State<'_, UpdateState>) -> Result<(), String> {
    baixar(&app, &upd).await
}

/// Lógica de `update_download`, reutilizada pela atualização automática.
async fn baixar(app: &AppHandle, upd: &UpdateState) -> Result<(), String> {
    let update = upd
        .update
        .lock()
        .clone()
        .ok_or_else(|| "Nenhuma atualização para baixar.".to_string())?;
    let versao = update.version.clone();
    publicar(app, upd, EstadoUpdate::baixando(0, &versao));

    let mut acumulado: u64 = 0;
    let mut ultimo: Option<u8> = Some(0);
    let resultado = update
        .download(
            |chunk, total| {
                acumulado += chunk as u64;
                let p = updater::percentual(acumulado, total);
                if deve_emitir(ultimo, p) {
                    ultimo = Some(p);
                    publicar(app, upd, EstadoUpdate::baixando(p, &versao));
                }
            },
            || {},
        )
        .await;
    match resultado {
        Ok(bytes) => {
            *upd.bytes.lock() = Some(bytes);
            publicar(app, upd, EstadoUpdate::pronta(&versao));
            Ok(())
        }
        Err(e) => {
            let msg = format!("Falha ao baixar a atualização: {e}");
            publicar(app, upd, EstadoUpdate::erro(&msg));
            Err(msg)
        }
    }
}

/// Instala e reinicia agora; recusa com erro se o núcleo está numa sala.
#[tauri::command]
pub async fn update_install_now(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    upd: State<'_, UpdateState>,
) -> Result<(), String> {
    let em_sala = state.core.snapshot().room.is_some();
    updater::checar_instalar_agora(em_sala)?;
    let update = upd.update.lock().clone();
    let bytes = upd.bytes.lock().clone();
    let (Some(update), Some(bytes)) = (update, bytes) else {
        return Err("A atualização ainda não foi baixada.".into());
    };
    publicar(&app, &upd, EstadoUpdate::instalando(&update.version));
    if let Err(e) = update.install(bytes) {
        let msg = format!("Falha ao instalar a atualização: {e}");
        publicar(&app, &upd, EstadoUpdate::erro(&msg));
        return Err(msg);
    }
    app.restart()
}

/// Marca para instalar quando o app fechar (`RunEvent::Exit`).
#[tauri::command]
pub async fn update_install_on_exit(upd: State<'_, UpdateState>) -> Result<(), String> {
    if upd.estado.lock().fase != updater::Fase::Pronta {
        return Err("A atualização ainda não está pronta.".into());
    }
    *upd.instalar_ao_sair.lock() = true;
    Ok(())
}

/// Ao abrir: se `settings.update.auto`, espera 10 s, checa, baixa e marca instalar ao fechar.
pub fn agendar_auto(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let state: Arc<AppState> = state.inner().clone();
    // Sem pubkey real nem agenda: a checagem sempre falharia.
    if !auto_habilitado(state.core.settings().update.auto, pubkey_ok(app)) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        // O usuário pode ter desligado a opção durante a espera.
        if !state.core.settings().update.auto {
            return;
        }
        let Some(upd) = app.try_state::<UpdateState>() else {
            return;
        };
        match checar(&app, &state, &upd).await {
            Ok(info) if info.disponivel => {}
            Ok(_) => return,
            Err(e) => {
                tracing::warn!("checagem automática de atualização falhou: {e}");
                return;
            }
        }
        // Falha de download já emite o estado de erro dentro de `baixar`.
        match baixar(&app, &upd).await {
            Ok(()) => *upd.instalar_ao_sair.lock() = true,
            Err(e) => tracing::warn!("download automático da atualização falhou: {e}"),
        }
    });
}

/// Atualização automática só roda com a opção ligada e uma chave pública real.
fn auto_habilitado(auto: bool, pubkey_ok: bool) -> bool {
    auto && pubkey_ok
}

/// No `RunEvent::Exit`: se `deve_instalar_ao_sair`, instala a atualização baixada (sem reiniciar).
pub fn instalar_ao_sair(app: &AppHandle) {
    let Some(upd) = app.try_state::<UpdateState>() else {
        return;
    };
    let fase = upd.estado.lock().fase;
    let marcada = *upd.instalar_ao_sair.lock();
    if !updater::deve_instalar_ao_sair(fase, marcada) {
        return;
    }
    let update = upd.update.lock().clone();
    let bytes = upd.bytes.lock().clone();
    if let (Some(update), Some(bytes)) = (update, bytes) {
        if let Err(e) = update.install(bytes) {
            tracing::warn!("falha ao instalar a atualização ao sair: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_sem_versao_nao_disponivel() {
        let i = info_de(None, None);
        assert!(!i.disponivel);
        assert!(i.versao.is_none());
    }

    #[test]
    fn info_com_versao_disponivel() {
        let i = info_de(Some("1.2.3".into()), Some("notas".into()));
        assert!(i.disponivel);
        assert_eq!(i.versao.as_deref(), Some("1.2.3"));
        assert_eq!(i.notas.as_deref(), Some("notas"));
    }

    #[test]
    fn deve_emitir_so_quando_muda() {
        assert!(!deve_emitir(Some(5), 5));
        assert!(deve_emitir(Some(5), 6));
        assert!(deve_emitir(None, 0));
    }

    #[test]
    fn auto_habilitado_combinacoes() {
        assert!(auto_habilitado(true, true));
        assert!(!auto_habilitado(true, false));
        assert!(!auto_habilitado(false, true));
        assert!(!auto_habilitado(false, false));
    }

    #[test]
    fn msg_erro_contem_original() {
        assert!(msg_erro_check("falhou xyz").contains("falhou xyz"));
    }

    #[test]
    fn pubkey_real_do_conf() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let pk = pubkey_do_conf(&conf).expect("conf tem plugins.updater.pubkey");
        assert_ne!(pk, updater::PUBKEY_PLACEHOLDER);
        assert!(updater::pubkey_configurada(pk));
    }
}
