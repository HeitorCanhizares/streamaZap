//! GPU do StreamaZap: dispositivo D3D11, anel de texturas, conversor para NV12 e leitura para testes.
//! No Windows usa D3D11 de verdade; nos demais sistemas compila um stub que devolve
//! `GpuError::Unsupported`.

mod clock;
#[forbid(unsafe_code)]
mod cpu;
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
mod staging;
#[cfg(windows)]
mod texture;
#[cfg(windows)]
pub mod timing;
#[cfg(windows)]
mod views;

#[cfg(not(windows))]
mod stub;

pub use clock::now_us;
pub use cpu::{convert_batch, cpu_threads, BatchOptions, CpuScaler, Nv12Mut, SrcImage};
pub use error::GpuError;
pub use types::{AdapterInfo, FormatSupport, PixelFormat, ReadbackTiming, UploadTiming, Vendor};

#[cfg(windows)]
pub use com::{pin_mta, ComGuard};
#[cfg(windows)]
pub use converter::{ConvertTiming, Converter};
#[cfg(windows)]
pub use device::{DeviceLock, GpuDevice};
#[cfg(windows)]
pub use readback::{read_back, upload};
#[cfg(windows)]
pub use staging::{MappedFrame, MappedUpload, ReadbackStaging, UploadStaging};
#[cfg(windows)]
pub use texture::{Texture, TextureRing};
#[cfg(windows)]
pub use timing::GpuTimer;

#[cfg(not(windows))]
pub use stub::{
    pin_mta, read_back, upload, ComGuard, ConvertTiming, Converter, DeviceLock, GpuDevice, GpuTimer, MappedFrame,
    MappedUpload, ReadbackStaging, Texture, TextureRing, UploadStaging,
};
