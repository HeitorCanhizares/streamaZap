//! Conexões até os publicadores (uma por participante, compartilhada pelas suas trilhas).

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::Mutex as AsyncMutex;
use zap_net::control::{ControlRx, ControlTx};
use zap_net::endpoint::{bind_server, client_endpoint};
use zap_net::handshake::{accept_handshake_with, connect_handshake, Hello, ServerAuth};
use zap_net::media_rx::spawn_receiver;
use zap_net::online::WsTx;
use zap_net::report::{spawn_reporter, ReportBuilder, ReportBuilderHandle};
use zap_net::tls::client_config;
use zap_net::Clock;
use zap_proto::{ControlMsg, MediaAddress, TileSize};

use super::router::Router;
use crate::engine::Shared;
use crate::ticket::make_ticket;
use crate::util;
use crate::CoreError;

/// Tempo para a conexão direta antes de cair na reversa.
const TEMPO_DIRETO: Duration = Duration::from_secs(6);
/// Tempo de espera pela conexão do publicador na rota reversa.
const TEMPO_REVERSO: Duration = Duration::from_secs(12);

pub struct PubConn {
    pub participant_id: String,
    pub(crate) conn: quinn::Connection,
    pub(crate) ctrl: ControlTx,
    pub router: Arc<Router>,
    pub report: ReportBuilderHandle,
    pub(crate) tasks: parking_lot::Mutex<Vec<tokio::task::JoinHandle<()>>>,
    /// Último `camada.atual` recebido do publicador.
    #[allow(dead_code)] // escrito pela tarefa de controle; leitura reservada à interface
    pub(crate) last_layer: Arc<Mutex<Option<ControlMsg>>>,
    /// Handle do runtime, para `request_keyframe` ser síncrono.
    handle: tokio::runtime::Handle,
    /// Conexão da ponte Online: `assinar`/`cancelar`/`keyframe.pedir` vão pelo WebSocket
    /// (`Some`) em vez do stream de controle QUIC. `None` = modo Local e rota direta.
    pub(crate) signal: Option<WsTx>,
    /// Só na `PubConn` da ponte: trilhas que passaram para a rota direta (`track_id` → conexão
    /// direta ao publicador). `subscribe`/`unsubscribe`/`request_keyframe` dessas trilhas vão
    /// pela conexão direta (stream de controle QUIC), não pelo WebSocket.
    pub(crate) direct: Mutex<HashMap<String, Arc<PubConn>>>,
}

/// Conecta direto em algum dos endereços; o primeiro que completar vence, dentro de `tempo`.
async fn conecta_direto(addrs: Vec<SocketAddr>, fingerprint: &str, tempo: Duration) -> Option<quinn::Connection> {
    if addrs.is_empty() {
        return None;
    }
    let ep = client_endpoint().ok()?;
    let mut set = tokio::task::JoinSet::new();
    for addr in addrs {
        let Ok(cfg) = client_config(Some(fingerprint)) else {
            continue;
        };
        let Ok(conectando) = ep.connect_with(cfg, addr, "zap") else {
            continue;
        };
        set.spawn(conectando);
    }
    let espera = async {
        while let Some(r) = set.join_next().await {
            if let Ok(Ok(conn)) = r {
                return Some(conn);
            }
        }
        None
    };
    // Ao sair, o JoinSet aborta as tentativas restantes.
    tokio::time::timeout(tempo, espera).await.ok().flatten()
}

/// Entrega ao tratamento da sala Online (`room_events::handle_media_msg`) as mensagens de mídia
/// que o servidor manda pelo controle QUIC da ponte (`camadas.pedidas`, `keyframe.pedir`,
/// `camada.atual`): elas não vêm pelo WebSocket.
pub(crate) type MediaHook = Arc<dyn Fn(ControlMsg) + Send + Sync>;

