//! Endpoint QUIC por onde os outros assistem as minhas trilhas.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use std::time::Duration;

use parking_lot::Mutex;
use zap_net::endpoint::{bind_server, client_endpoint};
use zap_net::handshake::{accept_handshake_with, connect_handshake, Hello, ServerAuth};
use zap_net::tls::{client_config, Identity};
use zap_proto::{MediaAddress, Room};

use super::media_serve::serve_viewer;
use super::{AudioPublisher, VideoPublisher};
use crate::ticket::verify_ticket;
use crate::{util, CoreError};

#[derive(Clone)]
pub enum PublishedTrack {
    Video(Arc<VideoPublisher>),
    Audio(Arc<AudioPublisher>),
}

/// Quem pode assistir: espectadores apresentam `ingresso` (ver `ticket`).
#[derive(Clone)]
pub struct MediaAuth {
    pub room: Room,
    pub secret: String,
    /// Ids atuais dos participantes da sala.
    pub members: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
}

pub struct MediaServer {
    pub endpoint: quinn::Endpoint,
    pub port: u16,
    pub fingerprint: String,
    pub(crate) tracks: Mutex<HashMap<String, PublishedTrack>>,
    /// Trilha de áudio registrada (uma por participante) para o `assinar{audio:true}`.
    pub(crate) auth: Mutex<Option<MediaAuth>>,
    pub(crate) clock: zap_net::Clock,
    pub(crate) next_viewer: AtomicU64,
    // Mantém a identidade viva junto do servidor (posse), sem leitura direta.
    #[allow(dead_code)]
    pub(crate) identity: Arc<Identity>,
    pub(crate) rt: tokio::runtime::Handle,
}

