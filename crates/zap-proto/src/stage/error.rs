//! `StageError`: código, campo e `erro` pronto (tarefa contrato-04).
use super::*;
use crate::control::ControlMsg;

impl StageError {
    /// `"proibido"` | `"invalido"` | `"limite"` (tabela de erros de `mensagens.md`).
    pub fn code(&self) -> &'static str {
        match self {
            StageError::OwnerOnly
            | StageError::HostOnly
            | StageError::AudienceCannotPublish
            | StageError::MicOutsideStage
            | StageError::MusicLocked => "proibido",
            StageError::StageOff
            | StageError::AlreadyHost
            | StageError::NotYourHand
            | StageError::UnknownParticipant
            | StageError::OwnerIsAlwaysHost => "invalido",
            StageError::HandTooSoon(_) => "limite",
        }
    }
    /// `"palco"` ou `"participante_id"`.
    pub fn field(&self) -> &'static str {
        match self {
            StageError::UnknownParticipant | StageError::OwnerIsAlwaysHost => "participante_id",
            _ => "palco",
        }
    }
    /// Só `HandTooSoon`.
    pub fn retry_ms(&self) -> Option<u64> {
        match self {
            StageError::HandTooSoon(ms) => Some(*ms),
            _ => None,
        }
    }
    /// `erro` pronto, com `campo`, `tentar_em_ms` e o `ref`.
    pub fn to_msg(&self, reference: Option<String>) -> ControlMsg {
        ControlMsg::Error {
            code: self.code().into(),
            message: self.to_string(),
            reference,
            field: Some(self.field().into()),
            plan: None,
            retry_ms: self.retry_ms(),
            protocol_min: None,
            protocol_max: None,
        }
    }
}
