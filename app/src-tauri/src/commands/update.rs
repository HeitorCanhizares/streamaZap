//! Comandos de atualização: `update_check`, `update_download`, `update_install_now`,
//! `update_install_on_exit`. Emitem `zap://update` (`crate::updater::EstadoUpdate`).

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;
use crate::update_agenda::Relogio;
use crate::update_aplicada::Pendente;
use crate::update_state::UpdateState;
use crate::updater::{self, AvisoInstalacao, EstadoUpdate, UpdateInfo};

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
pub(crate) async fn checar(app: &AppHandle, state: &AppState, upd: &UpdateState) -> Result<UpdateInfo, String> {
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
pub(crate) fn publicar(app: &AppHandle, upd: &UpdateState, e: EstadoUpdate) {
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
pub(crate) async fn baixar(app: &AppHandle, upd: &UpdateState) -> Result<(), String> {
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
    instalar_agora_interno(&app, &state, &upd).await
}

/// Monta o registro gravado antes de instalar (lido na abertura seguinte).
fn pendente_de(versao_atual: &str, versao_nova: &str, notas: Option<String>) -> Pendente {
    Pendente {
        de: versao_atual.to_string(),
        para: versao_nova.to_string(),
        notas,
    }
}

/// Grava o pendente em `app_data_dir`; falha só vai para o log (não impede instalar).
fn gravar_pendente_do_app(app: &AppHandle, update: &tauri_plugin_updater::Update) {
    let Ok(dir) = app.path().app_data_dir() else {
        tracing::warn!("sem diretório de dados: o aviso de 'Atualizado' não será gravado");
        return;
    };
    let p = pendente_de(
        &app.package_info().version.to_string(),
        &update.version,
        update.body.clone(),
    );
    if let Err(e) = crate::update_aplicada::gravar_pendente(&dir, &p) {
        tracing::warn!("falha ao gravar a atualização pendente: {e}");
    }
}

/// Instala e reinicia: recusa em sala (`updater::checar_instalar_agora`), exige update baixado,
/// grava o pendente, publica `EstadoUpdate::instalando`, instala e `app.restart()`.
/// Compartilhada por `update_install_now` e pela instalação sozinha.
pub(crate) async fn instalar_agora_interno(app: &AppHandle, state: &AppState, upd: &UpdateState) -> Result<(), String> {
    let em_sala = state.core.snapshot().room.is_some();
    updater::checar_instalar_agora(em_sala)?;
    let update = upd.update.lock().clone();
    let bytes = upd.bytes.lock().clone();
    let (Some(update), Some(bytes)) = (update, bytes) else {
        return Err("A atualização ainda não foi baixada.".into());
    };
    gravar_pendente_do_app(app, &update);
    publicar(app, upd, EstadoUpdate::instalando(&update.version));
    if let Err(e) = update.install(bytes) {
        let msg = format!("Falha ao instalar a atualização: {e}");
        publicar(app, upd, EstadoUpdate::erro(&msg));
        return Err(msg);
    }
    app.restart()
}

/// "Adiar 1 h": `adiado_ate = relogio.agora() + ADIAR`, limpa `aviso_em` e emite
/// `zap://update-aviso` com `segundos = 0` (fecha o aviso).
#[tauri::command]
pub async fn update_postpone(app: AppHandle, upd: State<'_, UpdateState>) -> Result<(), String> {
    let agora = upd.relogio.agora();
    *upd.adiado_ate.lock() = Some(agora + updater::ADIAR);
    *upd.aviso_em.lock() = None;
    let versao = upd.estado.lock().versao.clone().unwrap_or_default();
    if let Err(e) = app.emit(updater::EVENT_UPDATE_AVISO, AvisoInstalacao { versao, segundos: 0 }) {
        tracing::warn!("falha ao emitir o fechamento do aviso: {e}");
    }
    Ok(())
}

/// Entrega (uma vez) o "Atualizado para a vX" detectado na abertura: `upd.aplicada.lock().take()`.
#[tauri::command]
pub async fn update_applied_take(
    upd: State<'_, UpdateState>,
) -> Result<Option<crate::update_aplicada::Aplicada>, String> {
    Ok(upd.aplicada.lock().take())
}

/// Na abertura: `update_aplicada::detectar(app_data_dir, versão atual do app)`; se achou, guarda em
/// `upd.aplicada` e emite `zap://update-aplicada`. Chamada por `setup::init`.
pub fn anunciar_aplicada(app: &AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let versao = app.package_info().version.to_string();
    let Some(a) = crate::update_aplicada::detectar(&dir, &versao) else {
        return;
    };
    if let Some(upd) = app.try_state::<UpdateState>() {
        *upd.aplicada.lock() = Some(a.clone());
    }
    if let Err(e) = app.emit(updater::EVENT_UPDATE_APLICADA, a) {
        tracing::warn!("falha ao emitir zap://update-aplicada: {e}");
    }
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

/// Ao abrir: liga a checagem/instalação automática (ver `update_auto`).
pub fn agendar_auto(app: &AppHandle) {
    crate::update_auto::iniciar(app);
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
        // Grava antes para a próxima abertura anunciar "Atualizado".
        gravar_pendente_do_app(app, &update);
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
    fn msg_instalar_em_sala() {
        assert!(updater::checar_instalar_agora(true).is_err());
    }

    #[test]
    fn pendente_de_monta_campos() {
        assert_eq!(
            pendente_de("3.2.1", "3.4.0", Some("n".into())),
            Pendente {
                de: "3.2.1".into(),
                para: "3.4.0".into(),
                notas: Some("n".into())
            }
        );
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
