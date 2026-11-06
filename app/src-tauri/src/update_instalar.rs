//! Laço de instalação sozinha (modo `auto`): a cada 30 s confere se é seguro, abre o aviso de
//! 30 s ("Atualizando para a vX em 30 s · Adiar 1 h") e instala se ninguém adiar.

use tauri::{AppHandle, Emitter};

use crate::state::AppState;
use crate::update_agenda::Relogio;
use crate::update_state::UpdateState;
use crate::updater::{proximo_passo, AvisoInstalacao, Passo, EVENT_UPDATE_AVISO};

/// O que fazer de fato com o `Passo` de `proximo_passo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    Nada,
    /// Marcar `aviso_em = agora` e emitir `zap://update-aviso` com `segundos = 30`.
    AbrirAviso,
    /// Limpar `aviso_em` e emitir `zap://update-aviso` com `segundos = 0`.
    FecharAviso,
    /// Limpar `aviso_em`, fechar o aviso e instalar.
    Instalar,
}

/// Tabela: `Avisar` -> `AbrirAviso`; `Instalar` -> `Instalar`; `Contando` -> `Nada` (o aviso segue
/// aberto); `Esperar` com aviso aberto -> `FecharAviso` (ficou inseguro ou foi adiado no meio dos
/// 30 s); `Esperar` sem aviso -> `Nada`.
pub fn acao_do_passo(passo: Passo, aviso_aberto: bool) -> Acao {
    match passo {
        Passo::Avisar => Acao::AbrirAviso,
        Passo::Instalar => Acao::Instalar,
        Passo::Contando => Acao::Nada,
        Passo::Esperar if aviso_aberto => Acao::FecharAviso,
        Passo::Esperar => Acao::Nada,
    }
}

/// Emite o aviso; falha de emissão só vai para o log.
fn emitir_aviso(app: &AppHandle, versao: String, segundos: u32) {
    if let Err(e) = app.emit(EVENT_UPDATE_AVISO, AvisoInstalacao { versao, segundos }) {
        tracing::warn!("falha ao emitir o aviso de atualização: {e}");
    }
}

/// Um tick (chamado a cada `TICK_INSTALACAO`): lê `settings.update.install`, a fase em
/// `upd.estado`, `contexto_atual`, `upd.adiado_ate`, `upd.aviso_em` e `upd.relogio.agora()`,
/// chama `proximo_passo` + `acao_do_passo` e executa a ação. Instalar =
/// `crate::commands::update::instalar_agora_interno` (que ainda recusa em sala); erro vai
/// para o log e fecha o aviso.
pub async fn tick(app: &AppHandle, state: &AppState, upd: &UpdateState) {
    let cfg = state.core.settings().update;
    let agora = upd.relogio.agora();
    // Copia os valores antes de qualquer await: nada de guard do parking_lot atravessando.
    let (fase, versao) = {
        let est = upd.estado.lock();
        (est.fase, est.versao.clone().unwrap_or_default())
    };
    let ctx = crate::update_contexto::contexto_atual(app, state, upd);
    let adiado = *upd.adiado_ate.lock();
    let aviso_em = *upd.aviso_em.lock();
    let passo = proximo_passo(cfg.install, fase, &ctx, adiado, aviso_em, agora);
    match acao_do_passo(passo, aviso_em.is_some()) {
        Acao::Nada => {}
        Acao::AbrirAviso => {
            *upd.aviso_em.lock() = Some(agora);
            emitir_aviso(app, versao, 30);
        }
        Acao::FecharAviso => {
            *upd.aviso_em.lock() = None;
            emitir_aviso(app, versao, 0);
        }
        Acao::Instalar => {
            *upd.aviso_em.lock() = None;
            emitir_aviso(app, versao, 0);
            if let Err(e) = crate::commands::update::instalar_agora_interno(app, state, upd).await {
                tracing::warn!("instalação automática falhou: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acao_do_passo_tabela() {
        assert_eq!(acao_do_passo(Passo::Avisar, false), Acao::AbrirAviso);
        assert_eq!(acao_do_passo(Passo::Avisar, true), Acao::AbrirAviso);
        assert_eq!(acao_do_passo(Passo::Instalar, true), Acao::Instalar);
        assert_eq!(acao_do_passo(Passo::Instalar, false), Acao::Instalar);
        assert_eq!(acao_do_passo(Passo::Contando, true), Acao::Nada);
        assert_eq!(acao_do_passo(Passo::Esperar, true), Acao::FecharAviso);
        assert_eq!(acao_do_passo(Passo::Esperar, false), Acao::Nada);
    }
}
