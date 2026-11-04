//! Trilha de saída: produtor (`TrackInput`, thread da rede) e leitor (`TrackReader`, callback de áudio).
//! Portável (não usa cpal) para ser testável no Linux.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use rubato::{FastFixedIn, PolynomialDegree, Resampler};

use crate::FRAME_SAMPLES;

/// Estado compartilhado por atômicos entre produtor e leitor (f32/f64 guardados como bits).
pub(crate) struct TrackShared {
    pub volume_bits: AtomicU32,
    pub rate_bits: AtomicU64,
    /// Amostras f32 (todas as vias) atualmente no anel.
    pub buffered_samples: AtomicU64,
}

/// Lado produtor, entregue ao chamador de `AudioOut::add_track`.
pub struct TrackInput {
    pub(crate) producer: rtrb::Producer<f32>,
    pub(crate) shared: Arc<TrackShared>,
}

/// Quadros (por canal) consumidos do anel a cada chamada do reamostrador.
const CHUNK: usize = 32;
/// Maior sub-bloco de saída processado de uma vez (quadros); blocos maiores são fatiados.
const BLOCK: usize = 2048;
/// Folga do estágio: o reamostrador produz até ~43 quadros por chunk.
const STAGE_SLACK: usize = 128;
/// Razão relativa máxima aceita pelo reamostrador (cobre 1/0.98).
const MAX_REL: f64 = 1.1;
/// Fade de 5 ms @48 kHz, em quadros.
const FADE_FRAMES: f32 = 240.0;

/// Lado consumidor, vive no callback de áudio. Não pode alocar nem travar em `render_add`.
pub struct TrackReader {
    pub(crate) consumer: rtrb::Consumer<f32>,
    pub(crate) shared: Arc<TrackShared>,
    /// `None` só se a construção falhar (não esperado); aí a taxa é ignorada.
    resampler: Option<Box<FastFixedIn<f32>>>,
    in_bufs: Vec<Vec<f32>>,
    out_bufs: Vec<Vec<f32>>,
    /// Amostras intercaladas prontas para tocar (já reamostradas).
    stage: Vec<f32>,
    stage_len: usize,
    /// Taxa aplicada por último ao reamostrador.
    last_rate: f64,
    last_volume: f32,
    /// Ganho do fade de starvation (1.0 = som pleno).
    gain: f32,
    starved: bool,
    last_l: f32,
    last_r: f32,
}

/// Capacidade do anel em quadros de 20 ms (500 ms).
pub const RING_FRAMES: usize = 25;

/// Cria o par produtor/leitor de uma trilha (volume 1.0, taxa 1.0).
pub fn new_track() -> (TrackInput, TrackReader) {
    let (producer, consumer) = rtrb::RingBuffer::<f32>::new(RING_FRAMES * FRAME_SAMPLES);
    let shared = Arc::new(TrackShared {
        volume_bits: AtomicU32::new(1.0f32.to_bits()),
        rate_bits: AtomicU64::new(1.0f64.to_bits()),
        buffered_samples: AtomicU64::new(0),
    });
    // Tudo pré-alocado aqui: o callback de áudio não pode alocar.
    let resampler = FastFixedIn::<f32>::new(1.0, MAX_REL, PolynomialDegree::Cubic, CHUNK, 2)
        .ok()
        .map(Box::new);
    let out_cap = resampler.as_ref().map_or(CHUNK * 2, |r| r.output_frames_max());
    let reader = TrackReader {
        consumer,
        shared: shared.clone(),
        resampler,
        in_bufs: vec![vec![0.0; CHUNK]; 2],
        out_bufs: vec![vec![0.0; out_cap]; 2],
        stage: vec![0.0; (BLOCK + STAGE_SLACK) * 2],
        stage_len: 0,
        last_rate: 1.0,
        last_volume: 1.0,
        gain: 1.0,
        starved: false,
        last_l: 0.0,
        last_r: 0.0,
    };
    (TrackInput { producer, shared }, reader)
}

