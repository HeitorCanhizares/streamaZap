/// Erros do crate `zap-media`.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// Fonte ou recurso indisponível nesta plataforma (ex.: captura real fora do Windows).
    #[error("não suportado nesta plataforma: {0}")]
    Unsupported(String),
    #[error("configuração inválida: {0}")]
    InvalidConfig(String),
    #[error(transparent)]
    Gpu(#[from] zap_gpu::GpuError),
    #[error(transparent)]
    Capture(#[from] zap_capture::CaptureError),
    #[error(transparent)]
    Encode(#[from] zap_encode::EncodeError),
    #[error(transparent)]
    Audio(#[from] zap_audio::AudioError),
    /// O pipeline já parou (thread encerrada).
    #[error("pipeline parado: {0}")]
    Stopped(String),
}
