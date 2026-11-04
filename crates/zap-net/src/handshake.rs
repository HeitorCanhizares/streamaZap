//! Aperto de mão do stream de controle (docs/api/mensagens.md#aperto-de-mão-quic).

use zap_proto::Room;

mod client;
mod server;

pub use client::connect_handshake;
pub use server::{accept_handshake, accept_handshake_with};

/// Tempo máximo do aperto de mão.
pub const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub enum ServerAuth {
    Open,
    Password(String),
    /// Valida o `ingresso` de `ola`.
    Ticket(Box<dyn Fn(&str) -> bool + Send + Sync>),
}

/// Campos de `ola`.
#[derive(Clone, Debug, PartialEq)]
pub struct Hello {
    pub app_version: String,
    pub install_id: String,
    pub name: String,
    pub color: String,
    pub ticket: Option<String>,
    pub room_id: Option<String>,
    pub preview: Option<bool>,
}

/// Campos úteis de `bem_vindo`.
#[derive(Clone, Debug, PartialEq)]
pub struct Welcome {
    pub participant_id: String,
    pub room: Room,
    pub clock_us: Option<u64>,
    /// `segredo_midia` (base64 de 32 bytes), só no modo Local.
    pub media_secret: Option<String>,
}
