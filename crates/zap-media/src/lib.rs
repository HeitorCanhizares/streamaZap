//! Mídia do StreamaZap: pipelines de vídeo (captura -> camadas -> codificadores) e de áudio
//! (captura por app -> Opus), com um backend sintético portável usado nos testes de ponta a ponta.
//!
//! O pipeline real (captura, conversão, H.264) existe só no Windows; o sintético existe em todas
//! as plataformas.
//!
//! Uso básico: cria-se um `VideoPipeline` com a escada de camadas, recebe-se os quadros pelo canal
//! e liga-se as camadas desejadas com `set_requests` (nenhuma começa ligada):
//!
//! ```no_run
//! use zap_media::{EncoderChoiceSpec, VideoPipeline, VideoPipelineConfig, VideoSourceSpec};
//! use zap_proto::LayerSpec;
//! use zap_rate::allocator::LayerRequest;
//!
//! let ladder = vec![LayerSpec { layer: 0, width: 1280, height: 720, fps: 30, kbps: 3000 }];
//! let cfg = VideoPipelineConfig {
//!     track: 1,
//!     source: VideoSourceSpec::Synthetic { width: 1280, height: 720 },
//!     ladder,
//!     encoder: EncoderChoiceSpec::Auto,
//! };
//! let (tx, rx) = crossbeam_channel::bounded(256);
//! let pipeline = VideoPipeline::start(cfg, tx)?;
//! pipeline.set_requests(&[LayerRequest { layer: 0, kbps: 3000 }]);
//! let primeiro = rx.recv()?;
//! assert!(primeiro.keyframe);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod audio;
mod error;
mod logic;
mod synth_audio;
mod synth_video;
mod types;
mod video;
#[cfg(windows)]
mod win_video;

pub use audio::AudioPipeline;
pub use error::MediaError;
pub use logic::{synthetic_frame_size, LayerGate, Samples, SeqCounter};
pub use types::{
    ConversionSpec, ConverterKind, EncodedAudio, EncodedFrame, EncoderChoiceSpec, LayerStats, MonitorRef,
    PipelineStats, StageStats, VideoPipelineConfig, VideoPipelineOptions, VideoSourceSpec,
};
pub use video::VideoPipeline;
