//! Um espectador de uma trilha: preferências do `assinar`, último relatório e camada atual.

use std::sync::Arc;

use parking_lot::Mutex;
use zap_net::{control::ControlTx, media_tx::MediaSender};
use zap_proto::{ControlMsg, DelayState, Report, TileSize};
use zap_rate::allocator::{ViewerDecision, ViewerInput};

/// O que fazer com um quadro codificado para este espectador.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRoute {
    /// Camada atual: envia.
    Send,
    /// Keyframe da camada pendente: troca para ela, envia e avisa `camada.atual`.
    Switch,
    /// Outra camada: ignora.
    Skip,
}

#[derive(Clone, Debug)]
pub struct ViewerState {
    pub size: TileSize,
    pub requested_max: Option<u8>,
    pub audio: bool,
    pub bandwidth_kbps: u32,
    pub delay: DelayState,
    pub current: u8,
    pub pending: Option<u8>,
    /// Link de ponte Online: recebe **todos** os quadros das camadas em `bridge_layers`
    /// (quem escolhe a camada de cada espectador é o servidor) e não entra no alocador.
    pub bridge: bool,
    pub bridge_layers: Vec<u8>,
}

impl ViewerState {
    /// Começa na camada mais barata (`cheapest`), banda 1000 kbps, estado Normal, `Main`.
    pub fn new(cheapest: u8) -> Self {
        Self {
            size: TileSize::Main,
            requested_max: None,
            audio: false,
            bandwidth_kbps: 1000,
            delay: DelayState::Normal,
            current: cheapest,
            pending: None,
            bridge: false,
            bridge_layers: Vec::new(),
        }
    }
    /// Estado de um link de ponte Online: áudio ligado, fora do alocador.
    pub fn new_bridge() -> Self {
        let mut s = Self::new(0);
        s.bridge = true;
        s.audio = true;
        s
    }
    /// Define as camadas que a ponte recebe (todas as pedidas pelo servidor).
    pub fn set_bridge_layers(&mut self, layers: Vec<u8>) {
        self.bridge_layers = layers;
    }
    /// `max(teto_pedido, teto_tamanho)` (docs/api/balanceamento.md §2): miniatura → 2,
    /// principal/cheia → 0; `requested_max` ausente → 0. Número maior = camada menor.
    pub fn ceiling(&self) -> u8 {
        // Teto por tamanho: miniatura é camada pequena; principal/cheia não limita.
        let teto_tamanho = match self.size {
            TileSize::Thumbnail => 2,
            TileSize::Main | TileSize::Full => 0,
        };
        self.requested_max.unwrap_or(0).max(teto_tamanho)
    }
    pub fn apply_subscribe(&mut self, size: TileSize, max_layer: Option<u8>, audio: Option<bool>) {
        self.size = size;
        self.requested_max = max_layer;
        // Áudio só muda se o cliente disse algo: `None` mantém a preferência anterior.
        if let Some(a) = audio {
            self.audio = a;
        }
    }
    /// Copia `bandwidth_kbps` (se `Some`; senão mantém), `state` (se `Some`) do relatório.
    pub fn apply_report(&mut self, r: &Report) {
        self.bandwidth_kbps = r.bandwidth_kbps.unwrap_or(self.bandwidth_kbps);
        self.delay = r.state.unwrap_or(self.delay);
    }
    /// `ViewerInput { id, bandwidth_kbps, state: delay, max_layer: ceiling(), reserved_kbps }`.
    pub fn input(&self, id: u64, reserved_kbps: u32) -> ViewerInput {
        ViewerInput {
            id,
            bandwidth_kbps: self.bandwidth_kbps,
            state: self.delay,
            max_layer: self.ceiling(),
            reserved_kbps,
        }
    }
    /// Copia `current`/`pending` da decisão do alocador; devolve `true` se `current` mudou
    /// (o chamador manda `camada.atual`).
    pub fn apply_decision(&mut self, d: &ViewerDecision) -> bool {
        let changed = self.current != d.current;
        self.current = d.current;
        self.pending = d.pending;
        changed
    }
    /// `layer == current` → Send; keyframe com `pending == Some(layer)` → faz `current = layer`,
    /// `pending = None` e devolve Switch; senão Skip.
    pub fn route(&mut self, layer: u8, keyframe: bool) -> FrameRoute {
        if self.bridge {
            // Ponte: todo quadro (delta ou keyframe) das camadas pedidas passa; nunca troca.
            return if self.bridge_layers.contains(&layer) {
                FrameRoute::Send
            } else {
                FrameRoute::Skip
            };
        }
        if layer == self.current {
            FrameRoute::Send
        } else if keyframe && self.pending == Some(layer) {
            // Troca só em keyframe: quadro delta da camada nova não serve sem o anterior.
            self.current = layer;
            self.pending = None;
            FrameRoute::Switch
        } else {
            FrameRoute::Skip
        }
    }
}

