//! Erros do crate.

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("mensagem grande demais: {0} bytes")]
    TooLarge(usize),
    #[error("dados truncados")]
    Truncated,
    #[error("tipo de pacote inesperado: {0:#04x}")]
    BadType(u8),
    #[error("json inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mensagem desconhecida: {0}")]
    UnknownMessage(String),
    #[error("código de sala inválido: {0}")]
    InvalidRoomCode(String),
    #[error("base64 inválido")]
    Base64,
}
