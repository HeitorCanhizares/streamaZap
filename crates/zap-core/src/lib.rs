//! Orquestração do StreamaZap: sala, publicação de trilhas, assinaturas, threads de mídia e o
//! estado (snapshots) entregue à interface. Marco M3 (plano: docs/planos/m3-app.md).
//!
//! `unsafe` só em `thread_prio` (prioridade de thread no Windows).
#![deny(unsafe_code)]
// Esqueleto: a tarefa core-30 remove estes `allow` e deixa o clippy limpo.
#![allow(unused, dead_code, clippy::todo)]

pub mod audio_out;
pub mod discovery_view;
pub mod engine;
mod engine_room;
mod engine_share;
mod engine_tick;
mod engine_watch;
pub mod error;
pub mod notify;
pub mod playout;
pub mod publish;
pub mod room;
pub mod settings;
pub mod share;
pub mod snapshot;
pub mod sources;
pub mod state;
pub mod subscribe;
pub mod testutil;
mod thread_prio;
pub mod ticket;
pub mod types;
pub mod util;

pub use crate::engine::{Core, CoreOptions, UiSink};
pub use error::CoreError;
pub use settings::Settings;
pub use snapshot::Snapshot;
pub use types::*;
