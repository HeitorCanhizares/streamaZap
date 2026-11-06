//! Atualização automática: procura na abertura (10 s) e a cada 6 h (com sorteio), baixa em
//! segundo plano e entrega a instalação ao laço de `update_instalar`.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use zap_core::InstallMode;

use crate::state::AppState;
use crate::update_agenda::{sorteio_aleatorio, Agenda, Relogio};
use crate::update_state::UpdateState;
use crate::updater::{self, Fase, TICK_INSTALACAO};

/// `true` quando a checagem automática deve rodar agora: `check` ligado e chave pública real.
pub fn deve_checar(check: bool, pubkey_ok: bool) -> bool {
    check && pubkey_ok
}

/// Depois de baixar: `true` se deve marcar `instalar_ao_sair` (modos `OnExit` e `Auto`; no `Ask`
/// a interface oferece "Instalar agora" / "Ao fechar").
pub fn marcar_ao_sair(modo: InstallMode) -> bool {
    match modo {
        InstallMode::OnExit | InstallMode::Auto => true,
        InstallMode::Ask => false,
    }
}

/// Lê `plugins.updater.pubkey` da configuração e diz se é uma chave real.
fn pubkey_real(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .is_some_and(updater::pubkey_configurada)
}

/// `true` se já há uma versão sendo baixada ou pronta: não procura de novo (o `checar` zeraria os
/// bytes já baixados) nem rebaixa.
fn download_em_curso_ou_pronto(upd: &UpdateState) -> bool {
    matches!(upd.estado.lock().fase, Fase::Pronta | Fase::Baixando)
}

/// Uma volta do laço de checagem (já com a agenda vencida).
async fn checar_e_baixar(app: &AppHandle, state: &AppState, upd: &UpdateState, agenda: &mut Agenda) {
    let cfg = state.core.settings().update;
    let agora = upd.relogio.agora();
    if !deve_checar(cfg.check, true) {
        // Reconfere quando o usuário religar, na próxima janela.
        agenda.reagendar(agora, sorteio_aleatorio());
        return;
    }
    if download_em_curso_ou_pronto(upd) {
        agenda.reagendar(agora, sorteio_aleatorio());
        return;
    }
    match crate::commands::update::checar(app, state, upd).await {
        Ok(info) => {
            if info.disponivel && cfg.download {
                match crate::commands::update::baixar(app, upd).await {
                    Ok(()) => {
                        if marcar_ao_sair(cfg.install) {
                            *upd.instalar_ao_sair.lock() = true;
                        }
                    }
                    // O estado de erro já é emitido por `baixar`.
                    Err(e) => tracing::warn!("download automático da atualização falhou: {e}"),
                }
            }
            agenda.reagendar(upd.relogio.agora(), sorteio_aleatorio());
        }
        Err(e) => {
            tracing::warn!("checagem automática de atualização falhou: {e}");
            agenda.reagendar_falha(upd.relogio.agora());
        }
    }
}

/// Sobe os dois laços em `tauri::async_runtime::spawn` (se a chave pública não for real, não sobe
/// nada):
/// 1. checagem: `Agenda::nova(relogio.agora())`; a cada 5 s olha `agenda.vencida`; vencida e
///    `deve_checar(settings.update.check, ..)`: `commands::update::checar`; achou e
///    `settings.update.download`: `commands::update::baixar` e, se `marcar_ao_sair(install)`,
///    `upd.instalar_ao_sair = true`. Sucesso (achando ou não) -> `reagendar(agora,
///    sorteio_aleatorio())`; erro -> `reagendar_falha`. Settings são relidas a cada volta;
/// 2. instalação: a cada `TICK_INSTALACAO` chama `update_instalar::tick`.
pub fn iniciar(app: &AppHandle) {
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let state: Arc<AppState> = state.inner().clone();
    if !pubkey_real(app) {
        return;
    }

    let app_check = app.clone();
    let state_check = state.clone();
    tauri::async_runtime::spawn(async move {
        let mut agenda = match app_check.try_state::<UpdateState>() {
            Some(upd) => Agenda::nova(upd.relogio.agora()),
            None => return,
        };
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let Some(upd) = app_check.try_state::<UpdateState>() else {
                continue;
            };
            if !agenda.vencida(upd.relogio.agora()) {
                continue;
            }
            checar_e_baixar(&app_check, &state_check, &upd, &mut agenda).await;
        }
    });

    let app_inst = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TICK_INSTALACAO).await;
            let Some(upd) = app_inst.try_state::<UpdateState>() else {
                continue;
            };
            crate::update_instalar::tick(&app_inst, &state, &upd).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deve_checar_combinacoes() {
        assert!(deve_checar(true, true));
        assert!(!deve_checar(true, false));
        assert!(!deve_checar(false, true));
        assert!(!deve_checar(false, false));
    }

    #[test]
    fn marcar_ao_sair_por_modo() {
        assert!(marcar_ao_sair(InstallMode::OnExit));
        assert!(marcar_ao_sair(InstallMode::Auto));
        assert!(!marcar_ao_sair(InstallMode::Ask));
    }
}
