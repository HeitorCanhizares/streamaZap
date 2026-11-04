//! Medidas de um tile (para `TileView`).

use parking_lot::Mutex;

/// Janela de medição: 1 s, em microssegundos.
const JANELA_US: u64 = 1_000_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricsView {
    pub fps: f32,
    pub kbps: u32,
    pub layer: u8,
    pub width: u16,
    pub height: u16,
    pub reason: String,
}

#[derive(Default)]
pub struct TileMetrics {
    pub(crate) inner: Mutex<MetricsInner>,
}

#[derive(Default)]
pub(crate) struct MetricsInner {
    /// `(instante us, bytes)` dos quadros da última 1 s.
    pub frames: std::collections::VecDeque<(u64, u32)>,
    pub layer: u8,
    pub width: u16,
    pub height: u16,
    pub reason: String,
}

impl MetricsInner {
    /// Tira da frente os quadros fora da janela de 1 s em relação a `now_us`.
    /// `saturating_sub` evita estouro se um quadro vier com instante maior que `now_us`.
    fn podar(&mut self, now_us: u64) {
        while let Some(&(t, _)) = self.frames.front() {
            if now_us.saturating_sub(t) > JANELA_US {
                self.frames.pop_front();
            } else {
                break;
            }
        }
    }
}

impl TileMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    /// Registra um quadro recebido (descarta da janela o que tem mais de 1 s).
    pub fn on_frame(&self, now_us: u64, bytes: u32, layer: u8, width: u16, height: u16) {
        let mut m = self.inner.lock();
        m.frames.push_back((now_us, bytes));
        m.podar(now_us);
        m.layer = layer;
        m.width = width;
        m.height = height;
    }
    pub fn set_reason(&self, reason: &str) {
        let mut m = self.inner.lock();
        m.reason.clear();
        m.reason.push_str(reason);
    }
    /// fps = quadros na janela de 1 s; kbps = bytes*8/1000 na janela.
    pub fn view(&self, now_us: u64) -> MetricsView {
        let mut m = self.inner.lock();
        m.podar(now_us);
        let fps = m.frames.len() as f32;
        // u64 para a soma não estourar com muitos quadros grandes na janela.
        let soma_bytes: u64 = m.frames.iter().map(|&(_, b)| u64::from(b)).sum();
        let kbps = (soma_bytes * 8 / 1000) as u32;
        MetricsView {
            fps,
            kbps,
            layer: m.layer,
            width: m.width,
            height: m.height,
            reason: m.reason.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fps_conta_quadros_do_ultimo_segundo() {
        let m = TileMetrics::new();
        // 60 quadros espaçados ~16_667 us (~60 fps).
        for i in 0..60u64 {
            m.on_frame(i * 16_667, 1000, 0, 640, 360);
        }
        let v = m.view(59 * 16_667);
        assert!((v.fps - 60.0).abs() < 0.5, "fps = {}", v.fps);
    }

    #[test]
    fn quadros_antigos_saem_da_janela() {
        let m = TileMetrics::new();
        m.on_frame(0, 1000, 0, 640, 360);
        m.on_frame(100_000, 1000, 0, 640, 360);
        assert_eq!(m.view(100_000).fps, 2.0);
        // Em 2 s os dois já passaram da janela de 1 s.
        let v = m.view(2_000_000);
        assert_eq!(v.fps, 0.0);
        assert_eq!(v.kbps, 0);
    }

    #[test]
    fn kbps_soma_bytes() {
        let m = TileMetrics::new();
        // 10 quadros de 12_500 bytes em 1 s = 125_000 bytes = 1000 kbps.
        for i in 0..10u64 {
            m.on_frame(i * 100_000, 12_500, 0, 640, 360);
        }
        assert_eq!(m.view(900_000).kbps, 1000);
    }

    #[test]
    fn view_sem_quadros_e_zero() {
        let m = TileMetrics::new();
        m.set_reason("sem pacotes");
        let v = m.view(5_000_000);
        assert_eq!(v.fps, 0.0);
        assert_eq!(v.kbps, 0);
        assert_eq!(v.layer, 0);
        assert_eq!(v.width, 0);
        assert_eq!(v.height, 0);
        assert_eq!(v.reason, "sem pacotes");
    }

    #[test]
    fn reason_e_camada_ultimos_valores() {
        let m = TileMetrics::new();
        m.on_frame(0, 100, 1, 320, 180);
        m.on_frame(10_000, 100, 3, 1280, 720);
        m.set_reason("primeiro");
        m.set_reason("ultimo");
        let v = m.view(10_000);
        assert_eq!(v.layer, 3);
        assert_eq!(v.width, 1280);
        assert_eq!(v.height, 720);
        assert_eq!(v.reason, "ultimo");
    }

    #[test]
    fn thread_safe() {
        let m = Arc::new(TileMetrics::new());
        let handles: Vec<_> = (0..4u64)
            .map(|t| {
                let m = Arc::clone(&m);
                std::thread::spawn(move || {
                    for i in 0..1000u64 {
                        m.on_frame(t * 1_000_000 + i, 100, 0, 640, 360);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("thread de teste não deve panicar");
        }
        let _ = m.view(1_000);
    }
}
