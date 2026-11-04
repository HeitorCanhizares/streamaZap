//! Erro do núcleo. A mensagem (`Display`) é em português e vai direto para a interface;
//! `code()` é o código estável de `docs/api/README.md#erros` quando existe.

use zap_media::MediaError;
use zap_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Senha da sala incorreta")]
    BadPassword,
    #[error("Você não está em uma sala")]
    NoRoom,
    #[error("Você já está em uma sala")]
    AlreadyInRoom,
    #[error("Sala não encontrada")]
    RoomNotFound,
    #[error("Trilha não encontrada")]
    TrackNotFound,
    #[error("Não suportado: {0}")]
    Unsupported(String),
    #[error("Pedido inválido: {0}")]
    Invalid(String),
    #[error("Recusado pela sala: {0}")]
    Rejected(String),
    #[error("Não foi possível conectar: {0}")]
    Connect(String),
    #[error("Rede: {0}")]
    Net(#[from] NetError),
    #[error("Mídia: {0}")]
    Media(#[from] MediaError),
    #[error("Arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encerrado")]
    Stopped,
    /// Erro do servidor Online (`ApiError::Http` ou `erro`/`recusado` do QUIC/WebSocket):
    /// `message` vai direto para a interface; `code` é o de `docs/api/README.md#erros`.
    #[error("{message}")]
    Server { code: String, message: String },
}

impl From<zap_net::online::ApiError> for CoreError {
    /// `Http{code,message}` → `Server`; `Network(m)` → `Connect(m)`; `Protocol(m)` → `Connect(m)`.
    fn from(e: zap_net::online::ApiError) -> Self {
        use zap_net::online::ApiError;
        match e {
            ApiError::Http { code, message, .. } => CoreError::Server { code, message },
            ApiError::Network(m) | ApiError::Protocol(m) => CoreError::Connect(m),
        }
    }
}

/// Códigos estáveis do servidor (`docs/api/README.md#erros`) como `&'static str`.
fn codigo_estatico(code: &str) -> &'static str {
    match code {
        "protocolo_incompativel" => "protocolo_incompativel",
        "senha_servidor" => "senha_servidor",
        "nao_autenticado" => "nao_autenticado",
        "sala_nao_encontrada" => "sala_inexistente",
        "senha_sala" => "senha_sala",
        "sala_cheia" => "sala_cheia",
        "proibido" => "proibido",
        "limite" => "limite",
        "invalido" => "invalido",
        _ => "erro",
    }
}

impl CoreError {
    /// Código estável (ex.: `senha_sala`) ou `erro` genérico.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::BadPassword => "senha_sala",
            CoreError::NoRoom => "sem_sala",
            CoreError::AlreadyInRoom => "ja_na_sala",
            CoreError::RoomNotFound => "sala_inexistente",
            CoreError::TrackNotFound => "trilha_inexistente",
            CoreError::Unsupported(_) => "nao_suportado",
            CoreError::Invalid(_) => "invalido",
            CoreError::Rejected(_) => "recusado",
            CoreError::Connect(_) => "conexao",
            CoreError::Server { code, .. } => codigo_estatico(code),
            _ => "erro",
        }
    }

    /// Converte o erro do aperto de mão: `Rejected("senha_sala")` vira `BadPassword`.
    pub fn from_handshake(e: NetError) -> Self {
        match e {
            NetError::Rejected(c) if c == "senha_sala" => CoreError::BadPassword,
            NetError::Rejected(c) => CoreError::Rejected(c),
            other => CoreError::Net(other),
        }
    }
}
