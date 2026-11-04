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
//! | `state` / `rooms` / `util` | Estado em memória | |
//!
//! Decisões de projeto do servidor de teste (registradas aqui para o servidor de produção):
//! - REST e WebSocket em **HTTP puro** no loopback (o app aceita `http://` no endereço); só o QUIC
//!   usa certificado autoassinado, e `Handle::cert_fingerprint` é a impressão digital a fixar.
//! - Tudo fica em memória (`state::Hub`, um `Mutex`); sem persistência e sem limites de taxa.
//! - `camadas.pedidas`, `camada.atual` e `keyframe.pedir` saem pelo WebSocket do participante.
#![forbid(unsafe_code)]

pub mod bridge;
pub mod media;
pub mod notify;
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
    let _ = config;
    todo!("tarefa zt-17")
}