/// Para onde vai um pedido de `assinar`/`cancelar`/`keyframe.pedir` de uma trilha.
enum Target {
    /// Trilha na rota direta: stream de controle da conexão direta.
    Direct(Arc<PubConn>),
    /// Ponte Online: WebSocket.
    Signal(WsTx),
    /// Modo Local: stream de controle desta conexão.
    Control,
}

impl PubConn {
    /// Monta a conexão: receptor de mídia, roteador, reporter e leitor do controle.
    /// `on_media` (só na ponte Online) recebe `camadas.pedidas`, `keyframe.pedir` e `camada.atual`.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        participant_id: String,
        conn: quinn::Connection,
        ctrl: ControlTx,
        mut crx: ControlRx,
        clock: Clock,
        signal: Option<WsTx>,
        router: Option<Arc<Router>>,
        on_media: Option<MediaHook>,
    ) -> Arc<Self> {
        let report: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let (etx, erx) = tokio::sync::mpsc::channel(1024);
        let router = router.unwrap_or_else(|| Arc::new(Router::new()));
        let last_layer = Arc::new(Mutex::new(None));
        let mut tasks = Vec::new();
        tasks.push(spawn_receiver(conn.clone(), clock.clone(), etx, report.clone()));
        tasks.push(tokio::spawn(router.clone().run(erx)));
        tasks.push(spawn_reporter(ctrl.clone(), report.clone(), clock));
        let (ctrl2, ll) = (ctrl.clone(), last_layer.clone());
        tasks.push(tokio::spawn(async move {
            while let Ok(Some(msg)) = crx.recv().await {
                match msg {
                    ControlMsg::Ping { ts_ms } => {
                        let _ = ctrl2.send(&ControlMsg::Pong { ts_ms }).await;
                    }
                    m @ ControlMsg::LayerCurrent { .. } => {
                        *ll.lock() = Some(m.clone());
                        if let Some(h) = &on_media {
                            h(m);
                        }
                    }
                    m @ (ControlMsg::LayersRequested { .. } | ControlMsg::KeyframeRequest { .. }) => {
                        if let Some(h) = &on_media {
                            h(m);
                        }
                    }
                    _ => {}
                }
            }
        }));
        Arc::new(Self {
            participant_id,
            conn,
            ctrl,
            router,
            report,
            tasks: parking_lot::Mutex::new(tasks),
            last_layer,
            handle: tokio::runtime::Handle::current(),
            signal,
            direct: Mutex::new(HashMap::new()),
        })
    }

    /// Ponte Online: monta a `PubConn` a partir da conexão já autenticada de
    /// `online::bridge::connect_bridge` (`assemble` + `signal = Some(ws)`). Não passa por
    /// `ConnPool`; quem chama (`online::reconnect`) a guarda em `OnlineRoom::bridge` e em
    /// `ConnPool::online`. `participant_id` = `"bridge"`. `on_media`: ver `MediaHook`.
    pub(crate) fn connect_online(
        shared: &Arc<Shared>,
        link: crate::online::bridge::BridgeConnected,
        ws: zap_net::online::WsTx,
        on_media: Option<MediaHook>,
    ) -> Arc<Self> {
        Self::assemble(
            "bridge".to_owned(),
            link.conn,
            link.ctrl,
            link.crx,
            shared.clock.clone(),
            Some(ws),
            None,
            on_media,
        )
    }

    /// Rota direta (espectador): conecta em `addresses` ("ip:porta") em paralelo por
    /// `online::direct::DIRECT_TIMEOUT` com `client_config(Some(fingerprint))`, faz
    /// `connect_handshake` com `Hello{ticket: Some(ticket), room_id}` e devolve a `PubConn`
    /// A conexão direta **reaproveita o `Router` da ponte** (`router`): o `VideoRx` já registrado
    /// recebe quadros de qualquer das duas conexões. `signal = None` (a direta fala `assinar`
    /// pelo próprio stream de controle). `None` se ninguém respondeu.
    pub(crate) async fn connect_direct(
        shared: &Arc<Shared>,
        addresses: &[String],
        fingerprint: &str,
        ticket: &str,
        room_id: &str,
        router: Arc<Router>,
    ) -> Option<Arc<Self>> {
        // Endereços inválidos são ignorados.
        let addrs: Vec<SocketAddr> = addresses.iter().filter_map(|a| a.parse().ok()).collect();
        let conn = conecta_direto(addrs, fingerprint, crate::online::direct::DIRECT_TIMEOUT).await?;
        let hello = {
            let s = shared.settings.read();
            Hello {
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                install_id: s.install_id.clone(),
                name: s.name.clone(),
                color: s.color.clone(),
                ticket: Some(ticket.to_owned()),
                room_id: Some(room_id.to_owned()),
                preview: None,
            }
        };
        let (ctrl, crx, welcome) = match connect_handshake(&conn, hello, None).await {
            Ok(r) => r,
            Err(_) => {
                conn.close(0u32.into(), b"falha");
                return None;
            }
        };
        Some(Self::assemble(
            welcome.participant_id,
            conn,
            ctrl,
            crx,
            shared.clock.clone(),
            None,
            Some(router),
            None,
        ))
    }

    /// Tenta conectar em cada `address.addresses` (em paralelo, primeiro que conectar vence) por
    /// 6 s com `client_config(Some(fingerprint))`; `connect_handshake` com
    /// `Hello{ticket: Some(make_ticket(secret, my_id)), room_id, ..}`; sobe `spawn_receiver`
    /// (canal de 1024 eventos → `Router::run`), `spawn_reporter`, e a tarefa que lê o controle
    /// do publicador (`camada.atual` → `TileMetrics::set_reason`/layer; `Pong`).
    /// Se não conectar em 6 s: conexão reversa — `bind_server(identity, 0)`, token aleatório,
    /// `room.request_reverse(track_id, "ip:porta", token)` e espera por 12 s a conexão do
    /// publicador com `accept_handshake_with(ServerAuth::Ticket(|t| t == token))`.
    /// Falha total → `CoreError::Connect`.
    pub async fn connect(
        shared: &Arc<Shared>,
        participant_id: &str,
        track_id: &str,
        address: &MediaAddress,
    ) -> Result<Arc<Self>, CoreError> {
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let secret = room.media_secret();
        let my_id = room.my_id();
        let model = room.model();
        let room_model = model.lock().room.clone();
        let ticket = make_ticket(&secret, &my_id);
        let (install_id, name, color) = {
            let s = shared.settings.read();
            (s.install_id.clone(), s.name.clone(), s.color.clone())
        };
        let hello = Hello {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            install_id,
            name,
            color,
            ticket: Some(ticket),
            room_id: Some(room_model.id.clone()),
            preview: None,
        };

        let (conn, ctrl, crx) = if let Some(conn) = conecta_direto(
            address
                .addresses
                .iter()
                .filter_map(|a| a.parse::<IpAddr>().ok())
                .map(|ip| SocketAddr::new(ip, address.port))
                .collect(),
            &address.fingerprint,
            TEMPO_DIRETO,
        )
        .await
        {
            let (ctrl, crx, _welcome) = connect_handshake(&conn, hello, None).await?;
            (conn, ctrl, crx)
        } else {
            // Sem rota direta: pede ao publicador que se conecte a nós.
            let (ep, port) = bind_server(&shared.identity, 0).await?;
            let token = util::new_id("r_");
            let (interface, loopback) = (
                shared.settings.read().local.interface.clone(),
                shared.options.include_loopback,
            );
            let ip = util::local_addresses(interface.as_deref(), loopback)
                .into_iter()
                .next()
                .ok_or_else(|| CoreError::Connect("sem rota para o publicador".into()))?;
            let endereco = format!("{ip}:{port}");
            room.request_reverse(track_id, &endereco, &token).await?;
            let tk = token.clone();
            let aceita = async {
                let incoming = ep.accept().await?;
                let conn = incoming.await.ok()?;
                let auth = ServerAuth::Ticket(Box::new(move |t| t == tk));
                let (ctrl, crx, _hello) = accept_handshake_with(&conn, &auth, &room_model, my_id.clone(), None)
                    .await
                    .ok()?;
                Some((conn, ctrl, crx))
            };
            tokio::time::timeout(TEMPO_REVERSO, aceita)
                .await
                .ok()
                .flatten()
                .ok_or_else(|| CoreError::Connect("sem rota para o publicador".into()))?
        };
        Ok(Self::assemble(
            participant_id.to_owned(),
            conn,
            ctrl,
            crx,
            shared.clock.clone(),
            None,
            None,
            None,
        ))
    }

    /// Decide por onde o pedido da trilha vai (função pura, testável).
    fn target_for(&self, track_id: &str) -> Target {
        if let Some(d) = self.direct.lock().get(track_id).cloned() {
            return Target::Direct(d);
        }
        match &self.signal {
            Some(ws) => Target::Signal(ws.clone()),
            None => Target::Control,
        }
    }

    pub async fn subscribe(
        &self,
        track_id: &str,
        size: TileSize,
        max_layer: Option<u8>,
        audio: bool,
    ) -> Result<(), CoreError> {
        let msg = ControlMsg::Subscribe {
            track_id: track_id.to_owned(),
            size,
            max_layer,
            audio: Some(audio),
        };
        self.enviar(track_id, msg).await
    }
    pub async fn unsubscribe(&self, track_id: &str) -> Result<(), CoreError> {
        let msg = ControlMsg::Unsubscribe {
            track_id: track_id.to_owned(),
        };
        self.enviar(track_id, msg).await
    }
    pub fn request_keyframe(&self, track_id: &str) {
        let alvo = self.target_for(track_id);
        let ctrl = self.ctrl.clone();
        let track_id = track_id.to_owned();
        // Chamável fora do runtime: usa o handle guardado na criação.
        self.handle.spawn(async move {
            let msg = ControlMsg::KeyframeRequest { track_id, layer: None };
            match alvo {
                Target::Direct(d) => {
                    let _ = d.ctrl.send(&msg).await;
                }
                Target::Signal(ws) => {
                    let _ = ws.send(msg).await;
                }
                Target::Control => {
                    let _ = ctrl.send(&msg).await;
                }
            }
        });
    }
    /// Envia `msg` pelo canal certo para a trilha.
    async fn enviar(&self, track_id: &str, msg: ControlMsg) -> Result<(), CoreError> {
        match self.target_for(track_id) {
            Target::Direct(d) => d.ctrl.send(&msg).await?,
            Target::Signal(ws) => ws.send(msg).await.map_err(|e| CoreError::Connect(e.to_string()))?,
            Target::Control => self.ctrl.send(&msg).await?,
        }
        Ok(())
    }
    /// Cancela as tarefas e fecha a conexão.
    pub fn close(&self) {
        for t in self.tasks.lock().drain(..) {
            t.abort();
        }
        let diretas: Vec<Arc<PubConn>> = self.direct.lock().drain().map(|(_, d)| d).collect();
        for d in diretas {
            d.close();
        }
        self.conn.close(0u32.into(), b"fim");
    }
    pub fn is_closed(&self) -> bool {
        self.conn.close_reason().is_some()
    }
}

