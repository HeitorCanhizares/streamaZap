//! Sessão com o servidor Online: login, WebSocket, ping e lista de salas.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{mpsc, watch};
use zap_net::online::{RoomListing as NetListing, ServerClient, ServerInfo, WsTx};
use zap_proto::ControlMsg;

use super::preview::PreviewRegistry;
use super::status::ServerStatus;
use crate::engine::Shared;
use crate::CoreError;

/// Estado compartilhado da conexão com o servidor (um por `Core`; vive em `Shared::online`).
pub struct OnlineSession {
    /// `None` = sem endereço configurado ("não configurado").
    pub(crate) status: Mutex<Option<ServerStatus>>,
    pub(crate) info: Mutex<Option<ServerInfo>>,
    pub(crate) client: Mutex<Option<ServerClient>>,
    pub(crate) ws: Mutex<Option<WsTx>>,
    /// Última `salas.lista` (já convertida).
    pub(crate) rooms: Mutex<Vec<NetListing>>,
    /// Destino das mensagens do WebSocket que não são `salas.lista` (a sala ativa).
    pub(crate) route: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    /// Soma 1 a cada WebSocket (re)conectado com sucesso.
    pub(crate) epoch: watch::Sender<u64>,
    /// Acorda o laço de `session_run::run` (configurações mudaram).
    pub(crate) wake: tokio::sync::Notify,
    pub(crate) task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(crate) previews: PreviewRegistry,
}

impl OnlineSession {
    pub fn new() -> Arc<Self> {
        let (epoch, _) = watch::channel(0u64);
        Arc::new(Self {
            status: Mutex::new(None),
            info: Mutex::new(None),
            client: Mutex::new(None),
            ws: Mutex::new(None),
            rooms: Mutex::new(Vec::new()),
            route: Mutex::new(None),
            epoch,
            wake: tokio::sync::Notify::new(),
            task: Mutex::new(None),
            previews: PreviewRegistry::default(),
        })
    }
    pub fn status(&self) -> Option<ServerStatus> {
        self.status.lock().clone()
    }
    pub fn client(&self) -> Option<ServerClient> {
        self.client.lock().clone()
    }
    pub fn ws(&self) -> Option<WsTx> {
        self.ws.lock().clone()
    }
    pub fn rooms(&self) -> Vec<NetListing> {
        self.rooms.lock().clone()
    }
    pub fn info(&self) -> Option<ServerInfo> {
        self.info.lock().clone()
    }
    /// Define (ou limpa) o canal que recebe as mensagens da sala ativa.
    pub fn set_route(&self, tx: Option<mpsc::Sender<ControlMsg>>) {
        *self.route.lock() = tx;
    }
    pub fn epoch_rx(&self) -> watch::Receiver<u64> {
        self.epoch.subscribe()
    }
    /// Código (`ZAP-XXXX`) da sala Online com esse `id` na última lista; `None` se não está.
    pub fn code_for_room_id(&self, room_id: &str) -> Option<String> {
        self.rooms
            .lock()
            .iter()
            .find(|r| r.id == room_id)
            .map(|r| r.code.to_string())
    }

    /// Fluxo completo de (re)conexão, usado pelo laço de `session_run`:
    /// 1. endereço de `settings.server.address` vazio → `Err(Invalid("Servidor não configurado"))`;
    /// 2. `ServerClient::info(base)` (guarda em `info`); `PROTOCOL_VERSION` fora de
    ///    `[protocol_min, protocol_max]` → `Err(Server{code:"protocolo_incompativel", message:
    ///    "Este app e o servidor usam versões diferentes do protocolo. Atualize o StreamaZap."})`
    ///    e, se `protocol_min > PROTOCOL_VERSION`, `state.set_newer_protocol(Some(protocol_min))`;
    /// 3. `ServerClient::login(base, LoginRequest{name, color, install_id, protocol,
    ///    app_version, server_password})` (senha vazia → `None`); guarda em `client`;
    /// 4. `client.connect_ws()`; guarda `WsTx` em `ws` e devolve o `WsRx`.
    /// `base` = endereço com `https://` na frente se não houver esquema (`http://` é aceito).
    pub async fn connect(&self, shared: &Arc<Shared>) -> Result<zap_net::online::WsRx, CoreError> {
        let _ = shared;
        todo!("tarefa core-online-05")
    }
    /// Ping em ms: tempo de um `ServerClient::info(base)` (timeout 5 s → `Err(Connect)`).
    pub async fn measure_ping(&self, shared: &Arc<Shared>) -> Result<u32, CoreError> {
        let _ = shared;
        todo!("tarefa core-online-05")
    }
    /// Sobe `session_run::run` numa tarefa do runtime do `Core` (guarda o handle em `task`).
    pub fn start(self: &Arc<Self>, shared: &Arc<Shared>) {
        let _ = shared;
        todo!("tarefa core-online-05")
    }
    /// Aborta a tarefa atual, zera `client`/`ws`/`info`/`rooms`/`status`/`route` e chama `start`
    /// (as configurações do servidor mudaram).
    pub fn restart(self: &Arc<Self>, shared: &Arc<Shared>) {
        let _ = shared;
        todo!("tarefa core-online-05")
    }
    /// Aborta a tarefa e fecha o WebSocket (desligamento do `Core`).
    pub fn stop(&self) {
        todo!("tarefa core-online-05")
    }
}
