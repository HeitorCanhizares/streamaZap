#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("opus: {0}")]
    Opus(String),
    #[error("sistema de áudio: {0}")]
    Backend(String),
    #[error("nenhum dispositivo de áudio disponível")]
    NoDevice,
    #[error("processo {0} não encontrado ou sem áudio")]
    ProcessNotFound(u32),
    #[error("argumento inválido: {0}")]
    Invalid(String),
    #[error("não suportado nesta plataforma")]
    Unsupported,
}
