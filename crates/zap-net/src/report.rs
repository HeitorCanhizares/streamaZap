//! Construção do relatório do receptor (`relatorio`) e estimador de banda de recepção.

use std::sync::Arc;

use parking_lot::Mutex;
use zap_proto::media::{AudioHeader, ProbeHeader, VideoHeader};
use zap_proto::{DelayState, Report};
use zap_rate::{
    bwe::{Bwe, BweConfig},
    probe::ProbeMeter,
    unwrap::TimeUnwrapper,
    window::LossMeter,
};

mod reporter;

pub use reporter::spawn_reporter;

/// Intervalo dos relatórios.
pub const REPORT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

pub struct ReportBuilder {
    pub(crate) arrivals: Vec<(u16, u8, u32, u32, u64, u32)>,
    pub(crate) probes: Vec<(u16, u32, u32, u64, u32)>,
    pub(crate) lost: Vec<(u16, u8, u32)>,
    pub(crate) bwe: Option<Bwe>,
    pub(crate) loss: LossMeter,
    /// Mede os grupos de sondas (usado só quando `bwe` é `Some`).
    pub(crate) probe_meter: ProbeMeter,
    pub(crate) buffer_ms: Option<u32>,
    /// Quantos avisos de cabeçalho de keyframe chegaram (só para testes).
    #[cfg(test)]
    pub(crate) keyframe_starts: u32,
    /// Um único desembrulhador de tempo para vídeo, áudio e sonda (mesmo relógio do emissor).
    unwrap: TimeUnwrapper,
    /// Última chegada vista; serve de "agora" ao registrar perdas.
    last_arrival_us: u64,
    /// Último `seq` de áudio por trilha, para achar datagramas perdidos.
    audio_seq: std::collections::HashMap<u16, u32>,
}

/// Buracos maiores que isso (ex.: trilha reiniciada) não viram perda.
const MAX_BURACO_AUDIO: u32 = 50;

pub type ReportBuilderHandle = Arc<Mutex<ReportBuilder>>;

impl ReportBuilder {
    /// `estimate`: app = true (roda `Bwe` e preenche banda/estado); ponte = false.
    pub fn new(estimate: bool) -> Self {
        Self {
            arrivals: Vec::new(),
            probes: Vec::new(),
            lost: Vec::new(),
            bwe: estimate.then(|| {
                Bwe::new(BweConfig {
                    min_kbps: 150,
                    max_kbps: 100_000,
                    start_kbps: 1_000,
                })
            }),
            loss: LossMeter::new(2_000_000),
            probe_meter: ProbeMeter::new(),
            buffer_ms: None,
            #[cfg(test)]
            keyframe_starts: 0,
            unwrap: TimeUnwrapper::new(),
            last_arrival_us: 0,
            audio_seq: std::collections::HashMap::new(),
        }
    }

    /// Alimenta o estimador de banda com um pacote de mídia (vídeo ou áudio) e o medidor de sondas.
    /// `keyframe`: pacote de keyframe de vídeo (o estimador abre/estende a carência de rajada).
    fn feed_bwe(&mut self, send_us: u32, arrival_us: u64, bytes: u32, keyframe: bool) {
        let send = self.unwrap.unwrap(send_us);
        self.last_arrival_us = arrival_us;
        if let Some(bwe) = self.bwe.as_mut() {
            if keyframe {
                bwe.on_keyframe_packet(send, arrival_us, bytes);
            } else {
                bwe.on_packet(send, arrival_us, bytes);
            }
            self.probe_meter.on_media(arrival_us, bytes);
        }
    }

