//! Thread de envio: pega quadros do pipeline e entrega a cada espectador na camada certa.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError};
use zap_media::EncodedFrame;
use zap_proto::media::{VideoFlags, VideoHeader};

use super::video::VideoShared;
use super::viewer::{FrameRoute, LayerInfo};

/// Laço da thread "zap-send": `rx.recv_timeout(100 ms)` até `shared.stop`. Para cada quadro:
/// 1. se `frame.keyframe`: `shared.allocator.lock().on_keyframe(frame.layer)`;
/// 2. para cada espectador: `state.route(frame.layer, frame.keyframe)`:
///    Send → `rt.spawn` de uma tarefa curta que faz `sender.send_video(VideoHeader{flags
///    (keyframe/params), track, layer, width, height, seq, capture_us, send_us:0, size:0},
///    data.clone(), sender.deadline())`; Switch → idem + `announce_layer(..., "banda")`;
///    Skip → nada. Falha de envio: só `tracing::debug!`. Uma tarefa por espectador, sem bloquear
///    os outros.
pub(crate) fn run(shared: Arc<VideoShared>, rx: Receiver<EncodedFrame>, rt: tokio::runtime::Handle) {
    while !shared.stop.load(Ordering::Relaxed) {
        let f = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(f) => f,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if f.keyframe {
            shared.allocator.lock().on_keyframe(f.layer);
        }
        // Prévia local (o que eu transmito, na minha tela): direto, sem rede.
        shared.deliver_preview(&f);
        // Cópia da lista: não segura o lock enquanto roteia.
        let viewers = shared.viewers.lock().clone();
        for v in viewers {
            let route = v.state.lock().route(f.layer, f.keyframe);
            if route == FrameRoute::Skip {
                continue;
            }
            let mut bits = 0;
            if f.keyframe {
                bits |= VideoFlags::KEYFRAME;
            }
            if f.has_params {
                bits |= VideoFlags::PARAMS;
            }
            let header = VideoHeader {
                flags: VideoFlags(bits),
                track: f.track,
                layer: f.layer,
                width: f.width,
                height: f.height,
                seq: f.seq,
                capture_us: f.capture_us,
                send_us: 0,
                size: 0,
            };
            let data = f.data.clone();
            let info = if route == FrameRoute::Switch {
                let fps = shared
                    .ladder
                    .lock()
                    .iter()
                    .find(|l| l.layer == f.layer)
                    .map_or(0, |l| l.fps);
                Some(LayerInfo {
                    layer: f.layer,
                    width: f.width,
                    height: f.height,
                    fps,
                })
            } else {
                None
            };
            // Uma tarefa por espectador: um par lento não atrasa os outros.
            rt.spawn(async move {
                if let Some(info) = info {
                    v.announce_layer(&info, "banda").await;
                }
                match v.sender.send_video(header, data, v.sender.deadline()).await {
                    Ok(true) => {}
                    Ok(false) => tracing::debug!(espectador = v.id, seq = header.seq, "quadro descartado por prazo"),
                    Err(e) => tracing::debug!(espectador = v.id, erro = %e, "falha no envio de vídeo"),
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicU32;

    use bytes::Bytes;
    use parking_lot::Mutex;
    use tokio::sync::mpsc;
    use zap_media::{VideoPipeline, VideoPipelineConfig};
    use zap_net::{
        control::{ControlRx, ControlTx},
        endpoint,
        media_rx::{spawn_receiver, MediaEvent},
        media_tx::MediaSender,
        report::ReportBuilder,
        tls::{self, Identity},
        Clock,
    };
    use zap_proto::{ControlMsg, LayerSpec};
    use zap_rate::allocator::{Allocator, Budget, ViewerInput};

    use super::*;
    use crate::publish::viewer::ViewerLink;

    type Res = Result<(), Box<dyn std::error::Error>>;

    fn escada() -> Vec<LayerSpec> {
        [(0u8, 1280u16, 720u16, 30u8), (1, 960, 540, 30), (2, 640, 360, 15)]
            .into_iter()
            .map(|(layer, width, height, fps)| LayerSpec {
                layer,
                width,
                height,
                fps,
                kbps: 1000,
            })
            .collect()
    }

    fn shared() -> Result<Arc<VideoShared>, Box<dyn std::error::Error>> {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let cfg = VideoPipelineConfig {
            track: 1,
            source: zap_media::VideoSourceSpec::Synthetic {
                width: 1280,
                height: 720,
            },
            ladder: escada(),
            encoder: Default::default(),
        };
        let pipeline = VideoPipeline::start(cfg, tx)?;
        Ok(Arc::new(VideoShared {
            track: 1,
            pipeline: pipeline.into(),
            allocator: Mutex::new(Allocator::new(escada())),
            viewers: Mutex::new(Vec::new()),
            ladder: Mutex::new(escada()),
            stop: AtomicBool::new(false),
            upload_kbps: AtomicU32::new(0),
            clock: Clock::new(),
            bridge_requests: Mutex::new(Vec::new()),
            local_preview: Mutex::new(None),
        }))
    }

    struct Ponta {
        link: Arc<ViewerLink>,
        eventos: mpsc::Receiver<MediaEvent>,
        srv: quinn::Connection,
        _resto: (
            quinn::Connection,
            quinn::Endpoint,
            quinn::Endpoint,
            tokio::task::JoinHandle<()>,
        ),
    }

    async fn ponta(id: u64) -> Result<Ponta, Box<dyn std::error::Error>> {
        let ident = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&ident, 0).await?;
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        let conectando = cliente.connect_with(tls::client_config(Some(&ident.fingerprint()))?, addr, "zap")?;
        let srv = servidor.accept().await.ok_or("sem conexão")?.await?;
        let cli = conectando.await?;

        let (send, _recv) = cli.open_bi().await?;
        let link = ViewerLink::new(
            id,
            "x".into(),
            "t_1".into(),
            Arc::new(MediaSender::new(cli.clone(), Clock::new())),
            ControlTx::new(send),
            2,
        );
        let (ev_tx, eventos) = mpsc::channel(64);
        let rb = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let h = spawn_receiver(srv.clone(), Clock::new(), ev_tx, rb);
        Ok(Ponta {
            link,
            eventos,
            srv,
            _resto: (cli, servidor, cliente, h),
        })
    }

    fn quadro(layer: u8, seq: u32, keyframe: bool) -> EncodedFrame {
        EncodedFrame {
            track: 1,
            layer,
            seq,
            keyframe,
            has_params: keyframe,
            width: 1280,
            height: 720,
            capture_us: 42,
            data: Bytes::from(vec![seq as u8; 100]),
        }
    }

    /// Sobe `run` numa thread comum; devolve o emissor e a thread.
    fn sobe(
        sh: &Arc<VideoShared>,
        rt: tokio::runtime::Handle,
    ) -> (crossbeam_channel::Sender<EncodedFrame>, std::thread::JoinHandle<()>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let sh = sh.clone();
        (tx, std::thread::spawn(move || run(sh, rx, rt)))
    }

    async fn proximo(p: &mut Ponta) -> Option<MediaEvent> {
        tokio::time::timeout(Duration::from_millis(800), p.eventos.recv())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn envia_so_para_camada_atual() -> Res {
        let sh = shared()?;
        let mut a = ponta(1).await?;
        let mut b = ponta(2).await?;
        a.link.state.lock().current = 0;
        b.link.state.lock().current = 2;
        sh.viewers.lock().push(a.link.clone());
        sh.viewers.lock().push(b.link.clone());
        let (tx, th) = sobe(&sh, tokio::runtime::Handle::current());
        tx.send(quadro(0, 7, false))?;
        match proximo(&mut a).await {
            Some(MediaEvent::Video { header, data, .. }) => {
                assert_eq!(header.seq, 7);
                assert_eq!(header.layer, 0);
                assert_eq!(header.capture_us, 42);
                assert_eq!(data.len(), 100);
            }
            e => panic!("A deveria receber vídeo: {e:?}"),
        }
        assert!(proximo(&mut b).await.is_none(), "B não deve receber a camada 0");
        sh.stop.store(true, Ordering::Relaxed);
        th.join().map_err(|_| "thread")?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn switch_em_keyframe_da_pendente() -> Res {
        let sh = shared()?;
        let mut b = ponta(2).await?;
        {
            let mut st = b.link.state.lock();
            st.current = 2;
            st.pending = Some(0);
        }
        sh.viewers.lock().push(b.link.clone());
        let (tx, th) = sobe(&sh, tokio::runtime::Handle::current());
        tx.send(quadro(0, 1, false))?;
        assert!(proximo(&mut b).await.is_none(), "delta da pendente não passa");
        tx.send(quadro(0, 2, true))?;
        match proximo(&mut b).await {
            Some(MediaEvent::Video { header, .. }) => {
                assert!(header.flags.is_keyframe());
                assert!(header.flags.has_params());
            }
            e => panic!("esperava keyframe: {e:?}"),
        }
        assert_eq!(b.link.state.lock().current, 0);
        assert_eq!(b.link.state.lock().pending, None);
        let (_s, r) = tokio::time::timeout(Duration::from_secs(2), b.srv.accept_bi()).await??;
        let msg = ControlRx::new(r).recv().await?.ok_or("sem mensagem")?;
        assert_eq!(
            msg,
            ControlMsg::LayerCurrent {
                track_id: "t_1".into(),
                layer: 0,
                width: 1280,
                height: 720,
                fps: 30,
                reason: "banda".into(),
            }
        );
        sh.stop.store(true, Ordering::Relaxed);
        th.join().map_err(|_| "thread")?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn keyframe_chama_on_keyframe_no_alocador() -> Res {
        let sh = shared()?;
        let (tx, th) = sobe(&sh, tokio::runtime::Handle::current());
        let vi = ViewerInput {
            id: 1,
            bandwidth_kbps: 30_000,
            state: zap_proto::DelayState::Normal,
            max_layer: 0,
            reserved_kbps: 0,
        };
        // Roda o alocador sem aplicar keyframes até haver camada pendente.
        let mut t = 0u64;
        let pendente = loop {
            let out = sh.allocator.lock().update(t, std::slice::from_ref(&vi), Budget::Relay);
            if let Some(p) = out.viewers[0].pending {
                break p;
            }
            t += 100_000;
            assert!(t < 20_000_000, "nenhuma camada pendente");
        };
        tx.send(quadro(pendente, 0, true))?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let out = sh
            .allocator
            .lock()
            .update(t + 100_000, std::slice::from_ref(&vi), Budget::Relay);
        assert_eq!(out.viewers[0].current, pendente);
        sh.stop.store(true, Ordering::Relaxed);
        th.join().map_err(|_| "thread")?;
        Ok(())
    }

    #[test]
    fn para_ao_receber_stop() -> Res {
        let sh = shared()?;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let (tx, th) = sobe(&sh, rt.handle().clone());
        sh.stop.store(true, Ordering::Relaxed);
        th.join().map_err(|_| "thread")?;
        drop(tx);
        Ok(())
    }
}
