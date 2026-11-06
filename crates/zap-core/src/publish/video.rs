//! Publicador de uma trilha de vídeo.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use zap_media::{EncodedFrame, EncoderChoiceSpec, PipelineStats, VideoPipeline, VideoPipelineConfig, VideoSourceSpec};
use zap_proto::{LayerSpec, TileSize};
use zap_rate::allocator::{Allocator, LayerRequest};

use super::viewer::ViewerLink;
use crate::playout::PlayoutHub;
use crate::types::{TileId, VideoPacket};
use crate::CoreError;

/// Pipeline trocável: a câmera reinicia a captura em outro modo sem trocar o publicador
/// (`VideoPublisher::restart_capture`). Fica vazio só durante a troca (ou se o novo não abriu):
/// aí os pedidos não fazem nada e as estatísticas saem zeradas.
pub struct PipelineSlot {
    atual: Mutex<Option<VideoPipeline>>,
}

impl PipelineSlot {
    pub fn new(pipeline: VideoPipeline) -> Self {
        Self {
            atual: Mutex::new(Some(pipeline)),
        }
    }
    pub fn set_requests(&self, reqs: &[LayerRequest]) {
        if let Some(p) = self.atual.lock().as_ref() {
            p.set_requests(reqs);
        }
    }
    pub fn request_keyframe(&self, layer: u8) {
        if let Some(p) = self.atual.lock().as_ref() {
            p.request_keyframe(layer);
        }
    }
    pub fn set_ladder(&self, ladder: Vec<LayerSpec>) {
        if let Some(p) = self.atual.lock().as_ref() {
            p.set_ladder(ladder);
        }
    }
    pub fn stats(&self) -> PipelineStats {
        self.atual.lock().as_ref().map(VideoPipeline::stats).unwrap_or_default()
    }
    /// Tira o pipeline do slot. O drop (que espera a thread de vídeo) fica com quem chamou, fora
    /// do lock.
    fn take(&self) -> Option<VideoPipeline> {
        self.atual.lock().take()
    }
    fn put(&self, pipeline: VideoPipeline) {
        *self.atual.lock() = Some(pipeline);
    }
}

impl From<VideoPipeline> for PipelineSlot {
    fn from(pipeline: VideoPipeline) -> Self {
        Self::new(pipeline)
    }
}

/// Estado compartilhado entre a thread de envio, a tarefa do alocador e a API.
pub struct VideoShared {
    pub track: u16,
    pub pipeline: PipelineSlot,
    pub allocator: Mutex<Allocator>,
    pub viewers: Mutex<Vec<Arc<ViewerLink>>>,
    pub ladder: Mutex<Vec<LayerSpec>>,
    pub stop: AtomicBool,
    pub upload_kbps: AtomicU32,
    pub clock: zap_net::Clock,
    /// Camadas pedidas pelo servidor Online (`camadas.pedidas`); somadas (maior kbps por
    /// camada) ao pedido do alocador a cada tick.
    pub bridge_requests: Mutex<Vec<zap_rate::allocator::LayerRequest>>,
    /// Prévia local (eu vendo o que transmito): uma camada vai direto para o tile da interface,
    /// sem passar pela rede (a mais leve na miniatura, a do meio na tela principal).
    pub local_preview: Mutex<Option<LocalPreview>>,
}

/// Destino da prévia local de uma trilha.
pub struct LocalPreview {
    pub tile: TileId,
    pub out: Arc<PlayoutHub>,
    /// Tamanho pedido pela interface: escolhe a camada (`layer_for`).
    pub size: TileSize,
    /// Ainda sem keyframe entregue: o decodificador da interface só começa num keyframe.
    pub waiting_key: bool,
}

/// Camada da prévia local para o tamanho pedido: a mais leve (maior número) na miniatura; na
/// tela principal, a segunda melhor (camada 1), para não ligar a camada 0 só para eu me ver. Escada
/// com uma camada só: ela.
pub fn layer_for(ladder: &[LayerSpec], size: TileSize) -> Option<u8> {
    let mut camadas: Vec<u8> = ladder.iter().map(|l| l.layer).collect();
    camadas.sort_unstable();
    match size {
        TileSize::Thumbnail => camadas.last().copied(),
        TileSize::Main | TileSize::Full => camadas.get(1).or(camadas.last()).copied(),
    }
}

