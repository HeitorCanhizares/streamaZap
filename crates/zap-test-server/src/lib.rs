//! Servidor Online mínimo, em memória, do StreamaZap. **Não é para produção.**
//!
//! É a implementação de referência da API para quem for escrever o servidor de produção:
//! cada módulo explica qual trecho de `docs/api/` ele cumpre.
//!
//! | Módulo | Papel | Doc |
//! |---|---|---|
//! | `rest` / `rest_rooms` | REST (axum) | `docs/api/rest.md` |
//! | `ws` / `signaling` / `tracks` | WebSocket de sinalização | `docs/api/mensagens.md` |
//! | `bridge` / `media` | Ponte QUIC `zap/4` | `docs/api/midia.md` |
//! | `relay` / `routing` / `ticker` | Alocador de camadas por espectador | `docs/api/balanceamento.md` |
//! | `probe` | Necessidade de sondagem por espectador (a ponte sonda a banda) | `docs/api/balanceamento.md` §4 |
//! | `state` / `rooms` / `util` | Estado em memória | |
//!
//! Decisões de projeto do servidor de teste:
//! - REST e WebSocket em **HTTP puro** no loopback (o app aceita `http://` no endereço); só o QUIC
//!   usa certificado autoassinado, e `Handle::cert_fingerprint` é a impressão digital a fixar.
//! - Tudo fica em memória (`state::Hub`, um `Mutex`); sem persistência e sem limites de taxa.
//! - Canais como no servidor de produção (`docs/api/mensagens.md`, "Canal de cada mensagem"):
//!   `camadas.pedidas`, `camada.atual`, `keyframe.pedir` e `relatorio` saem só pelo stream de
//!   controle QUIC; eventos de sala e chat, pelo WebSocket; a resposta a um pedido volta pelo canal
//!   do pedido.
//!
//! O que o servidor de teste **não** faz (e o de produção faz): rota direta (`p2p.*`), latência
//! dos participantes, `sala.saiu`, eventos de sala pelo QUIC quando não há WebSocket, `{codigo}`
//! como id `s_…`, limites de taxa e o `camadas.pedidas` vazio logo depois de `trilha.publicada`.
#![forbid(unsafe_code)]

pub mod bridge;
pub mod media;
pub mod notify;
pub mod probe;
pub mod relay;
pub mod rest;
pub mod rest_rooms;
pub mod rooms;
pub mod rooms_join;
pub mod routing;
pub mod signaling;
pub mod state;
pub mod subs;
pub mod ticker;
pub mod tracks;
pub mod util;
pub mod ws;

use std::{net::SocketAddr, sync::Arc};

use axum::serve;

pub use state::Hub;

/// Erros de `spawn`.
#[derive(Debug, thiserror::Error)]
pub enum TestServerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("rede: {0}")]
    Net(#[from] zap_net::NetError),
}

/// Configuração do servidor de teste.
#[derive(Debug, Clone)]
pub struct Config {
    /// `nome` de `GET /v1/info`.
    pub name: String,
    /// Se `Some`, `POST /v1/sessoes` exige `senha_servidor` igual (`requer_senha: true`).
    pub server_password: Option<String>,
    /// `ponte` de `GET /v1/info`. Com `false` o QUIC nem é aberto para mídia.
    pub relay: bool,
    /// Limite de participantes por sala (`sala_cheia` ao passar).
    pub participants_per_room: u32,
    /// `midia.direta_permitida` na resposta de `entrar`.
    pub direct_allowed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Servidor de teste".into(),
            server_password: None,
            relay: true,
            participants_per_room: 16,
            direct_allowed: true,
        }
    }
}

/// Servidor em execução. Ao soltar o `Handle` as tarefas são abortadas.
pub struct Handle {
    /// `http://127.0.0.1:<porta>` (sem barra final).
    pub base_url: String,
    /// Endereço UDP da ponte QUIC.
    pub quic_addr: SocketAddr,
    /// SHA-256 hex do certificado da ponte (`--dev-cert-fp`).
    pub cert_fingerprint: String,
    /// Estado compartilhado (testes podem inspecionar).
    pub hub: Arc<Hub>,
    pub(crate) tasks: Vec<tokio::task::JoinHandle<()>>,
    /// Timer de 1 ms enquanto o servidor viver (ritmo das sondas e do ticker no Windows).
    pub(crate) _timer: zap_sys::platform::TimerResolution,
}

impl Drop for Handle {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

/// Sobe REST+WS em `127.0.0.1:0`, a ponte QUIC em `127.0.0.1:0` e o ticker de 100 ms.
pub async fn spawn(config: Config) -> Result<Handle, TestServerError> {
    let timer = zap_sys::platform::TimerResolution::high();
    let hub = Arc::new(Hub::new(config));
    let identity = zap_net::tls::Identity::generate("zap-test-server")?;
    let cert_fingerprint = identity.fingerprint();

    // QUIC antes do HTTP: `bridge_addr` já está preenchido quando `/v1/info` responder.
    let (endpoint, quic_addr, bridge_task) = bridge::start(hub.clone(), &identity).await?;
    // O endpoint precisa ficar vivo enquanto o `Handle` existir.
    let keeper = tokio::spawn(async move {
        let _endpoint = endpoint;
        std::future::pending::<()>().await;
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let app = rest::router(hub.clone());
    let http_task = tokio::spawn(async move {
        if let Err(e) = serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
            tracing::warn!("servidor HTTP de teste parou: {e}");
        }
    });
    let ticker_task = tokio::spawn(ticker::run(hub.clone()));

    Ok(Handle {
        base_url,
        quic_addr,
        cert_fingerprint,
        hub,
        tasks: vec![bridge_task, keeper, http_task, ticker_task],
        _timer: timer,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_sobe() {
        let h = spawn(Config::default()).await.unwrap();
        assert!(h.base_url.starts_with("http://127.0.0.1:"));
        assert_ne!(h.quic_addr.port(), 0);
        assert_eq!(h.cert_fingerprint.len(), 64);
        assert!(h.cert_fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn info_responde() {
        let h = spawn(Config::default()).await.unwrap();
        let info = zap_net::online::ServerClient::info(&h.base_url).await.unwrap();
        assert_eq!(info.protocol_min, 4);
        assert_eq!(info.quic.port, h.quic_addr.port());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn drop_aborta() {
        let h = spawn(Config::default()).await.unwrap();
        let addr = h.base_url.trim_start_matches("http://").to_string();
        assert!(tokio::net::TcpStream::connect(&addr).await.is_ok());
        drop(h);
        // A tarefa do HTTP é abortada e o listener fecha; dá até 1 s para o SO soltar a porta.
        let mut fechou = false;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if tokio::net::TcpStream::connect(&addr).await.is_err() {
                fechou = true;
                break;
            }
        }
        assert!(fechou, "a porta HTTP continuou aceitando conexões");
    }
}
