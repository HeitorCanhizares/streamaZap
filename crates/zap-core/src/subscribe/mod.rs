//! Assinatura: assistir a trilha de outro participante.

mod audio_rx;
mod metrics;
mod pool;
mod router;
mod video_rx;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::task::JoinHandle;

pub use audio_rx::{AudioRegistry, AudioRx};
pub use metrics::{MetricsView, TileMetrics};
pub use pool::{ConnPool, PubConn};
pub use router::{Router, TrackHandler};
pub use video_rx::VideoRx;

use zap_proto::{TileSize, Track, TrackKind};

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

/// Uma trilha sendo assistida num tile: vídeo (com o áudio do mesmo participante, se houver) ou
/// só o áudio, quando a pessoa compartilha só o som.
pub struct Subscription {
    pub tile: TileId,
    pub track: Track,
    pub(crate) conn: Arc<PubConn>,
    pub(crate) metrics: Arc<TileMetrics>,
    /// Receptor de vídeo (vivo enquanto a assinatura existir); `None` na assinatura só de áudio.
    pub(crate) video: Option<Arc<VideoRx>>,
    /// Número da trilha de áudio pega no `Router` (`acquire`), se houve.
    pub(crate) audio_number: Option<u16>,
    /// Tarefa que chama `VideoRx::tick` a cada 50 ms.
    pub(crate) tick_task: Mutex<Option<JoinHandle<()>>>,
    /// Pedido atual (tamanho/teto); `update` grava aqui.
    pub(crate) spec: Mutex<WatchSpec>,
    /// Já parou: o áudio pego no `Router` é solto uma vez só.
    stopped: AtomicBool,
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
        Self::start_with(shared, spec, conn).await
    }

    /// Registra os receptores na `conn` já pronta e envia `assinar`.
    async fn start_with(shared: &Arc<Shared>, spec: WatchSpec, conn: Arc<PubConn>) -> Result<Self, CoreError> {
        if spec.track.kind == TrackKind::Audio {
            return Self::start_audio_only(shared, spec, conn).await;
        }
        let spec_atual = spec.clone();
        let participant_id = spec.track.participant_id.clone();
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
        let video_dyn: Arc<dyn TrackHandler> = video.clone();

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
            match shared.audio_rx.hold(&participant_id) {
                Ok(rx) => {
                    conn.router.acquire(audio.number, rx);
                    audio_number = Some(audio.number);
                }
                Err(e) => {
                    handle.abort();
                    conn.router.unregister_if(spec.track.number, &video_dyn);
                    return Err(e);
                }
            }
        }
        if let Err(e) = conn
            .subscribe(&spec.track.id, spec.size, spec.max_layer, spec.audio_track.is_some())
            .await
        {
            handle.abort();
            conn.router.unregister_if(spec.track.number, &video_dyn);
            if let Some(n) = audio_number {
                conn.router.release(n);
                shared.audio_rx.unhold(&participant_id);
            }
            return Err(e);
        }
        Ok(Self {
            tile: spec.tile,
            track: spec.track,
            conn,
            metrics,
            video: Some(video),
            audio_number,
            tick_task: Mutex::new(Some(handle)),
            stopped: AtomicBool::new(false),
            spec: Mutex::new(spec_atual),
        })
    }

    /// Só o áudio: quem compartilha só o som não tem trilha de vídeo para levar o áudio junto.
    /// Pede `assinar` da própria trilha de áudio (o publicador liga o áudio dessa conexão).
    async fn start_audio_only(shared: &Arc<Shared>, spec: WatchSpec, conn: Arc<PubConn>) -> Result<Self, CoreError> {
        let rx = shared.audio_rx.hold(&spec.track.participant_id)?;
        let numero = spec.track.number;
        conn.router.acquire(numero, rx);
        if let Err(e) = conn.subscribe(&spec.track.id, spec.size, None, true).await {
            conn.router.release(numero);
            shared.audio_rx.unhold(&spec.track.participant_id);
            return Err(e);
        }
        Ok(Self {
            tile: spec.tile,
            track: spec.track.clone(),
            conn,
            metrics: Arc::new(TileMetrics::new()),
            video: None,
            audio_number: Some(numero),
            tick_task: Mutex::new(None),
            stopped: AtomicBool::new(false),
            spec: Mutex::new(spec),
        })
    }
    /// Como `start`, mas na conexão dada (rota direta sem ponte), sem passar pelo `ConnPool`.
    #[allow(dead_code)]
    pub(crate) async fn start_on(shared: &Arc<Shared>, spec: WatchSpec, conn: Arc<PubConn>) -> Result<Self, CoreError> {
        Self::start_with(shared, spec, conn).await
    }
    /// Pedido atual (o `update` grava tamanho/teto).
    #[allow(dead_code)]
    pub(crate) fn spec(&self) -> WatchSpec {
        self.spec.lock().clone()
    }
    /// Só áudio (sem vídeo)?
    pub fn audio_only(&self) -> bool {
        self.video.is_none()
    }
    /// Para sem mandar `cancelar` (a conexão já caiu): aborta o tique, tira do `Router` e do playout.
    /// Idempotente: o áudio só é solto uma vez.
    #[allow(dead_code)]
    pub(crate) fn stop_local(&self, shared: &Arc<Shared>) {
        if let Some(t) = self.tick_task.lock().take() {
            t.abort();
        }
        if let Some(v) = &self.video {
            let h: Arc<dyn TrackHandler> = v.clone();
            self.conn.router.unregister_if(self.track.number, &h);
        }
        if !self.stopped.swap(true, Ordering::Relaxed) {
            if let Some(n) = self.audio_number {
                self.conn.router.release(n);
                shared.audio_rx.unhold(&self.track.participant_id);
            }
        }
        shared.playout.remove_tile(self.tile);
    }
    /// Reenvia `assinar` com novo tamanho/teto.
    pub async fn update(&self, size: TileSize, max_layer: Option<u8>) -> Result<(), CoreError> {
        // Grava antes de enviar: o pedido vale mesmo se o envio falhar (reassinatura usa o spec).
        {
            let mut sp = self.spec.lock();
            sp.size = size;
            sp.max_layer = max_layer;
        }
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
        self.stop_local(shared);
    }
    pub fn metrics(&self) -> Arc<TileMetrics> {
        self.metrics.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use zap_proto::{Access, ControlMsg};

    #[test]
    fn watch_spec_clone_debug() {
        fn exige<T: Clone + std::fmt::Debug>() {}
        exige::<WatchSpec>();
    }

    /// Core + servidor de teste + `PubConn` da ponte (loopback), sem falar com produção.
    async fn ambiente() -> (Core, zap_test_server::Handle, Arc<PubConn>, std::path::PathBuf) {
        let srv = zap_test_server::spawn(zap_test_server::Config::default())
            .await
            .expect("srv");
        let fp = srv.cert_fingerprint.clone();
        let dir = std::env::temp_dir().join(format!("zapcore-sub-{}", uuid::Uuid::new_v4()));
        let d2 = dir.clone();
        let core = tokio::task::spawn_blocking(move || {
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                dev_cert_fp: Some(fp),
                ..Default::default()
            };
            Core::start_with(d2, crate::testutil::TestSink::new(), opts).expect("core")
        })
        .await
        .expect("join");
        let a = zap_net::online::ServerClient::login(
            &srv.base_url,
            zap_net::online::LoginRequest {
                name: "Teste".into(),
                color: "#3BF08A".into(),
                install_id: uuid::Uuid::new_v4().to_string(),
                protocol: zap_proto::PROTOCOL_VERSION,
                app_version: "teste".into(),
                server_password: None,
            },
        )
        .await
        .expect("login");
        let sala = a
            .create_room(zap_net::online::CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            })
            .await
            .expect("sala");
        let join = a.join(&sala.code, None).await.expect("join");
        let (wtx, _wrx) = a.connect_ws().await.expect("ws");
        wtx.send(ControlMsg::RoomJoin {
            room_id: join.room.id.clone(),
        })
        .await
        .expect("room join");
        let ponte = join.media.bridge.as_ref().expect("ponte").address.clone();
        let link = crate::online::bridge::connect_bridge(&core.shared, &ponte, &join.ticket, &join.room.id, false)
            .await
            .expect("bridge");
        let conn = PubConn::connect_online(&core.shared, link, wtx, None);
        // Mantém o WebSocket vivo durante o teste.
        std::mem::forget(_wrx);
        (core, srv, conn, dir)
    }

    fn spec_teste(numero: u16) -> WatchSpec {
        WatchSpec {
            track: Track {
                id: "t-sub".into(),
                participant_id: "dono".into(),
                number: numero,
                kind: TrackKind::Screen,
                title: "t".into(),
                layers: vec![],
                audio: None,
                address: None,
            },
            audio_track: None,
            tile: 7,
            size: TileSize::Thumbnail,
            max_layer: None,
        }
    }

    async fn encerrar(core: Core, dir: std::path::PathBuf) {
        let _ = tokio::task::spawn_blocking(move || drop(core)).await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn update_grava_spec() {
        let (core, _srv, conn, dir) = ambiente().await;
        let sub = Subscription::start_on(&core.shared, spec_teste(3), conn)
            .await
            .expect("start_on");
        assert_eq!(sub.spec().size, TileSize::Thumbnail);
        let _ = sub.update(TileSize::Full, Some(2)).await;
        let sp = sub.spec();
        assert_eq!(sp.size, TileSize::Full);
        assert_eq!(sp.max_layer, Some(2));
        sub.stop_local(&core.shared);
        encerrar(core, dir).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn start_on_registra_no_router_da_conn() {
        let (core, _srv, conn, dir) = ambiente().await;
        let sub = Subscription::start_on(&core.shared, spec_teste(4), conn)
            .await
            .expect("start_on");
        // Router + tick + a própria assinatura seguram o VideoRx.
        let video = sub.video.clone().expect("vídeo");
        // Router + tick + a própria assinatura + este clone.
        assert!(Arc::strong_count(&video) >= 4);
        sub.stop_local(&core.shared);
        for _ in 0..50 {
            if Arc::strong_count(&video) == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(Arc::strong_count(&video), 2, "router e tick soltaram o VideoRx");
        assert!(sub.tick_task.lock().is_none());
        encerrar(core, dir).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_local_nao_manda_cancelar() {
        let (core, _srv, conn, dir) = ambiente().await;
        let sub = Subscription::start_on(&core.shared, spec_teste(5), conn.clone())
            .await
            .expect("start_on");
        // Fecha a conexão: um `cancelar` falharia, mas `stop_local` é síncrono e não envia nada.
        conn.close();
        sub.stop_local(&core.shared);
        assert!(sub.tick_task.lock().is_none());
        // Idempotente.
        sub.stop_local(&core.shared);
        encerrar(core, dir).await;
    }
}
