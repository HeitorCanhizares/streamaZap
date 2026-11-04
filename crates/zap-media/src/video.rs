//! `VideoPipeline`: fachada comum ao backend sintético e ao real (Windows).
use crate::{EncodedFrame, MediaError, PipelineStats, VideoPipelineConfig, VideoSourceSpec};
use crossbeam_channel::Sender;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use zap_proto::LayerSpec;
use zap_rate::allocator::LayerRequest;

/// Estado de controle compartilhado entre a fachada e a thread de vídeo.
pub(crate) struct Control {
    pub ladder: Vec<LayerSpec>,
    /// `(camada, kbps)` ligadas. Camada fora da lista = desligada.
    pub requests: Vec<(u8, u32)>,
    /// Camadas com keyframe pendente.
    pub keyframes: Vec<u8>,
    /// A escada mudou: a thread de vídeo reconstrói camadas.
    pub ladder_dirty: bool,
    /// `requests` mudou desde a última leitura da thread.
    pub requests_dirty: bool,
}

pub(crate) struct Shared {
    pub control: Mutex<Control>,
    pub stats: Mutex<PipelineStats>,
    pub stop: AtomicBool,
    pub dropped_capture: AtomicU64,
}

impl Shared {
    pub(crate) fn new(ladder: Vec<LayerSpec>) -> Self {
        Self {
            control: Mutex::new(Control {
                ladder,
                requests: vec![],
                keyframes: vec![],
                ladder_dirty: false,
                requests_dirty: false,
            }),
            stats: Mutex::new(PipelineStats::default()),
            stop: AtomicBool::new(false),
            dropped_capture: AtomicU64::new(0),
        }
    }
    /// Consome o pedido de keyframe da camada (true se havia).
    pub(crate) fn take_keyframe(&self, layer: u8) -> bool {
        let mut c = self.control.lock();
        let antes = c.keyframes.len();
        c.keyframes.retain(|&l| l != layer);
        c.keyframes.len() != antes
    }
}

