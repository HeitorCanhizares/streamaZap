//! Publicador de áudio (Opus em datagramas).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError};
use parking_lot::Mutex;
use zap_audio::AudioSelection;
use zap_media::{AudioPipeline, EncodedAudio};
use zap_proto::media::AudioHeader;

use super::viewer::ViewerLink;
use crate::CoreError;

pub enum AudioSource {
    /// Backend sintético (testes e Linux).
    Synthetic,
    Capture(AudioSelection),
}

pub struct AudioPublisher {
    pub track: u16,
    pub(crate) pipeline: AudioPipeline,
    pub(crate) viewers: Arc<Mutex<Vec<Arc<ViewerLink>>>>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AudioPublisher {
    /// `AudioPipeline::start_synthetic` ou `start`; thread "zap-audio-send" que lê
    /// `EncodedAudio` e envia `send_audio(AudioHeader{track, seq, capture_us, send_us:0}, data)`
    /// a cada espectador com `state.audio == true`.
    pub fn start(track: u16, source: AudioSource) -> Result<Arc<Self>, CoreError> {
        // Canal pequeno: se o envio atrasar, o pipeline descarta em vez de acumular latência.
        let (tx, rx) = crossbeam_channel::bounded::<EncodedAudio>(64);
        let pipeline = match source {
            AudioSource::Synthetic => AudioPipeline::start_synthetic(track, tx)?,
            AudioSource::Capture(sel) => AudioPipeline::start(track, sel, tx)?,
        };
        let viewers: Arc<Mutex<Vec<Arc<ViewerLink>>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let viewers = viewers.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("zap-audio-send".into())
                .spawn(move || envio_loop(rx, viewers, stop))?
        };
        Ok(Arc::new(Self {
            track,
            pipeline,
            viewers,
            stop,
            thread: Mutex::new(Some(handle)),
        }))
    }

    pub fn add_viewer(&self, v: Arc<ViewerLink>) {
        self.viewers.lock().push(v);
    }

    pub fn remove_viewer(&self, id: u64) {
        self.viewers.lock().retain(|v| v.id != id);
    }

    pub fn set_selection(&self, sel: AudioSelection) {
        self.pipeline.set_selection(sel);
    }

    /// Para a thread de envio e espera por ela. Idempotente.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread.lock().take() {
            // Pânico da thread de envio não deve derrubar quem chama stop/drop.
            let _ = h.join();
        }
    }
}

