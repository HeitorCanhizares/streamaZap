//! Captura do StreamaZap: monitor (DXGI Desktop Duplication), janela (Windows.Graphics.Capture)
//! e câmera (Media Foundation). Os quadros ficam na GPU (`zap_gpu::Texture`). Nos demais sistemas
//! compila um stub que devolve `CaptureError::Unsupported`.

mod camera_mode;
mod error;
mod handle;
mod logic;
mod types;

#[cfg(windows)]
mod camera;
#[cfg(windows)]
mod camera_enum;
#[cfg(windows)]
mod cursor;
#[cfg(windows)]
mod dxgi;
#[cfg(windows)]
mod dxgi_dup;
#[cfg(windows)]
mod monitors;
#[cfg(windows)]
mod wgc;
#[cfg(windows)]
mod windows_list;

#[cfg(not(windows))]
mod stub;

pub use camera_mode::{pick_camera_mode, CameraMode};
pub use error::CaptureError;
pub use handle::CaptureHandle;
pub use logic::{cursor_local_pos, keep_window, order_monitors, StaleTimer, WindowCandidate};
pub use types::{CameraInfo, CapturedFrame, FrameSink, MonitorInfo, WindowInfo};

#[cfg(windows)]
pub use camera::capture_camera;
#[cfg(windows)]
pub use camera_enum::list_cameras;
#[cfg(windows)]
pub use dxgi::capture_monitor;
#[cfg(windows)]
pub use monitors::list_monitors;
#[cfg(windows)]
pub use wgc::{capture_window, wgc_supported};
#[cfg(windows)]
pub use windows_list::list_windows;

#[cfg(not(windows))]
pub use stub::{
    capture_camera, capture_monitor, capture_window, list_cameras, list_monitors, list_windows, wgc_supported,
};
