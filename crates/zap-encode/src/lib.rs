//! Codificador H.264 do StreamaZap. No Windows usa Media Foundation: MFT de hardware assíncrono
//! (com `IMFDXGIDeviceManager`) e MFT de software síncrono de reserva. Nos demais sistemas
//! compila um stub que devolve `EncodeError::Unsupported`. O parser de NAL (`nal`) e a lógica
//! pura (`logic`) são portáveis e testados no Linux.

mod error;
pub mod logic;
pub mod nal;
mod types;

#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod codec_api;
#[cfg(windows)]
mod encoder;
#[cfg(windows)]
mod enumerate;
#[cfg(windows)]
mod hw;
#[cfg(windows)]
mod hw_events;
#[cfg(windows)]
mod media_types;
#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod sample;
#[cfg(windows)]
mod sw;

#[cfg(not(windows))]
mod stub;

pub use error::EncodeError;
pub use types::{EncodedPicture, EncoderChoice, EncoderConfig, EncoderInfo};

#[cfg(windows)]
pub use encoder::H264Encoder;
#[cfg(windows)]
pub use enumerate::list_encoders;

#[cfg(not(windows))]
pub use stub::{list_encoders, H264Encoder};
