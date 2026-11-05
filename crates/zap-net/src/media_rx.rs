//! Recepção de mídia: streams uni de vídeo e datagramas de áudio/sonda.

mod frame;
mod gap;

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
pub use frame::{read_frame, read_frame_with};
pub use gap::GapTracker;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use zap_proto::media::{decode_datagram, AudioHeader, Datagram, VideoHeader};

use crate::{report::ReportBuilderHandle, Clock};

/// Limite do payload de um quadro.
pub const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;
/// Tempo para um buraco de `seq` se fechar antes de virar `Lost`.
pub const GAP_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

#[derive(Debug, Clone)]
pub enum MediaEvent {
    Video {
        header: VideoHeader,
        data: Bytes,
        arrival_us: u64,
    },
    Audio {
        header: AudioHeader,
        opus: Bytes,
        arrival_us: u64,
    },
    Lost {
        track: u16,
        layer: u8,
        seq: u32,
    },
}

/// Aceita streams uni e datagramas até a conexão fechar. Toda chegada vai para `reports`.
pub fn spawn_receiver(
    conn: quinn::Connection,
    clock: Clock,
    tx: mpsc::Sender<MediaEvent>,
    reports: ReportBuilderHandle,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let gap = Arc::new(Mutex::new(GapTracker::new()));
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                uni = conn.accept_uni() => {
                    let Ok(recv) = uni else { break };
                    tokio::spawn(trata_stream(recv, clock.clone(), tx.clone(), reports.clone(), gap.clone()));
                }
                dg = conn.read_datagram() => {
                    let Ok(bytes) = dg else { break };
                    if !trata_datagrama(&bytes, &clock, &tx, &reports).await {
                        break;
                    }
                }
                _ = tick.tick() => {
                    // Copia os buracos vencidos e solta o lock antes de qualquer await.
                    let lost = gap.lock().expire(clock.now_us());
                    for (track, layer, seq, visto_us) in lost {
                        reports.lock().on_lost(track, layer, seq, visto_us);
                        if tx.send(MediaEvent::Lost { track, layer, seq }).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    })
}

/// Lê um quadro e o entrega. Reset/truncado só vira log: o `GapTracker` acha o buraco de `seq`.
async fn trata_stream(
    recv: quinn::RecvStream,
    clock: Clock,
    tx: mpsc::Sender<MediaEvent>,
    reports: ReportBuilderHandle,
    gap: Arc<Mutex<GapTracker>>,
) {
    // Cabeçalho de keyframe avisa o estimador antes do corpo (carência). O lock é solto logo na chamada.
    let res = read_frame_with(recv, &clock, |h, t| {
        if h.flags.is_keyframe() {
            reports.lock().on_keyframe_start(t);
        }
    })
    .await;
    match res {
        Ok((header, data, arrival_us)) => {
            reports.lock().on_video(&header, arrival_us, (30 + data.len()) as u32);
            gap.lock().on_seq(header.track, header.layer, header.seq, arrival_us);
            // Erro aqui = receptor fechou; encerra em silêncio.
            let _ = tx
                .send(MediaEvent::Video {
                    header,
                    data,
                    arrival_us,
                })
                .await;
        }
        Err(e) => tracing::debug!(erro = %e, "quadro de vídeo não lido"),
    }
}

