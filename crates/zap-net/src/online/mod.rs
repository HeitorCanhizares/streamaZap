//! Cliente do servidor Online: REST (`docs/api/rest.md`) e WebSocket (`docs/api/mensagens.md`).
mod client;
mod error;
mod refresh;
mod types;
mod ws;

pub use client::ServerClient;
pub use error::ApiError;
pub use types::*;
pub use ws::{WsRx, WsTx};
