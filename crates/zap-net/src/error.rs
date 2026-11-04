//! Erros de rede.

use zap_proto::ProtoError;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("protocolo: {0}")]
    Proto(#[from] ProtoError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls: {0}")]
    Tls(String),
    #[error("conexao: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("conectar: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("escrita: {0}")]
    Write(#[from] quinn::WriteError),
    #[error("leitura: {0}")]
    Read(#[from] quinn::ReadError),
    #[error("datagrama: {0}")]
    Datagram(#[from] quinn::SendDatagramError),
    #[error("recusado: {0}")]
    Rejected(String),
    #[error("tempo esgotado")]
    Timeout,
    #[error("mensagem inesperada: {0}")]
    Unexpected(String),
    #[error("nenhuma porta livre a partir de {0}")]
    NoFreePort(u16),
    #[error("{0}")]
    Other(String),
}