impl Drop for AudioPublisher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Lê pacotes Opus até `stop` ou o canal fechar (pipeline encerrado). Ao sair, o `rx` cai e o
/// pipeline percebe pelo erro de envio que pode parar.
fn envio_loop(rx: Receiver<EncodedAudio>, viewers: Arc<Mutex<Vec<Arc<ViewerLink>>>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        let pacote = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(p) => p,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // Copia a lista para não segurar o lock de espectadores durante os envios.
        let lista: Vec<Arc<ViewerLink>> = viewers.lock().clone();
        for v in &lista {
            // Só quem assina áudio recebe; o guarda sai logo, antes do envio.
            if !v.state.lock().audio {
                continue;
            }
            let header = AudioHeader {
                track: pacote.track,
                seq: pacote.seq,
                capture_us: pacote.capture_us,
                send_us: 0,
            };
            if let Err(e) = v.sender.send_audio(header, &pacote.data) {
                tracing::debug!(espectador = v.id, erro = %e, "áudio não enviado");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::time::Instant;
    use tokio::sync::mpsc;
    use zap_media::MediaError;
    use zap_net::{
        control::ControlTx,
        endpoint,
        media_rx::{spawn_receiver, MediaEvent},
        media_tx::MediaSender,
        report::ReportBuilder,
        tls::{self, Identity},
        Clock,
    };

    /// Par QUIC no loopback (mesmo helper de viewer.rs).
    struct Par {
        cli: quinn::Connection,
        srv: quinn::Connection,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn par() -> Par {
        let id = Identity::generate("zap").unwrap();
        let (servidor, porta) = endpoint::bind_server(&id, 0).await.unwrap();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let cliente = endpoint::client_endpoint().unwrap();
        let conectando = cliente
            .connect_with(tls::client_config(Some(&id.fingerprint())).unwrap(), addr, "zap")
            .unwrap();
        let inc = servidor.accept().await.unwrap();
        let srv = inc.await.unwrap();
        let cli = conectando.await.unwrap();
        Par {
            cli,
            srv,
            _eps: (servidor, cliente),
        }
    }

    /// Espectador com um par QUIC próprio: `cli` envia, `srv` tem um receptor de mídia.
    /// Devolve o link, o canal de eventos recebidos e o par (para manter as conexões vivas).
    async fn espectador(id: u64, audio: bool) -> (Arc<ViewerLink>, mpsc::Receiver<MediaEvent>, Par) {
        let p = par().await;
        let (ev_tx, ev_rx) = mpsc::channel::<MediaEvent>(4096);
        let rb = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let _rx = spawn_receiver(p.srv.clone(), Clock::new(), ev_tx, rb);
        let (send, _recv) = p.cli.open_bi().await.unwrap();
        let link = ViewerLink::new(
            id,
            format!("e{id}"),
            "t".into(),
            Arc::new(MediaSender::new(p.cli.clone(), Clock::new())),
            ControlTx::new(send),
            2,
        );
        link.state.lock().audio = audio;
        (link, ev_rx, p)
    }

    #[test]
    fn start_sintetico_e_stop() {
        let p = AudioPublisher::start(1, AudioSource::Synthetic).unwrap();
        p.stop();
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn envia_so_para_quem_assina_audio() {
        let track = 7u16;
        let publicador = AudioPublisher::start(track, AudioSource::Synthetic).unwrap();
        let (a, mut rx_a, _pa) = espectador(1, true).await;
        let (b, mut rx_b, _pb) = espectador(2, false).await;
        publicador.add_viewer(a.clone());
        publicador.add_viewer(b.clone());

        // A recebe pacotes de áudio da trilha, com seq crescente, em até 1 s.
        let prazo = Instant::now() + Duration::from_secs(1);
        let mut seqs: Vec<u32> = Vec::new();
        while seqs.len() < 5 {
            let restante = prazo.saturating_duration_since(Instant::now());
            let evento = tokio::time::timeout(restante, rx_a.recv())
                .await
                .expect("espectador A não recebeu áudio em 1 s")
                .expect("canal de A fechou");
            if let MediaEvent::Audio { header, .. } = evento {
                if header.track == track {
                    seqs.push(header.seq);
                }
            }
        }
        assert!(seqs.windows(2).all(|w| w[0] < w[1]), "seq fora de ordem: {seqs:?}");

        // B não assina áudio: nada da trilha em 300 ms.
        let fim = Instant::now() + Duration::from_millis(300);
        loop {
            let restante = fim.saturating_duration_since(Instant::now());
            if restante.is_zero() {
                break;
            }
            match tokio::time::timeout(restante, rx_b.recv()).await {
                Ok(Some(MediaEvent::Audio { header, .. })) => {
                    assert_ne!(header.track, track, "B recebeu áudio sem assinar");
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        publicador.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn remove_viewer_para_de_enviar() {
        let track = 3u16;
        let publicador = AudioPublisher::start(track, AudioSource::Synthetic).unwrap();
        let (a, mut rx_a, _pa) = espectador(1, true).await;
        publicador.add_viewer(a);

        // Confirma que o fluxo existe antes de remover.
        let mut recebidos = 0;
        while recebidos < 3 {
            let evento = tokio::time::timeout(Duration::from_secs(1), rx_a.recv())
                .await
                .expect("sem áudio antes de remover")
                .expect("canal fechou");
            if matches!(evento, MediaEvent::Audio { .. }) {
                recebidos += 1;
            }
        }

        publicador.remove_viewer(1);
        // Dá tempo para pacotes já em voo, descarta-os e então exige silêncio.
        tokio::time::sleep(Duration::from_millis(150)).await;
        while rx_a.try_recv().is_ok() {}
        let fim = Instant::now() + Duration::from_millis(300);
        loop {
            let restante = fim.saturating_duration_since(Instant::now());
            if restante.is_zero() {
                break;
            }
            match tokio::time::timeout(restante, rx_a.recv()).await {
                Ok(Some(MediaEvent::Audio { .. })) => panic!("áudio depois de remover o espectador"),
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        publicador.stop();
    }

    #[cfg(not(windows))]
    #[test]
    fn capture_fora_do_windows_unsupported() {
        let r = AudioPublisher::start(1, AudioSource::Capture(AudioSelection::All));
        assert!(matches!(r, Err(CoreError::Media(MediaError::Unsupported(_)))));
    }
}
