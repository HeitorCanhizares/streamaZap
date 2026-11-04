//! Alocador de camadas por espectador (`docs/api/balanceamento.md#3-algoritmo-a-cada-100-ms`).

mod fill;
mod hysteresis;

use crate::Us;
use fill::{fill, FillViewer};
use hysteresis::Hysteresis;
use std::collections::HashMap;
use zap_proto::{DelayState, LayerSpec};

#[derive(Debug, Clone)]
pub struct ViewerInput {
    pub id: u64,
    pub bandwidth_kbps: u32,
    pub state: DelayState,
    /// Teto já combinado (número maior = camada menor).
    pub max_layer: u8,
    /// Áudio + outras trilhas.
    pub reserved_kbps: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum Budget {
    Relay,
    Direct { upload_kbps: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerDecision {
    pub id: u64,
    pub current: u8,
    /// Camada aguardando keyframe.
    pub pending: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRequest {
    pub layer: u8,
    pub kbps: u32,
}

#[derive(Debug, Clone, Default)]
pub struct AllocOutput {
    pub viewers: Vec<ViewerDecision>,
    pub requests: Vec<LayerRequest>,
    /// Camadas que precisam de keyframe agora.
    pub keyframes: Vec<u8>,
}

/// Intervalo mínimo entre keyframes da mesma camada (1 s).
const INTERVALO_KEYFRAME: Us = 1_000_000;
/// Intervalo mínimo entre duas emissões de pedidos (500 ms).
const INTERVALO_PEDIDO: Us = 500_000;

#[derive(Debug, Clone)]
struct ViewerState {
    current: u8,
    pending: Option<u8>,
    hyst: Hysteresis,
}

#[derive(Debug, Clone)]
pub struct Allocator {
    layers: Vec<LayerSpec>,
    viewers: HashMap<u64, ViewerState>,
    last_keyframe: HashMap<u8, Us>,
    last_requests: Vec<LayerRequest>,
    last_request_at: Option<Us>,
}

/// Camada existente mais próxima de `l` (empate: a mais barata).
fn nearest(layers: &[LayerSpec], l: u8) -> u8 {
    layers
        .iter()
        .map(|c| c.layer)
        .min_by_key(|&c| (c.abs_diff(l), std::cmp::Reverse(c)))
        .unwrap_or(l)
}

impl Allocator {
    pub fn new(layers: Vec<LayerSpec>) -> Self {
        Self {
            layers,
            viewers: HashMap::new(),
            last_keyframe: HashMap::new(),
            last_requests: Vec::new(),
            last_request_at: None,
        }
    }

    pub fn set_layers(&mut self, layers: Vec<LayerSpec>) {
        self.layers = layers;
        if self.layers.is_empty() {
            return;
        }
        let layers = &self.layers;
        for v in self.viewers.values_mut() {
            v.current = nearest(layers, v.current);
            v.pending = v.pending.map(|p| nearest(layers, p)).filter(|&p| p != v.current);
        }
    }

    fn kbps_of(&self, layer: u8) -> Option<u32> {
        self.layers.iter().find(|c| c.layer == layer).map(|c| c.kbps)
    }

    pub fn update(&mut self, now: Us, viewers: &[ViewerInput], budget: Budget) -> AllocOutput {
        let Some(cheapest) = self.layers.last().map(|c| c.layer) else {
            return AllocOutput::default();
        };
        // Esquece quem sumiu; cria estado de quem chegou na camada mais barata.
        self.viewers.retain(|id, _| viewers.iter().any(|v| v.id == *id));
        for v in viewers {
            self.viewers.entry(v.id).or_insert_with(|| ViewerState {
                current: cheapest,
                pending: None,
                hyst: Hysteresis::new(),
            });
        }

        let available: Vec<f64> = viewers
            .iter()
            .map(|v| f64::from(v.bandwidth_kbps) * 0.9 - f64::from(v.reserved_kbps))
            .collect();
        let fill_in: Vec<FillViewer> = viewers
            .iter()
            .zip(&available)
            .map(|(v, &a)| FillViewer {
                available_kbps: a,
                max_layer: v.max_layer,
            })
            .collect();
        let upload = match budget {
            Budget::Relay => None,
            Budget::Direct { upload_kbps } => Some(f64::from(upload_kbps) * 0.9),
        };
        let alvo = fill(&self.layers, &fill_in, upload);

        let mut keyframes: Vec<u8> = Vec::new();
        for (i, v) in viewers.iter().enumerate() {
            let reference = {
                let Some(st) = self.viewers.get(&v.id) else { continue };
                st.pending.unwrap_or(st.current)
            };
            let ref_kbps = self.kbps_of(reference).map_or(0.0, f64::from);
            let Some(st) = self.viewers.get_mut(&v.id) else {
                continue;
            };
            let desejado = st.hyst.decide(
                now,
                alvo[i],
                reference,
                available[i],
                ref_kbps,
                v.state == DelayState::Overuse,
            );
            if desejado == st.current {
                // Voltou para a camada atual: cancela o que estava pendente.
                st.pending = None;
            } else if desejado != reference {
                st.pending = Some(desejado);
            }
            // Pendente precisa de keyframe; repete a cada 1 s por camada até chegar.
            if let Some(p) = st.pending {
                let livre = self
                    .last_keyframe
                    .get(&p)
                    .is_none_or(|&t| now.saturating_sub(t) >= INTERVALO_KEYFRAME);
                if livre {
                    self.last_keyframe.insert(p, now);
                    keyframes.push(p);
                }
            }
        }

        // Pedidos por camada em uso (atual ou pendente).
        let mut em_uso: Vec<(u8, f64)> = Vec::new();
        for (v, &a) in viewers.iter().zip(&available) {
            let Some(st) = self.viewers.get(&v.id) else { continue };
            for l in std::iter::once(st.current).chain(st.pending) {
                match em_uso.iter_mut().find(|(c, _)| *c == l) {
                    Some((_, m)) => *m = m.min(a),
                    None => em_uso.push((l, a)),
                }
            }
        }
        em_uso.sort_by_key(|(l, _)| *l);
        let novos: Vec<LayerRequest> = em_uso
            .iter()
            .filter_map(|&(l, m)| {
                let nominal = f64::from(self.kbps_of(l)?);
                let kbps = (m / 1.15).clamp(0.5 * nominal, nominal).round() as u32;
                Some(LayerRequest { layer: l, kbps })
            })
            .collect();

        let mudou = novos.len() != self.last_requests.len()
            || novos.iter().zip(&self.last_requests).any(|(n, o)| {
                n.layer != o.layer || (f64::from(n.kbps) - f64::from(o.kbps)).abs() > 0.1 * f64::from(o.kbps)
            });
        let liberado = self
            .last_request_at
            .is_none_or(|t| now.saturating_sub(t) >= INTERVALO_PEDIDO);
        if mudou && liberado {
            self.last_requests = novos;
            self.last_request_at = Some(now);
        }

        let viewers_out = viewers
            .iter()
            .filter_map(|v| {
                self.viewers.get(&v.id).map(|st| ViewerDecision {
                    id: v.id,
                    current: st.current,
                    pending: st.pending,
                })
            })
            .collect();
        AllocOutput {
            viewers: viewers_out,
            requests: self.last_requests.clone(),
            keyframes,
        }
    }

    pub fn on_keyframe(&mut self, layer: u8) {
        for v in self.viewers.values_mut() {
            if v.pending == Some(layer) {
                v.current = layer;
                v.pending = None;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn vi(id: u64, bw: u32, max: u8) -> ViewerInput {
        ViewerInput {
            id,
            bandwidth_kbps: bw,
            state: DelayState::Normal,
            max_layer: max,
            reserved_kbps: 0,
        }
    }

    /// Avança de 100 em 100 ms, aplicando os keyframes pedidos. Devolve a última saída.
    fn roda(a: &mut Allocator, ini: Us, fim: Us, v: &[ViewerInput], b: Budget) -> AllocOutput {
        let mut t = ini;
        let mut out = AllocOutput::default();
        while t < fim {
            out = a.update(t, v, b);
            for &k in &out.keyframes {
                a.on_keyframe(k);
            }
            t += 100_000;
        }
        out
    }

    #[test]
    fn exemplo_ponte() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 30_000, 0), vi(2, 3_400, 0), vi(3, 20_000, 2)];
        let out = roda(&mut a, 0, 12_000_000, &v, Budget::Relay);
        let cur: Vec<u8> = out.viewers.iter().map(|d| d.current).collect();
        assert_eq!(cur, vec![0, 1, 2]);
        let ls: Vec<u8> = out.requests.iter().map(|r| r.layer).collect();
        assert_eq!(ls, vec![0, 1, 2]);
    }

    #[test]
    fn local_3_espectadores() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 100_000, 0), vi(2, 100_000, 0), vi(3, 100_000, 0)];
        let out = roda(&mut a, 0, 12_000_000, &v, Budget::Direct { upload_kbps: 10_000 });
        assert!(out.viewers.iter().all(|d| d.current == 1));
    }

    #[test]
    fn local_1_espectador() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 100_000, 0)];
        let out = roda(&mut a, 0, 12_000_000, &v, Budget::Direct { upload_kbps: 10_000 });
        assert_eq!(out.viewers[0].current, 0);
    }

    #[test]
    fn overuse_desce_na_hora() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 30_000, 0)];
        let out = roda(&mut a, 0, 5_000_000, &v, Budget::Relay);
        assert_eq!(out.viewers[0].current, 0);
        let mut ruim = vi(1, 1_000, 0);
        ruim.state = DelayState::Overuse;
        let out = a.update(5_000_000, &[ruim], Budget::Relay);
        assert_eq!(out.viewers[0].pending, Some(2));
        assert_eq!(out.keyframes, vec![2]);
    }

    #[test]
    fn keyframe_max_1_por_s_por_camada() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 30_000, 0)];
        let mut t = 0;
        let mut vistos: Vec<Us> = Vec::new();
        while t < 10_000_000 {
            let out = a.update(t, &v, Budget::Relay);
            if out.keyframes.contains(&0) {
                vistos.push(t);
            }
            t += 100_000;
        }
        assert!(!vistos.is_empty());
        assert!(vistos.windows(2).all(|w| w[1] - w[0] >= 1_000_000));
    }

    #[test]
    fn requests_estaveis() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 30_000, 0), vi(2, 3_400, 0)];
        let mut t = 0;
        let mut anterior: Option<Vec<LayerRequest>> = None;
        let mut mudancas: Vec<Us> = Vec::new();
        while t < 10_000_000 {
            let out = a.update(t, &v, Budget::Relay);
            for &k in &out.keyframes {
                a.on_keyframe(k);
            }
            if anterior.as_ref() != Some(&out.requests) {
                mudancas.push(t);
            }
            anterior = Some(out.requests);
            t += 100_000;
        }
        assert!(mudancas.windows(2).all(|w| w[1] - w[0] >= 500_000));
        // Depois de estabilizar, nada muda mais.
        assert!(mudancas.last().copied().unwrap_or(0) < 9_000_000);
    }

    #[test]
    fn espectador_novo_comeca_na_mais_barata() {
        let mut a = Allocator::new(escada());
        let out = a.update(0, &[vi(1, 30_000, 0)], Budget::Relay);
        assert_eq!(out.viewers[0].current, 2);
        assert_eq!(out.viewers[0].pending, None);
    }

    #[test]
    fn set_layers_remove_camada() {
        let mut a = Allocator::new(escada());
        let v = [vi(1, 3_400, 0)];
        let out = roda(&mut a, 0, 6_000_000, &v, Budget::Relay);
        assert_eq!(out.viewers[0].current, 1);
        a.set_layers(vec![camada(0, 8000), camada(2, 400)]);
        let out = a.update(6_000_000, &v, Budget::Relay);
        assert!(out.viewers[0].current == 0 || out.viewers[0].current == 2);
        assert_ne!(out.viewers[0].pending, Some(1));
    }
}
