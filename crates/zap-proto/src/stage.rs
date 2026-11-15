//! Regras puras do modo palco (`docs/planos/modo-palco.md` §2). Sem I/O: o tempo vem de quem chama.
//!
//! Usado pelo dono da sala Local, pelo servidor e pela conferência do app (como `music`).
//!
//! Cada grupo de funções de [`Stage`] fica num submódulo (um dono de arquivo por tarefa).
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Valor em `recursos`.
pub const FEATURE: &str = "palco";
/// Levantar a mão de novo depois de levantar.
pub const HAND_INTERVAL: Duration = Duration::from_secs(5);
/// Pedir de novo depois de recusado.
pub const HAND_AFTER_DECLINE: Duration = Duration::from_secs(30);
/// Anfitrião que volta nesse prazo continua anfitrião.
pub const HOST_GRACE: Duration = Duration::from_secs(60);

/// Um presente na sala, para as regras que olham todo mundo.
pub struct StageMember<'a> {
    pub id: &'a str,
    pub has_feature: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicAction {
    Add,
    Remove,
    Vote,
    Pause,
    Seek,
    Info,
}

/// O que quem chama tem de fazer depois de uma ação (§2.8).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StageChange {
    /// Mandar `palco.estado` a todos.
    pub changed: bool,
    /// Viraram plateia com o palco ligado: todas as trilhas saem. Não inclui `must_leave`.
    pub demoted: Vec<String>,
    /// O palco desligou: todo microfone sai.
    pub drop_mics: bool,
    /// Apps sem o recurso: saem da sala (`sala.encerrada {motivo: "atualizar_app"}`).
    pub must_leave: Vec<String>,
    /// Quem vota na música mudou: `MusicQueue::left` para quem não vota mais.
    pub voters_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StageError {
    #[error("Só o dono da sala faz isso")]
    OwnerOnly,
    #[error("Só os anfitriões fazem isso")]
    HostOnly,
    #[error("No modo palco, só os anfitriões transmitem")]
    AudienceCannotPublish,
    #[error("O microfone só existe no modo palco, para os anfitriões")]
    MicOutsideStage,
    #[error("No palco, a música fica com os anfitriões")]
    MusicLocked,
    #[error("O modo palco está desligado")]
    StageOff,
    #[error("Você já está no palco")]
    AlreadyHost,
    #[error("Só dá para levantar a própria mão")]
    NotYourHand,
    #[error("Essa pessoa não está na sala")]
    UnknownParticipant,
    #[error("O dono da sala é sempre anfitrião")]
    OwnerIsAlwaysHost,
    #[error("Espere {} s para pedir de novo", .0.div_ceil(1000))]
    HandTooSoon(u64),
}

/// Estado do palco de uma sala.
#[derive(Clone, Debug)]
pub struct Stage {
    pub(crate) on: bool,
    pub(crate) owner: String,
    /// Anfitriões sem o dono, na ordem em que subiram.
    pub(crate) hosts: Vec<String>,
    pub(crate) hands: Vec<String>,
    pub(crate) audience_music: bool,
    /// Por pessoa: quando levantou a mão pela última vez.
    pub(crate) last_raise: HashMap<String, Instant>,
    /// Por pessoa: quando o dono recusou o pedido.
    pub(crate) declined_at: HashMap<String, Instant>,
    /// Anfitriões que saíram há pouco (`HOST_GRACE`), por id antigo.
    pub(crate) gone_hosts: HashMap<String, Instant>,
}

impl Stage {
    pub fn new(owner_id: &str, on: bool) -> Stage {
        Stage {
            on,
            owner: owner_id.to_string(),
            hosts: Vec::new(),
            hands: Vec::new(),
            audience_music: false,
            last_raise: HashMap::new(),
            declined_at: HashMap::new(),
            gone_hosts: HashMap::new(),
        }
    }
    pub fn is_on(&self) -> bool {
        self.on
    }
    pub fn hands(&self) -> &[String] {
        &self.hands
    }
    pub fn audience_music(&self) -> bool {
        self.audience_music
    }
}

mod admin;
mod apply;
mod error;
mod hands;
mod policy;
mod presence;
mod state;

#[cfg(test)]
mod tests_admin;
#[cfg(test)]
mod tests_apply;
#[cfg(test)]
mod tests_error;
#[cfg(test)]
mod tests_hands;
#[cfg(test)]
mod tests_policy;
#[cfg(test)]
mod tests_presence;
#[cfg(test)]
mod tests_state;
