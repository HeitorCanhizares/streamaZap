//! Mãos levantadas e recusa (tarefa contrato-08).
use super::*;

/// Quanto falta (em ms) para `prazo` passar, ou `None` se já passou (limite: decorrido >= prazo).
fn restante_ms(now: Instant, desde: Instant, prazo: Duration) -> Option<u64> {
    let decorrido = now.saturating_duration_since(desde);
    if decorrido >= prazo {
        None
    } else {
        Some((prazo - decorrido).as_millis() as u64)
    }
}

impl Stage {
    /// Levanta ou baixa a própria mão. Respeita o intervalo entre levantadas e a espera após recusa.
    pub fn raise_hand(&mut self, by: &str, raised: bool, now: Instant) -> Result<StageChange, StageError> {
        if !self.on {
            return Err(StageError::StageOff);
        }
        if raised {
            if self.is_host(by) {
                return Err(StageError::AlreadyHost);
            }
            if self.hands.iter().any(|h| h == by) {
                return Ok(StageChange::default());
            }
            let apos_recusa = self
                .declined_at
                .get(by)
                .and_then(|&t| restante_ms(now, t, HAND_AFTER_DECLINE));
            let apos_levantar = self
                .last_raise
                .get(by)
                .and_then(|&t| restante_ms(now, t, HAND_INTERVAL));
            // Se os dois prazos valem, o cliente espera o maior.
            if let Some(ms) = apos_recusa.into_iter().chain(apos_levantar).max() {
                return Err(StageError::HandTooSoon(ms));
            }
            self.hands.push(by.to_string());
            self.last_raise.insert(by.to_string(), now);
            Ok(StageChange {
                changed: true,
                ..StageChange::default()
            })
        } else {
            if self.is_host(by) {
                return Ok(StageChange::default());
            }
            let Some(pos) = self.hands.iter().position(|h| h == by) else {
                return Ok(StageChange::default());
            };
            // Os prazos não mudam: levantar de novo ainda conta desde a última levantada.
            self.hands.remove(pos);
            Ok(StageChange {
                changed: true,
                ..StageChange::default()
            })
        }
    }

    /// O dono recusa o pedido de `target`: a mão baixa e começa a espera de `HAND_AFTER_DECLINE`.
    pub fn decline(&mut self, by: &str, target: &str, now: Instant) -> Result<StageChange, StageError> {
        if by != self.owner {
            return Err(StageError::OwnerOnly);
        }
        if !self.on {
            return Err(StageError::StageOff);
        }
        let Some(pos) = self.hands.iter().position(|h| h == target) else {
            return Ok(StageChange::default());
        };
        self.hands.remove(pos);
        self.declined_at.insert(target.to_string(), now);
        Ok(StageChange {
            changed: true,
            ..StageChange::default()
        })
    }
}
