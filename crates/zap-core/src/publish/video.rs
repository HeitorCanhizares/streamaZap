//! Publicador de uma trilha de vídeo.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crossbeam_channel::Receiver;
use parking_lot::Mutex;
use zap_media::{EncodedFrame, VideoPipeline, VideoPipelineConfig};
use zap_proto::LayerSpec;
use zap_rate::allocator::Allocator;

use super::viewer::ViewerLink;
use crate::CoreError;

/// Estado compartilhado entre a thread de envio, a tarefa do alocador e a API.
pub struct VideoShared {
    pub track: u16,
    pub pipeline: VideoPipeline,
    pub allocator: Mutex<Allocator>,
    pub viewers: Mutex<Vec<Arc<ViewerLink>>>,
    pub ladder: Mutex<Vec<LayerSpec>>,
    pub stop: AtomicBool,
    pub upload_kbps: AtomicU32,
    pub clock: zap_net::Clock,
}

#[derive(Clone, Debug, Default)]
pub struct PublisherStats {
    /// Camadas codificando agora.
    pub layers: Vec<u8>,
    /// Espectadores por camada (índice = camada, 0..=2).
    pub viewers_per_layer: Vec<u32>,
    pub encode_ms: f32,
    pub encoder_name: String,
}

pub struct VideoPublisher {
    pub track: u16,
    pub(crate) shared: Arc<VideoShared>,
    pub(crate) thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl VideoPublisher {
    /// Cria o canal, `VideoPipeline::start(cfg, tx)`, o `Allocator::new(ladder)`; sobe a thread
    /// "zap-send" (`send_loop::run`, que usa `rt` para as tarefas de envio) e a tarefa
    /// `rt.spawn(alloc_loop::run)` (a cada 100 ms). Nenhuma camada codifica até haver espectador.
    pub fn start(
        cfg: VideoPipelineConfig,
        rt: tokio::runtime::Handle,
        clock: zap_net::Clock,
    ) -> Result<Arc<Self>, CoreError> {
        // Canal pequeno: se o envio atrasar, o pipeline descarta em vez de acumular latência.
        let (tx, rx) = crossbeam_channel::bounded::<EncodedFrame>(64);
        let ladder = cfg.ladder.clone();
        let track = cfg.track;
        let pipeline = VideoPipeline::start(cfg, tx)?;
        let shared = Arc::new(VideoShared {
            track,
            pipeline,
            allocator: Mutex::new(Allocator::new(ladder.clone())),
            viewers: Mutex::new(vec![]),
            ladder: Mutex::new(ladder),
            stop: AtomicBool::new(false),
            upload_kbps: AtomicU32::new(50_000),
            clock,
        });
        let rx: Receiver<EncodedFrame> = rx;
        let s = shared.clone();
        let r = rt.clone();
        let handle = std::thread::Builder::new()
            .name("zap-send".into())
            .spawn(move || super::send_loop::run(s, rx, r))?;
        rt.spawn(super::alloc_loop::run(shared.clone(), rt.clone()));
        Ok(Arc::new(Self {
            track,
            shared,
            thread: Mutex::new(Some(handle)),
        }))
    }
    pub fn add_viewer(&self, v: Arc<ViewerLink>) {
        // O alocador cria o estado dele no próximo tick.
        self.shared.viewers.lock().push(v);
    }
    pub fn remove_viewer(&self, id: u64) {
        self.shared.viewers.lock().retain(|v| v.id != id);
    }
    pub fn request_keyframe(&self, layer: u8) {
        self.shared.pipeline.request_keyframe(layer);
    }
    /// Troca a escada ao vivo: `pipeline.set_ladder`, `allocator.set_layers`; os espectadores
    /// continuam (a camada atual vira a mais próxima) e recebem keyframe da camada (o alocador
    /// pede no próximo tick).
    pub fn set_ladder(&self, ladder: Vec<LayerSpec>) {
        self.shared.pipeline.set_ladder(ladder.clone());
        self.shared.allocator.lock().set_layers(ladder.clone());
        *self.shared.ladder.lock() = ladder;
    }
    pub fn ladder(&self) -> Vec<LayerSpec> {
        self.shared.ladder.lock().clone()
    }
    pub fn stats(&self) -> PublisherStats {
        let st = self.shared.pipeline.stats();
        let mut viewers_per_layer = vec![0u32; 3];
        for v in self.shared.viewers.lock().iter() {
            let cur = usize::from(v.state.lock().current);
            if let Some(n) = viewers_per_layer.get_mut(cur) {
                *n += 1;
            }
        }
        PublisherStats {
            layers: st.layers.iter().filter(|l| l.enabled).map(|l| l.layer).collect(),
            viewers_per_layer,
            encode_ms: st.layers.first().map_or(0.0, |l| (l.encode.avg_us / 1000.0) as f32),
            encoder_name: st.encoder_name,
        }
    }
    /// Para tudo e espera as threads. Idempotente.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.thread.lock().take() {
            // Pânico da thread de envio não deve derrubar quem chama stop/drop.
            let _ = h.join();
        }
    }
}

