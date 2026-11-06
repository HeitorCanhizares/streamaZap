//! Monta o `ContextoInstalacao` (o que decide se é seguro instalar agora) a partir do app.

use std::time::Duration;

use tauri::{AppHandle, Manager};
use zap_core::Snapshot;

use crate::state::AppState;
use crate::update_state::UpdateState;
use crate::updater::ContextoInstalacao;

/// Parte pura: `em_sala = snap.room.is_some()`, `transmitindo = snap.share.is_some()`,
/// `previa_aberta = previas > 0`; o resto vem dos parâmetros.
pub fn contexto_de(snap: &Snapshot, previas: usize, minimizado: bool, ocioso: Duration) -> ContextoInstalacao {
    ContextoInstalacao {
        em_sala: snap.room.is_some(),
        transmitindo: snap.share.is_some(),
        previa_aberta: previas > 0,
        minimizado,
        ocioso,
    }
}

/// Lê o app: `state.core.snapshot()`, `upd.previas.lock().len()`, janela `main` minimizada
/// (`app.get_webview_window("main")` + `is_minimized()`; sem janela ou erro = `false`) e
/// `zap_core::idle_time()`.
pub fn contexto_atual(app: &AppHandle, state: &AppState, upd: &UpdateState) -> ContextoInstalacao {
    let snap = state.core.snapshot();
    let previas = upd.previas.lock().len();
    let minimizado = app
        .get_webview_window("main")
        .and_then(|w| w.is_minimized().ok())
        .unwrap_or(false);
    contexto_de(&snap, previas, minimizado, zap_core::idle_time())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Snapshot {
        let mut s = Snapshot::example();
        s.room = None;
        s.share = None;
        s
    }

    #[test]
    fn fora_de_sala_sem_nada() {
        let c = contexto_de(&base(), 0, false, Duration::from_secs(5));
        assert!(!c.em_sala);
        assert!(!c.transmitindo);
        assert!(!c.previa_aberta);
        assert!(!c.minimizado);
        assert_eq!(c.ocioso, Duration::from_secs(5));
    }

    #[test]
    fn em_sala_detectado() {
        let mut s = base();
        s.room = Snapshot::example().room;
        assert!(s.room.is_some());
        assert!(contexto_de(&s, 0, false, Duration::ZERO).em_sala);
    }

    #[test]
    fn transmitindo_detectado() {
        let mut s = base();
        s.share = Snapshot::example().share;
        assert!(s.share.is_some());
        assert!(contexto_de(&s, 0, false, Duration::ZERO).transmitindo);
    }

    #[test]
    fn previa_aberta_com_um_ou_mais() {
        let s = base();
        assert!(contexto_de(&s, 1, false, Duration::ZERO).previa_aberta);
        assert!(contexto_de(&s, 3, false, Duration::ZERO).previa_aberta);
        assert!(!contexto_de(&s, 0, false, Duration::ZERO).previa_aberta);
    }

    #[test]
    fn minimizado_e_ocioso_repassados() {
        let c = contexto_de(&base(), 0, true, Duration::from_secs(130));
        assert!(c.minimizado);
        assert_eq!(c.ocioso, Duration::from_secs(130));
    }
}
