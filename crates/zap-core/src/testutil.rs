//! Utilidades de teste (também usadas por `tests/`): um `UiSink` que guarda tudo.
#![doc(hidden)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::engine::UiSink;
use crate::snapshot::Snapshot;
use crate::types::{ChatLine, TileId, VideoPacket};

#[derive(Default)]
pub struct TestSink {
    pub snapshots: Mutex<Vec<Arc<Snapshot>>>,
    pub chats: Mutex<Vec<ChatLine>>,
    /// `(instante de entrega, tile, pacote)`.
    pub videos: Mutex<Vec<(Instant, TileId, VideoPacket)>>,
}

impl TestSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn last_snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshots.lock().last().cloned()
    }
    pub fn video_count(&self, tile: TileId) -> usize {
        self.videos.lock().iter().filter(|(_, t, _)| *t == tile).count()
    }
    /// Espera (poll de 20 ms) até `cond` ser verdadeira ou `timeout`; devolve o resultado final.
    pub async fn wait_for(&self, timeout: Duration, mut cond: impl FnMut(&TestSink) -> bool) -> bool {
        let fim = Instant::now() + timeout;
        loop {
            if cond(self) {
                return true;
            }
            if Instant::now() >= fim {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl UiSink for TestSink {
    fn state(&self, snapshot: Arc<Snapshot>) {
        self.snapshots.lock().push(snapshot);
    }
    fn chat(&self, msg: ChatLine) {
        self.chats.lock().push(msg);
    }
    fn video(&self, tile: TileId, packet: VideoPacket) {
        self.videos.lock().push((Instant::now(), tile, packet));
    }
}
