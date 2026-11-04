//! Canais de vídeo por tile: o `watch` registra o `Channel`, o `TauriSink` envia os pacotes.

use std::collections::HashMap;

use parking_lot::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};
use zap_core::{TileId, VideoPacket};

/// Mapa tile -> valor, separado do `Channel` para ser testável sem janela.
pub(crate) struct Registry<T: Clone> {
    map: HashMap<TileId, T>,
}

impl<T: Clone> Default for Registry<T> {
    fn default() -> Self {
        Self { map: HashMap::new() }
    }
}

impl<T: Clone> Registry<T> {
    pub(crate) fn register(&mut self, tile: TileId, value: T) {
        self.map.insert(tile, value);
    }

    pub(crate) fn remove(&mut self, tile: TileId) {
        self.map.remove(&tile);
    }

    pub(crate) fn get(&self, tile: TileId) -> Option<T> {
        self.map.get(&tile).cloned()
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

#[derive(Default)]
pub struct VideoChannels {
    map: Mutex<Registry<Channel<InvokeResponseBody>>>,
}

impl VideoChannels {
    /// Registra (ou substitui) o canal do tile.
    pub fn register(&self, tile: TileId, channel: Channel<InvokeResponseBody>) {
        self.map.lock().register(tile, channel);
    }
    /// Remove o canal do tile (idempotente).
    pub fn remove(&self, tile: TileId) {
        self.map.lock().remove(tile);
    }
    /// Envia `packet.to_wire()` como `InvokeResponseBody::Raw`. Tile sem canal: descarta.
    /// Erro de envio (janela fechada): remove o canal e loga em `debug`.
    pub fn send(&self, tile: TileId, packet: &VideoPacket) {
        // Clona sob o lock e solta antes de enviar, para não bloquear outras threads.
        let Some(channel) = self.map.lock().get(tile) else {
            return;
        };
        if let Err(e) = channel.send(InvokeResponseBody::Raw(packet.to_wire())) {
            tracing::debug!("falha ao enviar vídeo do tile {tile}: {e}");
            self.remove(tile);
        }
    }
    pub fn len(&self) -> usize {
        self.map.lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_substitui() {
        let mut r = Registry::<u32>::default();
        r.register(1, 10);
        r.register(1, 20);
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(1), Some(20));
    }

    #[test]
    fn registry_remove_idempotente() {
        let mut r = Registry::<u32>::default();
        r.register(1, 10);
        r.remove(1);
        r.remove(1);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn registry_get_ausente_none() {
        let r = Registry::<u32>::default();
        assert_eq!(r.get(7), None);
    }
}
