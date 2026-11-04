//! Recepção de áudio: Opus → PCM → saída.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use zap_audio::{TrackInput, FRAME_SAMPLES};
use zap_net::media_rx::MediaEvent;
use zap_net::report::ReportBuilderHandle;
use zap_rate::jitter::MediaKind;

use super::router::TrackHandler;
use crate::audio_out::AudioOutHandle;
use crate::playout::PlayoutHub;

/// Áudio de um participante.
pub struct AudioRx {
    pub(crate) participant_id: String,
    pub(crate) input: Mutex<TrackInput>,
    pub(crate) decoder: Mutex<zap_audio::OpusDec>,
    pub(crate) playout: Arc<PlayoutHub>,
    pub(crate) report: Mutex<Option<ReportBuilderHandle>>,
    pub(crate) clock: zap_net::Clock,
    pub(crate) last_seq: Mutex<Option<u32>>,
    pub(crate) volume: Mutex<(f32, bool)>,
}

impl AudioRx {
    /// Aplica `volume` (0..=1, `muted` → 0) em `TrackInput::set_volume`.
    pub fn set_volume(&self, volume: f32, muted: bool) {
        let efetivo = if muted { 0.0 } else { volume };
        self.input.lock().set_volume(efetivo);
        *self.volume.lock() = (volume, muted);
    }
    /// A cada 50 ms: `input.set_rate(clock_do_participante.audio_rate())`.
    pub fn tick(&self) {
        let rate = self.playout.clock_for(&self.participant_id).lock().audio_rate();
        self.input.lock().set_rate(rate);
    }
}

impl TrackHandler for AudioRx {
    /// `Audio`: `clock.observe(now, capture_us, Audio)`; decodifica (`OpusDec::decode`); perda
    /// de 1 quadro (seq pulou 1) → PLC/FEC; pula mais que isso → só continua; `input.push(pcm)`.
    /// `Lost` de áudio é ignorado.
    fn on_event(&self, ev: MediaEvent) {
        let MediaEvent::Audio {
            header,
            opus,
            arrival_us,
        } = ev
        else {
            return;
        };
        self.playout
            .clock_for(&self.participant_id)
            .lock()
            .observe(arrival_us, header.capture_us, MediaKind::Audio);

        let seq = header.seq;
        let mut pcm = [0f32; FRAME_SAMPLES];
        let mut last = self.last_seq.lock();
        let prev = *last;
        // Atrasado ou duplicado: descarta sem mexer no estado.
        if matches!(prev, Some(p) if seq <= p) {
            return;
        }
        *last = Some(seq);
        let mut dec = self.decoder.lock();
        // Perdeu exatamente 1 quadro: o FEC deste pacote recupera o anterior.
        if matches!(prev, Some(p) if seq == p.wrapping_add(2)) {
            match dec.decode(None, Some(&opus), &mut pcm) {
                Ok(()) => {
                    self.input.lock().push(&pcm);
                }
                Err(e) => tracing::debug!("falha no FEC do opus: {e}"),
            }
        }
        match dec.decode(Some(&opus), None, &mut pcm) {
            Ok(()) => {
                self.input.lock().push(&pcm);
            }
            Err(e) => tracing::debug!("falha ao decodificar opus: {e}"),
        }
    }
}

/// Um `AudioRx` por participante, criado sob demanda (`AudioOutHandle::add_track` com id =
/// hash estável do participant_id), com o volume inicial de `Settings.sound.initial_volume`.
pub struct AudioRegistry {
    pub(crate) out: AudioOutHandle,
    pub(crate) playout: Arc<PlayoutHub>,
    pub(crate) clock: zap_net::Clock,
    pub(crate) by_participant: Mutex<HashMap<String, Arc<AudioRx>>>,
    pub(crate) volumes: Mutex<HashMap<String, (f32, bool)>>,
    pub(crate) initial_volume: Mutex<f32>,
}

