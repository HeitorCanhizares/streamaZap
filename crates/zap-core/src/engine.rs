//! Fachada do núcleo: `Core` (API que o Tauri chama), `UiSink` e o estado compartilhado.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc::UnboundedSender;
use zap_net::discovery::Discovery;
use zap_net::tls::Identity;

use crate::audio_out::AudioOutHandle;
use crate::notify::Notifier;
use crate::playout::PlayoutHub;
use crate::publish::{MediaAuth, MediaServer};
use crate::room::{ActiveRoom, RoomCtx, RoomEvent};
use crate::settings::Settings;
use crate::share::ShareSession;
use crate::snapshot::{Me, Snapshot};
use crate::state::UiState;
use crate::subscribe::{AudioRegistry, ConnPool, Subscription};
use crate::types::*;
use crate::CoreError;

/// Saída para a interface. Chamado de threads do núcleo: não pode bloquear.
pub trait UiSink: Send + Sync + 'static {
    /// No máx. 4/s, e na hora em mudança de tela.
    fn state(&self, snapshot: Arc<Snapshot>);
    fn chat(&self, msg: ChatLine);
    /// Já no instante de exibir.
    fn video(&self, tile: TileId, packet: VideoPacket);
}

#[derive(Clone, Debug)]
pub struct CoreOptions {
    /// Sobe a descoberta UDP.
    pub discovery: bool,
    /// `DiscoveryConfig::broadcast` (false nos testes).
    pub broadcast: bool,
    pub discovery_port: u16,
    /// Inclui 127.0.0.1 nos endereços anunciados (testes).
    pub include_loopback: bool,
    /// Cria a saída de áudio (`AudioOutHandle::start`); false = stub.
    pub audio_out: bool,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self {
            discovery: true,
            broadcast: true,
            discovery_port: zap_proto::DISCOVERY_PORT,
            include_loopback: false,
            audio_out: true,
        }
    }
}

/// Estado compartilhado por todas as partes do núcleo.
pub struct Shared {
    pub rt: tokio::runtime::Handle,
    pub data_dir: PathBuf,
    pub options: CoreOptions,
    pub settings: RwLock<Settings>,
    pub identity: Arc<Identity>,
    pub ui: Arc<dyn UiSink>,
    pub state: Arc<Mutex<UiState>>,
    pub notifier: Notifier,
    pub clock: zap_net::Clock,
    pub discovery: Mutex<Option<Arc<Discovery>>>,
    pub room: Mutex<Option<Arc<ActiveRoom>>>,
    pub room_events: UnboundedSender<RoomEvent>,
    pub share: tokio::sync::Mutex<ShareSession>,
    pub media_server: tokio::sync::Mutex<Option<Arc<MediaServer>>>,
    pub subs: Mutex<HashMap<TileId, Arc<Subscription>>>,
    pub pool: ConnPool,
    pub playout: Arc<PlayoutHub>,
    pub audio_rx: AudioRegistry,
    pub audio_out: AudioOutHandle,
    pub stop: AtomicBool,
}

