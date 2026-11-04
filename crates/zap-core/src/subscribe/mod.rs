//! Assinatura: assistir a trilha de outro participante.

mod audio_rx;
mod metrics;
mod pool;
mod router;
mod video_rx;

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::task::JoinHandle;

pub use audio_rx::{AudioRegistry, AudioRx};
pub use metrics::{MetricsView, TileMetrics};
pub use pool::{ConnPool, PubConn};
pub use router::{Router, TrackHandler};
pub use video_rx::VideoRx;

use zap_proto::{TileSize, Track};

use crate::engine::Shared;
use crate::types::TileId;
use crate::CoreError;

/// Pedido de assistir uma trilha de vídeo.
#[derive(Clone, Debug)]
pub struct WatchSpec {
    pub track: Track,
    /// Número da trilha de áudio do mesmo participante, se houver.
    pub audio_track: Option<Track>,
    pub tile: TileId,
    pub size: TileSize,
    pub max_layer: Option<u8>,
}

/// Uma trilha sendo assistida num tile.
pub struct Subscription {
    pub tile: TileId,
    pub track: Track,
    pub(crate) conn: Arc<PubConn>,
    pub(crate) metrics: Arc<TileMetrics>,
    // Mantém o receptor de vídeo vivo enquanto a assinatura existir.
    #[allow(dead_code)]
    pub(crate) video: Arc<VideoRx>,
    /// Número da trilha de áudio registrada no `Router`, se houve.
    pub(crate) audio_number: Option<u16>,
    /// Tarefa que chama `VideoRx::tick` a cada 50 ms.
    pub(crate) tick_task: Mutex<Option<JoinHandle<()>>>,
}

impl Subscription {
    /// Conecta ao publicador, registra os receptores no `Router` e envia `assinar`.
    pub async fn start(shared: &Arc<Shared>, spec: WatchSpec) -> Result<Self, CoreError> {
        let address = spec.track.address.clone();
        let participant_id = spec.track.participant_id.clone();
        let conn = shared
            .pool
            .get_or_connect(shared, &participant_id, &spec.track.id, address.as_ref())
            .await?;
        let metrics = Arc::new(TileMetrics::new());
        let request_kf: Arc<dyn Fn() + Send + Sync> = Arc::new({
            let c = conn.clone();
            let id = spec.track.id.clone();
            move || c.request_keyframe(&id)
        });
        let video = VideoRx::new(
            spec.tile,
            participant_id.clone(),
            shared.playout.clone(),
            metrics.clone(),
            conn.report.clone(),
            shared.clock.clone(),
            request_kf,
        );
        conn.router.register(spec.track.number, video.clone());

        // O tick só vive enquanto a assinatura existir (abortado em `stop`).
        let tick_video = video.clone();
        let clock = shared.clock.clone();
        let handle = shared.rt.spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_millis(50));
            loop {
                iv.tick().await;
                tick_video.tick(clock.now_us());
            }
        });

        let mut audio_number = None;
        if let Some(audio) = &spec.audio_track {
            match shared.audio_rx.get_or_create(&participant_id) {
                Ok(rx) => {
                    conn.router.register(audio.number, rx);
                    audio_number = Some(audio.number);
                }
                Err(e) => {
                    handle.abort();
                    conn.router.unregister(spec.track.number);
                    return Err(e);
                }
            }
        }
        if let Err(e) = conn
            .subscribe(&spec.track.id, spec.size, spec.max_layer, spec.audio_track.is_some())
            .await
        {
            handle.abort();
            conn.router.unregister(spec.track.number);
            if let Some(n) = audio_number {
                conn.router.unregister(n);
            }
            return Err(e);
        }
        Ok(Self {
            tile: spec.tile,
            track: spec.track,
            conn,
            metrics,
            video,
            audio_number,
            tick_task: Mutex::new(Some(handle)),
        })
    }
    /// Reenvia `assinar` com novo tamanho/teto.
    pub async fn update(&self, size: TileSize, max_layer: Option<u8>) -> Result<(), CoreError> {
        self.conn
            .subscribe(&self.track.id, size, max_layer, self.audio_number.is_some())
            .await
    }
    /// `cancelar`, desregistra do `Router`, `shared.playout.remove_tile(tile)`.
    pub async fn stop(&self, shared: &Arc<Shared>) {
        if let Some(t) = self.tick_task.lock().take() {
            t.abort();
        }
        // Erro ao cancelar é ignorado: a conexão pode já ter caído.
        let _ = self.conn.unsubscribe(&self.track.id).await;
        self.conn.router.unregister(self.track.number);
        if let Some(n) = self.audio_number {
            self.conn.router.unregister(n);
        }
        shared.playout.remove_tile(self.tile);
    }
    pub fn metrics(&self) -> Arc<TileMetrics> {
        self.metrics.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_spec_clone_debug() {
        fn exige<T: Clone + std::fmt::Debug>() {}
        exige::<WatchSpec>();
    }
}