impl AudioRegistry {
    pub fn new(out: AudioOutHandle, playout: Arc<PlayoutHub>, clock: zap_net::Clock, initial_volume: f32) -> Self {
        Self {
            out,
            playout,
            clock,
            by_participant: Mutex::new(HashMap::new()),
            volumes: Mutex::new(HashMap::new()),
            initial_volume: Mutex::new(initial_volume),
        }
    }
    pub fn get_or_create(&self, participant_id: &str) -> Result<Arc<AudioRx>, crate::CoreError> {
        let mut mapa = self.by_participant.lock();
        if let Some(rx) = mapa.get(participant_id) {
            return Ok(Arc::clone(rx));
        }
        let decoder = zap_audio::OpusDec::new().map_err(|e| crate::CoreError::Unsupported(e.to_string()))?;
        let input = self.out.add_track(stable_id(participant_id));
        let volume = self
            .volumes
            .lock()
            .get(participant_id)
            .copied()
            .unwrap_or((*self.initial_volume.lock(), false));
        let rx = Arc::new(AudioRx {
            participant_id: participant_id.to_string(),
            input: Mutex::new(input),
            decoder: Mutex::new(decoder),
            playout: Arc::clone(&self.playout),
            report: Mutex::new(None),
            clock: self.clock.clone(),
            last_seq: Mutex::new(None),
            volume: Mutex::new(volume),
        });
        rx.set_volume(volume.0, volume.1);
        mapa.insert(participant_id.to_string(), Arc::clone(&rx));
        Ok(rx)
    }
    /// Guarda e aplica (também para quem ainda não existe).
    pub fn set_volume(&self, participant_id: &str, volume: f32, muted: bool) {
        let v = if volume.is_nan() { 1.0 } else { volume.clamp(0.0, 1.0) };
        self.volumes.lock().insert(participant_id.to_string(), (v, muted));
        let rx = self.by_participant.lock().get(participant_id).cloned();
        if let Some(rx) = rx {
            rx.set_volume(v, muted);
        }
    }
    pub fn remove(&self, participant_id: &str) {
        if self.by_participant.lock().remove(participant_id).is_some() {
            self.out.remove_track(stable_id(participant_id));
        }
    }
}

/// FNV-1a de 64 bits: id estável da trilha de saída para o participante.
fn stable_id(participant_id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in participant_id.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::UiSink;
    use crate::types::{ChatLine, TileId, VideoPacket};
    use crate::Snapshot;
    use zap_proto::media::AudioHeader;

    struct NoUi;
    impl UiSink for NoUi {
        fn state(&self, _s: Arc<Snapshot>) {}
        fn chat(&self, _m: ChatLine) {}
        fn video(&self, _t: TileId, _p: VideoPacket) {}
    }

    fn registry() -> AudioRegistry {
        let clock = zap_net::Clock::new();
        let hub = Arc::new(PlayoutHub::start(clock.clone(), Arc::new(NoUi)));
        AudioRegistry::new(AudioOutHandle::start(), hub, clock, 0.8)
    }

    fn opus_seno() -> bytes::Bytes {
        let mut pcm = [0f32; FRAME_SAMPLES];
        for n in 0..zap_audio::FRAME {
            let v = (n as f32 * 0.05).sin() * 0.5;
            pcm[2 * n] = v;
            pcm[2 * n + 1] = v;
        }
        let mut enc = zap_audio::OpusEnc::new(64).expect("encoder");
        enc.encode(&pcm).expect("encode")
    }

    fn evento(seq: u32, opus: &bytes::Bytes) -> MediaEvent {
        MediaEvent::Audio {
            header: AudioHeader {
                track: 1,
                seq,
                capture_us: u64::from(seq) * 20_000,
                send_us: 0,
            },
            opus: opus.clone(),
            arrival_us: u64::from(seq) * 20_000 + 5_000,
        }
    }

    #[test]
    fn volume_clamp_e_mudo() {
        let r = registry();
        let rx = r.get_or_create("a").unwrap();
        r.set_volume("a", 3.0, false);
        assert_eq!(*rx.volume.lock(), (1.0, false));
        r.set_volume("a", -1.0, true);
        assert_eq!(*rx.volume.lock(), (0.0, true));
    }

    #[test]
    fn registry_reusa_audio_rx_do_mesmo_participante() {
        let r = registry();
        let a = r.get_or_create("a").unwrap();
        let b = r.get_or_create("a").unwrap();
        let c = r.get_or_create("c").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
        r.remove("a");
        let d = r.get_or_create("a").unwrap();
        assert!(!Arc::ptr_eq(&a, &d));
    }

    #[test]
    fn volume_definido_antes_de_existir_vale_depois() {
        let r = registry();
        r.set_volume("a", 0.3, true);
        let rx = r.get_or_create("a").unwrap();
        assert_eq!(*rx.volume.lock(), (0.3, true));
        let rx2 = r.get_or_create("b").unwrap();
        assert_eq!(*rx2.volume.lock(), (0.8, false));
    }

    #[test]
    fn stable_id_e_deterministico_e_diferente() {
        assert_eq!(stable_id("ana"), stable_id("ana"));
        assert_ne!(stable_id("ana"), stable_id("bia"));
    }

    #[test]
    fn quadro_opus_valido_e_decodificado_e_empurrado() {
        let r = registry();
        let rx = r.get_or_create("a").unwrap();
        let opus = opus_seno();
        rx.on_event(evento(1, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(1));
        assert!(rx.input.lock().buffered_ms() >= 0.0);
        // Perdeu 1 quadro: FEC + atual.
        rx.on_event(evento(3, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(3));
        rx.tick();
    }

    #[test]
    fn seq_atrasado_descartado() {
        let r = registry();
        let rx = r.get_or_create("a").unwrap();
        let opus = opus_seno();
        rx.on_event(evento(5, &opus));
        rx.on_event(evento(4, &opus));
        rx.on_event(evento(5, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(5));
        rx.on_event(evento(20, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(20));
    }
}
