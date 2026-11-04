//! `OnlineRoom`: a sala Online em que estou (criada por mim ou onde entrei por código).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use zap_net::online::JoinMedia;
use zap_proto::{RoomCode, Track};

use super::direct::DirectState;
use super::publish_bridge::BridgePublish;
use super::session::OnlineSession;
use crate::engine::Shared;
use crate::room::{RoomEvent, RoomModel};
use crate::subscribe::PubConn;
use crate::types::CreateRoom;
use crate::CoreError;

pub struct OnlineRoom {
    pub model: Arc<Mutex<RoomModel>>,
    /// `participante_id` dado pelo servidor.
    pub my_id: String,
    pub code: RoomCode,
    /// Senha da sala (guardada para renovar o ingresso ao reconectar).
    pub password: Option<String>,
    /// `"<CODIGO>:<senha ou vazio>"`: chave dos ingressos da rota direta (ver `online/mod.rs`).
    pub media_secret: String,
    pub(crate) shared: Weak<Shared>,
    pub(crate) events: UnboundedSender<RoomEvent>,
    pub(crate) session: Arc<OnlineSession>,
    /// `midia` da última resposta de `entrar` (ponte e `direta_permitida`).
    pub(crate) media: Mutex<JoinMedia>,
    /// Ingresso vigente da ponte.
    pub(crate) ticket: Mutex<String>,
    /// Conexão da ponte (também registrada em `ConnPool` para as assinaturas).
    pub(crate) bridge: tokio::sync::Mutex<Option<Arc<PubConn>>>,
    /// `trilha.publicar` esperando `trilha.publicada` (chave: `local_id`).
    pub(crate) pending: Mutex<HashMap<String, oneshot::Sender<Track>>>,
    pub(crate) publish: BridgePublish,
    pub(crate) direct: DirectState,
    /// `true` depois de `leave`/sala encerrada: as tarefas de reconexão desistem.
    pub(crate) left: AtomicBool,
    /// Tarefas de fundo da sala (eventos, reconexão); abortadas em `leave`.
    pub(crate) tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl OnlineRoom {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        shared: &Arc<Shared>,
        model: RoomModel,
        my_id: String,
        code: RoomCode,
        password: Option<String>,
        media: JoinMedia,
        ticket: String,
    ) -> Arc<Self> {
        let media_secret = format!("{}:{}", code, password.clone().unwrap_or_default());
        Arc::new(Self {
            model: Arc::new(Mutex::new(model)),
            my_id,
            code,
            password,
            media_secret,
            shared: Arc::downgrade(shared),
            events: shared.room_events.clone(),
            session: shared.online.clone(),
            media: Mutex::new(media),
            ticket: Mutex::new(ticket),
            bridge: tokio::sync::Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            publish: BridgePublish::default(),
            direct: DirectState::default(),
            left: AtomicBool::new(false),
            tasks: Mutex::new(Vec::new()),
        })
    }

    /// `Shared` vivo ou `CoreError::Stopped` (o `Core` foi desligado).
    pub(crate) fn shared(&self) -> Result<Arc<Shared>, CoreError> {
        self.shared.upgrade().ok_or(CoreError::Stopped)
    }

    /// Cria a sala no servidor e entra nela:
    /// 1. exige `shared.online.client()` (senão `Invalid("Servidor não conectado")`);
    /// 2. `client.create_room(CreateRoomRequest{name, access (Password se há senha), password})`;
    /// 3. segue o mesmo caminho de `join` com o código devolvido (quem cria já é membro; `entrar`
    ///    é idempotente e entrega o ingresso).
    pub async fn create(shared: &Arc<Shared>, req: CreateRoom) -> Result<Arc<Self>, CoreError> {
        let _ = (shared, req);
        todo!("tarefa core-online-08")
    }

    /// Entra numa sala Online (fluxo de `docs/api/README.md`, "Entrar numa sala Online"):
    /// 1. `client.join(&code, password)` → `JoinResponse` (erros do servidor viram
    ///    `CoreError::Server`: `senha_sala`, `sala_cheia`, `sala_nao_encontrada`...);
    /// 2. cria o canal de mensagens (`mpsc::channel(1024)`), `session.set_route(Some(tx))`;
    /// 3. `ws.send(RoomJoin{room_id})`; espera a primeira `RoomState` (10 s; `Connect` se não
    ///    vier) e monta o `RoomModel::from_state(room, participants, tracks)` — `my_id` =
    ///    `participant_id` do `JoinResponse`;
    /// 4. `OnlineRoom::new`, guarda o `RoomModel`, sobe `room_events::run` (tarefa) e conecta a
    ///    ponte (`reconnect::connect_bridge_for`) se `settings.server.use_relay` e o servidor
    ///    oferece `media.bridge`; se `direct_allowed`: `shared.ensure_media_server()` e
    ///    `direct::send_my_addresses`;
    /// 5. falha depois de `session.set_route` → `set_route(None)` e `client.leave(&code)`.
    pub async fn join(shared: &Arc<Shared>, code: RoomCode, password: Option<String>) -> Result<Arc<Self>, CoreError> {
        let _ = (shared, code, password);
        todo!("tarefa core-online-08")
    }

    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        let _ = text;
        todo!("tarefa core-online-08")
    }
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        todo!("tarefa core-online-08")
    }

    /// Sai da sala: `left = true`, `ws.send(RoomLeave)`, `client.leave(&code)`, fecha a ponte
    /// (`PubConn::close` + tira do `ConnPool`), `publish.detach_all()`, `session.set_route(None)`,
    /// aborta `tasks`. Erros de rede são ignorados (log).
    pub async fn leave(&self) {
        todo!("tarefa core-online-08")
    }
}
