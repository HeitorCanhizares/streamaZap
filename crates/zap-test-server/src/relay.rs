//! Alocador de camadas por trilha (`docs/api/balanceamento.md`).
//!
//! Cada trilha de vídeo tem um `TrackRelay`: a lista de assinantes (espectadores) e um
//! `zap_rate::Allocator` em modo `Relay`. O servidor de produção faz o mesmo: alimenta o
//! alocador com o `banda_kbps` e o `estado` dos relatórios de cada espectador e age sobre a saída.
//!
//! Tempo entra como parâmetro (`now_us`), então tudo é testável sem relógio real.

use std::collections::HashMap;

use zap_proto::{DelayState, LayerSpec, TileSize};
use zap_rate::{
    allocator::{Allocator, Budget, LayerRequest, ViewerInput},
    Us,
};

/// Um espectador assinando a trilha.
#[derive(Debug, Clone)]
pub struct ViewerEntry {
    pub participant_id: String,
    /// Id numérico no alocador (`ParticipantData::num`).
    pub num: u64,
    pub size: TileSize,
    /// `assinar.camada_max` (ausente = automático).
    pub max_layer_req: Option<u8>,
    /// `assinar.audio` (padrão `true`).
    pub audio: bool,
    /// Ingresso de prévia: teto fixo 2, sem áudio.
    pub preview: bool,
    /// Último `relatorio.banda_kbps`; 0 antes do primeiro relatório (= camada mais barata).
    pub bandwidth_kbps: u32,
    pub state: DelayState,
    /// Camada que o espectador recebe agora.
    pub current: u8,
    /// Camada aguardando keyframe.
    pub pending: Option<u8>,
    /// Última camada anunciada com `camada.atual` (`None` = ainda não anunciou).
    pub announced: Option<u8>,
}

/// O que o ticker deve fazer depois de um `tick`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayEvent {
    /// Mandar `camada.atual` ao espectador `viewer` (num) com o `motivo`.
    LayerChanged {
        viewer: u64,
        layer: u8,
        reason: &'static str,
    },
    /// Mandar `camadas.pedidas` ao publicador.
    LayersRequested(Vec<LayerRequest>),
    /// Mandar `keyframe.pedir` ao publicador para esta camada.
    KeyframeNeeded(u8),
}

pub struct TrackRelay {
    pub allocator: Allocator,
    pub layers: Vec<LayerSpec>,
    pub viewers: HashMap<u64, ViewerEntry>,
    /// Último `LayersRequested` emitido (evita repetir o mesmo pedido).
    last_requests: Vec<LayerRequest>,
}

/// Camada mais barata existente (maior número); 0 se a escada estiver vazia.
fn cheapest(layers: &[LayerSpec]) -> u8 {
    layers.iter().map(|l| l.layer).max().unwrap_or(0)
}

/// Camada existente mais próxima de `l` (empate: a mais barata).
fn nearest(layers: &[LayerSpec], l: u8) -> u8 {
    layers
        .iter()
        .map(|c| c.layer)
        .min_by_key(|&c| (c.abs_diff(l), std::cmp::Reverse(c)))
        .unwrap_or(l)
}

/// Teto combinado (número maior = camada menor): `max(camada_max.unwrap_or(0), teto do tamanho)`,
/// com miniatura = 2 e principal/cheia = 0; prévia = 2 sempre; limitado à camada mais barata
/// existente em `layers`.
pub fn max_layer_for(v: &ViewerEntry, layers: &[LayerSpec]) -> u8 {
    let base = match v.size {
        TileSize::Thumbnail => 2,
        TileSize::Main | TileSize::Full => 0,
    };
    let mut teto = v.max_layer_req.unwrap_or(0).max(base);
    if v.preview {
        teto = 2;
    }
    teto.min(cheapest(layers))
}

impl TrackRelay {
    pub fn new(layers: Vec<LayerSpec>) -> Self {
        Self {
            allocator: Allocator::new(layers.clone()),
            layers,
            viewers: HashMap::new(),
            last_requests: Vec::new(),
        }
    }

    /// Troca a escada (`trilha.atualizar`): `Allocator::set_layers` e realinha `current`/`pending`.
    pub fn set_layers(&mut self, layers: Vec<LayerSpec>) {
        self.layers = layers.clone();
        self.allocator.set_layers(layers);
        if self.layers.is_empty() {
            return;
        }
        for v in self.viewers.values_mut() {
            v.current = nearest(&self.layers, v.current);
            v.pending = v.pending.map(|p| nearest(&self.layers, p)).filter(|&p| p != v.current);
        }
    }