pub struct ViewerLink {
    pub id: u64,
    /// `instalacao_id` de quem assiste (só para log/estatística).
    pub label: String,
    pub track_id: String,
    pub sender: Arc<MediaSender>,
    pub ctrl: ControlTx,
    pub state: Mutex<ViewerState>,
}

impl ViewerLink {
    pub fn new(
        id: u64,
        label: String,
        track_id: String,
        sender: Arc<MediaSender>,
        ctrl: ControlTx,
        cheapest: u8,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            label,
            track_id,
            sender,
            ctrl,
            state: Mutex::new(ViewerState::new(cheapest)),
        })
    }
    /// `camada.atual` para o espectador (`motivo`: "inicio" | "banda" | "teto" | "tamanho" | "publicador").
    /// Falha de envio só vira log: a troca de camada já valeu no servidor.
    pub async fn announce_layer(&self, info: &LayerInfo, reason: &str) {
        let msg = ControlMsg::LayerCurrent {
            track_id: self.track_id.clone(),
            layer: info.layer,
            width: info.width,
            height: info.height,
            fps: info.fps,
            reason: reason.to_owned(),
        };
        if let Err(e) = self.ctrl.send(&msg).await {
            tracing::debug!(espectador = self.id, erro = %e, "camada.atual não enviada");
        }
    }
}

/// Dados de uma camada para `camada.atual`.
#[derive(Clone, Copy, Debug)]
pub struct LayerInfo {
    pub layer: u8,
    pub width: u16,
    pub height: u16,
    pub fps: u8,
}