    pub fn on_video(&mut self, h: &VideoHeader, arrival_us: u64, bytes: u32) {
        self.arrivals
            .push((h.track, h.layer, h.seq, h.send_us, arrival_us, bytes));
        self.feed_bwe(h.send_us, arrival_us, bytes, h.flags.is_keyframe());
    }
    /// `perda` mede só perda **de rede**: buracos de `seq` nos datagramas de áudio. Quadros de
    /// vídeo vão em streams confiáveis (não se perdem na rede, só atrasam), e os que o emissor
    /// descarta por prazo são decisão dele: vão em `perdidos`, mas não contam na `perda` nem no
    /// estimador (senão o emissor derrubaria a própria banda ao descartar quadros velhos).
    pub fn on_audio(&mut self, h: &AudioHeader, arrival_us: u64, bytes: u32) {
        self.arrivals.push((h.track, 0, h.seq, h.send_us, arrival_us, bytes));
        if let Some(ultimo) = self.audio_seq.get(&h.track).copied() {
            let buraco = h.seq.wrapping_sub(ultimo).wrapping_sub(1);
            if buraco < MAX_BURACO_AUDIO {
                for _ in 0..buraco {
                    self.loss.record(arrival_us, false);
                }
            }
        }
        // Atrasado/fora de ordem (seq menor) conta como recebido sem mexer no último.
        let ultimo = self.audio_seq.entry(h.track).or_insert(h.seq);
        if h.seq.wrapping_sub(*ultimo) < u32::MAX / 2 {
            *ultimo = h.seq;
        }
        self.loss.record(arrival_us, true);
        self.feed_bwe(h.send_us, arrival_us, bytes, false);
    }
    pub fn on_probe(&mut self, h: &ProbeHeader, arrival_us: u64, bytes: u32) {
        self.probes.push((h.cluster, h.seq, h.send_us, arrival_us, bytes));
        // Sondas medem banda, mas não entram na conta de perda.
        let send = self.unwrap.unwrap(h.send_us);
        self.last_arrival_us = arrival_us;
        if let Some(bwe) = self.bwe.as_mut() {
            bwe.on_probe_packet(send, arrival_us, bytes);
            // Grupo anterior que fechou por troca de cluster.
            if let Some(r) = self.probe_meter.on_probe(h.cluster, arrival_us, bytes) {
                bwe.on_probe_cluster(arrival_us, r.kbps);
            }
        }
    }
    /// Quadro de vídeo que não chegou (descartado pelo emissor por prazo): vai em `perdidos`, não
    /// na `perda`.
    /// Também avisa o estimador: descarte do emissor é sinal de que a rota não aguenta a taxa.
    /// `seen_us` = instante (relógio do receptor) em que o buraco de `seq` foi visto, não quando
    /// venceu: o estimador avalia a carência de rajada nesse instante.
    pub fn on_lost(&mut self, track: u16, layer: u8, seq: u32, seen_us: u64) {
        self.lost.push((track, layer, seq));
        if let Some(bwe) = self.bwe.as_mut() {
            bwe.on_frame_dropped(seen_us);
        }
    }
    /// Cabeçalho de um keyframe chegou (o quadro ainda vai chegar): avisa o estimador (carência).
    pub fn on_keyframe_start(&mut self, arrival_us: u64) {
        #[cfg(test)]
        {
            self.keyframe_starts += 1;
        }
        if let Some(bwe) = self.bwe.as_mut() {
            bwe.on_keyframe_start(arrival_us);
        }
    }
    pub fn set_buffer_ms(&mut self, ms: u32) {
        self.buffer_ms = Some(ms);
    }
    /// Esvazia as listas e monta o relatório.
    pub fn take(&mut self, now: u64) -> Report {
        let loss = self.loss.fraction(now);
        if let Some(bwe) = self.bwe.as_mut() {
            bwe.on_loss_fraction(now, loss);
            if let Some(r) = self.probe_meter.poll(now) {
                bwe.on_probe_cluster(now, r.kbps);
            }
            bwe.update(now);
        }
        Report {
            arrivals: std::mem::take(&mut self.arrivals),
            probes: std::mem::take(&mut self.probes),
            lost: std::mem::take(&mut self.lost),
            loss,
            bandwidth_kbps: self.bwe.as_ref().map(|b| b.estimate_kbps()),
            state: self.bwe.as_ref().map(|b| b.state()),
            buffer_ms: self.buffer_ms,
        }
    }
    /// Estado atual do detector de atraso (para o envio imediato em `Overuse`).
    pub fn is_overuse(&self) -> bool {
        self.bwe.as_ref().is_some_and(|b| b.state() == DelayState::Overuse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_proto::media::VideoFlags;

    fn vh(seq: u32, send_us: u32) -> VideoHeader {
        VideoHeader {
            flags: VideoFlags::default(),
            track: 3,
            layer: 1,
            width: 640,
            height: 360,
            seq,
            capture_us: 0,
            send_us,
            size: 1200,
        }
    }
    fn ah(seq: u32, send_us: u32) -> AudioHeader {
        AudioHeader {
            track: 7,
            seq,
            capture_us: 0,
            send_us,
        }
    }
    fn ph(seq: u32, send_us: u32) -> ProbeHeader {
        ProbeHeader {
            cluster: 2,
            seq,
            send_us,
        }
    }

    #[test]
    fn take_esvazia_listas() {
        let mut r = ReportBuilder::new(false);
        r.on_video(&vh(1, 1000), 2000, 1200);
        r.on_audio(&ah(1, 1500), 2500, 100);
        r.on_probe(&ph(1, 1600), 2600, 800);
        r.on_lost(3, 1, 9, 15_000);
        let rep = r.take(3000);
        assert_eq!(rep.arrivals.len(), 2);
        assert_eq!(rep.probes.len(), 1);
        assert_eq!(rep.lost.len(), 1);
        let rep2 = r.take(4000);
        assert!(rep2.arrivals.is_empty() && rep2.probes.is_empty() && rep2.lost.is_empty());
    }

    #[test]
    fn campos_da_chegada() {
        let mut r = ReportBuilder::new(false);
        r.on_video(&vh(5, 1000), 2000, 1200);
        let rep = r.take(3000);
        assert_eq!(rep.arrivals[0], (3, 1, 5, 1000, 2000, 1200));
    }

    #[test]
    fn sem_estimador_ponte() {
        let rep = ReportBuilder::new(false).take(1000);
        assert_eq!(rep.bandwidth_kbps, None);
        assert_eq!(rep.state, None);
    }

    #[test]
    fn com_estimador_app() {
        let mut r = ReportBuilder::new(true);
        for i in 0..100u32 {
            r.on_video(&vh(i, i * 1000), 10_000 + i as u64 * 1000, 1200);
        }
        let rep = r.take(110_000);
        assert!(rep.bandwidth_kbps.is_some());
        assert!(rep.state.is_some());
    }

    #[test]
    fn perda_calculada() {
        // 10 datagramas de áudio esperados (seq 0..=9), o 5 não chegou: 10% de perda.
        let mut r = ReportBuilder::new(false);
        for i in (0..10u32).filter(|i| *i != 5) {
            r.on_audio(&ah(i, i * 20_000), 10_000 + u64::from(i) * 20_000, 200);
        }
        let rep = r.take(300_000);
        assert!((rep.loss - 0.1).abs() < 0.02, "perda = {}", rep.loss);
    }

    #[test]
    fn quadro_descartado_pelo_emissor_nao_conta_perda() {
        let mut r = ReportBuilder::new(false);
        for i in 0..9u32 {
            r.on_video(&vh(i, i * 1000), 10_000 + i as u64 * 1000, 1200);
        }
        r.on_lost(3, 1, 9, 15_000);
        let rep = r.take(20_000);
        assert_eq!(rep.loss, 0.0);
        assert_eq!(rep.lost, vec![(3, 1, 9)]);
    }

    #[test]
    fn buffer_ms_persiste() {
        let mut r = ReportBuilder::new(false);
        r.set_buffer_ms(120);
        assert_eq!(r.take(1000).buffer_ms, Some(120));
        assert_eq!(r.take(2000).buffer_ms, Some(120));
    }

    #[test]
    fn sonda_nao_conta_perda() {
        let mut r2 = ReportBuilder::new(false);
        for i in 0..10 {
            r2.on_probe(&ph(i, 1000), 2000, 800);
        }
        assert_eq!(r2.take(3000).loss, 0.0);
    }

    #[test]
    fn is_overuse_false_no_inicio() {
        assert!(!ReportBuilder::new(true).is_overuse());
        assert!(!ReportBuilder::new(false).is_overuse());
    }

    #[test]
    fn sonda_sobe_banda() {
        let mut r = ReportBuilder::new(true);
        // Vídeo de 400 kbps por 2 s: 1200 B a cada 24 ms, atraso de rede constante de 10 ms.
        let mut t = 0u64;
        let mut seq = 0u32;
        while t < 2_000_000 {
            r.on_video(&vh(seq, t as u32), t + 10_000, 1200);
            seq += 1;
            t += 24_000;
        }
        // Rajada de ~3350 kbps: 70 sondas de 1200 B a cada ~2,86 ms (~200 ms).
        let inicio = t;
        let mut fim = inicio;
        for i in 0..70u32 {
            let s = inicio + u64::from(i) * 2_850;
            r.on_probe(&ph(i, s as u32), s + 10_000, 1200);
            fim = s + 10_000;
        }
        let rep = r.take(fim + 60_000);
        assert!(rep.bandwidth_kbps >= Some(3195), "banda = {:?}", rep.bandwidth_kbps);
    }

    #[test]
    fn sonda_nao_vira_sobrecarga() {
        let mut r = ReportBuilder::new(true);
        let mut t = 0u64;
        let mut seq = 0u32;
        while t < 1_000_000 {
            r.on_video(&vh(seq, t as u32), t + 10_000, 1200);
            seq += 1;
            t += 24_000;
        }
        let mut proximo_take = t + 100_000;
        for i in 0..70u32 {
            let s = t + u64::from(i) * 2_850;
            // Atraso de chegada crescente durante as sondas.
            let chegada = s + 10_000 + u64::from(i) * 400;
            r.on_probe(&ph(i, s as u32), chegada, 1200);
            while proximo_take <= chegada {
                let rep = r.take(proximo_take);
                assert_ne!(rep.state, Some(DelayState::Overuse));
                proximo_take += 100_000;
            }
        }
        for k in 1..=5u64 {
            let rep = r.take(t + 200_000 + k * 100_000);
            assert_ne!(rep.state, Some(DelayState::Overuse));
        }
    }

    #[test]
    fn ponte_ignora_sondas_no_estimador() {
        let mut r = ReportBuilder::new(false);
        for i in 0..70u32 {
            let s = u64::from(i) * 2_850;
            r.on_probe(&ph(i, s as u32), s + 10_000, 1200);
        }
        let rep = r.take(400_000);
        assert_eq!(rep.bandwidth_kbps, None);
        assert_eq!(rep.probes.len(), 70);
        assert_eq!(rep.loss, 0.0);
    }

    fn vh_key(seq: u32, send_us: u32) -> VideoHeader {
        let mut h = vh(seq, send_us);
        h.flags = VideoFlags(VideoFlags::KEYFRAME);
        h
    }

    /// Vídeo comum ~2500 kbps (1200 B a cada ~3,84 ms) por 2 s, atraso constante de 10 ms.
    fn video_estavel(r: &mut ReportBuilder) -> (u64, u32) {
        let mut t = 0u64;
        let mut seq = 0u32;
        while t < 2_000_000 {
            r.on_video(&vh(seq, t as u32), t + 10_000, 1200);
            seq += 1;
            t += 3_840;
        }
        (t, seq)
    }

    /// `ms` de pacotes (1 a cada 250 us) com atraso crescente (+400 us por pacote); `take` a cada 100 ms.
    /// Devolve se algum `take` deu `Overuse`.
    fn rajada_atrasada(r: &mut ReportBuilder, t: u64, seq0: u32, keyframe: bool, ms: u64) -> bool {
        let mut overuse = false;
        let mut proximo_take = t + 100_000;
        for i in 0..(ms * 4) as u32 {
            let s = t + u64::from(i) * 250;
            let chegada = s + 10_000 + u64::from(i) * 400;
            let h = if keyframe {
                vh_key(seq0 + i, s as u32)
            } else {
                vh(seq0 + i, s as u32)
            };
            r.on_video(&h, chegada, 1200);
            while proximo_take <= chegada {
                if r.take(proximo_take).state == Some(DelayState::Overuse) {
                    overuse = true;
                }
                proximo_take += 100_000;
            }
        }
        overuse
    }

    #[test]
    fn keyframe_abre_carencia() {
        let mut r = ReportBuilder::new(true);
        let (t, seq) = video_estavel(&mut r);
        r.take(t + 10_000);
        r.on_keyframe_start(t);
        assert!(!rajada_atrasada(&mut r, t, seq, true, 400));
    }

    #[test]
    fn video_sem_keyframe_com_atraso_crescente_vira_overuse() {
        let mut r = ReportBuilder::new(true);
        let (t, seq) = video_estavel(&mut r);
        r.take(t + 10_000);
        assert!(rajada_atrasada(&mut r, t, seq, false, 1000));
    }

    /// Rajada de keyframe com fila crescente, depois pacotes comuns com atraso estável (estado
    /// volta a `Normal`, fila continua alta). Devolve (builder, fim da rajada, fim do estável).
    fn cenario_descarte() -> (ReportBuilder, u64, u64) {
        let mut r = ReportBuilder::new(true);
        let (t, seq) = video_estavel(&mut r);
        r.take(t + 10_000);
        r.on_keyframe_start(t);
        rajada_atrasada(&mut r, t, seq, true, 200);
        let atraso = 10_000 + 799 * 400;
        let fim_rajada = t + 799 * 250 + atraso;
        let mut s = t + 200_000;
        let mut q = seq + 800;
        while s < t + 1_200_000 {
            r.on_video(&vh(q, s as u32), s + atraso, 1200);
            q += 1;
            s += 1_000;
        }
        (r, fim_rajada, s + atraso)
    }

    #[test]
    fn perda_usa_instante_visto() {
        // O último evento de keyframe é a chegada do último pacote da rajada. Dentro da
        // carência (<= 1 s dele, com fila) os descartes não contam; fora, o estimador
        // ajusta a estimativa à vazão que chega (aqui sobe; dentro, fica no alvo anterior).
        let (mut a, fim, _) = cenario_descarte();
        let (mut b, _, _) = cenario_descarte();
        let agora = fim + 1_050_000;
        for _ in 0..3 {
            a.on_lost(3, 1, 9, fim + 900_000);
            b.on_lost(3, 1, 9, fim + 1_020_000);
        }
        let dentro = a.take(agora).bandwidth_kbps.unwrap();
        let fora = b.take(agora).bandwidth_kbps.unwrap();
        assert!(fora > dentro, "fora = {fora}, dentro = {dentro}");
    }

    #[test]
    fn on_keyframe_start_conta() {
        let mut r = ReportBuilder::new(true);
        r.on_keyframe_start(1000);
        assert_eq!(r.keyframe_starts, 1);
    }
}
