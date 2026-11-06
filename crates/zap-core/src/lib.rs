//! Orquestração do StreamaZap: sala, publicação de trilhas, assinaturas, threads de mídia e o
//! estado (snapshots) entregue à interface. Marco M3 (plano: docs/planos/m3-app.md).
//!
//! Módulos públicos usados pela frente `tauri`:
//! - `engine`: `Core`, `CoreOptions` e `UiSink` (ponto de entrada e canal de eventos);
//! - `settings`: `Settings` (configurações persistidas);
//! - `snapshot`: `Snapshot` (estado entregue à interface);
//! - `types`: tipos de dados compartilhados (reexportados na raiz);
//! - `error`: `CoreError`;
//! - `discovery_view`, `sources`, `ticket`, `notify`: visões auxiliares de descoberta, fontes,
//!   convites e notificações.
//!
//! `unsafe` só em `thread_prio` (prioridade de thread no Windows).
#![deny(unsafe_code)]

pub mod audio_out;
mod camera_test;
pub mod diag;
pub mod diag_build;
pub mod diag_collect;
pub mod diag_text;
pub mod discovery_view;
pub mod engine;
mod engine_music;
mod engine_room;
mod engine_share;
mod engine_tick;
mod engine_watch;
pub mod error;
pub mod music;
pub mod notify;
pub mod online;
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
pub use diag::{DiagInput, DiagReport};
pub use engine_music::MusicReport;
pub use error::CoreError;
pub use online::{ServerState, ServerStatus, ServerTestResult};
pub use settings::Settings;
pub use snapshot::Snapshot;
pub use types::*;
