//! Quem pode transmitir e mexer na música (tarefa contrato-06).
use super::*;
use crate::types::{AudioSource, TrackKind};

impl Stage {
    /// Matriz de `trilha.publicar` (§2.5). Ordem: palco ligado e plateia -> `AudienceCannotPublish`;
    /// microfone com o palco desligado -> `MicOutsideStage`; senão ok.
    pub fn can_publish(&self, id: &str, kind: TrackKind, source: AudioSource) -> Result<(), StageError> {
        if self.on && !self.is_host(id) {
            return Err(StageError::AudienceCannotPublish);
        }
        // `source` só importa em áudio; em vídeo é ignorado.
        if kind == TrackKind::Audio && source == AudioSource::Mic && !self.on {
            return Err(StageError::MicOutsideStage);
        }
        Ok(())
    }
    /// Matriz de música (§2.6).
    pub fn check_music(&self, id: &str, action: MusicAction) -> Result<(), StageError> {
        if action == MusicAction::Info {
            return Ok(());
        }
        // Palco desligado = regra de hoje; a regra própria da fila é de quem chama.
        if self.on && !self.audience_music && !self.is_host(id) {
            return Err(StageError::MusicLocked);
        }
        Ok(())
    }
    /// Tira qualquer item e pula direto: o dono, ou anfitrião com o palco ligado.
    pub fn music_moderator(&self, id: &str) -> bool {
        id == self.owner || (self.on && self.is_host(id))
    }
    /// Vota para pular: palco desligado ou chave ligada, todos; senão só anfitriões.
    pub fn is_voter(&self, id: &str) -> bool {
        !self.on || self.audience_music || self.is_host(id)
    }
    /// Quantos dos `present` votam (base do `precisa` da música).
    pub fn voters<'a>(&self, present: impl IntoIterator<Item = &'a str>) -> usize {
        present.into_iter().filter(|id| self.is_voter(id)).count()
    }
}
