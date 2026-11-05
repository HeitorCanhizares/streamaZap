//! Envio de quadros de vídeo com prazo.

use std::sync::atomic::Ordering;
use std::time::Duration;

use bytes::Bytes;
use zap_proto::media::{VideoHeader, VIDEO_HEADER_LEN};

use super::keyframe::{vence_em, KeyframeState};
use super::{MediaSender, RESET_LATE};
use crate::NetError;

/// Guarda de um keyframe em voo: em qualquer saída (inclusive future cancelado) registra o fim.
struct GuardaKeyframe<'a> {
    estado: &'a KeyframeState,
    id: u64,
}

impl Drop for GuardaKeyframe<'_> {
    fn drop(&mut self) {
        self.estado.end(self.id, tokio::time::Instant::now());
    }
}

impl MediaSender {
    /// Envia o quadro num stream novo. `send_us` do cabeçalho é preenchido aqui. Se não terminar
    /// no prazo, reseta com `RESET_LATE` e devolve `Ok(false)`. Keyframe: prioridade 1 e prazo
    /// estendido (`keyframe_deadline`). Quadro comum que entrou depois de um keyframe em voo só
    /// começa a contar o prazo quando o keyframe termina.
    pub async fn send_video(
        &self,
        mut header: VideoHeader,
        payload: Bytes,
        deadline: Duration,
    ) -> Result<bool, NetError> {
        // O prazo conta desde antes de abrir o stream.
        let inicio = tokio::time::Instant::now();
        header.send_us = self.clock.send_us();
        header.size = payload.len() as u32;
        let kf = header.flags.is_keyframe();
        let deadline = if kf {
            deadline.max(self.keyframe_deadline())
        } else {
            deadline
        };

        // Admissão: se a fila à frente (bytes ainda não confirmados, sem contar keyframes em voo,
        // que têm prazo próprio) não esvazia a tempo na taxa atual do QUIC (cwnd/RTT), descarta já:
        // gastar banda com um quadro que seria resetado no meio só atrasaria os próximos.
        let total = (VIDEO_HEADER_LEN + payload.len()) as u64;
        let caminho = self.conn.stats().path;
        let rtt_us = caminho.rtt.as_micros().min(u128::from(u64::MAX)) as u64;
        let rtt_base_us = self.rtt_base_us.fetch_min(rtt_us, Ordering::Relaxed).min(rtt_us);
        let fila = self
            .backlog
            .load(Ordering::Relaxed)
            .saturating_sub(self.keyframes.bytes_em_voo());
        if fila > 0 {
            // A fila (bytes sem ACK) já inclui o que está parado no gargalo; somar o RTT atual,
            // que também já contém essa fila, contaria a espera duas vezes. Vai o RTT de base.
            let rtt = caminho.rtt.as_secs_f64().max(0.001);
            let taxa = caminho.cwnd as f64 / rtt; // bytes/s
            let rtt_base = (rtt_base_us as f64 / 1e6).max(0.001);
            let espera = (fila + total) as f64 / taxa.max(1.0) + rtt_base;
            if espera > deadline.as_secs_f64() {
                tracing::debug!(seq = header.seq, fila, "quadro descartado na admissão");
                return Ok(false);
            }
        }
        let _guarda = kf.then(|| GuardaKeyframe {
            estado: &self.keyframes,
            id: self.keyframes.begin(inicio, total),
        });
        self.backlog.fetch_add(total, Ordering::Relaxed);
        let resultado = self.envia_quadro(header, payload, deadline, inicio).await;
        self.backlog.fetch_sub(total, Ordering::Relaxed);
        resultado
    }

