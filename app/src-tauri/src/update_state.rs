//! Estado compartilhado do updater, gerenciado pelo Tauri (`app.manage`).

use std::collections::HashSet;
use std::time::Duration;

use parking_lot::Mutex;
use tauri_plugin_updater::Update;

use crate::update_agenda::RelogioSistema;
use crate::update_aplicada::Aplicada;
use crate::updater::EstadoUpdate;

/// Guarda a atualização encontrada, os bytes baixados (assinatura já verificada pelo plugin no
/// install), a fase atual e a marca "instalar ao fechar".
pub struct UpdateState {
    pub estado: Mutex<EstadoUpdate>,
    pub update: Mutex<Option<Update>>,
    pub bytes: Mutex<Option<Vec<u8>>>,
    pub instalar_ao_sair: Mutex<bool>,
    /// Não instalar sozinho antes deste instante (relógio monotônico desde a abertura).
    pub adiado_ate: Mutex<Option<Duration>>,
    /// Instante em que o aviso de 30 s foi aberto (`None` = sem aviso em curso).
    pub aviso_em: Mutex<Option<Duration>>,
    /// Tiles de prévia de sala abertos (`room_preview`), para `ContextoInstalacao.previa_aberta`.
    pub previas: Mutex<HashSet<u32>>,
    /// "Atualizado para a vX" detectado na abertura, até a interface buscar (`update_applied_take`).
    pub aplicada: Mutex<Option<Aplicada>>,
    /// Relógio monotônico desde a abertura (base de `adiado_ate` e `aviso_em`).
    pub relogio: RelogioSistema,
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
            adiado_ate: Mutex::new(None),
            aviso_em: Mutex::new(None),
            previas: Mutex::new(HashSet::new()),
            aplicada: Mutex::new(None),
            relogio: RelogioSistema::novo(),
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