impl TrackInput {
    /// Empurra PCM f32 estéreo intercalado @48 kHz. Devolve quantas amostras coube (descarta o resto).
    pub fn push(&mut self, pcm: &[f32]) -> usize {
        let n = pcm.len().min(self.producer.slots());
        if n == 0 {
            return 0;
        }
        // Soma antes de escrever: o leitor nunca subtrai algo ainda não contado.
        self.shared.buffered_samples.fetch_add(n as u64, Ordering::Release);
        match self.producer.write_chunk_uninit(n) {
            Ok(chunk) => chunk.fill_from_iter(pcm[..n].iter().copied()),
            Err(_) => {
                self.shared.buffered_samples.fetch_sub(n as u64, Ordering::Release);
                0
            }
        }
    }
    /// Volume linear 0.0..=2.0 (fora disso é limitado).
    pub fn set_volume(&self, v: f32) {
        let v = if v.is_nan() { 1.0 } else { v.clamp(0.0, 2.0) };
        self.shared.volume_bits.store(v.to_bits(), Ordering::Relaxed);
    }
    /// Razão de reprodução, limitada a 0.98..=1.02.
    pub fn set_rate(&self, r: f64) {
        let r = if r.is_nan() { 1.0 } else { r.clamp(0.98, 1.02) };
        self.shared.rate_bits.store(r.to_bits(), Ordering::Relaxed);
    }
    /// Quanto áudio espera no anel, em ms.
    pub fn buffered_ms(&self) -> f32 {
        // 48 amostras por ms por canal, 2 canais.
        self.shared.buffered_samples.load(Ordering::Acquire) as f32 / (48.0 * 2.0)
    }
}

impl TrackReader {
    /// Move `k` amostras (par) do anel para o fim do estágio, sem reamostrar.
    fn pop_to_stage(&mut self, k: usize) {
        if k == 0 {
            return;
        }
        if let Ok(chunk) = self.consumer.read_chunk(k) {
            let (a, b) = chunk.as_slices();
            let dst = &mut self.stage[self.stage_len..self.stage_len + k];
            dst[..a.len()].copy_from_slice(a);
            dst[a.len()..].copy_from_slice(b);
            chunk.commit_all();
            self.stage_len += k;
            self.shared.buffered_samples.fetch_sub(k as u64, Ordering::Release);
        }
    }

    /// Garante (se houver dados) `n` quadros no estágio.
    fn fill_stage(&mut self, n: usize, rate: f64) {
        if self.stage_len / 2 >= n {
            return;
        }
        let direct = (rate - 1.0).abs() < 1e-6 || self.resampler.is_none();
        if direct {
            let want = (n - self.stage_len / 2) * 2;
            let k = want.min(self.consumer.slots()) & !1;
            self.pop_to_stage(k);
            return;
        }
        if (rate - self.last_rate).abs() > 1e-5 {
            if let Some(r) = self.resampler.as_mut() {
                // Razão de saída/entrada = 1/rate; com rampa para evitar salto de tom.
                let _ = r.set_resample_ratio_relative(1.0 / rate, true);
            }
            self.last_rate = rate;
        }
        while self.stage_len / 2 < n && self.consumer.slots() >= CHUNK * 2 {
            let Ok(chunk) = self.consumer.read_chunk(CHUNK * 2) else {
                break;
            };
            let (a, b) = chunk.as_slices();
            for (i, &s) in a.iter().chain(b.iter()).enumerate() {
                self.in_bufs[i & 1][i >> 1] = s;
            }
            chunk.commit_all();
            self.shared
                .buffered_samples
                .fetch_sub((CHUNK * 2) as u64, Ordering::Release);
            let Some(r) = self.resampler.as_mut() else {
                break;
            };
            let Ok((_, produced)) = r.process_into_buffer(&self.in_bufs, &mut self.out_bufs, None) else {
                break;
            };
            let room = (self.stage.len() - self.stage_len) / 2;
            for i in 0..produced.min(room) {
                self.stage[self.stage_len] = self.out_bufs[0][i];
                self.stage[self.stage_len + 1] = self.out_bufs[1][i];
                self.stage_len += 2;
            }
        }
    }