pub struct VideoPipeline {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl VideoPipeline {
    /// `Synthetic` -> backend sintético (qualquer plataforma). Outras fontes: Windows real;
    /// fora do Windows devolve `MediaError::Unsupported`. Escada vazia = `InvalidConfig`.
    /// Nenhuma camada começa ligada: o chamador liga com `set_requests`.
    pub fn start(cfg: VideoPipelineConfig, out: Sender<EncodedFrame>) -> Result<Self, MediaError> {
        if cfg.ladder.is_empty() {
            return Err(MediaError::InvalidConfig("escada vazia".into()));
        }
        for (i, a) in cfg.ladder.iter().enumerate() {
            if cfg.ladder[..i].iter().any(|b| b.layer == a.layer) {
                return Err(MediaError::InvalidConfig(format!(
                    "camada {} duplicada na escada",
                    a.layer
                )));
            }
        }
        let shared = Arc::new(Shared::new(cfg.ladder.clone()));
        let handle = match cfg.source {
            VideoSourceSpec::Synthetic { .. } => crate::synth_video::spawn(cfg, shared.clone(), out)?,
            #[cfg(windows)]
            _ => crate::win_video::spawn(cfg, shared.clone(), out)?,
            #[cfg(not(windows))]
            _ => return Err(MediaError::Unsupported("captura real só no Windows".into())),
        };
        Ok(Self {
            shared,
            thread: Some(handle),
        })
    }
    /// Liga as camadas listadas (com o kbps dado, repassado ao codificador) e desliga as demais.
    pub fn set_requests(&self, reqs: &[LayerRequest]) {
        let mut c = self.shared.control.lock();
        let novas: Vec<(u8, u32)> = reqs
            .iter()
            .filter(|r| c.ladder.iter().any(|l| l.layer == r.layer))
            .map(|r| (r.layer, r.kbps))
            .collect();
        c.requests = novas;
        c.requests_dirty = true;
    }
    pub fn request_keyframe(&self, layer: u8) {
        let mut c = self.shared.control.lock();
        if !c.keyframes.contains(&layer) {
            c.keyframes.push(layer);
        }
    }
    pub fn set_ladder(&self, ladder: Vec<LayerSpec>) {
        let mut c = self.shared.control.lock();
        // Pedidos de camadas que sumiram da escada não valem mais.
        c.requests.retain(|(l, _)| ladder.iter().any(|s| s.layer == *l));
        c.ladder = ladder;
        c.ladder_dirty = true;
    }
    pub fn stats(&self) -> PipelineStats {
        let mut st = self.shared.stats.lock().clone();
        st.dropped_capture = self.shared.dropped_capture.load(Ordering::Relaxed);
        st
    }
}

impl Drop for VideoPipeline {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.thread.take() {
            if h.join().is_err() {
                tracing::warn!("thread de vídeo terminou com pânico");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn camada(layer: u8) -> LayerSpec {
        LayerSpec {
            layer,
            width: 640,
            height: 360,
            fps: 30,
            kbps: 1000,
        }
    }

    fn cfg(ladder: Vec<LayerSpec>, source: VideoSourceSpec) -> VideoPipelineConfig {
        VideoPipelineConfig {
            track: 1,
            source,
            ladder,
            encoder: crate::EncoderChoiceSpec::Auto,
        }
    }

    fn sintetico() -> VideoSourceSpec {
        VideoSourceSpec::Synthetic {
            width: 640,
            height: 360,
        }
    }

    /// Pipeline sem thread, só para exercitar os controles.
    fn sem_thread(ladder: Vec<LayerSpec>) -> VideoPipeline {
        VideoPipeline {
            shared: Arc::new(Shared::new(ladder)),
            thread: None,
        }
    }

    fn req(layer: u8, kbps: u32) -> LayerRequest {
        LayerRequest { layer, kbps }
    }

    #[test]
    fn start_escada_vazia_erro() {
        let (tx, _rx) = crossbeam_channel::bounded(4);
        let r = VideoPipeline::start(cfg(vec![], sintetico()), tx);
        assert!(matches!(r, Err(MediaError::InvalidConfig(_))));
    }

    #[test]
    fn start_camadas_duplicadas_erro() {
        let (tx, _rx) = crossbeam_channel::bounded(4);
        let r = VideoPipeline::start(cfg(vec![camada(0), camada(0)], sintetico()), tx);
        assert!(matches!(r, Err(MediaError::InvalidConfig(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn start_monitor_fora_do_windows_unsupported() {
        let (tx, _rx) = crossbeam_channel::bounded(4);
        let c = cfg(vec![camada(0)], VideoSourceSpec::Monitor(crate::MonitorRef(1)));
        assert!(matches!(VideoPipeline::start(c, tx), Err(MediaError::Unsupported(_))));
    }

    #[test]
    fn set_requests_ignora_camada_fora_da_escada() {
        let p = sem_thread(vec![camada(0), camada(1)]);
        p.set_requests(&[req(0, 500), req(7, 900), req(1, 800)]);
        let c = p.shared.control.lock();
        assert_eq!(c.requests, vec![(0, 500), (1, 800)]);
        assert!(c.requests_dirty);
    }

    #[test]
    fn request_keyframe_nao_duplica() {
        let p = sem_thread(vec![camada(0)]);
        p.request_keyframe(0);
        p.request_keyframe(0);
        assert_eq!(p.shared.control.lock().keyframes, vec![0]);
    }

    #[test]
    fn take_keyframe_consome() {
        let p = sem_thread(vec![camada(0)]);
        p.request_keyframe(0);
        assert!(p.shared.take_keyframe(0));
        assert!(!p.shared.take_keyframe(0));
    }

    #[test]
    fn set_ladder_marca_dirty_e_filtra_requests() {
        let p = sem_thread(vec![camada(0), camada(1)]);
        p.set_requests(&[req(0, 500), req(1, 800)]);
        p.set_ladder(vec![camada(1)]);
        let c = p.shared.control.lock();
        assert!(c.ladder_dirty);
        assert_eq!(c.requests, vec![(1, 800)]);
        assert_eq!(c.ladder.len(), 1);
    }

    // Depende de media-02 (backend sintético).
    #[test]
    fn drop_encerra_thread_rapido() {
        let (tx, _rx) = crossbeam_channel::bounded(64);
        let p = VideoPipeline::start(cfg(vec![camada(0)], sintetico()), tx).expect("start");
        let t = Instant::now();
        drop(p);
        assert!(t.elapsed() < Duration::from_secs(1));
    }
}
