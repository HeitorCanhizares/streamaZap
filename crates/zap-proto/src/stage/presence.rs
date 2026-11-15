//! Saída e volta de anfitriões (tarefa contrato-09).
use super::*;

impl Stage {
    /// Saída: tira mão e papel; anfitrião vai para a lista de "voltou logo".
    pub fn left(&mut self, id: &str, now: Instant) -> StageChange {
        let tinha_mao = self.hands.iter().any(|h| h == id);
        self.hands.retain(|h| h != id);
        // Prazos são por id de participante (muda a cada conexão no Local): limpa para não vazar.
        self.last_raise.remove(id);
        self.declined_at.remove(id);
        let era_anfitriao = self.hosts.iter().any(|h| h == id);
        if era_anfitriao {
            self.hosts.retain(|h| h != id);
            self.gone_hosts.insert(id.to_string(), now);
        }
        // O dono que sai continua dono: nada de papel. As trilhas saem pela saída normal.
        StageChange {
            changed: tinha_mao || era_anfitriao,
            ..StageChange::default()
        }
    }
    /// Volta em até `HOST_GRACE`: `new_id` vira anfitrião. Também limpa a lista vencida.
    pub fn rejoined(&mut self, old_id: &str, new_id: &str, now: Instant) -> StageChange {
        // Exatamente `HOST_GRACE` já venceu.
        self.gone_hosts
            .retain(|_, &mut t| now.saturating_duration_since(t) < HOST_GRACE);
        if self.gone_hosts.remove(old_id).is_none() {
            return StageChange::default();
        }
        if new_id != self.owner && !self.hosts.iter().any(|h| h == new_id) {
            self.hosts.push(new_id.to_string());
        }
        StageChange {
            changed: true,
            ..StageChange::default()
        }
    }
}
