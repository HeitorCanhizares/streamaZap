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
    pub(crate) last_layer: Arc<Mutex<Option<ControlMsg>>>,
    /// Handle do runtime, para `request_keyframe` ser síncrono.
    handle: tokio::runtime::Handle,
}

/// Conecta direto em algum dos IPs; o primeiro que completar vence.
async fn conecta_direto(address: &MediaAddress) -> Option<quinn::Connection> {
    let ips: Vec<IpAddr> = address.addresses.iter().filter_map(|a| a.parse().ok()).collect();
    if ips.is_empty() {
        return None;
    }
    let ep = client_endpoint().ok()?;
    let mut set = tokio::task::JoinSet::new();
    for ip in ips {
        let Ok(cfg) = client_config(Some(&address.fingerprint)) else {
            continue;
        };
        let Ok(conectando) = ep.connect_with(cfg, SocketAddr::new(ip, address.port), "zap") else {
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
    tokio::time::timeout(TEMPO_DIRETO, espera).await.ok().flatten()
}

impl PubConn {
    /// Monta a conexão: receptor de mídia, roteador, reporter e leitor do controle.
    fn assemble(
        participant_id: String,
        conn: quinn::Connection,
        ctrl: ControlTx,
        mut crx: ControlRx,
        clock: Clock,
    ) -> Arc<Self> {
        let report: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let (etx, erx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(Router::new());
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
                    m @ ControlMsg::LayerCurrent { .. } => *ll.lock() = Some(m),
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
        })
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

        let (conn, ctrl, crx) = if let Some(conn) = conecta_direto(address).await {
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
        ))
    }
    pub async fn subscribe(
        &self,
        track_id: &str,
        size: TileSize,
        max_layer: Option<u8>,
        audio: bool,
    ) -> Result<(), CoreError> {
        self.ctrl
            .send(&ControlMsg::Subscribe {
                track_id: track_id.to_owned(),
                size,
                max_layer,
                audio: Some(audio),
            })
            .await?;
        Ok(())
    }
    pub async fn unsubscribe(&self, track_id: &str) -> Result<(), CoreError> {
        self.ctrl
            .send(&ControlMsg::Unsubscribe {
                track_id: track_id.to_owned(),
            })
            .await?;
        Ok(())
    }
    pub fn request_keyframe(&self, track_id: &str) {
        let ctrl = self.ctrl.clone();
        let track_id = track_id.to_owned();
        // Chamável fora do runtime: usa o handle guardado na criação.
        self.handle.spawn(async move {
            let _ = ctrl.send(&ControlMsg::KeyframeRequest { track_id, layer: None }).await;
        });
    }
    /// Cancela as tarefas e fecha a conexão.
    pub fn close(&self) {
        for t in self.tasks.lock().drain(..) {
            t.abort();
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
}

impl ConnPool {
    /// Reaproveita a conexão do participante se ainda aberta; senão `PubConn::connect`.
    pub async fn get_or_connect(
        &self,
        shared: &Arc<Shared>,
        participant_id: &str,
        track_id: &str,
        address: &MediaAddress,
    ) -> Result<Arc<PubConn>, CoreError> {
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
    pub async fn close_all(&self) {
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
        let pc = PubConn::assemble("p1".into(), conn, ctrl, ControlRx::new(recv), Clock::new());
        assert!(!pc.is_closed());
        pc.close();
        assert!(pc.is_closed());
        srv.abort();
    }

    #[tokio::test]
    async fn pool_vazio_close_all_ok() {
        let pool = ConnPool::default();
        pool.close_all().await;
        assert!(pool.map.lock().await.is_empty());
    }
}
