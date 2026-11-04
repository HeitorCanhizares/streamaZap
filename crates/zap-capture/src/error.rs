/// Erros do crate `zap-capture`.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// Plataforma sem captura (Linux/macOS).
    #[error("captura não suportada nesta plataforma")]
    Unsupported,
    #[error("monitor não encontrado")]
    MonitorNotFound,
    #[error("janela não encontrada")]
    WindowNotFound,
    /// A janela capturada foi fechada.
    #[error("a janela capturada foi fechada")]
    WindowClosed,
    #[error("câmera não encontrada")]
    CameraNotFound,
    #[error("câmera em uso por outro programa")]
    CameraBusy,
    #[error("argumento inválido: {0}")]
    InvalidArgument(String),
    /// Erro vindo do `zap-gpu`.
    #[error(transparent)]
    Gpu(#[from] zap_gpu::GpuError),
    /// Falha de uma chamada do Windows (contexto e mensagem com o HRESULT).
    #[error("falha do Windows em {context}: {message}")]
    Os { context: &'static str, message: String },
}