    /// Adiciona ou substitui (mesmo `num`) um assinante. Novo assinante começa em `current` =
    /// camada mais barata, `announced` = `None`, `bandwidth_kbps` = 0. Substituir preserva
    /// `bandwidth_kbps`, `state`, `current`, `pending` e `announced` já conhecidos.
    pub fn subscribe(&mut self, mut entry: ViewerEntry) {
        if let Some(old) = self.viewers.get(&entry.num) {
            entry.bandwidth_kbps = old.bandwidth_kbps;
            entry.state = old.state;
            entry.current = old.current;
            entry.pending = old.pending;
            entry.announced = old.announced;
        } else {
            entry.current = cheapest(&self.layers);
            entry.pending = None;
            entry.announced = None;
            entry.bandwidth_kbps = 0;
            entry.state = DelayState::Normal;
        }
        self.viewers.insert(entry.num, entry);
    }

    pub fn unsubscribe(&mut self, num: u64) {
        self.viewers.remove(&num);
    }

    /// Atualiza `banda_kbps`/`estado` do espectador (campos ausentes não mudam).
    pub fn on_report(&mut self, num: u64, bandwidth_kbps: Option<u32>, state: Option<DelayState>) {
        if let Some(v) = self.viewers.get_mut(&num) {
            if let Some(b) = bandwidth_kbps {
                v.bandwidth_kbps = b;
            }
            if let Some(s) = state {
                v.state = s;
            }
        }
    }

    /// Roda `Allocator::update(now_us, ..., Budget::Relay)` com `ViewerInput` de cada assinante
    /// (`reserved_kbps` = 0 no servidor de teste), copia `current`/`pending` de volta para os
    /// `ViewerEntry` e devolve eventos: `LayerChanged` para quem tem `announced != Some(current)`
    /// (motivo `inicio` se `announced` era `None`, senão `banda`, e `teto` quando o espectador
    /// está no teto e a banda comportaria a camada acima; atualiza `announced`);
    /// `LayersRequested` quando `AllocOutput.requests` não está vazio e mudou desde o último
    /// evento emitido; `KeyframeNeeded` para cada camada em `AllocOutput.keyframes`.
    pub fn tick(&mut self, now_us: Us) -> Vec<RelayEvent> {
        if self.viewers.is_empty() {
            return Vec::new();
        }
        // Ordem estável por id para o alocador ser determinístico.
        let mut nums: Vec<u64> = self.viewers.keys().copied().collect();
        nums.sort_unstable();
        let inputs: Vec<ViewerInput> = nums
            .iter()
            .filter_map(|n| self.viewers.get(n))
            .map(|v| ViewerInput {
                id: v.num,
                bandwidth_kbps: v.bandwidth_kbps,
                state: v.state,
                max_layer: max_layer_for(v, &self.layers),
                reserved_kbps: 0,
            })
            .collect();
        let out = self.allocator.update(now_us, &inputs, Budget::Relay);

        for d in &out.viewers {
            if let Some(v) = self.viewers.get_mut(&d.id) {
                v.current = d.current;
                v.pending = d.pending;
            }
        }

        let mut events = Vec::new();
        for n in &nums {
            let Some(v) = self.viewers.get(n) else { continue };
            if v.announced == Some(v.current) {
                continue;
            }
            let reason = if v.announced.is_none() {
                "inicio"
            } else {
                let teto = max_layer_for(v, &self.layers);
                // Camada logo acima (mais cara) da atual, se existir.
                let acima = self
                    .layers
                    .iter()
                    .filter(|l| l.layer < v.current)
                    .max_by_key(|l| l.layer);
                match acima {
                    Some(a) if v.current == teto && f64::from(v.bandwidth_kbps) * 0.9 >= f64::from(a.kbps) => "teto",
                    _ => "banda",
                }
            };
            events.push(RelayEvent::LayerChanged {
                viewer: v.num,
                layer: v.current,
                reason,
            });
        }
        for e in &events {
            if let RelayEvent::LayerChanged { viewer, layer, .. } = e {
                if let Some(v) = self.viewers.get_mut(viewer) {
                    v.announced = Some(*layer);
                }
            }
        }

        if !out.requests.is_empty() && out.requests != self.last_requests {
            self.last_requests = out.requests.clone();
            events.push(RelayEvent::LayersRequested(out.requests.clone()));
        }
        events.extend(out.keyframes.iter().map(|&c| RelayEvent::KeyframeNeeded(c)));
        events
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

    fn viewer(num: u64, size: TileSize) -> ViewerEntry {
        ViewerEntry {
            participant_id: format!("p{num}"),
            num,
            size,
            max_layer_req: None,
            audio: true,
            preview: false,
            bandwidth_kbps: 0,
            state: DelayState::Normal,
            current: 0,
            pending: None,
            announced: None,
        }
    }

    #[test]
    fn max_layer_miniatura() {
        let l = escada();
        assert_eq!(max_layer_for(&viewer(1, TileSize::Thumbnail), &l), 2);
        assert_eq!(max_layer_for(&viewer(1, TileSize::Main), &l), 0);
        let mut v = viewer(1, TileSize::Main);
        v.max_layer_req = Some(1);
        assert_eq!(max_layer_for(&v, &l), 1);
        let mut p = viewer(1, TileSize::Main);
        p.preview = true;
        assert_eq!(max_layer_for(&p, &l), 2);
        let duas = vec![camada(0, 8000), camada(2, 400)];
        p.max_layer_req = Some(5);
        assert_eq!(max_layer_for(&p, &duas), 2);
        assert_eq!(max_layer_for(&viewer(1, TileSize::Thumbnail), &[camada(0, 1000)]), 0);
    }

    #[test]
    fn novo_assinante_comeca_barato() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        let ev = r.tick(0);
        assert!(ev.contains(&RelayEvent::LayerChanged {
            viewer: 1,
            layer: 2,
            reason: "inicio"
        }));
        assert!(r
            .tick(100_000)
            .iter()
            .all(|e| !matches!(e, RelayEvent::LayerChanged { .. })));
    }

