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
    /// Última camada recebida de cada trilha: detecta a troca de camada.
    pub(crate) last_layer: std::collections::HashMap<u16, u8>,
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
    ///
    /// Troca de camada: quem recebe tem uma camada por trilha de cada vez, e o `seq` de cada
    /// camada anda mesmo quando ela não está chegando. Ao voltar para uma camada recebida antes, o
    /// salto de `seq` (às vezes centenas) não é perda: a contagem dela recomeça. Por isso um quadro
    /// numa camada diferente da última recebida na trilha não abre buracos.
    pub fn on_seq(&mut self, track: u16, layer: u8, seq: u32, now_us: u64) {
        let trocou = self.last_layer.insert(track, layer).is_some_and(|l| l != layer);
        let st = self.state.entry((track, layer)).or_default();
        if trocou {
            st.next_expected = None;
        }
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

    /// Devolve os (trilha, camada, seq, instante visto em µs) cujo buraco passou de
    /// `crate::media_rx::GAP_TIMEOUT` e os remove. O instante visto é quando o buraco apareceu.
    pub fn expire(&mut self, now_us: u64) -> Vec<(u16, u8, u32, u64)> {
        let timeout_us = GAP_TIMEOUT.as_micros() as u64;
        let mut out = Vec::new();
        for (&(track, layer), st) in self.state.iter_mut() {
            let mut keep = Vec::with_capacity(st.missing.len());
            for &(seq, visto) in &st.missing {
                if now_us.saturating_sub(visto) >= timeout_us {
                    out.push((track, layer, seq, visto));
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
        assert_eq!(g.expire(300_000), vec![(T, L, 2, 0)]);
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
        assert_eq!(g.expire(300_000), vec![(1, 0, 2, 0)]);
    }

    #[test]
    fn wrap_u32() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, u32::MAX - 1, 0);
        g.on_seq(T, L, u32::MAX, 0);
        g.on_seq(T, L, 1, 0);
        assert_eq!(g.expire(300_000), vec![(T, L, 0, 0)]);
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
        assert_eq!(g.expire(300_000), vec![(T, L, 1, 0)]);
        assert!(g.expire(300_000).is_empty());
        assert!(g.expire(1_000_000).is_empty());
    }

    #[test]
    fn expire_devolve_instante_visto() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 1000);
        g.on_seq(T, L, 1, 2000);
        g.on_seq(T, L, 3, 5000);
        // Buraco de seq 2 foi visto em 5000; expira quando passa de 300 ms.
        assert_eq!(g.expire(305_000), vec![(T, L, 2, 5000)]);
    }

    #[test]
    fn varios_buracos_cada_um_com_seu_instante() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 1000);
        g.on_seq(T, L, 1, 2000);
        g.on_seq(T, L, 3, 5000);
        // Abre o buraco 4 em 9000, depois do buraco 2 (visto em 5000).
        g.on_seq(T, L, 5, 9000);
        assert_eq!(g.expire(400_000), vec![(T, L, 2, 5000), (T, L, 4, 9000)]);
    }

    #[test]
    fn volta_para_camada_antiga_nao_abre_buraco() {
        let mut g = GapTracker::new();
        // Camada 2, depois a 1 por um tempo, e volta para a 2 com o `seq` muito à frente.
        for s in 0..5 {
            g.on_seq(T, 2, s, 0);
        }
        for s in 100..110 {
            g.on_seq(T, 1, s, 1000);
        }
        g.on_seq(T, 2, 305, 2000);
        g.on_seq(T, 2, 306, 3000);
        assert!(g.expire(10_000_000).is_empty());
        // Buraco depois da volta ainda conta.
        g.on_seq(T, 2, 308, 4000);
        assert_eq!(g.expire(10_000_000), vec![(T, 2, 307, 4000)]);
    }

    #[test]
    fn quadros_da_camada_velha_no_meio_da_troca_nao_abrem_buraco() {
        let mut g = GapTracker::new();
        for s in 0..5 {
            g.on_seq(T, 1, s, 0);
        }
        // Keyframe da camada nova chega antes dos últimos quadros da camada velha.
        g.on_seq(T, 0, 900, 1000);
        g.on_seq(T, 1, 5, 1100);
        g.on_seq(T, 0, 901, 1200);
        g.on_seq(T, 0, 902, 1300);
        assert!(g.expire(10_000_000).is_empty());
    }

    #[test]
    fn buraco_antigo_nao_muda_instante_ao_abrir_outro() {
        let mut g = GapTracker::new();
        g.on_seq(T, L, 0, 1000);
        g.on_seq(T, L, 1, 2000);
        g.on_seq(T, L, 3, 5000);
        g.on_seq(T, L, 5, 9000);
        let st = &g.state[&(T, L)];
        assert!(st.missing.contains(&(2, 5000)));
        assert!(st.missing.contains(&(4, 9000)));
    }
}
