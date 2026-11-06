//! Tipos e formatos do protocolo 4 do StreamaZap.
//!
//! Fonte da verdade: `docs/api/`. Este crate não faz I/O e não depende de plataforma: só
//! descreve mensagens de controle (JSON), cabeçalhos binários de mídia e anúncios da descoberta.
#![forbid(unsafe_code)]

pub mod auth;
pub mod control;
pub mod discovery;
pub mod error;
pub mod framing;
pub mod ids;
pub mod media;
pub mod types;
pub mod version;

pub use control::{ControlMsg, DelayState, MusicItem, MusicNow, Report};
pub use error::ProtoError;
pub use ids::RoomCode;
pub use types::*;
pub use version::*;