impl Drop for VideoPublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use zap_media::{EncoderChoiceSpec, VideoSourceSpec};
    use zap_net::{
        control::ControlTx,
        endpoint,
        media_tx::MediaSender,
        tls::{self, Identity},
        Clock,
    };
    use zap_rate::ladder::{screen_ladder, Preset};

    fn cfg() -> VideoPipelineConfig {
        VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Synthetic {
                width: 1280,
                height: 720,
            },
            ladder: screen_ladder(Preset::P720p30, 1280, 720),
            encoder: EncoderChoiceSpec::Auto,
        }
    }

    fn publicador() -> Arc<VideoPublisher> {
        VideoPublisher::start(cfg(), tokio::runtime::Handle::current(), Clock::new()).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_e_stop_sem_espectador() {
        let p = publicador();
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        assert!(p.stats().layers.is_empty());
        p.stop();
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_ladder_atualiza_ladder() {
        let p = publicador();
        let nova = screen_ladder(Preset::P720p30, 1280, 720)
            .into_iter()
            .take(1)
            .collect::<Vec<_>>();
        p.set_ladder(nova.clone());
        assert_eq!(p.ladder(), nova);
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stats_viewers_per_layer_conta_current() {
        let id = Identity::generate("zap").unwrap();
        let (servidor, porta) = endpoint::bind_server(&id, 0).await.unwrap();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let cliente = endpoint::client_endpoint().unwrap();
        let conectando = cliente
            .connect_with(tls::client_config(Some(&id.fingerprint())).unwrap(), addr, "zap")
            .unwrap();
        let inc = servidor.accept().await.unwrap();
        let _srv = inc.await.unwrap();
        let cli = conectando.await.unwrap();

        let p = publicador();
        let mut links = vec![];
        for i in 0..2u64 {
            let (send, _recv) = cli.open_bi().await.unwrap();
            let l = ViewerLink::new(
                i,
                "x".into(),
                "t".into(),
                Arc::new(MediaSender::new(cli.clone(), Clock::new())),
                ControlTx::new(send),
                2,
            );
            p.add_viewer(l.clone());
            links.push(l);
        }
        // Ambos começam na camada 2; sobe um deles para a 1 (o alocador pode mexer, então
        // confere pela soma e pelo estado lido logo em seguida).
        links[0].state.lock().current = 1;
        links[1].state.lock().current = 1;
        let st = p.stats();
        assert_eq!(st.viewers_per_layer.len(), 3);
        assert_eq!(st.viewers_per_layer.iter().sum::<u32>(), 2);
        p.remove_viewer(0);
        assert_eq!(p.stats().viewers_per_layer.iter().sum::<u32>(), 1);
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn drop_para_threads() {
        let p = publicador();
        let weak = Arc::downgrade(&p.shared);
        drop(p);
        // A tarefa do alocador pode demorar um tick para soltar o Arc.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert!(weak.upgrade().is_none());
    }
}