    #[test]
    fn sem_assinantes_sem_eventos() {
        let mut r = TrackRelay::new(escada());
        assert!(r.tick(0).is_empty());
    }

    #[test]
    fn relatorio_sobe_camada() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        r.on_report(1, Some(30000), Some(DelayState::Normal));
        let mut keyframe = false;
        let mut pendente = false;
        for i in 0..=100u64 {
            let ev = r.tick(i * 100_000);
            keyframe |= ev.iter().any(|e| matches!(e, RelayEvent::KeyframeNeeded(_)));
            pendente |= r.viewers[&1].pending.is_some();
        }
        assert!(keyframe);
        assert!(pendente);
        // Sem keyframe confirmado, current continua na camada barata.
        assert!(r.viewers[&1].current <= 2);
    }

    #[test]
    fn dois_viewers_camadas_diferentes() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        r.subscribe(viewer(2, TileSize::Main));
        r.on_report(1, Some(30000), None);
        r.on_report(2, Some(3400), None);
        let mut pedidas: Vec<u8> = Vec::new();
        for i in 0..=200u64 {
            let ev = r.tick(i * 100_000);
            for e in ev {
                match e {
                    RelayEvent::KeyframeNeeded(c) => {
                        r.allocator.on_keyframe(c);
                        for v in r.viewers.values_mut() {
                            if v.pending == Some(c) {
                                v.current = c;
                                v.pending = None;
                            }
                        }
                    }
                    RelayEvent::LayersRequested(reqs) => {
                        for q in reqs {
                            let nominal = r.layers.iter().find(|l| l.layer == q.layer).unwrap().kbps;
                            assert!(q.kbps <= nominal);
                            pedidas.push(q.layer);
                        }
                    }
                    RelayEvent::LayerChanged { .. } => {}
                }
            }
        }
        assert!(pedidas.contains(&2));
        assert!(pedidas.len() >= 2);
    }

    #[test]
    fn unsubscribe_remove() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        r.unsubscribe(1);
        assert!(r.viewers.is_empty());
        assert!(r.tick(0).is_empty());
    }

    #[test]
    fn subscribe_substituir_preserva_estado() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        r.on_report(1, Some(5000), Some(DelayState::Underuse));
        r.tick(0);
        let mut novo = viewer(1, TileSize::Thumbnail);
        novo.bandwidth_kbps = 7;
        r.subscribe(novo);
        let v = &r.viewers[&1];
        assert_eq!(v.size, TileSize::Thumbnail);
        assert_eq!(v.bandwidth_kbps, 5000);
        assert_eq!(v.state, DelayState::Underuse);
        assert_eq!(v.current, 2);
        assert_eq!(v.announced, Some(2));
    }

    #[test]
    fn set_layers_realinha() {
        let mut r = TrackRelay::new(escada());
        r.subscribe(viewer(1, TileSize::Main));
        r.viewers.get_mut(&1).unwrap().current = 1;
        r.viewers.get_mut(&1).unwrap().pending = Some(0);
        r.set_layers(vec![camada(0, 4000), camada(2, 300)]);
        let v = &r.viewers[&1];
        assert_eq!(v.current, 2); // empate 0/2: a mais barata
        assert_eq!(v.pending, Some(0));
        r.set_layers(vec![camada(0, 4000)]);
        let v = &r.viewers[&1];
        assert_eq!(v.current, 0);
        assert_eq!(v.pending, None);
    }
}
