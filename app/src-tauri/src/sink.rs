//! `UiSink` do Tauri: snapshots e chat viram eventos; vídeo vai pelos canais.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use zap_core::{ChatLine, Snapshot, TileId, UiSink, VideoPacket};

use crate::video::VideoChannels;

pub const EVENT_STATE: &str = "zap://state";
pub const EVENT_CHAT: &str = "zap://chat";

pub struct TauriSink {
    pub app: AppHandle,
    pub videos: Arc<VideoChannels>,
}

impl UiSink for TauriSink {
    /// `app.emit(EVENT_STATE, &*snapshot)`; erro só em log, para não derrubar o núcleo.
    fn state(&self, snapshot: Arc<Snapshot>) {
        if let Err(e) = self.app.emit(EVENT_STATE, &*snapshot) {
            tracing::warn!("falha ao emitir {EVENT_STATE}: {e}");
        }
    }
    /// `app.emit(EVENT_CHAT, &msg)`; erro só em log.
    fn chat(&self, msg: ChatLine) {
        if let Err(e) = self.app.emit(EVENT_CHAT, &msg) {
            tracing::warn!("falha ao emitir {EVENT_CHAT}: {e}");
        }
    }
    /// Vídeo não passa por evento: vai direto para o canal do tile.
    fn video(&self, tile: TileId, packet: VideoPacket) {
        self.videos.send(tile, &packet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constantes_dos_eventos() {
        assert_eq!(EVENT_STATE, "zap://state");
        assert_eq!(EVENT_CHAT, "zap://chat");
    }
}
