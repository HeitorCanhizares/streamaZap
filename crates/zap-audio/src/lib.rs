//! Áudio do StreamaZap (marco M2, frente `audio`).
//!
//! Portável: [`opus`], [`mix`], [`clock`], [`out_track`], [`out_mixer`], tipos em [`types`].
//! Só Windows: [`apps`], [`loopback`], [`capture`], [`out`], [`com`].

pub const RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
/// Amostras por canal em 20 ms.
pub const FRAME: usize = 960;
/// Amostras f32 intercaladas em um quadro de 20 ms.
pub const FRAME_SAMPLES: usize = FRAME * CHANNELS;

pub mod app_tree;
pub mod clock;
mod error;
pub mod mix;
pub mod opus;
pub mod out_mixer;
pub mod out_track;
mod types;

#[cfg(windows)]
pub mod apps;
#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod com;
#[cfg(windows)]
pub mod loopback;
#[cfg(windows)]
pub mod out;

pub use error::AudioError;
pub use opus::{OpusDec, OpusEnc};
pub use out_track::TrackInput;
pub use types::{AppSound, AudioApp, AudioSelection, CapturedAudio};

#[cfg(windows)]
pub use apps::list_audio_apps;
#[cfg(windows)]
pub use capture::AppCapture;
#[cfg(windows)]
pub use out::AudioOut;