    async fn envia_quadro(
        &self,
        header: VideoHeader,
        payload: Bytes,
        deadline: Duration,
        inicio: tokio::time::Instant,
    ) -> Result<bool, NetError> {
        let kf = header.flags.is_keyframe();
        let mut stream = self.conn.open_uni().await?;
        // Keyframe passa na frente dos quadros comuns; falha de prioridade não é fatal.
        let _ = stream.set_priority(if kf { 1 } else { 0 });

        let mut h = [0u8; VIDEO_HEADER_LEN];
        header.encode(&mut h);

        // `None` = estourou o prazo (o stream precisa de reset); `Some` = a E/S terminou.
        let resultado = {
            // O quadro só "terminou" quando o par confirmou todos os bytes: `write_all` volta assim
            // que os bytes entram no buffer local (janela de envio grande), e um quadro parado ali,
            // ou sendo retransmitido, chegaria atrasado sem nunca ser resetado.
            let io = async {
                stream.write_all(&h).await?;
                stream.write_all(&payload).await?;
                // Stream já fechado não é problema aqui: `stopped` diz o que aconteceu.
                let _ = stream.finish();
                match stream.stopped().await {
                    Ok(None) => Ok(true),
                    // O par parou de ler: não adianta insistir.
                    Ok(Some(_)) => Ok(false),
                    Err(quinn::StoppedError::ConnectionLost(e)) => Err(quinn::WriteError::ConnectionLost(e)),
                    Err(quinn::StoppedError::ZeroRttRejected) => Err(quinn::WriteError::ZeroRttRejected),
                }
            };
            tokio::pin!(io);

            loop {
                // `Notified` habilitado ANTES do snapshot: um fim entre os dois não se perde.
                let notificado = self.keyframes.fim.notified();
                tokio::pin!(notificado);
                notificado.as_mut().enable();
                let snap = self.keyframes.snapshot();
                // O piso de 2 × RTT do prazo usa o RTT de agora, não o da entrada: se a fila
                // cresceu (retransmissão no gargalo), o quadro ainda tem o mínimo do contrato.
                let prazo = deadline.max(2 * self.rtt());
                let vence = if kf {
                    Some(inicio + prazo)
                } else {
                    vence_em(inicio, prazo, &snap.em_voo, &snap.fins)
                };
                tokio::select! {
                    r = &mut io => break Some(r),
                    _ = async {
                        match vence {
                            Some(v) => tokio::time::sleep_until(v).await,
                            None => std::future::pending::<()>().await,
                        }
                    } => {
                        // Recalcula com o estado novo: um keyframe pode ter terminado no meio.
                        let agora = tokio::time::Instant::now();
                        let prazo = deadline.max(2 * self.rtt());
                        let vence = if kf {
                            Some(inicio + prazo)
                        } else {
                            let s = self.keyframes.snapshot();
                            vence_em(inicio, prazo, &s.em_voo, &s.fins)
                        };
                        if matches!(vence, Some(v) if v <= agora) {
                            break None;
                        }
                    }
                    _ = &mut notificado => {}
                }
            }
        };

        match resultado {
            Some(Ok(entregue)) => Ok(entregue),
            Some(Err(quinn::WriteError::Stopped(_))) => Ok(false),
            Some(Err(e)) => Err(NetError::Write(e)),
            None => {
                let _ = stream.reset(quinn::VarInt::from_u32(RESET_LATE));
                tracing::debug!(seq = header.seq, "quadro de vídeo atrasado, stream resetado");
                Ok(false)
            }
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
        tls::{self, Identity},
        Clock,
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

    fn cabecalho(flags: u8) -> VideoHeader {
        VideoHeader {
            flags: VideoFlags(flags),
            track: 4,
            layer: 2,
            width: 1280,
            height: 720,
            seq: 77,
            capture_us: 123_456,
            send_us: 0,
            size: 0,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quadro_chega_inteiro() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let tx = MediaSender::new(cli, relogio);
        let payload = Bytes::from(vec![0xABu8; 50_000]);
        let ok = tx
            .send_video(cabecalho(VideoFlags::KEYFRAME), payload, Duration::from_secs(5))
            .await?;
        assert!(ok);

        let mut rx = srv.accept_uni().await?;
        let dados = rx.read_to_end(1 << 20).await?;
        assert_eq!(dados.len(), VIDEO_HEADER_LEN + 50_000);
        let h = VideoHeader::decode(&dados)?;
        assert_eq!(h.seq, 77);
        assert_eq!(h.track, 4);
        assert_eq!(h.layer, 2);
        assert!(h.flags.is_keyframe());
        assert_eq!(h.size, 50_000);
        assert!(h.send_us >= 15_000, "send_us {} não preenchido", h.send_us);
        assert!(dados[VIDEO_HEADER_LEN..].iter().all(|&b| b == 0xAB));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keyframe_usa_prioridade() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let flags = VideoFlags::KEYFRAME | VideoFlags::PARAMS;
        let ok = tx
            .send_video(cabecalho(flags), Bytes::from_static(&[1, 2, 3]), Duration::from_secs(5))
            .await?;
        assert!(ok);
        let mut rx = srv.accept_uni().await?;
        let dados = rx.read_to_end(1024).await?;
        assert_eq!(dados.len(), VIDEO_HEADER_LEN + 3);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn prazo_estourado_reseta() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let payload = Bytes::from(vec![0u8; 64 * 1024 * 1024]);
        let t0 = std::time::Instant::now();
        let ok = tx.send_video(cabecalho(0), payload, Duration::from_millis(100)).await?;
        assert!(!ok);
        assert!(t0.elapsed() < Duration::from_secs(1), "demorou {:?}", t0.elapsed());

        // O receptor só agora lê: deve ver o reset.
        let mut rx = srv.accept_uni().await?;
        let r = tokio::time::timeout(Duration::from_secs(5), rx.read_to_end(usize::MAX)).await?;
        match r {
            Err(quinn::ReadToEndError::Read(quinn::ReadError::Reset(c))) => {
                assert_eq!(c, quinn::VarInt::from_u32(RESET_LATE));
            }
            outro => return Err(format!("esperava reset, veio {outro:?}").into()),
        }
        Ok(())
    }

    /// Lê um stream uni até o fim, em segundo plano, devolvendo o tamanho.
    fn drena(srv: quinn::Connection) -> tokio::task::JoinHandle<Result<usize, String>> {
        tokio::spawn(async move {
            let mut rx = srv.accept_uni().await.map_err(|e| e.to_string())?;
            let dados = rx.read_to_end(usize::MAX).await.map_err(|e| e.to_string())?;
            Ok(dados.len())
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keyframe_nao_estoura_em_150ms() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let leitor = drena(srv);
        let payload = Bytes::from(vec![7u8; 2 * 1024 * 1024]);
        let ok = tx
            .send_video(cabecalho(VideoFlags::KEYFRAME), payload, Duration::from_millis(150))
            .await?;
        assert!(ok);
        assert_eq!(leitor.await?.map_err(Erro::from)?, VIDEO_HEADER_LEN + 2 * 1024 * 1024);
        assert_eq!(tx.keyframes.bytes_em_voo(), 0);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keyframe_prazo_estendido_ainda_reseta() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let payload = Bytes::from(vec![0u8; 64 * 1024 * 1024]);
        let t0 = std::time::Instant::now();
        let ok = tx
            .send_video(cabecalho(VideoFlags::KEYFRAME), payload, Duration::from_millis(100))
            .await?;
        let dt = t0.elapsed();
        assert!(!ok);
        assert!(
            dt >= Duration::from_millis(500) && dt <= Duration::from_millis(1500),
            "demorou {dt:?}"
        );
        assert_eq!(tx.keyframes.bytes_em_voo(), 0);
        assert_eq!(tx.keyframes.snapshot().fins.len(), 1);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn admissao_ignora_keyframe_em_voo() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let grande = 100 * 1024 * 1024u64;
        tx.backlog.store(grande, Ordering::Relaxed);

        // Sem keyframe em voo, a fila grande derruba o quadro na admissão.
        let ok = tx
            .send_video(cabecalho(0), Bytes::from_static(&[1, 2, 3]), Duration::from_millis(150))
            .await?;
        assert!(!ok);

        // Com a fila toda sendo de keyframe em voo, o quadro pequeno entra.
        let id = tx.keyframes.begin(tokio::time::Instant::now(), grande);
        let leitor = drena(srv);
        let ok = tx
            .send_video(cabecalho(0), Bytes::from_static(&[1, 2, 3]), Duration::from_millis(150))
            .await?;
        tx.keyframes.end(id, tokio::time::Instant::now());
        assert!(ok);
        assert_eq!(leitor.await?.map_err(Erro::from)?, VIDEO_HEADER_LEN + 3);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quadro_atras_de_keyframe_conta_a_partir_do_fim() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = std::sync::Arc::new(MediaSender::new(cli, Clock::new()));
        let id = tx.keyframes.begin(tokio::time::Instant::now(), 1);
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Quadro maior que a janela do stream (16 MiB): fica travado até alguém ler.
        let tamanho = 20 * 1024 * 1024;
        let tx2 = tx.clone();
        let envio = tokio::spawn(async move {
            tx2.send_video(
                cabecalho(0),
                Bytes::from(vec![3u8; tamanho]),
                Duration::from_millis(100),
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !envio.is_finished(),
            "o prazo não devia contar antes do fim do keyframe"
        );

        tx.keyframes.end(id, tokio::time::Instant::now());
        let leitor = drena(srv);
        assert!(envio.await??);
        assert_eq!(leitor.await?.map_err(Erro::from)?, VIDEO_HEADER_LEN + tamanho);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn payload_vazio_ok() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        let ok = tx
            .send_video(cabecalho(0), Bytes::new(), Duration::from_secs(5))
            .await?;
        assert!(ok);
        let mut rx = srv.accept_uni().await?;
        let dados = rx.read_to_end(1024).await?;
        assert_eq!(dados.len(), VIDEO_HEADER_LEN);
        assert_eq!(VideoHeader::decode(&dados)?.size, 0);
        Ok(())
    }
}
