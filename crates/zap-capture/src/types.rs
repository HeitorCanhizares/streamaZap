use crate::CaptureError;

/// Monitor ligado ao desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    /// `HMONITOR` como inteiro.
    pub id: isize,
    /// Índice a partir de 1 (principal = 1; depois por `x`, depois `y`).
    pub index: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    /// LUID do adaptador (`(High << 32) | Low`) que dirige o monitor.
    pub adapter_luid: i64,
}

/// Janela de nível superior capturável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub pid: u32,
    /// Nome do executável, ex.: "cs2.exe".
    pub process_name: String,
    pub width: u32,
    pub height: u32,
}

/// Câmera de vídeo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraInfo {
    /// Symbolic link do dispositivo.
    pub id: String,
    pub name: String,
}

/// Um quadro capturado, ainda na GPU.
pub struct CapturedFrame {
    /// BGRA (monitor, janela) ou NV12 (câmera).
    pub texture: zap_gpu::Texture,
    pub width: u32,
    pub height: u32,
    /// Relógio monotônico do processo em µs (`zap_gpu::now_us()`).
    pub capture_us: u64,
    /// `false` quando é reentrega do último quadro (sem imagem nova).
    pub new_content: bool,
}

/// Destino dos quadros. Chamado na thread de captura; deve voltar rápido.
pub trait FrameSink: Send + 'static {
    fn on_frame(&self, frame: CapturedFrame);
    fn on_error(&self, err: CaptureError);
}
