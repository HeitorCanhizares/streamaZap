//! Leitura de um quadro de vídeo de um stream uni.

use bytes::Bytes;
use zap_proto::{
    media::{VideoHeader, VIDEO_HEADER_LEN},
    ProtoError,
};

use crate::{media_rx::MAX_FRAME_BYTES, Clock, NetError};

/// Teto da pré-alocação: o tamanho vem do remetente e não é confiável.
const PREALOC_MAX: usize = 1024 * 1024;
/// Bytes além do `size` anunciado que ainda lemos esperando o FIN.
const MAX_SOBRA: usize = 64 * 1024;

/// Lê um quadro de um stream uni: cabeçalho + `size` bytes (limite `crate::media_rx::MAX_FRAME_BYTES`).
pub async fn read_frame(mut recv: quinn::RecvStream, clock: &Clock) -> Result<(VideoHeader, Bytes, u64), NetError> {
    let mut cabecalho = [0u8; VIDEO_HEADER_LEN];
    recv.read_exact(&mut cabecalho).await.map_err(erro_leitura)?;
    let header = VideoHeader::decode(&cabecalho)?;

    // Recusa antes de alocar: um remetente malicioso não pode nos forçar a reservar memória
    if header.size > MAX_FRAME_BYTES {
        return Err(ProtoError::TooLarge(header.size as usize).into());
    }

    let tamanho = header.size as usize;
    let mut corpo = Vec::with_capacity(tamanho.min(PREALOC_MAX));
    while corpo.len() < tamanho {
        let falta = tamanho - corpo.len();
        let Some(pedaco) = recv.read_chunk(usize::MAX, true).await? else {
            // Stream acabou antes do que o cabeçalho anunciou
            return Err(ProtoError::Truncated.into());
        };
        // Bytes além do `size` anunciado são descartados, não viram erro
        let aproveita = falta.min(pedaco.bytes.len());
        corpo.extend_from_slice(&pedaco.bytes[..aproveita]);
    }

    // Chegada medida ao fim da leitura (último byte), como em docs/api/midia.md
    let arrival_us = clock.now_us();

    // Consome até o FIN antes de soltar o stream. Soltar um `RecvStream` que não terminou faz o
    // quinn mandar STOP_SENDING, e o emissor (que espera a confirmação em `stopped()`) entenderia
    // "não entregue". O FIN vem colado no último pacote; sobra além disso é descartada até um teto.
    let mut sobra = 0usize;
    while sobra <= MAX_SOBRA {
        match recv.read_chunk(usize::MAX, true).await {
            Ok(Some(pedaco)) => sobra += pedaco.bytes.len(),
            Ok(None) | Err(_) => break,
        }
    }
    Ok((header, Bytes::from(corpo), arrival_us))
}

fn erro_leitura(e: quinn::ReadExactError) -> NetError {
    match e {
        quinn::ReadExactError::FinishedEarly(_) => ProtoError::Truncated.into(),
        quinn::ReadExactError::ReadError(e) => NetError::Read(e),
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use zap_proto::media::VideoFlags;

    use super::*;
    use crate::{
        endpoint::{bind_server, client_endpoint},
        tls::{self, Identity},
    };

    type Erro = Box<dyn std::error::Error>;

    /// Par QUIC em loopback. Devolve as duas conexões e os endpoints (para não caírem no meio).
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

    fn cabecalho(size: u32) -> VideoHeader {
        VideoHeader {
            flags: VideoFlags(VideoFlags::KEYFRAME),
            track: 9,
            layer: 1,
            width: 640,
            height: 360,
            seq: 42,
            capture_us: 1_000,
            send_us: 0,
            size,
        }
    }

    /// Escreve cabeçalho e payload num stream uni e fecha.
    async fn envia(cli: &quinn::Connection, h: &VideoHeader, payload: &[u8]) -> Result<(), Erro> {
        let mut buf = [0u8; VIDEO_HEADER_LEN];
        h.encode(&mut buf);
        let mut send = cli.open_uni().await?;
        send.write_all(&buf).await?;
        send.write_all(payload).await?;
        send.finish()?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quadro_ok() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        let payload: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let h = cabecalho(payload.len() as u32);
        envia(&cli, &h, &payload).await?;

        let rx = srv.accept_uni().await?;
        let (recebido, dados, chegada) = read_frame(rx, &relogio).await?;
        assert_eq!(recebido, h);
        assert_eq!(dados.as_ref(), payload.as_slice());
        assert!(chegada > 0, "arrival_us não preenchido");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn payload_zero() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        let h = cabecalho(0);
        envia(&cli, &h, &[]).await?;

        let rx = srv.accept_uni().await?;
        let (recebido, dados, _) = read_frame(rx, &relogio).await?;
        assert_eq!(recebido, h);
        assert!(dados.is_empty());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn truncado() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        // Anuncia 100 bytes, manda 50 e fecha
        envia(&cli, &cabecalho(100), &[0xCD; 50]).await?;

        let rx = srv.accept_uni().await?;
        let r = tokio::time::timeout(Duration::from_secs(5), read_frame(rx, &relogio)).await?;
        assert!(matches!(r, Err(NetError::Proto(ProtoError::Truncated))), "veio {r:?}");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resetado() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        let h = cabecalho(100);
        let mut buf = [0u8; VIDEO_HEADER_LEN];
        h.encode(&mut buf);
        let mut send = cli.open_uni().await?;
        send.write_all(&buf).await?;
        send.write_all(&[0xEE; 50]).await?;
        // Reset no meio do quadro
        send.reset(quinn::VarInt::from_u32(0x10))?;

        let rx = srv.accept_uni().await?;
        let r = tokio::time::timeout(Duration::from_secs(5), read_frame(rx, &relogio)).await?;
        assert!(matches!(r, Err(NetError::Read(_))), "veio {r:?}");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn grande_demais() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        // Só o cabeçalho: a recusa tem de vir antes de ler qualquer payload
        envia(&cli, &cabecalho(MAX_FRAME_BYTES + 1), &[]).await?;

        let rx = srv.accept_uni().await?;
        let r = tokio::time::timeout(Duration::from_secs(5), read_frame(rx, &relogio)).await?;
        assert!(matches!(r, Err(NetError::Proto(ProtoError::TooLarge(_)))), "veio {r:?}");
        Ok(())
    }
}
