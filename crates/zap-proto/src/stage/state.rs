//! Espelho, papéis e `palco.estado` (tarefa contrato-05).
use super::*;
use crate::control::ControlMsg;

impl Stage {
    /// Espelho a partir de um `palco.estado` (sem os prazos). `None` se `msg` não for `StageState`.
    pub fn from_msg(owner_id: &str, msg: &ControlMsg) -> Option<Stage> {
        let ControlMsg::StageState {
            on,
            hosts,
            hands,
            audience_music,
        } = msg
        else {
            return None;
        };
        let mut stage = Stage::new(owner_id, *on);
        for h in hosts {
            // O dono não entra na lista; duplicatas somem, a ordem fica.
            if h != owner_id && !stage.hosts.contains(h) {
                stage.hosts.push(h.clone());
            }
        }
        for h in hands {
            if !stage.hands.contains(h) {
                stage.hands.push(h.clone());
            }
        }
        stage.audience_music = *audience_music;
        Some(stage)
    }
    /// O dono sempre; os outros pela lista (vale com o palco desligado).
    pub fn is_host(&self, id: &str) -> bool {
        self.owner == id || self.hosts.iter().any(|h| h == id)
    }
    /// `palco.estado`: o dono primeiro, depois os anfitriões na ordem em que subiram.
    pub fn msg(&self) -> ControlMsg {
        let mut hosts = Vec::with_capacity(self.hosts.len() + 1);
        hosts.push(self.owner.clone());
        hosts.extend(self.hosts.iter().cloned());
        ControlMsg::StageState {
            on: self.on,
            hosts,
            hands: self.hands.clone(),
            audience_music: self.audience_music,
        }
    }
}
