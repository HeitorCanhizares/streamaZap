//! `AudioPipeline`: AppCapture -> Opus -> EncodedAudio (ou o sintético).
use crate::{EncodedAudio, MediaError};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zap_audio::AudioSelection;

pub struct AudioPipeline {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(windows)]
    capture: Option<zap_audio::AppCapture>,
}

impl AudioPipeline {
    /// Backend sintético (todas as plataformas): Opus de silêncio de 20 ms.
    pub fn start_synthetic(track: u16, out: Sender<EncodedAudio>) -> Result<Self, MediaError> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = crate::synth_audio::spawn(track, stop.clone(), out)?;
        Ok(Self {
            stop,
            thread: Some(thread),
            #[cfg(windows)]
            capture: None,
        })
    }

    /// Real (Windows): `AppCapture::start(sel)` -> thread que codifica cada `CapturedAudio` com
    /// `OpusEnc(128 kbps)`. Fora do Windows: `MediaError::Unsupported`.
    #[cfg(not(windows))]
    pub fn start(_track: u16, _sel: AudioSelection, _out: Sender<EncodedAudio>) -> Result<Self, MediaError> {
        Err(MediaError::Unsupported("áudio por app só no Windows".into()))
    }

    /// Real (Windows): `AppCapture::start(sel)` -> thread que codifica cada `CapturedAudio` com
    /// `OpusEnc(128 kbps)`. Fora do Windows: `MediaError::Unsupported`.
    #[cfg(windows)]
    pub fn start(track: u16, sel: AudioSelection, out: Sender<EncodedAudio>) -> Result<Self, MediaError> {
        use std::time::Duration;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = crossbeam_channel::bounded::<zap_audio::CapturedAudio>(8);
        let mut enc = zap_audio::OpusEnc::new(128)?;
        let capture = zap_audio::AppCapture::start(sel, tx)?;
        let t_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("zap-audio-enc".into())
            .spawn(move || {
                let mut seq: u32 = 0;
                while !t_stop.load(Ordering::SeqCst) {
                    let blk = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(b) => b,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    };
                    let data = match enc.encode(&blk.pcm) {
                        Ok(d) => d,
                        Err(e) => {
                            // Bloco perdido: não consome número de sequência.
                            tracing::warn!("falha ao codificar Opus: {e}");
                            continue;
                        }
                    };
                    let pkt = EncodedAudio {
                        track,
                        seq,
                        capture_us: blk.capture_us,
                        data,
                    };
                    if out.send(pkt).is_err() {
                        break;
                    }
                    seq = seq.wrapping_add(1);
                }
            })
            .map_err(|e| MediaError::Stopped(format!("thread de áudio: {e}")))?;
        Ok(Self {
            stop,
            thread: Some(thread),
            capture: Some(capture),
        })
    }

    /// Troca a seleção de apps (só real; no sintético não faz nada).
    pub fn set_selection(&self, sel: AudioSelection) {
        #[cfg(windows)]
        if let Some(c) = &self.capture {
            c.set_selection(sel);
        }
        #[cfg(not(windows))]
        let _ = sel;
    }
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // A captura cai antes do join: fecha o canal e destrava a thread de codificação.
        #[cfg(windows)]
        drop(self.capture.take());
        if let Some(t) = self.thread.take() {
            if t.join().is_err() {
                tracing::warn!("thread de áudio terminou em pânico");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[cfg(not(windows))]
    #[test]
    fn start_fora_do_windows_unsupported() {
        let (tx, _rx) = crossbeam_channel::bounded(4);
        let r = AudioPipeline::start(1, AudioSelection::All, tx);
        assert!(matches!(r, Err(MediaError::Unsupported(_))));
    }

    #[test]
    fn set_selection_no_sintetico_nao_entra_em_panico() {
        let (tx, _rx) = crossbeam_channel::bounded(64);
        if let Ok(p) = AudioPipeline::start_synthetic(1, tx) {
            p.set_selection(AudioSelection::None);
            p.set_selection(AudioSelection::Apps(vec![1, 2]));
        }
    }

    #[test]
    fn drop_encerra_rapido() {
        let (tx, _rx) = crossbeam_channel::bounded(64);
        if let Ok(p) = AudioPipeline::start_synthetic(1, tx) {
            let t0 = Instant::now();
            drop(p);
            assert!(t0.elapsed() < Duration::from_millis(500));
        }
    }

    #[cfg(windows)]
    #[test]
    fn start_all_entrega_blocos() {
        let (tx, rx) = crossbeam_channel::bounded(256);
        let p = match AudioPipeline::start(1, AudioSelection::All, tx) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("pulando: áudio indisponível: {e}");
                return;
            }
        };
        let fim = Instant::now() + Duration::from_millis(1500);
        let mut n = 0;
        while Instant::now() < fim {
            if rx.recv_timeout(Duration::from_millis(100)).is_ok() {
                n += 1;
            }
        }
        drop(p);
        assert!(n >= 10, "recebidos {n}");
    }
}
