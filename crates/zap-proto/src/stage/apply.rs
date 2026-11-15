//! Despacho das mensagens `palco.*` (tarefa contrato-10).
use super::*;
use crate::control::ControlMsg;
use std::time::Instant;

impl Stage {
    /// `palco.*` C->S: `Some(resultado)`; outra mensagem: `None`. `palco.mao` com o
    /// `participante_id` de outro e `levantada: false` = `decline`; com `true` = `NotYourHand`.
    pub fn apply(
        &mut self,
        by: &str,
        msg: &ControlMsg,
        members: &[StageMember<'_>],
        now: Instant,
    ) -> Option<Result<StageChange, StageError>> {
        match msg {
            ControlMsg::StageSet { on } => Some(self.set_on(by, *on, members)),
            ControlMsg::StageHost { participant_id, host } => Some(self.set_host(by, participant_id, *host, members)),
            ControlMsg::StageHand { raised, participant_id } => match participant_id.as_deref() {
                None => Some(self.raise_hand(by, *raised, now)),
                Some(t) if t == by => Some(self.raise_hand(by, *raised, now)),
                // Mão de outra pessoa: só dá para recusar (baixar), nunca levantar.
                Some(t) if !*raised => Some(self.decline(by, t, now)),
                Some(_) => Some(Err(StageError::NotYourHand)),
            },
            ControlMsg::StageMusic { audience } => Some(self.set_audience_music(by, *audience)),
            _ => None,
        }
    }
}
