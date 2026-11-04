//! `impl Core`: assistir trilhas.

use crate::engine::Core;
use crate::types::*;
use crate::CoreError;

impl Core {
    /// Acha a trilha no modelo da sala (`TrackNotFound`); trilha própria → `Invalid`;
    /// `tile` já em uso → substitui (para a anterior); acha a trilha de áudio do mesmo dono;
    /// `Subscription::start` numa tarefa do runtime (função síncrona: dispara e devolve `Ok`;
    /// falha de conexão vira `push_error` + remove o tile). Se já assistindo a mesma trilha no
    /// mesmo tile, só `Subscription::update` (tamanho/teto).
    pub fn watch(&self, track_id: &str, tile: TileId, size: TileSize, max_layer: Option<u8>) -> Result<(), CoreError> {
        todo!("tarefa core-42")
    }
    /// Para a assinatura do tile, remove de `UiState.tiles`, `touch()`.
    pub fn unwatch(&self, tile: TileId) {
        todo!("tarefa core-42")
    }
}