impl Shared {
    /// Sala atual (clone do `Arc`).
    pub fn current_room(&self) -> Option<Arc<ActiveRoom>> {
        self.room.lock().clone()
    }
    /// Valores atuais das configurações para uma sala nova.
    pub fn room_ctx(&self) -> Arc<RoomCtx> {
        let s = self.settings.read();
        Arc::new(RoomCtx {
            identity: self.identity.clone(),
            install_id: s.install_id.clone(),
            name: s.name.clone(),
            color: s.color.clone(),
            port: s.local.port,
            interface: s.local.interface.clone(),
            include_loopback: self.options.include_loopback,
            app_version: env!("CARGO_PKG_VERSION").into(),
            events: self.room_events.clone(),
        })
    }
    /// Cria o `MediaServer` na primeira vez (porta preferida = `settings.local.port + 1`) e o
    /// reaproveita; configura `set_auth` com a sala atual (`room`, `media_secret`, `members`).
    pub async fn ensure_media_server(self: &Arc<Self>) -> Result<Arc<MediaServer>, CoreError> {
        // Trava assíncrona: duas chamadas concorrentes não criam dois servidores.
        let mut guard = self.media_server.lock().await;
        let server = match guard.as_ref() {
            Some(existente) => existente.clone(),
            None => {
                let porta = self.settings.read().local.port.saturating_add(1);
                let novo =
                    MediaServer::start(self.identity.clone(), porta, self.clock.clone(), self.rt.clone()).await?;
                *guard = Some(novo.clone());
                novo
            }
        };
        // Sem sala, nenhum espectador é aceito (`set_auth(None)` recusa o ingresso).
        let auth = match self.current_room() {
            Some(sala) => {
                let model = sala.model();
                let room = model.lock().room.clone();
                let para_membros = model.clone();
                Some(MediaAuth {
                    room,
                    secret: sala.media_secret(),
                    members: Arc::new(move || para_membros.lock().participants.iter().map(|p| p.id.clone()).collect()),
                })
            }
            None => None,
        };
        server.set_auth(auth);
        Ok(server)
    }
    /// Reconstrói `UiState.room` a partir do modelo da sala (ou `None`) e faz `notifier.touch()`.
    pub fn refresh_room_view(&self) {
        // Gera o `RoomView` antes de travar o estado: nunca os dois locks ao mesmo tempo.
        let view = match self.current_room() {
            Some(sala) => {
                let model = sala.model();
                let v = model.lock().to_view();
                Some(v)
            }
            None => None,
        };
        self.state.lock().set_room(view);
        self.notifier.touch();
    }
    /// Reconstrói `UiState.me` a partir das configurações.
    pub fn refresh_me(&self) {
        let me = {
            let s = self.settings.read();
            Me {
                name: s.name.clone(),
                color: s.color.clone(),
                install_id: s.install_id.clone(),
            }
        };
        self.state.lock().set_me(me);
        self.notifier.touch();
    }
}

pub struct Core {
    pub(crate) shared: Arc<Shared>,
    /// Runtime próprio do núcleo (2 threads de trabalho); todo `async fn` público salta para ele
    /// com `shared.rt.spawn(..).await`.
    pub(crate) runtime: Option<tokio::runtime::Runtime>,
}

impl Core {
    /// `start_with(data_dir, ui, CoreOptions::default())`.
    pub fn start(data_dir: PathBuf, ui: Arc<dyn UiSink>) -> Result<Self, CoreError> {
        Self::start_with(data_dir, ui, CoreOptions::default())
    }

    /// Cria o runtime, carrega `settings::load` + identidade, monta `Shared` (UiState com `Me`,
    /// `Notifier::spawn`, `PlayoutHub::start`, `AudioOutHandle` conforme `options`), sobe a
    /// descoberta (`Discovery::start` com `DiscoveryConfig{port, broadcast}` + presença) se
    /// habilitada (falha de descoberta = só log + `push_error`), `engine_room::pump_room_events`
    /// e `engine_tick::run`. Primeiro snapshot já entregue no fim.
    pub fn start_with(data_dir: PathBuf, ui: Arc<dyn UiSink>, options: CoreOptions) -> Result<Self, CoreError> {
        todo!("tarefa core-38")
    }

    pub fn settings(&self) -> Settings {
        self.shared.settings.read().clone()
    }
    /// Aplica `f`, salva (`settings::save`), atualiza `Me` e o nome/presença da descoberta e
    /// faz `touch()`. Erro de gravação: `push_error`.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        todo!("tarefa core-38")
    }
    pub fn sources(&self) -> Sources {
        crate::sources::list_sources()
    }
    pub fn thumbnail(&self, src: &SourceRef, w: u32, h: u32) -> Result<Rgba, CoreError> {
        crate::sources::thumbnail(src, w, h)
    }
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.shared.state.lock().snapshot()
    }
    /// `audio_rx.set_volume(participant_id, volume, muted)`.
    pub fn set_volume(&self, participant_id: &str, volume: f32, muted: bool) {
        todo!("tarefa core-38")
    }
    /// Guarda em `settings.video` (sem salvar de novo se igual) e `playout.set_smoothing`.
    pub fn set_smoothing(&self, auto: bool, ms: u32) {
        todo!("tarefa core-38")
    }
    /// Para tudo (sala, compartilhamento, assinaturas) e derruba o runtime.
    pub fn shutdown(&mut self) {
        todo!("tarefa core-38")
    }
}