impl VideoShared {
    /// Camada da prévia local (`None` sem prévia). Trava a prévia e a escada uma depois da outra.
    pub fn preview_layer(&self) -> Option<u8> {
        let size = self.local_preview.lock().as_ref().map(|p| p.size)?;
        layer_for(&self.ladder.lock(), size)
    }

    /// Pedidos que não vêm do alocador: os da ponte Online e o da prévia local, unidos.
    pub fn extra_requests(&self) -> Vec<LayerRequest> {
        let ponte = self.bridge_requests.lock().clone();
        let previa: Vec<_> = self
            .preview_layer()
            .and_then(|camada| {
                let ladder = self.ladder.lock();
                ladder.iter().find(|l| l.layer == camada).map(|l| LayerRequest {
                    layer: l.layer,
                    kbps: l.kbps,
                })
            })
            .into_iter()
            .collect();
        super::alloc_loop::merge_requests(&ponte, &previa)
    }

    /// Entrega o quadro à prévia local, se ela pede esta camada. Começa num keyframe.
    pub fn deliver_preview(&self, f: &EncodedFrame) {
        let mut previa = self.local_preview.lock();
        let Some(p) = previa.as_mut() else { return };
        // Ordem das travas: prévia e depois escada (`preview_layer` não segura as duas juntas).
        if layer_for(&self.ladder.lock(), p.size) != Some(f.layer) {
            return;
        }
        if p.waiting_key && !f.keyframe {
            return;
        }
        p.waiting_key = false;
        p.out.push_now(
            p.tile,
            VideoPacket {
                track: f.track,
                layer: f.layer,
                keyframe: f.keyframe,
                width: f.width,
                height: f.height,
                seq: f.seq,
                capture_us: f.capture_us,
                data: f.data.clone(),
            },
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct PublisherStats {
    /// Camadas codificando agora.
    pub layers: Vec<u8>,
    /// Espectadores por camada (índice = camada, 0..=2).
    pub viewers_per_layer: Vec<u32>,
    pub encode_ms: f32,
    pub encoder_name: String,
    /// Conversor de cada camada ligada, em ordem de camada (camadas ainda sem pipe ficam de fora).
    pub converters: Vec<(u8, zap_media::ConverterKind)>,
}

pub struct VideoPublisher {
    pub track: u16,
    pub(crate) shared: Arc<VideoShared>,
    pub(crate) thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Ponta de envio do canal da thread "zap-send": um pipeline novo (`restart_capture`) entra no
    /// mesmo canal, e a thread não cai quando o antigo sai.
    tx: Sender<EncodedFrame>,
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
        let pipeline = VideoPipeline::start(cfg, tx.clone())?.into();
        let shared = Arc::new(VideoShared {
            track,
            pipeline,
            allocator: Mutex::new(Allocator::new(ladder.clone())),
            viewers: Mutex::new(vec![]),
            ladder: Mutex::new(ladder),
            stop: AtomicBool::new(false),
            upload_kbps: AtomicU32::new(50_000),
            clock,
            bridge_requests: Mutex::new(Vec::new()),
            local_preview: Mutex::new(None),
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
            tx,
        }))
    }
    /// Reinicia a captura com outra fonte e escada (a câmera mudou de modo) sem trocar o
    /// publicador: espectadores, ponte Online, prévia local, registro no `MediaServer` e a thread
    /// de envio continuam. O pipeline antigo cai antes de o novo abrir (a webcam não abre duas
    /// vezes). As camadas religam no próximo tick do alocador e começam por keyframe. Erro: o
    /// publicador fica sem pipeline (nada codifica) e quem chamou deve parar a trilha.
    pub fn restart_capture(
        &self,
        source: VideoSourceSpec,
        ladder: Vec<LayerSpec>,
        encoder: EncoderChoiceSpec,
    ) -> Result<(), CoreError> {
        // O drop espera a thread de vídeo e fecha a captura; fora do lock do slot.
        let antigo = self.shared.pipeline.take();
        drop(antigo);
        let novo = VideoPipeline::start(
            VideoPipelineConfig {
                track: self.track,
                source,
                ladder: ladder.clone(),
                encoder,
            },
            self.tx.clone(),
        )?;
        self.shared.allocator.lock().set_layers(ladder.clone());
        *self.shared.ladder.lock() = ladder;
        // O decodificador da prévia recomeça no keyframe do pipeline novo.
        if let Some(p) = self.shared.local_preview.lock().as_mut() {
            p.waiting_key = true;
        }
        self.shared.pipeline.put(novo);
        Ok(())
    }
    pub fn add_viewer(&self, v: Arc<ViewerLink>) {
        // O alocador cria o estado dele no próximo tick.
        self.shared.viewers.lock().push(v);
    }
    pub fn remove_viewer(&self, id: u64) {
        self.shared.viewers.lock().retain(|v| v.id != id);
    }
    /// Online: define as camadas/bitrates pedidos pelo servidor (substitui o pedido anterior;
    /// vazio = nenhuma) e aplica no pipeline já (sem esperar o tick de 100 ms).
    pub fn set_bridge_requests(&self, reqs: Vec<zap_rate::allocator::LayerRequest>) {
        *self.shared.bridge_requests.lock() = reqs;
        // Com espectadores diretos o tick de 100 ms faz a união com o pedido do alocador.
        let diretos = self.shared.viewers.lock().iter().any(|v| !v.state.lock().bridge);
        if !diretos {
            self.shared.pipeline.set_requests(&self.shared.extra_requests());
        }
    }
    /// Liga a prévia local no `tile` (substitui a anterior): a camada de `layer_for(size)` passa
    /// a ser codificada mesmo sem espectador e chega à interface pela `PlayoutHub`, sem rede.
    pub fn set_local_preview(&self, tile: TileId, out: Arc<PlayoutHub>, size: TileSize) {
        *self.shared.local_preview.lock() = Some(LocalPreview {
            tile,
            out,
            size,
            waiting_key: true,
        });
        // Liga a camada já (sem esperar o tick) e pede o keyframe para a interface começar.
        let diretos = self.shared.viewers.lock().iter().any(|v| !v.state.lock().bridge);
        if !diretos {
            self.shared.pipeline.set_requests(&self.shared.extra_requests());
        }
        if let Some(camada) = self.shared.preview_layer() {
            self.shared.pipeline.request_keyframe(camada);
        }
    }
    /// Desliga a prévia local, se ela está no `tile`.
    pub fn clear_local_preview(&self, tile: TileId) {
        let mut previa = self.shared.local_preview.lock();
        if previa.as_ref().is_some_and(|p| p.tile == tile) {
            *previa = None;
        }
    }
    /// A prévia local está no `tile`?
    pub fn has_local_preview(&self, tile: TileId) -> bool {
        self.shared
            .local_preview
            .lock()
            .as_ref()
            .is_some_and(|p| p.tile == tile)
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
            converters: st
                .layers
                .iter()
                .filter(|l| l.enabled)
                .filter_map(|l| l.converter.map(|c| (l.layer, c)))
                .collect(),
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
    async fn set_bridge_requests_vazio_desliga() {
        use std::time::{Duration, Instant};
        use zap_rate::allocator::LayerRequest;

        // As estatísticas do pipeline saem em janelas de 1 s: espera a condição até 3 s.
        async fn espera(p: &VideoPublisher, cond: impl Fn(&[zap_media::LayerStats]) -> bool) -> bool {
            let ini = Instant::now();
            while ini.elapsed() < Duration::from_secs(3) {
                if cond(&p.shared.pipeline.stats().layers) {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            false
        }

        let p = publicador();
        let camada = p.ladder()[0].layer;
        p.set_bridge_requests(vec![LayerRequest {
            layer: camada,
            kbps: 500,
        }]);
        let ligou = espera(&p, |ls| ls.iter().any(|l| l.layer == camada && l.enabled)).await;
        assert!(ligou, "a camada pedida pela ponte deveria ligar");

        p.set_bridge_requests(vec![]);
        assert!(p.shared.bridge_requests.lock().is_empty());
        let desligou = espera(&p, |ls| !ls.is_empty() && ls.iter().all(|l| !l.enabled)).await;
        assert!(desligou, "pedido vazio sem espectadores deveria desligar tudo");
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

    /// Interface de mentira: guarda `(camada, keyframe, largura, altura)` de cada quadro entregue.
    #[derive(Default)]
    struct FakeUi {
        got: Mutex<Vec<(u8, bool, u16, u16)>>,
    }
    impl crate::engine::UiSink for FakeUi {
        fn state(&self, _snapshot: Arc<crate::Snapshot>) {}
        fn chat(&self, _msg: crate::types::ChatLine) {}
        fn video(&self, _tile: TileId, p: VideoPacket) {
            self.got.lock().push((p.layer, p.keyframe, p.width, p.height));
        }
    }

    /// Espera (até 3 s) a prévia receber um quadro que satisfaz `cond`; devolve o índice dele.
    async fn espera_quadro(ui: &FakeUi, cond: impl Fn(&(u8, bool, u16, u16)) -> bool) -> Option<usize> {
        let ini = std::time::Instant::now();
        while ini.elapsed() < std::time::Duration::from_secs(3) {
            if let Some(i) = ui.got.lock().iter().position(&cond) {
                return Some(i);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        None
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_capture_troca_escada_e_mantem_previa() {
        let ui = Arc::new(FakeUi::default());
        let hub = Arc::new(PlayoutHub::start(Clock::new(), ui.clone()));
        let p = publicador();
        p.set_local_preview(7, hub.clone(), TileSize::Thumbnail);
        // Antes: a prévia recebe a camada mais leve da escada da tela.
        let antiga = p.ladder();
        let camada_antiga = layer_for(&antiga, TileSize::Thumbnail).unwrap();
        assert!(espera_quadro(&ui, |q| q.0 == camada_antiga).await.is_some());

        let nova = vec![
            LayerSpec {
                layer: 0,
                width: 1920,
                height: 1080,
                fps: 30,
                kbps: 3000,
            },
            LayerSpec {
                layer: 2,
                width: 800,
                height: 450,
                fps: 15,
                kbps: 300,
            },
        ];
        assert!(antiga.iter().all(|l| (l.width, l.height) != (800, 450)));
        p.restart_capture(
            VideoSourceSpec::Synthetic {
                width: 1920,
                height: 1080,
            },
            nova.clone(),
            EncoderChoiceSpec::Auto,
        )
        .unwrap();
        assert_eq!(p.ladder(), nova);

        // Mesmo publicador e mesma prévia: o pipeline novo entrega, começando por keyframe.
        let i = espera_quadro(&ui, |q| (q.2, q.3) == (800, 450))
            .await
            .expect("a prévia deveria receber o pipeline novo");
        assert!(ui.got.lock()[i].1, "o primeiro quadro novo deveria ser keyframe");
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

    #[test]
    fn camada_da_previa_pelo_tamanho() {
        let c = |layer: u8| LayerSpec {
            layer,
            width: 640,
            height: 360,
            fps: 30,
            kbps: 500,
        };
        let escada = vec![c(0), c(1), c(2)];
        // Miniatura: a mais leve; tela principal: a do meio (não liga a 0 só para eu me ver).
        assert_eq!(super::layer_for(&escada, TileSize::Thumbnail), Some(2));
        assert_eq!(super::layer_for(&escada, TileSize::Main), Some(1));
        assert_eq!(super::layer_for(&escada, TileSize::Full), Some(1));
        assert_eq!(super::layer_for(&[c(0)], TileSize::Full), Some(0));
        assert_eq!(super::layer_for(&[], TileSize::Main), None);
    }
}
