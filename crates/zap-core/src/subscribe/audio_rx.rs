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

/// Recuo de `seq` maior que isso (2 s de áudio) não é pacote atrasado: é um áudio novo de quem
/// transmite, que parou e voltou (a contagem recomeça do zero a cada transmissão).
const RECOMECO: u32 = 100;

/// Áudio de um participante.
pub struct AudioRx {
    pub(crate) participant_id: String,
    pub(crate) input: Mutex<TrackInput>,
    pub(crate) decoder: Mutex<zap_audio::OpusDec>,
    pub(crate) playout: Arc<PlayoutHub>,
    #[allow(dead_code)] // construtor de relatório preenchido sob demanda
    pub(crate) report: Mutex<Option<ReportBuilderHandle>>,
    #[allow(dead_code)] // relógio guardado para uso futuro do receptor
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
        let mut prev = *last;
        let mut dec = self.decoder.lock();
        if let Some(p) = prev {
            if seq <= p {
                // Atrasado ou duplicado: descarta sem mexer no estado.
                if p - seq <= RECOMECO {
                    return;
                }
                // Recuou muito: quem transmite parou e voltou (a contagem recomeçou do zero).
                // Sem isso, todo o som novo era descartado como "atrasado" até a contagem passar
                // da antiga, e nem sair e entrar na sala resolvia.
                tracing::info!(participante = %self.participant_id, de = p, para = seq, "áudio recomeçou");
                prev = None;
                if let Ok(novo) = zap_audio::OpusDec::new() {
                    *dec = novo;
                }
            }
        }
        *last = Some(seq);
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
///
/// As assinaturas pegam o receptor com [`AudioRegistry::hold`] e soltam com
/// [`AudioRegistry::unhold`]; quando a última solta, o participante sai do mixer de saída. O
/// mixer tem um limite de trilhas e cada entrada numa sala gera um id novo: sem soltar, ele
/// enchia numa noite longa e quem entrava depois ficava mudo.
pub struct AudioRegistry {
    pub(crate) out: AudioOutHandle,
    pub(crate) playout: Arc<PlayoutHub>,
    pub(crate) clock: zap_net::Clock,
    pub(crate) by_participant: Mutex<HashMap<String, Arc<AudioRx>>>,
    /// Quantas assinaturas usam o receptor de cada participante. Trava depois de `by_participant`.
    pub(crate) holders: Mutex<HashMap<String, usize>>,
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
            holders: Mutex::new(HashMap::new()),
            volumes: Mutex::new(HashMap::new()),
            initial_volume: Mutex::new(initial_volume),
        }
    }
    /// Receptor do participante (cria se não existe), sem contar como uso.
    pub fn get_or_create(&self, participant_id: &str) -> Result<Arc<AudioRx>, crate::CoreError> {
        let mut mapa = self.by_participant.lock();
        self.criar(&mut mapa, participant_id)
    }
    /// Receptor do participante para uma assinatura: conta um uso (soltar com `unhold`).
    pub fn hold(&self, participant_id: &str) -> Result<Arc<AudioRx>, crate::CoreError> {
        let mut mapa = self.by_participant.lock();
        let rx = self.criar(&mut mapa, participant_id)?;
        *self.holders.lock().entry(participant_id.to_string()).or_default() += 1;
        Ok(rx)
    }
    /// Solta um uso; o último tira o participante do mixer de saída.
    pub fn unhold(&self, participant_id: &str) {
        let removido = {
            let mut mapa = self.by_participant.lock();
            let mut holders = self.holders.lock();
            match holders.get_mut(participant_id) {
                Some(n) if *n > 1 => {
                    *n -= 1;
                    false
                }
                Some(_) => {
                    holders.remove(participant_id);
                    mapa.remove(participant_id).is_some()
                }
                None => false,
            }
        };
        if removido {
            self.out.remove_track(stable_id(participant_id));
        }
    }
    /// Tira todos do mixer (saí da sala). Os volumes escolhidos continuam guardados.
    pub fn clear(&self) {
        let ids: Vec<String> = {
            let mut mapa = self.by_participant.lock();
            self.holders.lock().clear();
            mapa.drain().map(|(id, _)| id).collect()
        };
        for id in ids {
            self.out.remove_track(stable_id(&id));
        }
    }
    fn criar(
        &self,
        mapa: &mut HashMap<String, Arc<AudioRx>>,
        participant_id: &str,
    ) -> Result<Arc<AudioRx>, crate::CoreError> {
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
        let removido = {
            let mut mapa = self.by_participant.lock();
            self.holders.lock().remove(participant_id);
            mapa.remove(participant_id).is_some()
        };
        if removido {
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
    fn audio_que_recomeca_do_zero_volta_a_tocar() {
        let r = registry();
        let rx = r.get_or_create("a").unwrap();
        let opus = opus_seno();
        rx.on_event(evento(5_000, &opus));
        // Atraso pequeno: descartado.
        rx.on_event(evento(4_950, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(5_000));
        // Quem transmite parou e voltou: a contagem recomeçou.
        rx.on_event(evento(0, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(0));
        rx.on_event(evento(1, &opus));
        assert_eq!(*rx.last_seq.lock(), Some(1));
    }

    #[test]
    fn ultimo_uso_tira_do_mixer_e_o_volume_fica() {
        let r = registry();
        r.set_volume("a", 0.3, false);
        let a1 = r.hold("a").unwrap();
        let a2 = r.hold("a").unwrap();
        assert!(Arc::ptr_eq(&a1, &a2));
        r.unhold("a");
        assert!(r.by_participant.lock().contains_key("a"), "ainda há uma assinatura");
        r.unhold("a");
        assert!(!r.by_participant.lock().contains_key("a"), "a última soltou");
        assert!(r.holders.lock().is_empty());
        // Soltar a mais não quebra nada.
        r.unhold("a");
        // Voltou a assistir: receptor novo, com o volume escolhido antes.
        let a3 = r.hold("a").unwrap();
        assert!(!Arc::ptr_eq(&a1, &a3));
        assert_eq!(*a3.volume.lock(), (0.3, false));
    }

    #[test]
    fn clear_tira_todos() {
        let r = registry();
        let _a = r.hold("a").unwrap();
        let _b = r.get_or_create("b").unwrap();
        r.clear();
        assert!(r.by_participant.lock().is_empty());
        assert!(r.holders.lock().is_empty());
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