impl MediaServer {
    /// `bind_server(&identity, preferred_port)` e tarefa de aceite: para cada conexão,
    /// `accept_handshake_with(.., ServerAuth::Ticket(verifica), room, "viewer", None)` onde
    /// `verifica` usa `auth` (`ticket::verify_ticket(secret, members(), t)`; sem `auth` = recusa),
    /// e então `media_serve::serve_viewer`.
    pub async fn start(
        identity: Arc<Identity>,
        preferred_port: u16,
        clock: zap_net::Clock,
        rt: tokio::runtime::Handle,
    ) -> Result<Arc<Self>, CoreError> {
        let (endpoint, port) = bind_server(&identity, preferred_port).await?;
        let fingerprint = identity.fingerprint();
        let server = Arc::new(MediaServer {
            endpoint: endpoint.clone(),
            port,
            fingerprint,
            tracks: Mutex::new(HashMap::new()),
            auth: Mutex::new(None),
            clock,
            next_viewer: AtomicU64::new(0),
            identity,
            rt: rt.clone(),
        });
        // Referência fraca: o laço não segura o servidor vivo; termina quando o endpoint fecha.
        let fraco = Arc::downgrade(&server);
        rt.spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let Some(server) = fraco.upgrade() else { break };
                tokio::spawn(async move {
                    let Ok(conn) = incoming.await else { return };
                    let auth = server.auth.lock().clone();
                    let Some(auth) = auth else {
                        conn.close(1u32.into(), b"sem_auth");
                        return;
                    };
                    let secret = auth.secret.clone();
                    let members = auth.members.clone();
                    let verifica = ServerAuth::Ticket(Box::new(move |t| verify_ticket(&secret, &members(), t)));
                    let r = accept_handshake_with(&conn, &verifica, &auth.room, "viewer".to_owned(), None).await;
                    if let Ok((tx, rx, hello)) = r {
                        serve_viewer(server.clone(), conn, tx, rx, hello.install_id).await;
                    }
                });
            }
        });
        Ok(server)
    }
    pub fn set_auth(&self, a: Option<MediaAuth>) {
        *self.auth.lock() = a;
    }
    pub fn register(&self, track_id: String, t: PublishedTrack) {
        self.tracks.lock().insert(track_id, t);
    }
    pub fn unregister(&self, track_id: &str) -> Option<PublishedTrack> {
        self.tracks.lock().remove(track_id)
    }
    /// Endereço para anunciar na trilha: `util::local_addresses`, `port`, `fingerprint`.
    pub fn address(&self, interface: Option<&str>, include_loopback: bool) -> MediaAddress {
        MediaAddress {
            addresses: util::local_addresses(interface, include_loopback),
            port: self.port,
            fingerprint: self.fingerprint.clone(),
        }
    }
    /// Conexão reversa: conecta em `addr` (pinando `fingerprint`), `connect_handshake` com
    /// `Hello{ticket: Some(token), ..}` (sem senha) e atende o espectador com `serve_viewer`.
    /// Timeout de 6 s no connect.
    pub async fn connect_back(
        self: &Arc<Self>,
        addr: SocketAddr,
        fingerprint: &str,
        token: &str,
    ) -> Result<(), CoreError> {
        let endpoint = client_endpoint()?;
        let config = client_config(Some(fingerprint))?;
        let conectando = endpoint
            .connect_with(config, addr, "zap")
            .map_err(|e| CoreError::Connect(e.to_string()))?;
        let conn = tokio::time::timeout(Duration::from_secs(6), conectando)
            .await
            .map_err(|_| CoreError::Connect("tempo esgotado".to_owned()))?
            .map_err(|e| CoreError::Connect(e.to_string()))?;
        let hello = Hello {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            install_id: "publisher".to_owned(),
            name: "publisher".to_owned(),
            color: "#000000".to_owned(),
            ticket: Some(token.to_owned()),
            room_id: None,
            preview: None,
        };
        let (tx, rx, _welcome) = connect_handshake(&conn, hello, None)
            .await
            .map_err(CoreError::from_handshake)?;
        let server = self.clone();
        let label = token.to_owned();
        self.rt.spawn(async move {
            // `endpoint` vive junto com a conexão: soltá-lo cedo fecharia o socket.
            let _endpoint = endpoint;
            serve_viewer(server, conn, tx, rx, label).await;
        });
        Ok(())
    }
    /// Fecha o endpoint e todas as conexões.
    pub fn shutdown(&self) {
        self.endpoint.close(0u32.into(), b"fim");
        self.tracks.lock().clear();
        *self.auth.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_net::NetError;
    use zap_proto::{Access, RoomMode};

    fn sala() -> Room {
        Room {
            id: "s_1".into(),
            code: None,
            name: "Corrida".into(),
            owner_id: "p_1".into(),
            access: Access::Open,
            mode: RoomMode::Online,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    async fn servidor() -> Result<Arc<MediaServer>, Box<dyn std::error::Error>> {
        let id = Arc::new(Identity::generate("zap")?);
        Ok(MediaServer::start(id, 0, zap_net::Clock::new(), tokio::runtime::Handle::current()).await?)
    }

    fn ola(ticket: Option<&str>) -> Hello {
        Hello {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            install_id: "i1".into(),
            name: "v".into(),
            color: "#111111".into(),
            ticket: ticket.map(str::to_owned),
            room_id: None,
            preview: None,
        }
    }

    async fn cliente(
        s: &MediaServer,
        ticket: Option<&str>,
    ) -> Result<Result<(), NetError>, Box<dyn std::error::Error>> {
        let ep = client_endpoint()?;
        let addr = SocketAddr::from(([127, 0, 0, 1], s.port));
        let conn = ep
            .connect_with(client_config(Some(&s.fingerprint))?, addr, "zap")?
            .await?;
        Ok(connect_handshake(&conn, ola(ticket), None).await.map(|_| ()))
    }

    fn com_auth(s: &MediaServer, secret: &str) {
        s.set_auth(Some(MediaAuth {
            room: sala(),
            secret: secret.to_owned(),
            members: Arc::new(|| vec!["p_1".to_owned()]),
        }));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_porta_e_fingerprint() -> Result<(), Box<dyn std::error::Error>> {
        let s = servidor().await?;
        assert_ne!(s.port, 0);
        assert!(!s.fingerprint.is_empty());
        let a = s.address(None, true);
        assert!(a.addresses.iter().any(|x| x == "127.0.0.1"));
        assert_eq!(a.port, s.port);
        s.shutdown();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn register_unregister() -> Result<(), Box<dyn std::error::Error>> {
        let s = servidor().await?;
        let a = AudioPublisher::start(1, crate::publish::AudioSource::Synthetic)?;
        s.register("t1".into(), PublishedTrack::Audio(a));
        assert!(s.unregister("t1").is_some());
        assert!(s.unregister("t1").is_none());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sem_auth_recusa_conexao() -> Result<(), Box<dyn std::error::Error>> {
        let s = servidor().await?;
        let ep = client_endpoint()?;
        let addr = SocketAddr::from(([127, 0, 0, 1], s.port));
        let conn = ep
            .connect_with(client_config(Some(&s.fingerprint))?, addr, "zap")?
            .await?;
        let r = connect_handshake(&conn, ola(Some("x")), None).await;
        assert!(r.is_err());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_correto_entra_incorreto_recusado() -> Result<(), Box<dyn std::error::Error>> {
        let s = servidor().await?;
        com_auth(&s, "segredo");
        let bom = crate::ticket::make_ticket("segredo", "p_1");
        assert!(cliente(&s, Some(&bom)).await?.is_ok());
        let ruim = crate::ticket::make_ticket("outro", "p_1");
        let r = cliente(&s, Some(&ruim)).await?;
        assert!(matches!(r, Err(NetError::Rejected(ref c)) if c == "ingresso"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connect_back_atende_espectador() -> Result<(), Box<dyn std::error::Error>> {
        let s = servidor().await?;
        let id = Identity::generate("zap")?;
        let (ep, porta) = bind_server(&id, 0).await?;
        let tarefa = tokio::spawn(async move {
            let incoming = ep.accept().await.ok_or("sem conexao")?;
            let conn = incoming.await?;
            let auth = ServerAuth::Ticket(Box::new(|t| t == "tok"));
            let (_tx, _rx, hello) = accept_handshake_with(&conn, &auth, &sala(), "p_9".into(), None).await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((hello, conn, ep))
        });
        let addr = SocketAddr::from(([127, 0, 0, 1], porta));
        s.connect_back(addr, &id.fingerprint(), "tok").await?;
        let (hello, _conn, _ep) = tarefa.await?.map_err(|e| e.to_string())?;
        assert_eq!(hello.ticket.as_deref(), Some("tok"));
        Ok(())
    }
}