impl LayerInfo {
    /// Copia camada, largura, altura e fps da especificação do publicador.
    pub fn from_spec(l: &zap_proto::LayerSpec) -> Self {
        Self {
            layer: l.layer,
            width: l.width,
            height: l.height,
            fps: l.fps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estado(size: TileSize, requested: Option<u8>) -> ViewerState {
        let mut s = ViewerState::new(2);
        s.size = size;
        s.requested_max = requested;
        s
    }

    fn relatorio(banda: Option<u32>, estado: Option<DelayState>) -> Report {
        Report {
            arrivals: Vec::new(),
            probes: Vec::new(),
            lost: Vec::new(),
            loss: 0.0,
            bandwidth_kbps: banda,
            state: estado,
            buffer_ms: None,
        }
    }

    #[test]
    fn teto_por_tamanho() {
        assert_eq!(estado(TileSize::Thumbnail, None).ceiling(), 2);
        assert_eq!(estado(TileSize::Main, None).ceiling(), 0);
        assert_eq!(estado(TileSize::Full, None).ceiling(), 0);
    }

    #[test]
    fn teto_combina_pedido_e_tamanho() {
        assert_eq!(estado(TileSize::Main, Some(1)).ceiling(), 1);
        assert_eq!(estado(TileSize::Thumbnail, Some(0)).ceiling(), 2);
        assert_eq!(estado(TileSize::Thumbnail, Some(2)).ceiling(), 2);
    }

    #[test]
    fn subscribe_audio_so_muda_se_some() {
        let mut s = ViewerState::new(2);
        s.apply_subscribe(TileSize::Full, Some(1), Some(true));
        assert!(s.audio);
        assert_eq!(s.size, TileSize::Full);
        assert_eq!(s.requested_max, Some(1));

        s.apply_subscribe(TileSize::Main, None, None);
        assert!(s.audio, "audio ausente deve manter o valor anterior");
        assert_eq!(s.size, TileSize::Main);
        assert_eq!(s.requested_max, None);
    }

    #[test]
    fn report_copia_banda_e_estado_e_mantem_quando_ausente() {
        let mut s = ViewerState::new(2);
        s.apply_report(&relatorio(Some(4200), Some(DelayState::Overuse)));
        assert_eq!(s.bandwidth_kbps, 4200);
        assert_eq!(s.delay, DelayState::Overuse);

        s.apply_report(&relatorio(None, None));
        assert_eq!(s.bandwidth_kbps, 4200);
        assert_eq!(s.delay, DelayState::Overuse);
    }

    #[test]
    fn input_usa_teto() {
        let mut s = estado(TileSize::Thumbnail, Some(0));
        s.bandwidth_kbps = 3000;
        s.delay = DelayState::Underuse;
        let i = s.input(7, 128);
        assert_eq!(i.id, 7);
        assert_eq!(i.bandwidth_kbps, 3000);
        assert_eq!(i.state, DelayState::Underuse);
        assert_eq!(i.max_layer, 2);
        assert_eq!(i.reserved_kbps, 128);
    }

    #[test]
    fn decision_detecta_mudanca() {
        let mut s = ViewerState::new(2);
        let igual = ViewerDecision {
            id: 1,
            current: 2,
            pending: Some(1),
        };
        assert!(!s.apply_decision(&igual));
        assert_eq!(s.pending, Some(1));

        let nova = ViewerDecision {
            id: 1,
            current: 1,
            pending: None,
        };
        assert!(s.apply_decision(&nova));
        assert_eq!(s.current, 1);
        assert_eq!(s.pending, None);
    }

    #[test]
    fn route_bridge_envia_so_camadas_pedidas() {
        let mut st = ViewerState::new_bridge();
        st.set_bridge_layers(vec![0, 2]);
        for kf in [false, true] {
            assert_eq!(st.route(0, kf), FrameRoute::Send);
            assert_eq!(st.route(1, kf), FrameRoute::Skip);
            assert_eq!(st.route(2, kf), FrameRoute::Send);
        }
    }

    #[test]
    fn route_normal_inalterado() {
        let mut st = ViewerState::new(2);
        assert_eq!(st.route(2, false), FrameRoute::Send);
        assert_eq!(st.route(0, true), FrameRoute::Skip);
        st.pending = Some(0);
        assert_eq!(st.route(0, false), FrameRoute::Skip);
        assert_eq!(st.route(0, true), FrameRoute::Switch);
    }

    #[test]
    fn route_send_na_camada_atual() {
        let mut s = ViewerState::new(2);
        assert_eq!(s.route(2, false), FrameRoute::Send);
        assert_eq!(s.route(2, true), FrameRoute::Send);
        assert_eq!(s.current, 2);
    }

    #[test]
    fn route_switch_so_com_keyframe_da_pendente() {
        let mut s = ViewerState::new(2);
        s.pending = Some(1);
        assert_eq!(s.route(1, false), FrameRoute::Skip);
        assert_eq!(s.current, 2);
        assert_eq!(s.pending, Some(1));

        assert_eq!(s.route(1, true), FrameRoute::Switch);
        assert_eq!(s.current, 1);
        assert_eq!(s.pending, None);
    }

    #[test]
    fn route_skip_outra_camada() {
        let mut s = ViewerState::new(2);
        s.pending = Some(0);
        assert_eq!(s.route(1, true), FrameRoute::Skip);
        assert_eq!(s.route(1, false), FrameRoute::Skip);
        assert_eq!(s.current, 2);
        assert_eq!(s.pending, Some(0));
    }

    use zap_net::{
        control::ControlRx,
        endpoint,
        tls::{self, Identity},
        Clock,
    };

    /// Par QUIC no loopback (mesmo helper de zap-net/handshake/client.rs).
    struct Par {
        cli: quinn::Connection,
        srv: quinn::Connection,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn par() -> Result<Par, Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?;
        let inc = servidor.accept().await.ok_or("sem conexão")?;
        let srv = inc.await?;
        let cli = conectando.await?;
        Ok(Par {
            cli,
            srv,
            _eps: (servidor, cliente),
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn announce_layer_envia_camada_atual() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (send, _recv_a) = p.cli.open_bi().await?;
        let tx = ControlTx::new(send);

        // Anuncia antes de aceitar: o lado B só vê o stream quando chegam dados.
        let link = ViewerLink::new(
            1,
            "x".into(),
            "t_1".into(),
            Arc::new(MediaSender::new(p.cli.clone(), Clock::new())),
            tx,
            2,
        );
        link.announce_layer(
            &LayerInfo {
                layer: 1,
                width: 1280,
                height: 720,
                fps: 30,
            },
            "banda",
        )
        .await;

        let (_send_b, recv_b) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv_b);
        let recebida = rx.recv().await?.ok_or("stream fechou sem mensagem")?;
        assert_eq!(
            recebida,
            ControlMsg::LayerCurrent {
                track_id: "t_1".into(),
                layer: 1,
                width: 1280,
                height: 720,
                fps: 30,
                reason: "banda".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn from_spec_copia_campos() {
        let spec = zap_proto::LayerSpec {
            layer: 2,
            width: 640,
            height: 360,
            fps: 15,
            kbps: 800,
        };
        let info = LayerInfo::from_spec(&spec);
        assert_eq!(info.layer, 2);
        assert_eq!(info.width, 640);
        assert_eq!(info.height, 360);
        assert_eq!(info.fps, 15);
    }
}
