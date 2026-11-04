//! Stub para plataformas sem captura: tudo devolve `CaptureError::Unsupported`.
use crate::{CameraInfo, CaptureError, CaptureHandle, FrameSink, MonitorInfo, WindowInfo};
use zap_gpu::GpuDevice;

pub fn list_monitors() -> Result<Vec<MonitorInfo>, CaptureError> {
    Err(CaptureError::Unsupported)
}
pub fn list_windows() -> Result<Vec<WindowInfo>, CaptureError> {
    Err(CaptureError::Unsupported)
}
pub fn list_cameras() -> Result<Vec<CameraInfo>, CaptureError> {
    Err(CaptureError::Unsupported)
}
pub fn wgc_supported() -> bool {
    false
}
pub fn capture_monitor(
    _dev: &GpuDevice,
    _monitor: &MonitorInfo,
    _sink: impl FrameSink,
) -> Result<CaptureHandle, CaptureError> {
    Err(CaptureError::Unsupported)
}
pub fn capture_window(_dev: &GpuDevice, _hwnd: isize, _sink: impl FrameSink) -> Result<CaptureHandle, CaptureError> {
    Err(CaptureError::Unsupported)
}
pub fn capture_camera(
    _dev: &GpuDevice,
    _id: &str,
    _want_w: u32,
    _want_h: u32,
    _want_fps: u32,
    _sink: impl FrameSink,
) -> Result<CaptureHandle, CaptureError> {
    Err(CaptureError::Unsupported)
}

#[cfg(test)]
#[cfg(not(windows))]
mod tests {
    use super::*;

    #[test]
    fn todas_as_funcoes_retornam_unsupported() {
        assert!(matches!(list_monitors(), Err(CaptureError::Unsupported)));
        assert!(matches!(list_windows(), Err(CaptureError::Unsupported)));
        assert!(matches!(list_cameras(), Err(CaptureError::Unsupported)));
        assert!(!wgc_supported());

        // No stub o GpuDevice também não existe; sem ele não há como chamar capture_*.
        match GpuDevice::create_default() {
            Err(zap_gpu::GpuError::Unsupported) => {}
            Err(outro) => panic!("erro inesperado do GPU: {outro:?}"),
            Ok(_) => {}
        }
    }
}
