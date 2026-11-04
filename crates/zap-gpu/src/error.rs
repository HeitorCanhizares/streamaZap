/// Erros do crate `zap-gpu`.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// Plataforma sem D3D11 (Linux/macOS) ou recurso indisponível.
    #[error("GPU não suportada nesta plataforma")]
    Unsupported,
    /// Nenhum adaptador encontrado (ou LUID desconhecido).
    #[error("adaptador de vídeo não encontrado")]
    AdapterNotFound,
    /// Argumento inválido (tamanho zero, buffer curto, formato errado...).
    #[error("argumento inválido: {0}")]
    InvalidArgument(String),
    /// Falha de uma chamada do Windows (HRESULT e contexto).
    #[error("falha do Windows em {context}: {message}")]
    Os { context: &'static str, message: String },
}
