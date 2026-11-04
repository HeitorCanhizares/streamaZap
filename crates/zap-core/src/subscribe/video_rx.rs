//! Recepção de vídeo de um tile: relógio, fila de reprodução e pedido de keyframe.

use std::sync::Arc;

use parking_lot::Mutex;
use zap_net::media_rx::MediaEvent;
use zap_net::report::ReportBuilderHandle;
use zap_rate::jitter::MediaKind;

use super::metrics::TileMetrics;
use super::router::TrackHandler;
use crate::playout::PlayoutHub;
use crate::types::{TileId, VideoPacket};

pub struct VideoRx {
    pub(crate) tile: TileId,
    pub(crate) participant_id: String,
    pub(crate) playout: Arc<PlayoutHub>,
    pub(crate) metrics: Arc<TileMetrics>,
    pub(crate) report: ReportBuilderHandle,
    pub(crate) clock: zap_net::Clock,
    /// Pede keyframe ao publicador (envia `keyframe.pedir`); implementado pelo `PubConn`.
    pub(crate) request_keyframe: Arc<dyn Fn() + Send + Sync>,
    pub(crate) state: Mutex<VideoRxState>,
}

#[derive(Default)]
pub(crate) struct VideoRxState {
    /// Esperando keyframe (início ou depois de perda).
    pub need_key: bool,
    pub need_key_since_us: u64,
    pub last_request_us: u64,
}

