//! Envio de quadros de vídeo com prazo.

use std::sync::atomic::Ordering;
use std::time::Duration;

use bytes::Bytes;
use zap_proto::media::{VideoHeader, VIDEO_HEADER_LEN};

use super::{MediaSender, RESET_LATE};
use crate::NetError;

impl MediaSender {
    /// Envia o quadro num stream novo. `send_us` do cabeçalho é preenchido aqui. Se não terminar
    /// em `deadline`, reseta com `RESET_LATE` e devolve `Ok(false)`. Keyframe: prioridade 1.
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

        // Admissão: se a fila à frente (bytes ainda não confirmados) não esvazia a tempo na taxa
        // atual do QUIC (cwnd/RTT), descarta já: gastar banda com um quadro que seria resetado no
        // meio só atrasaria os próximos.
        let total = (VIDEO_HEADER_LEN + payload.len()) as u64;
        let fila = self.backlog.load(Ordering::Relaxed);
        if fila > 0 {
            let caminho = self.conn.stats().path;
            let rtt = caminho.rtt.as_secs_f64().max(0.001);
            let taxa = caminho.cwnd as f64 / rtt; // bytes/s
            let espera = (fila + total) as f64 / taxa.max(1.0) + rtt;
            if espera > deadline.as_secs_f64() {
                tracing::debug!(seq = header.seq, fila, "quadro descartado na admissão");
                return Ok(false);
            }
        }
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
        let mut stream = self.conn.open_uni().await?;
        // Keyframe passa na frente dos quadros comuns; falha de prioridade não é fatal.
        let _ = stream.set_priority(if header.flags.is_keyframe() { 1 } else { 0 });

        let mut h = [0u8; VIDEO_HEADER_LEN];
        header.encode(&mut h);

        // O quadro só "terminou" quando o par confirmou todos os bytes: `write_all` volta assim que
        // os bytes entram no buffer local (janela de envio grande), e um quadro parado ali, ou
        // sendo retransmitido, chegaria atrasado sem nunca ser resetado.
        let resultado = tokio::time::timeout_at(inicio + deadline, async {
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
        })
        .await;

        match resultado {
            Ok(Ok(entregue)) => Ok(entregue),
            Ok(Err(quinn::WriteError::Stopped(_))) => Ok(false),
            Ok(Err(e)) => Err(NetError::Write(e)),
            Err(_) => {
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
