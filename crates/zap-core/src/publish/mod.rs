//! Publicação: uma trilha própria (pipeline + alocador + espectadores) e o servidor de mídia.

mod alloc_loop;
mod audio;
mod media_serve;
mod media_server;
mod send_loop;
mod video;
mod viewer;

pub use alloc_loop::{estimate_upload_kbps, run_alloc_tick};
pub use audio::{AudioPublisher, AudioSource};
pub use media_server::{MediaAuth, MediaServer, PublishedTrack};
pub use video::{PublisherStats, VideoPublisher};
pub use viewer::{FrameRoute, ViewerLink, ViewerState};