impl VideoRx {
    pub fn new(
        tile: TileId,
        participant_id: String,
        playout: Arc<PlayoutHub>,
        metrics: Arc<TileMetrics>,
        report: ReportBuilderHandle,
        clock: zap_net::Clock,
        request_keyframe: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<Self> {
        let agora = clock.now_us();
        Arc::new(Self {
            tile,
            participant_id,
            playout,
            metrics,
            report,
            clock,
            request_keyframe,
            state: Mutex::new(VideoRxState {
                need_key: true,
                need_key_since_us: agora,
                last_request_us: 0,
            }),
        })
    }
    /// A cada 50 ms (chamado por uma tarefa do `PubConn`): atualiza o buffer do relatório e, se
    /// `need_key` há mais de 300 ms e o último pedido foi há mais de 1 s, pede keyframe.
    pub fn tick(&self, now_us: u64) {
        let ms = self.playout.clock_for(&self.participant_id).lock().current_delay_ms();
        self.report.lock().set_buffer_ms(ms.round() as u32);
        let pedir = {
            let mut st = self.state.lock();
            if st.need_key
                && now_us.saturating_sub(st.need_key_since_us) >= 300_000
                && now_us.saturating_sub(st.last_request_us) >= 1_000_000
            {
                st.last_request_us = now_us;
                true
            } else {
                false
            }
        };
        // Chama fora do lock de estado.
        if pedir {
            (self.request_keyframe)();
        }
    }
}

impl TrackHandler for VideoRx {
    /// `Video`: relógio do participante observa a chegada; quadro delta com `need_key` →
    /// descarta; keyframe → `need_key = false`; registra métricas e agenda no playout.
    /// `Lost` → `need_key = true` (marca o instante). Começa com `need_key = true`.
    fn on_event(&self, ev: MediaEvent) {
        match ev {
            MediaEvent::Video {
                header,
                data,
                arrival_us,
            } => {
                self.playout.clock_for(&self.participant_id).lock().observe(
                    arrival_us,
                    header.capture_us,
                    MediaKind::Video,
                );
                self.metrics
                    .on_frame(arrival_us, data.len() as u32, header.layer, header.width, header.height);
                let keyframe = header.flags.is_keyframe();
                {
                    let mut st = self.state.lock();
                    if st.need_key {
                        if !keyframe {
                            return;
                        }
                        st.need_key = false;
                    }
                }
                self.playout.push(
                    &self.participant_id,
                    self.tile,
                    VideoPacket {
                        track: header.track,
                        layer: header.layer,
                        keyframe,
                        width: header.width,
                        height: header.height,
                        seq: header.seq,
                        capture_us: header.capture_us,
                        data,
                    },
                );
            }
            MediaEvent::Lost { .. } => {
                let mut st = self.state.lock();
                if !st.need_key {
                    st.need_key = true;
                    st.need_key_since_us = self.clock.now_us();
                }
            }
            MediaEvent::Audio { .. } => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::UiSink;
    use crate::Snapshot;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;
    use zap_net::report::ReportBuilder;
    use zap_proto::media::{VideoFlags, VideoHeader};

    #[derive(Default)]
    struct FakeUi {
        got: Mutex<Vec<u32>>,
    }
    impl UiSink for FakeUi {
        fn state(&self, _s: Arc<Snapshot>) {}
        fn chat(&self, _m: crate::types::ChatLine) {}
        fn video(&self, _t: TileId, p: VideoPacket) {
            self.got.lock().push(p.seq);
        }
    }

    struct Fx {
        rx: Arc<VideoRx>,
        ui: Arc<FakeUi>,
        pedidos: Arc<AtomicU32>,
        report: ReportBuilderHandle,
        metrics: Arc<TileMetrics>,
        _hub: Arc<PlayoutHub>,
    }

    fn fx() -> Fx {
        let clock = zap_net::Clock::new();
        let ui = Arc::new(FakeUi::default());
        let hub = Arc::new(PlayoutHub::start(clock.clone(), ui.clone()));
        let metrics = Arc::new(TileMetrics::new());
        let report: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let pedidos = Arc::new(AtomicU32::new(0));
        let p = pedidos.clone();
        let rx = VideoRx::new(
            1,
            "a".into(),
            hub.clone(),
            metrics.clone(),
            report.clone(),
            clock,
            Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
        );
        Fx {
            rx,
            ui,
            pedidos,
            report,
            metrics,
            _hub: hub,
        }
    }

    fn ev(seq: u32, key: bool) -> MediaEvent {
        MediaEvent::Video {
            header: VideoHeader {
                flags: VideoFlags(if key { VideoFlags::KEYFRAME } else { 0 }),
                track: 1,
                layer: 0,
                width: 16,
                height: 16,
                seq,
                capture_us: u64::from(seq) * 33_000,
                send_us: 0,
                size: 3,
            },
            data: bytes::Bytes::from_static(b"abc"),
            arrival_us: u64::from(seq) * 33_000,
        }
    }

    fn lost() -> MediaEvent {
        MediaEvent::Lost {
            track: 1,
            layer: 0,
            seq: 0,
        }
    }

    fn espera() {
        std::thread::sleep(Duration::from_millis(400));
    }

    #[test]
    fn descarta_delta_antes_do_primeiro_keyframe() {
        let f = fx();
        f.rx.on_event(ev(1, false));
        f.rx.on_event(ev(2, false));
        espera();
        assert!(f.ui.got.lock().is_empty());
    }

    #[test]
    fn keyframe_libera_e_entrega_quadros_seguintes() {
        let f = fx();
        f.rx.on_event(ev(1, false));
        f.rx.on_event(ev(2, true));
        f.rx.on_event(ev(3, false));
        f.rx.on_event(ev(4, false));
        espera();
        assert_eq!(*f.ui.got.lock(), vec![2, 3, 4]);
    }

    #[test]
    fn perda_exige_novo_keyframe() {
        let f = fx();
        f.rx.on_event(ev(1, true));
        f.rx.on_event(lost());
        f.rx.on_event(ev(2, false));
        f.rx.on_event(ev(3, true));
        f.rx.on_event(ev(4, false));
        espera();
        assert_eq!(*f.ui.got.lock(), vec![1, 3, 4]);
    }

    #[test]
    fn tick_pede_keyframe_apos_300ms_e_limita_a_1_por_segundo() {
        let f = fx();
        // Instantes artificiais, longe de zero (last_request_us começa em 0).
        let t0 = 10_000_000;
        f.rx.state.lock().need_key_since_us = t0;
        f.rx.tick(t0 + 100_000);
        assert_eq!(f.pedidos.load(Ordering::SeqCst), 0);
        f.rx.tick(t0 + 300_000);
        assert_eq!(f.pedidos.load(Ordering::SeqCst), 1);
        f.rx.tick(t0 + 900_000);
        assert_eq!(f.pedidos.load(Ordering::SeqCst), 1);
        f.rx.tick(t0 + 1_300_000);
        assert_eq!(f.pedidos.load(Ordering::SeqCst), 2);
        // Com keyframe recebido, não pede mais.
        f.rx.on_event(ev(1, true));
        f.rx.tick(t0 + 5_000_000);
        assert_eq!(f.pedidos.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn tick_atualiza_buffer_ms_no_relatorio() {
        let f = fx();
        assert!(f.report.lock().take(0).buffer_ms.is_none());
        f.rx.tick(0);
        assert!(f.report.lock().take(0).buffer_ms.is_some());
    }

    #[test]
    fn metrics_conta_quadros() {
        let f = fx();
        f.rx.on_event(ev(1, false));
        f.rx.on_event(ev(2, true));
        // Métricas contam também o delta descartado (chegou pela rede).
        let v = f.metrics.view(66_000);
        assert!((v.fps - 2.0).abs() < 1e-6);
        assert_eq!(v.width, 16);
    }
}
