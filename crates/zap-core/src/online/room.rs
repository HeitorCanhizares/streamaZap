//! `OnlineRoom`: a sala Online em que estou (criada por mim ou onde entrei por código).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::{mpsc, mpsc::UnboundedSender, oneshot};
use zap_net::online::{CreateRoomRequest, JoinMedia, JoinResponse};
use zap_proto::{Access, ControlMsg, RoomCode, RoomMode, Track};

use super::direct::{self, DirectState};
use super::publish_bridge::BridgePublish;
use super::session::OnlineSession;
use super::{reconnect, room_events};
use crate::engine::Shared;
use crate::room::{RoomEvent, RoomModel};
use crate::subscribe::PubConn;
use crate::types::CreateRoom;
use crate::util::new_id;
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
    /// Instante do último `chat.digitando` enviado (limite de 1 a cada 3 s).
    last_typing: Mutex<Option<Instant>>,
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
            last_typing: Mutex::new(None),
        })
    }

    /// Ponte configurada (`use_relay`) e oferecida pelo servidor (`media.bridge`).
    pub fn bridge_enabled(&self, shared: &Shared) -> bool {
        shared.settings.read().server.use_relay && self.media.lock().bridge.is_some()
    }

    /// `Shared` vivo ou `CoreError::Stopped` (o `Core` foi desligado).
    pub(crate) fn shared(&self) -> Result<Arc<Shared>, CoreError> {
        self.shared.upgrade().ok_or(CoreError::Stopped)
    }

    /// Cria a sala no servidor e entra nela (quem cria já é membro; `entrar` é idempotente e
    /// entrega o ingresso).
    pub async fn create(shared: &Arc<Shared>, req: CreateRoom) -> Result<Arc<Self>, CoreError> {
        if req.mode != RoomMode::Online {
            return Err(CoreError::Invalid("A sala não é do modo Online".into()));
        }
        let client = shared
            .online
            .client()
            .ok_or_else(|| CoreError::Invalid("Servidor não conectado".into()))?;
        let password = req.password.filter(|p| !p.is_empty());
        let listing = client
            .create_room(CreateRoomRequest {
                name: req.name,
                access: if password.is_some() {
                    Access::Password
                } else {
                    Access::Open
                },
                password: password.clone(),
            })
            .await?;
        Self::join(shared, listing.code, password).await
    }

    /// Entra numa sala Online (fluxo de `docs/api/README.md`, "Entrar numa sala Online").
    pub async fn join(shared: &Arc<Shared>, code: RoomCode, password: Option<String>) -> Result<Arc<Self>, CoreError> {
        let session = shared.online.clone();
        let client = session
            .client()
            .ok_or_else(|| CoreError::Invalid("Servidor não conectado".into()))?;
        let resp = client.join(&code, password.as_deref()).await?;

        let (tx, mut rx) = mpsc::channel::<ControlMsg>(1024);
        session.set_route(Some(tx));
        match Self::entra(shared, &session, &code, password, resp, &mut rx).await {
            Ok(room) => {
                let eventos = shared.rt.spawn(room_events::run(room.clone(), rx));
                let ws = shared.rt.spawn(reconnect::watch_ws(room.clone()));
                let ponte = shared.rt.spawn(reconnect::watch_bridge(room.clone()));
                room.tasks.lock().extend([eventos, ws, ponte]);
                if let Err(e) = reconnect::connect_bridge_for(&room).await {
                    // Sem ponte a sala segue de pé.
                    let _ = room.events.send(RoomEvent::Error(e.to_string()));
                }
                let direta = room.media.lock().direct_allowed;
                if direta {
                    match shared.ensure_media_server().await {
                        Ok(_) => {
                            if let Err(e) = direct::send_my_addresses(&room).await {
                                tracing::warn!("não consegui anunciar meus endereços: {e}");
                            }
                        }
                        Err(e) => tracing::warn!("servidor de mídia indisponível: {e}"),
                    }
                }
                Ok(room)
            }
            Err(e) => {
                session.set_route(None);
                let _ = client.leave(&code).await;
                Err(e)
            }
        }
    }

    /// `sala.entrar`, espera do primeiro `sala.estado` e montagem da sala.
    async fn entra(
        shared: &Arc<Shared>,
        session: &Arc<OnlineSession>,
        code: &RoomCode,
        password: Option<String>,
        resp: JoinResponse,
        rx: &mut mpsc::Receiver<ControlMsg>,
    ) -> Result<Arc<Self>, CoreError> {
        let ws = session
            .ws()
            .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
        ws.send(ControlMsg::RoomJoin {
            room_id: resp.room.id.clone(),
        })
        .await
        .map_err(|e| CoreError::Connect(e.to_string()))?;

        let espera = async {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ControlMsg::RoomState {
                        room,
                        participants,
                        tracks,
                    } => return Ok((room, participants, tracks)),
                    ControlMsg::Error { code, message, .. } => return Err(CoreError::Server { code, message }),
                    _ => {}
                }
            }
            Err(CoreError::Connect("servidor não respondeu".into()))
        };
        let (sala, participantes, trilhas) = tokio::time::timeout(Duration::from_secs(10), espera)
            .await
            .map_err(|_| CoreError::Connect("servidor não respondeu".into()))??;

        let model = RoomModel::from_state(sala, participantes, trilhas);
        Ok(Self::new(
            shared,
            model,
            resp.participant_id,
            code.clone(),
            password,
            resp.media,
            resp.ticket,
        ))
    }

    /// Pedido de música da sala (`musica.*`) para o servidor, que é a fonte da fila no Online. O
    /// estado volta como `musica.estado` (ou `erro`).
    pub async fn music(&self, msg: ControlMsg) -> Result<(), CoreError> {
        let ws = self
            .session
            .ws()
            .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
        ws.send(msg).await.map_err(|e| CoreError::Connect(e.to_string()))
    }

    /// Manda uma mensagem de chat (1 a 1000 caracteres). O eco volta pelo servidor.
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        let n = text.chars().count();
        if n == 0 || n > 1000 {
            return Err(CoreError::Invalid("A mensagem deve ter de 1 a 1000 caracteres".into()));
        }
        let ws = self
            .session
            .ws()
            .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
        ws.send(ControlMsg::ChatSend {
            text,
            client_id: new_id("c_"),
        })
        .await
        .map_err(|e| CoreError::Connect(e.to_string()))
    }

    /// "Está digitando": no máximo um aviso a cada 3 s (o excedente é descartado).
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        {
            let mut ultimo = self.last_typing.lock();
            let agora = Instant::now();
            if ultimo.is_some_and(|t| agora.duration_since(t) < Duration::from_secs(3)) {
                return Ok(());
            }
            *ultimo = Some(agora);
        }
        let ws = self
            .session
            .ws()
            .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
        ws.send(ControlMsg::ChatTyping { participant_id: None })
            .await
            .map_err(|e| CoreError::Connect(e.to_string()))
    }

    /// Eu sou o dono da sala?
    pub fn is_owner(&self) -> bool {
        self.model.lock().room.owner_id == self.my_id
    }

    /// Sai da sala. O dono **encerra** a sala (`DELETE /v1/salas/{codigo}`), como na sala Local;
    /// sem isso o servidor a mantinha listada por até 2 min depois de o dono sair. Erros de rede são
    /// ignorados; pode ser chamada mais de uma vez.
    pub async fn leave(&self) {
        // Já saiu (ou a sala foi encerrada): não manda nada ao servidor de novo.
        let ja_saiu = self.left.swap(true, Ordering::SeqCst);
        let dono = self.is_owner();
        if !dono {
            if let Some(ws) = self.session.ws() {
                let _ = ws.send(ControlMsg::RoomLeave {}).await;
            }
        }
        self.session.set_route(None);
        for t in self.tasks.lock().drain(..) {
            t.abort();
        }
        self.publish.detach_all();
        // Fecha as conexões diretas e esvazia os pedidos pendentes. Não avisa o servidor: o `sala.sair` já avisa.
        let conns = std::mem::take(&mut *self.direct.conns.lock());
        for conn in conns.into_values() {
            conn.close();
        }
        self.direct.pending.lock().clear();
        if let Some(b) = self.bridge.lock().await.take() {
            b.close();
        }
        if let Ok(shared) = self.shared() {
            *shared.pool.online.lock() = None;
        }
        // Por último: o servidor pode demorar, e o resto já está desfeito.
        if ja_saiu {
            return;
        }
        if let Some(client) = self.session.client() {
            if dono {
                match client.delete_room(&self.code).await {
                    Ok(()) => return,
                    // Servidor sem a rota (ou recusou): pelo menos sai.
                    Err(e) => tracing::debug!("encerrar a sala no servidor: {e}"),
                }
            }
            if let Err(e) = client.leave(&self.code).await {
                tracing::debug!("sair da sala no servidor: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;
    use zap_test_server::Config;

    async fn novo_core(url: String) -> (Core, std::path::PathBuf) {
        let (core, d) = tokio::task::spawn_blocking(|| {
            let d = std::env::temp_dir().join(format!("zapcore-room-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
            s.server.use_relay = false;
        });
        core.shared.online.start(&core.shared);
        assert!(ate(|| core.shared.online.ws().is_some()).await, "sem websocket");
        (core, d)
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    async fn ate(mut ok: impl FnMut() -> bool) -> bool {
        let fim = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ok()
    }

    fn req(nome: &str, senha: Option<&str>) -> CreateRoom {
        CreateRoom {
            name: nome.into(),
            mode: RoomMode::Online,
            password: senha.map(String::from),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn create_sala_aberta_e_modelo_tem_o_dono() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let sala = OnlineRoom::create(&core.shared, req("Minha sala", None)).await.unwrap();
        {
            let m = sala.model.lock();
            assert_eq!(m.room.name, "Minha sala");
            assert_eq!(m.participants.len(), 1);
            assert_eq!(m.participants[0].id, sala.my_id);
            assert_eq!(m.room.owner_id, sala.my_id);
        }
        sala.leave().await;
        encerrar(core, d).await;
    }

    /// O dono sai: a sala é encerrada no servidor e ninguém mais entra pelo código.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dono_sai_e_a_sala_e_encerrada() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(srv.base_url.clone()).await;
        let (b, db) = novo_core(srv.base_url.clone()).await;
        let sala_a = OnlineRoom::create(&a.shared, req("Sala", None)).await.unwrap();
        assert!(sala_a.is_owner());
        let codigo = sala_a.code.clone();
        sala_a.leave().await;
        let res = OnlineRoom::join(&b.shared, codigo, None).await;
        assert!(res.is_err(), "a sala devia ter sido encerrada");
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn create_exige_modo_online() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let mut r = req("x", None);
        r.mode = RoomMode::Local;
        let res = OnlineRoom::create(&core.shared, r).await;
        assert!(matches!(res, Err(CoreError::Invalid(_))));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn join_por_codigo_segundo_cliente() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(srv.base_url.clone()).await;
        let (b, db) = novo_core(srv.base_url.clone()).await;
        let sala_a = OnlineRoom::create(&a.shared, req("Sala", None)).await.unwrap();
        let sala_b = OnlineRoom::join(&b.shared, sala_a.code.clone(), None).await.unwrap();
        assert!(ate(|| sala_a.model.lock().participants.len() == 2).await);
        assert!(ate(|| sala_b.model.lock().participants.len() == 2).await);
        sala_b.leave().await;
        sala_a.leave().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn join_senha_errada_senha_sala() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(srv.base_url.clone()).await;
        let (b, db) = novo_core(srv.base_url.clone()).await;
        let sala_a = OnlineRoom::create(&a.shared, req("Sala", Some("segredo")))
            .await
            .unwrap();
        let r = OnlineRoom::join(&b.shared, sala_a.code.clone(), Some("errada".into())).await;
        assert!(matches!(r, Err(CoreError::Server { ref code, .. }) if code == "senha_sala"));
        let ok = OnlineRoom::join(&b.shared, sala_a.code.clone(), Some("segredo".into())).await;
        assert!(ok.is_ok());
        if let Ok(s) = ok {
            s.leave().await;
        }
        sala_a.leave().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn join_codigo_inexistente() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let codigo = RoomCode::parse("ZAP-2222").unwrap();
        let r = OnlineRoom::join(&core.shared, codigo, None).await;
        assert!(matches!(r, Err(CoreError::Server { .. })), "{:?}", r.err());
        // A falha não deixa rota pendurada.
        assert!(core.shared.online.route.lock().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn chat_vai_e_volta() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(srv.base_url.clone()).await;
        let (b, db) = novo_core(srv.base_url.clone()).await;
        let sala_a = OnlineRoom::create(&a.shared, req("Sala", None)).await.unwrap();
        let sala_b = OnlineRoom::join(&b.shared, sala_a.code.clone(), None).await.unwrap();
        sala_a.send_chat("olá".into()).await.unwrap();
        assert!(ate(|| sala_b.model.lock().chat.iter().any(|l| l.text == "olá")).await);
        assert!(ate(|| sala_a.model.lock().chat.iter().any(|l| l.text == "olá")).await);
        sala_b.send_typing().await.unwrap();
        // O segundo aviso dentro de 3 s é descartado sem erro.
        sala_b.send_typing().await.unwrap();
        sala_b.leave().await;
        sala_a.leave().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn chat_vazio_ou_longo_invalido() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let sala = OnlineRoom::create(&core.shared, req("Sala", None)).await.unwrap();
        assert!(matches!(
            sala.send_chat(String::new()).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            sala.send_chat("a".repeat(1001)).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(sala.send_chat("a".repeat(1000)).await.is_ok());
        sala.leave().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn leave_remove_participante() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(srv.base_url.clone()).await;
        let (b, db) = novo_core(srv.base_url.clone()).await;
        let sala_a = OnlineRoom::create(&a.shared, req("Sala", None)).await.unwrap();
        let sala_b = OnlineRoom::join(&b.shared, sala_a.code.clone(), None).await.unwrap();
        assert!(ate(|| sala_a.model.lock().participants.len() == 2).await);
        sala_b.leave().await;
        assert!(ate(|| sala_a.model.lock().participants.len() == 1).await);
        sala_a.leave().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn leave_idempotente() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let sala = OnlineRoom::create(&core.shared, req("Sala", None)).await.unwrap();
        sala.leave().await;
        sala.leave().await;
        assert!(sala.left.load(Ordering::SeqCst));
        assert!(sala.tasks.lock().is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn leave_limpa_pending_e_conns() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let sala = OnlineRoom::create(&core.shared, req("Sala", None)).await.unwrap();
        let track: zap_proto::Track = serde_json::from_value(serde_json::json!({
            "id": "t1", "participante_id": "p2", "numero": 1, "tipo": "tela", "titulo": "t"
        }))
        .unwrap();
        sala.direct.pending.lock().insert(
            7,
            crate::subscribe::WatchSpec {
                track,
                audio_track: None,
                tile: 7,
                size: zap_proto::TileSize::Main,
                max_layer: None,
            },
        );
        assert!(!sala.direct.pending.lock().is_empty());
        sala.leave().await;
        assert!(sala.direct.pending.lock().is_empty());
        assert!(sala.direct.conns.lock().is_empty());
        encerrar(core, d).await;
    }
}