/// Trata um datagrama. Devolve `false` se o receptor fechou.
async fn trata_datagrama(
    bytes: &Bytes,
    clock: &Clock,
    tx: &mpsc::Sender<MediaEvent>,
    reports: &ReportBuilderHandle,
) -> bool {
    let arrival_us = clock.now_us();
    match decode_datagram(bytes) {
        Ok(Datagram::Audio(header, opus)) => {
            reports.lock().on_audio(&header, arrival_us, bytes.len() as u32);
            let opus = Bytes::copy_from_slice(opus);
            tx.send(MediaEvent::Audio {
                header,
                opus,
                arrival_us,
            })
            .await
            .is_ok()
        }
        Ok(Datagram::Probe(header, total)) => {
            reports.lock().on_probe(&header, arrival_us, total as u32);
            true
        }
        Err(e) => {
            tracing::debug!(erro = %e, "datagrama inválido ignorado");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use zap_proto::media::VideoFlags;

    use super::*;
    use crate::{
        endpoint::{bind_server, client_endpoint},
        media_tx::MediaSender,
        report::ReportBuilder,
        tls::{self, Identity},
    };

    type Erro = Box<dyn std::error::Error>;

    async fn par_loopback() -> Result<(quinn::Connection, quinn::Connection, quinn::Endpoint, quinn::Endpoint), Erro> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = bind_server(&id, 0).await?;
        let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let aceita = tokio::spawn(async move {
            let inc = servidor.accept().await.ok_or_else(|| "sem conexão".to_string())?;
            let conn = inc.await.map_err(|e| e.to_string())?;
            Ok::<_, String>((servidor, conn))
        });
        let cliente = client_endpoint()?;
        let conn_cli = cliente
            .connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?
            .await?;
        let (servidor, conn_srv) = aceita.await?.map_err(|e| -> Erro { e.into() })?;
        Ok((conn_cli, conn_srv, cliente, servidor))
    }

    fn cab(seq: u32) -> VideoHeader {
        VideoHeader {
            flags: VideoFlags(0),
            track: 3,
            layer: 0,
            width: 320,
            height: 180,
            seq,
            capture_us: 1_000 + seq as u64,
            send_us: 0,
            size: 0,
        }
    }

    struct Cena {
        sender: MediaSender,
        rx: mpsc::Receiver<MediaEvent>,
        rb: ReportBuilderHandle,
        srv: quinn::Connection,
        handle: tokio::task::JoinHandle<()>,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn cena() -> Result<Cena, Erro> {
        let (cli, srv, e1, e2) = par_loopback().await?;
        let clock = Clock::new();
        let rb: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(false)));
        let (tx, rx) = mpsc::channel(64);
        let handle = spawn_receiver(srv.clone(), clock.clone(), tx, rb.clone());
        Ok(Cena {
            sender: MediaSender::new(cli, clock),
            rx,
            rb,
            srv,
            handle,
            _eps: (e1, e2),
        })
    }

    async fn proximo(rx: &mut mpsc::Receiver<MediaEvent>) -> Result<MediaEvent, Erro> {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await?
            .ok_or_else(|| "canal fechado".into())
    }

    async fn envia(c: &Cena, seq: u32, dados: &[u8]) -> Result<(), Erro> {
        c.sender
            .send_video(cab(seq), Bytes::copy_from_slice(dados), Duration::from_secs(2))
            .await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quadros_em_ordem() -> Result<(), Erro> {
        let mut c = cena().await?;
        // Streams são lidos em tasks paralelas: enviar e esperar cada um garante a ordem de seq.
        for seq in 0..10u32 {
            envia(&c, seq, &[seq as u8; 100]).await?;
            match proximo(&mut c.rx).await? {
                MediaEvent::Video { header, data, .. } => {
                    assert_eq!(header.seq, seq);
                    assert_eq!(header.capture_us, 1_000 + seq as u64);
                    assert_eq!(data.as_ref(), &[seq as u8; 100][..]);
                }
                outro => return Err(format!("evento inesperado: {outro:?}").into()),
            }
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn audio_chega() -> Result<(), Erro> {
        let mut c = cena().await?;
        let h = AudioHeader {
            track: 5,
            seq: 7,
            capture_us: 99,
            send_us: 0,
        };
        c.sender.send_audio(h, &[1, 2, 3])?;
        match proximo(&mut c.rx).await? {
            MediaEvent::Audio { header, opus, .. } => {
                assert_eq!(header.seq, 7);
                assert_eq!(opus.as_ref(), &[1, 2, 3]);
            }
            outro => return Err(format!("evento inesperado: {outro:?}").into()),
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sonda_so_vai_para_relatorio() -> Result<(), Erro> {
        let mut c = cena().await?;
        c.sender.send_probe(1, 1, 500)?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(c.rx.try_recv().is_err());
        let r = c.rb.lock().take(c.sender.clock.now_us());
        assert_eq!(r.probes.len(), 1);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn buraco_vira_lost() -> Result<(), Erro> {
        let mut c = cena().await?;
        for seq in [0u32, 1, 3] {
            envia(&c, seq, &[0u8; 10]).await?;
            // Espera chegar para fixar a ordem de registro no detector de buracos.
            proximo(&mut c.rx).await?;
        }
        let mut perdido = None;
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < fim {
            if let MediaEvent::Lost { seq, .. } = proximo(&mut c.rx).await? {
                perdido = Some(seq);
                break;
            }
        }
        assert_eq!(perdido, Some(2));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relatorio_registra() -> Result<(), Erro> {
        let mut c = cena().await?;
        for seq in 0..10u32 {
            envia(&c, seq, &[0u8; 50]).await?;
        }
        for _ in 0..10 {
            proximo(&mut c.rx).await?;
        }
        let r = c.rb.lock().take(c.sender.clock.now_us());
        assert_eq!(r.arrivals.len(), 10);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn encerra_com_a_conexao() -> Result<(), Erro> {
        let c = cena().await?;
        c.srv.close(quinn::VarInt::from_u32(0), b"fim");
        tokio::time::timeout(Duration::from_secs(1), c.handle).await??;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keyframe_avisa_o_estimador() -> Result<(), Erro> {
        let (cli, srv, e1, e2) = par_loopback().await?;
        let clock = Clock::new();
        let rb: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(true)));
        let (tx, mut rx) = mpsc::channel(64);
        let _handle = spawn_receiver(srv.clone(), clock.clone(), tx, rb.clone());
        let sender = MediaSender::new(cli, clock);

        let mut chave = cab(0);
        chave.flags = VideoFlags(VideoFlags::KEYFRAME);
        sender
            .send_video(chave, Bytes::copy_from_slice(&[1u8; 100]), Duration::from_secs(2))
            .await?;
        envia_sem_flag(&sender, 1, &[2u8; 100]).await?;

        for _ in 0..2 {
            proximo(&mut rx).await?;
        }
        assert_eq!(rb.lock().keyframe_starts, 1);
        // Mantém os endpoints vivos até o fim do teste.
        let _eps = (e1, e2);
        Ok(())
    }

    async fn envia_sem_flag(sender: &MediaSender, seq: u32, dados: &[u8]) -> Result<(), Erro> {
        sender
            .send_video(cab(seq), Bytes::copy_from_slice(dados), Duration::from_secs(2))
            .await?;
        Ok(())
    }
}
