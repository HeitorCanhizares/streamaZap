//! Roteamento de quadros: para quais espectadores repassar e quando trocar de camada.

use crate::relay::TrackRelay;

/// Resultado de `TrackRelay::on_frame`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameRouting {
    /// Espectadores (num) que devem receber este quadro.
    pub to: Vec<u64>,
    /// Espectadores que trocaram de camada neste keyframe: `(num, camada nova)`. O chamador manda
    /// `camada.atual` (motivo `banda`, ou `inicio` se nunca anunciou).
    pub switched: Vec<(u64, u8)>,
}

impl TrackRelay {
    /// Decide o destino de um quadro de vídeo da camada `layer`.
    ///
    /// Se `keyframe`: chama `allocator.on_keyframe(layer)` e, em cada `ViewerEntry` com
    /// `pending == Some(layer)`, faz `current = layer; pending = None` (a troca só acontece
    /// aqui, num keyframe da camada nova) e registra em `switched`.
    /// Depois `to` = espectadores com `current == layer` (inclui os que acabaram de trocar).
    /// Quadro não-keyframe nunca troca ninguém.
    pub fn on_frame(&mut self, layer: u8, keyframe: bool) -> FrameRouting {
        let mut switched: Vec<(u64, u8)> = Vec::new();
        if keyframe {
            self.allocator.on_keyframe(layer);
            // Ordem por num: o resultado não depende da ordem de iteração do HashMap.
            let mut nums: Vec<u64> = self.viewers.keys().copied().collect();
            nums.sort_unstable();
            for n in nums {
                if let Some(v) = self.viewers.get_mut(&n) {
                    if v.pending == Some(layer) {
                        v.current = layer;
                        v.pending = None;
                        switched.push((n, layer));
                    }
                }
            }
        }
        let mut to: Vec<u64> = self
            .viewers
            .values()
            .filter(|v| v.current == layer)
            .map(|v| v.num)
            .collect();
        to.sort_unstable();
        FrameRouting { to, switched }
    }

    /// Espectadores (num) que querem áudio (`audio && !preview`).
    pub fn audio_targets(&self) -> Vec<u64> {
        let mut nums: Vec<u64> = self
            .viewers
            .values()
            .filter(|v| v.audio && !v.preview)
            .map(|v| v.num)
            .collect();
        nums.sort_unstable();
        nums
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::relay::ViewerEntry;
    use zap_proto::{DelayState, LayerSpec, TileSize};

    fn camada(layer: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer,
            width: 1280,
            height: 720,
            fps: 30,
            kbps,
        }
    }

    fn escada() -> Vec<LayerSpec> {
        vec![camada(0, 8000), camada(1, 2500), camada(2, 400)]
    }

    fn viewer(num: u64, current: u8, pending: Option<u8>) -> ViewerEntry {
        ViewerEntry {
            participant_id: format!("p{num}"),
            num,
            size: TileSize::Main,
            max_layer_req: None,
            audio: true,
            preview: false,
            bandwidth_kbps: 0,
            state: DelayState::Normal,
            current,
            pending,
            announced: None,
        }
    }

    #[test]
    fn quadro_vai_so_para_camada_atual() {
        let mut r = TrackRelay::new(escada());
        r.viewers.insert(1, viewer(1, 0, None));
        r.viewers.insert(2, viewer(2, 2, None));
        let out = r.on_frame(0, false);
        assert_eq!(out.to, vec![1]);
        assert!(out.switched.is_empty());
        let out = r.on_frame(2, false);
        assert_eq!(out.to, vec![2]);
    }

    #[test]
    fn troca_so_em_keyframe() {
        let mut r = TrackRelay::new(escada());
        r.viewers.insert(1, viewer(1, 2, Some(1)));

        // Quadro sem keyframe da camada nova: nada muda.
        let out = r.on_frame(1, false);
        assert!(out.to.is_empty());
        assert!(out.switched.is_empty());
        assert_eq!(r.viewers[&1].current, 2);
        assert_eq!(r.viewers[&1].pending, Some(1));

        // Keyframe da camada nova: troca.
        let out = r.on_frame(1, true);
        assert_eq!(out.switched, vec![(1, 1)]);
        assert_eq!(out.to, vec![1]);
        assert_eq!(r.viewers[&1].current, 1);
        assert_eq!(r.viewers[&1].pending, None);
        // announced não é tocado aqui: quem anuncia é o chamador.
        assert_eq!(r.viewers[&1].announced, None);
    }

    #[test]
    fn keyframe_de_outra_camada_nao_troca() {
        let mut r = TrackRelay::new(escada());
        r.viewers.insert(1, viewer(1, 2, Some(1)));
        let out = r.on_frame(0, true);
        assert!(out.switched.is_empty());
        assert!(out.to.is_empty());
        assert_eq!(r.viewers[&1].current, 2);
        assert_eq!(r.viewers[&1].pending, Some(1));
    }

    #[test]
    fn audio_targets_ignora_previa_e_sem_audio() {
        let mut r = TrackRelay::new(escada());
        r.viewers.insert(3, viewer(3, 0, None));
        r.viewers.insert(1, viewer(1, 0, None));
        let mut previa = viewer(2, 0, None);
        previa.preview = true;
        r.viewers.insert(2, previa);
        let mut mudo = viewer(4, 0, None);
        mudo.audio = false;
        r.viewers.insert(4, mudo);
        assert_eq!(r.audio_targets(), vec![1, 3]);
    }
}