#[derive(Default)]
pub struct ConnPool {
    pub(crate) map: AsyncMutex<HashMap<String, Arc<PubConn>>>,
    /// Ponte da sala Online (uma conexão para todas as trilhas); `get_or_connect` a devolve
    /// quando a sala atual é Online.
    pub(crate) online: parking_lot::Mutex<Option<Arc<PubConn>>>,
}

impl ConnPool {
    /// Reaproveita a conexão do participante se ainda aberta; senão `PubConn::connect`.
    pub async fn get_or_connect(
        &self,
        shared: &Arc<Shared>,
        participant_id: &str,
        track_id: &str,
        address: Option<&MediaAddress>,
    ) -> Result<Arc<PubConn>, CoreError> {
        if matches!(
            shared.current_room().as_deref(),
            Some(crate::room::ActiveRoom::Online(_))
        ) {
            return self.online_conn();
        }
        let address = address.ok_or(CoreError::TrackNotFound)?;
        // A trava fica durante a conexão: evita duas conexões simultâneas ao mesmo participante.
        let mut map = self.map.lock().await;
        if let Some(c) = map.get(participant_id) {
            if !c.is_closed() {
                return Ok(c.clone());
            }
        }
        let c = PubConn::connect(shared, participant_id, track_id, address).await?;
        map.insert(participant_id.to_owned(), c.clone());
        Ok(c)
    }
    /// Ponte da sala Online, se ainda aberta.
    fn online_conn(&self) -> Result<Arc<PubConn>, CoreError> {
        match self.online.lock().clone() {
            Some(c) if !c.is_closed() => Ok(c),
            _ => Err(CoreError::Connect("ponte indisponível".into())),
        }
    }
    pub async fn close_all(&self) {
        let ponte = self.online.lock().take();
        if let Some(c) = ponte {
            c.close();
        }
        let mut map = self.map.lock().await;
        for (_, c) in map.drain() {
            c.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_net::tls::Identity;

    #[tokio::test(flavor = "multi_thread")]
    async fn pubconn_is_closed_false_em_conexao_nova() {
        let id = Identity::generate("zap").expect("id");
        let (ep, porta) = bind_server(&id, 0).await.expect("bind");
        let srv = tokio::spawn(async move {
            let conn = ep.accept().await.expect("incoming").await.expect("conn");
            // Mantém a conexão viva até o cliente fechar.
            let _ = conn.accept_bi().await;
            conn.closed().await;
        });
        let cep = client_endpoint().expect("cliente");
        let cfg = client_config(Some(&id.fingerprint())).expect("cfg");
        let conn = cep
            .connect_with(cfg, SocketAddr::from(([127, 0, 0, 1], porta)), "zap")
            .expect("connect")
            .await
            .expect("conexão");
        let (send, recv) = conn.open_bi().await.expect("bi");
        let ctrl = ControlTx::new(send);
        // Abre o stream de verdade para o servidor aceitar.
        ctrl.send(&ControlMsg::Ping { ts_ms: 1 }).await.expect("ping");
        let pc = PubConn::assemble(
            "p1".into(),
            conn,
            ctrl,
            ControlRx::new(recv),
            Clock::new(),
            None,
            None,
            None,
        );
        assert!(!pc.is_closed());
        pc.close();
        assert!(pc.is_closed());
        srv.abort();
    }

    /// Monta uma `PubConn` real contra um servidor QUIC mínimo (só mantém a conexão aberta).
    async fn pubconn_real(signal: Option<WsTx>) -> Arc<PubConn> {
        let id = Identity::generate("zap").expect("id");
        let (ep, porta) = bind_server(&id, 0).await.expect("bind");
        tokio::spawn(async move {
            let conn = ep.accept().await.expect("incoming").await.expect("conn");
            let _ = conn.accept_bi().await;
            conn.closed().await;
        });
        let cep = client_endpoint().expect("cliente");
        let cfg = client_config(Some(&id.fingerprint())).expect("cfg");
        let conn = cep
            .connect_with(cfg, SocketAddr::from(([127, 0, 0, 1], porta)), "zap")
            .expect("connect")
            .await
            .expect("conexão");
        let (send, recv) = conn.open_bi().await.expect("bi");
        let ctrl = ControlTx::new(send);
        ctrl.send(&ControlMsg::Ping { ts_ms: 1 }).await.expect("ping");
        PubConn::assemble(
            "p".into(),
            conn,
            ctrl,
            ControlRx::new(recv),
            Clock::new(),
            signal,
            None,
            None,
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn target_for_prefere_direta() {
        let ponte = pubconn_real(None).await;
        assert!(matches!(ponte.target_for("t1"), Target::Control));
        let direta = pubconn_real(None).await;
        ponte.direct.lock().insert("t1".into(), direta.clone());
        assert!(matches!(ponte.target_for("t1"), Target::Direct(_)));
        assert!(matches!(ponte.target_for("t2"), Target::Control));
        ponte.close();
        assert!(direta.is_closed(), "close() da ponte fecha as diretas");
    }

    async fn login(srv: &zap_test_server::Handle) -> zap_net::online::ServerClient {
        zap_net::online::ServerClient::login(
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
        .expect("login")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_online_assina_pelo_websocket() {
        use crate::engine::{Core, CoreOptions};
        use zap_proto::{Access, LayerSpec, TrackKind};

        let srv = zap_test_server::spawn(zap_test_server::Config::default())
            .await
            .expect("srv");
        let fp = srv.cert_fingerprint.clone();
        let dir = std::env::temp_dir().join(format!("zapcore-pool-{}", uuid::Uuid::new_v4()));
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

        // Cliente A (o app) e cliente B (publicador).
        let a = login(&srv).await;
        let sala = a
            .create_room(zap_net::online::CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            })
            .await
            .expect("sala");
        let join_a = a.join(&sala.code, None).await.expect("join a");
        let (wtx_a, mut wrx_a) = a.connect_ws().await.expect("ws a");
        wtx_a
            .send(ControlMsg::RoomJoin {
                room_id: join_a.room.id.clone(),
            })
            .await
            .expect("room join a");
        let ponte_a = join_a.media.bridge.as_ref().expect("ponte").address.clone();
        let link =
            crate::online::bridge::connect_bridge(&core.shared, &ponte_a, &join_a.ticket, &join_a.room.id, false)
                .await
                .expect("bridge");
        let conn = PubConn::connect_online(&core.shared, link, wtx_a.clone(), None);
        assert!(matches!(conn.target_for("qualquer"), Target::Signal(_)));

        let b = login(&srv).await;
        let join_b = b.join(&sala.code, None).await.expect("join b");
        let (wtx_b, mut wrx_b) = b.connect_ws().await.expect("ws b");
        wtx_b
            .send(ControlMsg::RoomJoin {
                room_id: join_b.room.id.clone(),
            })
            .await
            .expect("room join b");
        wtx_b
            .send(ControlMsg::TrackPublish {
                local_id: "l1".into(),
                kind: TrackKind::Screen,
                title: "tela".into(),
                layers: vec![LayerSpec {
                    layer: 0,
                    width: 1280,
                    height: 720,
                    fps: 30,
                    kbps: 2000,
                }],
                audio: None,
                address: None,
            })
            .await
            .expect("publish");
        // Descobre a trilha publicada por B (chega no WebSocket de A ou de B).
        let track_id = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                tokio::select! {
                    m = wrx_a.recv() => if let Some(Ok(ControlMsg::TrackPublished { track, .. })) = m { return track.id; },
                    m = wrx_b.recv() => if let Some(Ok(ControlMsg::TrackPublished { track, .. })) = m { return track.id; },
                }
            }
        })
        .await
        .expect("trilha publicada");

        conn.subscribe(&track_id, TileSize::Main, None, false)
            .await
            .expect("assinar");
        let rota = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match wrx_a.recv().await {
                    Some(Ok(ControlMsg::MediaRoute { route, .. })) => return route,
                    Some(_) => {}
                    None => panic!("ws de A fechou"),
                }
            }
        })
        .await
        .expect("midia.rota");
        assert_eq!(rota, zap_proto::control::Route::Relay);

        conn.close();
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(dir);
        })
        .await
        .expect("join");
    }

    #[test]
    fn online_conn_sem_ponte_erro() {
        let pool = ConnPool::default();
        assert!(matches!(pool.online_conn(), Err(CoreError::Connect(_))));
    }

    #[tokio::test]
    async fn pool_vazio_close_all_ok() {
        let pool = ConnPool::default();
        pool.close_all().await;
        assert!(pool.map.lock().await.is_empty());
    }
}
