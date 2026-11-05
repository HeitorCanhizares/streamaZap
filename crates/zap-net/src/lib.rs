//! Rede do StreamaZap: conexão QUIC `zap/4` (controle, quadros de vídeo com prazo, datagramas
//! de áudio e sonda), relatórios do receptor e descoberta de salas na LAN/Radmin.
//!
//! Especificação: `docs/api/midia.md` e `docs/api/mensagens.md`.
#![forbid(unsafe_code)]

pub mod clock;
pub mod control;
pub mod discovery;
pub mod endpoint;
pub mod error;
pub mod handshake;
pub mod media_rx;
pub mod media_tx;
pub mod online;
pub mod probe;
pub mod report;
pub mod tls;

pub use clock::Clock;
pub use error::NetError;
