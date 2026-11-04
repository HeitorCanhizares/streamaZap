//! Estado compartilhado do updater, gerenciado pelo Tauri (`app.manage`).

use parking_lot::Mutex;
use tauri_plugin_updater::Update;

use crate::updater::EstadoUpdate;

/// Guarda a atualização encontrada, os bytes baixados (assinatura já verificada pelo plugin no
/// install), a fase atual e a marca "instalar ao fechar".
pub struct UpdateState {
    pub estado: Mutex<EstadoUpdate>,
    pub update: Mutex<Option<Update>>,
    pub bytes: Mutex<Option<Vec<u8>>>,
    pub instalar_ao_sair: Mutex<bool>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            estado: Mutex::new(EstadoUpdate {
                fase: crate::updater::Fase::Nada,
                progresso: 0,
                versao: None,
                erro: None,
            }),
            update: Mutex::new(None),
            bytes: Mutex::new(None),
            instalar_ao_sair: Mutex::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::Fase;

    #[test]
    fn padrao_e_nada_e_nao_instala_ao_sair() {
        let u = UpdateState::default();
        assert_eq!(u.estado.lock().fase, Fase::Nada);
        assert!(!*u.instalar_ao_sair.lock());
        assert!(u.update.lock().is_none());
        assert!(u.bytes.lock().is_none());
    }
}
