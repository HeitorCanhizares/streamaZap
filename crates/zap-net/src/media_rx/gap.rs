//! Detecção de buracos de `seq`.

use super::GAP_TIMEOUT;

/// Máximo de buracos guardados por (trilha, camada). Evita crescer sem limite após um salto grande.
const MAX_MISSING: usize = 1024;
/// Metade do espaço de `seq`: distância menor que isso é "à frente", maior é "atrasado".
const AHEAD_LIMIT: u32 = 0x8000_0000;

/// Detector de buracos de `seq` por (trilha, camada). Puro, sem I/O.
#[derive(Default)]
pub struct GapTracker {
    pub(crate) state: std::collections::HashMap<(u16, u8), GapState>,
}

#[derive(Default)]
pub(crate) struct GapState {
    pub(crate) next_expected: Option<u32>,
    /// seq faltantes e o instante (µs) em que o buraco foi visto.
    pub(crate) missing: Vec<(u32, u64)>,
}

impl GapTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra a chegada de `seq`; devolve nada, só atualiza buracos.
    pub fn on_seq(&mut self, track: u16, layer: u8, seq: u32, now_us: u64) {
        let st = self.state.entry((track, layer)).or_default();
        let next = match st.next_expected {
            // Primeiro seq da trilha: não há como saber o que veio antes, então não abre buracos.
            None => {
                st.next_expected = Some(seq.wrapping_add(1));
                return;
            }
            Some(next) => next,
        };

        if seq == next {
            st.next_expected = Some(next.wrapping_add(1));
        } else if seq.wrapping_sub(next) < AHEAD_LIMIT {
            // Salto para a frente: tudo entre `next` e `seq` (exclusivo) virou buraco.
            // Se o salto for maior que o limite, só os mais recentes importam; pula os antigos.
            let gap = seq.wrapping_sub(next);
            let skip = if gap as usize > MAX_MISSING {
                gap - MAX_MISSING as u32
            } else {
                0
            };
            let mut s = next.wrapping_add(skip);
            while s != seq {
                st.missing.push((s, now_us));
                s = s.wrapping_add(1);
            }
            if st.missing.len() > MAX_MISSING {
                let excesso = st.missing.len() - MAX_MISSING;
                // Os primeiros são os mais antigos (inseridos antes).
                st.missing.drain(..excesso);
            }
            st.next_expected = Some(seq.wrapping_add(1));
        } else if let Some(pos) = st.missing.iter().position(|&(s, _)| s == seq) {
            // Atrasado, mas dentro do prazo: o buraco se fechou.
            st.missing.remove(pos);
        }
        // Senão: duplicado ou atrasado já expirado; ignora.
    }

    /// Devolve os (trilha, camada, seq) cujo buraco passou de `crate::media_rx::GAP_TIMEOUT` e os remove.
    pub fn expire(&mut self, now_us: u64) -> Vec<(u16, u8, u32)> {
        let timeout_us = GAP_TIMEOUT.as_micros() as u64;
        let mut out = Vec::new();
        for (&(track, layer), st) in self.state.iter_mut() {
            let mut keep = Vec::with_capacity(st.missing.len());
            for &(seq, visto) in &st.missing {
                if now_us.saturating_sub(visto) >= timeout_us {
                    out.push((track, layer, seq));
                } else {
                    keep.push((seq, visto));
                }
            }
            st.missing = keep;
        }
        out.sort_unstable();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: u16 = 7;
    const L: u8 = 0;

    #[test]
    fn sequencia_sem_buraco() {
        let mut g = GapTracker::new();
        for s in 0..10 {
            g.on_seq(T, L, s, 0);
        }
        assert!(g.expire(1_000_000).is_empty());
    }

    #[test]
    fn buraco_expira_em_300ms() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 0);
        g.on_seq(T, L, 1, 0);
        g.on_seq(T, L, 3, 0);
        assert!(g.expire(299_999).is_empty());
        assert_eq!(g.expire(300_000), vec![(T, L, 2)]);
    }

    #[test]
    fn reordenado_fecha_buraco() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 0);
        g.on_seq(T, L, 1, 0);
        g.on_seq(T, L, 3, 0);
        g.on_seq(T, L, 2, 100_000);
        assert!(g.expire(1_000_000).is_empty());
    }

    #[test]
    fn multiplas_trilhas_independentes() {
        let mut g = GapTracker::new();
        g.on_seq(1, 0, 0, 0);
        g.on_seq(1, 0, 1, 0);
        g.on_seq(1, 0, 3, 0);
        g.on_seq(2, 0, 0, 0);
        g.on_seq(2, 0, 1, 0);
        g.on_seq(2, 0, 2, 0);
        assert_eq!(g.expire(300_000), vec![(1, 0, 2)]);
    }

    #[test]
    fn wrap_u32() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, u32::MAX - 1, 0);
        g.on_seq(T, L, u32::MAX, 0);
        g.on_seq(T, L, 1, 0);
        assert_eq!(g.expire(300_000), vec![(T, L, 0)]);
    }

    #[test]
    fn duplicado_ignorado() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 0);
        g.on_seq(T, L, 1, 0);
        g.on_seq(T, L, 1, 10);
        g.on_seq(T, L, 0, 20);
        assert!(g.expire(1_000_000).is_empty());
        let st = &g.state[&(T, L)];
        assert_eq!(st.next_expected, Some(2));
        assert!(st.missing.is_empty());
    }

    #[test]
    fn limite_de_buracos() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 0);
        g.on_seq(T, L, 5000, 0);
        let st = &g.state[&(T, L)];
        assert!(st.missing.len() <= MAX_MISSING);
        // Os mantidos são os mais recentes: o último buraco é 4999.
        assert_eq!(st.missing.last().map(|&(s, _)| s), Some(4999));
        assert_eq!(st.missing.first().map(|&(s, _)| s), Some(5000 - MAX_MISSING as u32));
    }

    #[test]
    fn expire_so_uma_vez() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 0);
        g.on_seq(T, L, 2, 0);
        assert_eq!(g.expire(300_000), vec![(T, L, 1)]);
        assert!(g.expire(300_000).is_empty());
        assert!(g.expire(1_000_000).is_empty());
    }
}
