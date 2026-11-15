//! Ações do dono e dos anfitriões: ligar, promover, chave da música (tarefa contrato-07).
use super::*;

impl Stage {
    /// Liga ou desliga o palco (só o dono). Repetir o estado atual não muda nada.
    pub fn set_on(&mut self, by: &str, on: bool, members: &[StageMember<'_>]) -> Result<StageChange, StageError> {
        if by != self.owner {
            return Err(StageError::OwnerOnly);
        }
        if on == self.on {
            return Ok(StageChange::default());
        }
        if on {
            let must_leave: Vec<String> = members
                .iter()
                .filter(|m| !m.has_feature && m.id != self.owner)
                .map(|m| m.id.to_string())
                .collect();
            let demoted: Vec<String> = members
                .iter()
                .filter(|m| m.has_feature && !self.is_host(m.id))
                .map(|m| m.id.to_string())
                .collect();
            self.on = true;
            Ok(StageChange {
                changed: true,
                demoted,
                drop_mics: false,
                must_leave,
                // Os votos da plateia somem quando a música fica só com os anfitriões.
                voters_changed: !self.audience_music,
            })
        } else {
            self.on = false;
            self.hands.clear();
            self.last_raise.clear();
            self.declined_at.clear();
            // A chave da música fica guardada; com o palco desligado todos votam de novo.
            Ok(StageChange {
                changed: true,
                drop_mics: true,
                ..StageChange::default()
            })
        }
    }

    /// Promove ou tira um anfitrião (só o dono).
    pub fn set_host(
        &mut self,
        by: &str,
        target: &str,
        host: bool,
        members: &[StageMember<'_>],
    ) -> Result<StageChange, StageError> {
        if by != self.owner {
            return Err(StageError::OwnerOnly);
        }
        if !members.iter().any(|m| m.id == target) {
            return Err(StageError::UnknownParticipant);
        }
        if target == self.owner {
            return if host {
                Ok(StageChange::default())
            } else {
                Err(StageError::OwnerIsAlwaysHost)
            };
        }
        let already = self.hosts.iter().any(|h| h == target);
        let voters_changed = self.on && !self.audience_music;
        if host {
            if already {
                return Ok(StageChange::default());
            }
            self.hosts.push(target.to_string());
            self.hands.retain(|h| h != target);
            // O prazo de pedir de novo zera.
            self.declined_at.remove(target);
            self.last_raise.remove(target);
            Ok(StageChange {
                changed: true,
                voters_changed,
                ..StageChange::default()
            })
        } else {
            if !already {
                return Ok(StageChange::default());
            }
            self.hosts.retain(|h| h != target);
            Ok(StageChange {
                changed: true,
                demoted: if self.on { vec![target.to_string()] } else { Vec::new() },
                voters_changed,
                ..StageChange::default()
            })
        }
    }

    /// Liga ou desliga a música da plateia (só anfitrião, com o palco ligado).
    pub fn set_audience_music(&mut self, by: &str, allowed: bool) -> Result<StageChange, StageError> {
        if !self.is_host(by) {
            return Err(StageError::HostOnly);
        }
        if !self.on {
            return Err(StageError::StageOff);
        }
        if allowed == self.audience_music {
            return Ok(StageChange::default());
        }
        self.audience_music = allowed;
        Ok(StageChange {
            changed: true,
            // Desligar tira os votos da plateia; ligar só aumenta os votantes.
            voters_changed: !allowed,
            ..StageChange::default()
        })
    }
}
