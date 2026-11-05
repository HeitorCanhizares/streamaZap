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
pub mod movimento;
#[cfg(windows)]
mod wgc;
#[cfg(windows)]
mod windows_list;

#[cfg(not(windows))]
mod stub;

/// Vez dos testes que duplicam o monitor. O DXGI só aceita uma duplicação por monitor em cada
/// processo: em paralelo, o segundo `DuplicateOutput` dá E_INVALIDARG e o teste pula em silêncio
/// (visto no CI Windows). Segure a guarda até o fim do teste.
#[cfg(all(test, windows))]
pub(crate) fn vez_da_duplicacao() -> std::sync::MutexGuard<'static, ()> {
    static VEZ: std::sync::Mutex<()> = std::sync::Mutex::new(());
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

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