    /// Lê da trilha, aplica taxa (reamostrador de razão variável) e volume e SOMA em `out`
    /// (estéreo intercalado @48 kHz). Sem dados suficientes: completa com silêncio usando fade
    /// de 5 ms (sem estalo) e volta com fade ao chegarem dados.
    pub fn render_add(&mut self, out: &mut [f32]) {
        let rate = f64::from_bits(self.shared.rate_bits.load(Ordering::Relaxed));
        let target = f32::from_bits(self.shared.volume_bits.load(Ordering::Relaxed));
        let frames = out.len() / 2;
        for chunk in out[..frames * 2].chunks_mut(BLOCK * 2) {
            let n = chunk.len() / 2;
            self.fill_stage(n, rate);
            let avail = (self.stage_len / 2).min(n);
            self.starved = avail < n;
            let v0 = self.last_volume;
            for (i, frame) in chunk.chunks_exact_mut(2).enumerate() {
                if i < avail {
                    self.last_l = self.stage[2 * i];
                    self.last_r = self.stage[2 * i + 1];
                    self.gain = (self.gain + 1.0 / FADE_FRAMES).min(1.0);
                } else {
                    // Segura a última amostra e apaga em 5 ms em vez de cortar para zero.
                    self.gain = (self.gain - 1.0 / FADE_FRAMES).max(0.0);
                }
                let v = v0 + (target - v0) * ((i + 1) as f32 / n as f32);
                frame[0] += self.last_l * self.gain * v;
                frame[1] += self.last_r * self.gain * v;
            }
            self.last_volume = target;
            let used = avail * 2;
            self.stage.copy_within(used..self.stage_len, 0);
            self.stage_len -= used;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<f32> {
        (0..n).map(|i| (i as f32) * 1e-5).collect()
    }

    #[test]
    fn push_e_buffered_ms() {
        let (mut tx, _rx) = new_track();
        assert_eq!(tx.push(&vec![0.1; 960 * 2]), 960 * 2);
        assert!((tx.buffered_ms() - 20.0).abs() < 0.01);
    }

    #[test]
    fn push_cheio_devolve_parcial() {
        let (mut tx, _rx) = new_track();
        let cap = RING_FRAMES * FRAME_SAMPLES;
        assert_eq!(tx.push(&vec![0.0; cap + 100]), cap);
        assert_eq!(tx.push(&[0.0; 10]), 0);
    }

    #[test]
    fn render_taxa_1_reproduz_exato() {
        let (mut tx, mut rx) = new_track();
        let src = ramp(1920);
        tx.push(&src);
        let mut out = vec![0.0; 1920];
        rx.render_add(&mut out);
        assert_eq!(out, src);
        assert_eq!(tx.buffered_ms(), 0.0);
    }

    #[test]
    fn volume_escala() {
        let (mut tx, mut rx) = new_track();
        tx.set_volume(0.5);
        tx.push(&vec![0.8; 3840]);
        let mut out = vec![0.0; 3840];
        rx.render_add(&mut out);
        assert!((out[3838] - 0.4).abs() < 1e-5);
        assert!((out[3839] - 0.4).abs() < 1e-5);
        assert!(out[0] > 0.4);
    }

    #[test]
    fn soma_aditiva() {
        let (mut tx, mut rx) = new_track();
        tx.push(&vec![0.5; 200]);
        let mut out = vec![0.25; 200];
        rx.render_add(&mut out);
        assert!(out.iter().all(|&x| (x - 0.75).abs() < 1e-6));
    }

    #[test]
    fn starvation_sem_estalo() {
        let (mut tx, mut rx) = new_track();
        let pcm: Vec<f32> = (0..480)
            .flat_map(|i| {
                let s = 0.5 * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect();
        tx.push(&pcm);
        let mut out = vec![0.0; 1920 * 2];
        rx.render_add(&mut out);
        for ch in 0..2 {
            let m = out
                .chunks_exact(2)
                .map(|f| f[ch])
                .collect::<Vec<_>>()
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max);
            assert!(m < 0.1, "salto {m}");
        }
        assert!(out[out.len() - 1].abs() < 1e-6);
    }

    fn consumido(rate: f64) -> u64 {
        let (mut tx, mut rx) = new_track();
        tx.set_rate(rate);
        tx.push(&vec![0.1; RING_FRAMES * FRAME_SAMPLES]);
        let before = tx.shared.buffered_samples.load(Ordering::Acquire);
        let mut out = vec![0.0; 20_000 * 2];
        rx.render_add(&mut out);
        before - tx.shared.buffered_samples.load(Ordering::Acquire)
    }

    #[test]
    fn taxa_102_consome_mais() {
        let base = consumido(1.0) as f64;
        let rapido = consumido(1.02) as f64;
        let esperado = base * 1.02;
        assert!((rapido - esperado).abs() / base < 0.005, "{rapido} vs {esperado}");
    }

    #[test]
    fn set_rate_clamp() {
        let (tx, _rx) = new_track();
        tx.set_rate(5.0);
        assert_eq!(f64::from_bits(tx.shared.rate_bits.load(Ordering::Relaxed)), 1.02);
        tx.set_rate(0.0);
        assert_eq!(f64::from_bits(tx.shared.rate_bits.load(Ordering::Relaxed)), 0.98);
        tx.set_rate(f64::NAN);
        assert_eq!(f64::from_bits(tx.shared.rate_bits.load(Ordering::Relaxed)), 1.0);
    }
}
