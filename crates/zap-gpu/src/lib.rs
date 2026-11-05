//! GPU do StreamaZap: dispositivo D3D11, anel de texturas, conversor para NV12 e leitura para testes.
//! No Windows usa D3D11 de verdade; nos demais sistemas compila um stub que devolve
//! `GpuError::Unsupported`.

mod clock;
mod error;
mod types;

#[cfg(windows)]
mod com;
#[cfg(windows)]
mod converter;
#[cfg(windows)]
mod device;
#[cfg(windows)]
mod readback;
#[cfg(windows)]
mod texture;

#[cfg(not(windows))]
mod stub;

pub use clock::now_us;
pub use error::GpuError;
pub use types::{AdapterInfo, PixelFormat, Vendor};

#[cfg(windows)]
pub use com::{pin_mta, ComGuard};
#[cfg(windows)]
pub use converter::Converter;
#[cfg(windows)]
pub use device::GpuDevice;
#[cfg(windows)]
pub use readback::{read_back, upload};
#[cfg(windows)]
pub use texture::{Texture, TextureRing};

#[cfg(not(windows))]
pub use stub::{pin_mta, read_back, upload, ComGuard, Converter, GpuDevice, Texture, TextureRing};
