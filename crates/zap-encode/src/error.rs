/// Erros do crate `zap-encode`.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// Plataforma sem Media Foundation (Linux/macOS).
    #[error("codificação não suportada nesta plataforma")]
    Unsupported,
    /// Nenhum codificador H.264 disponível para a escolha pedida.
    #[error("nenhum codificador H.264 disponível")]
    NoEncoder,
    #[error("configuração inválida: {0}")]
    InvalidConfig(String),
    /// O codificador parou (thread encerrada ou erro fatal do MFT).
    #[error("codificador parado: {0}")]
    Stopped(String),
    /// Erro vindo do `zap-gpu`.
    #[error(transparent)]
    Gpu(#[from] zap_gpu::GpuError),
    /// Falha de uma chamada do Windows (contexto e mensagem com o HRESULT).
    #[error("falha do Windows em {context}: {message}")]
    Os { context: &'static str, message: String },
}

#[cfg(windows)]
impl EncodeError {
    /// Atalho para erros do `windows`: `EncodeError::os("MFTEnumEx", &e)`.
    pub(crate) fn os(context: &'static str, err: &windows::core::Error) -> Self {
        EncodeError::Os {
            context,
            message: format!("{err}"),
        }
    }
}
