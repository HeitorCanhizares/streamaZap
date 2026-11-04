//! Envio de mídia: quadros de vídeo em streams uni com prazo, áudio e sonda em datagramas.

use std::time::Duration;

use bytes::Bytes;
use zap_proto::media::{encode_audio, encode_probe, AudioHeader, ProbeHeader};

mod video;

use crate::{Clock, NetError};

/// Código de reset de stream para "atrasado".
pub const RESET_LATE: u32 = 0x10;

/// Piso do prazo de um quadro de vídeo: nem em rede muito rápida o prazo fica menor que isso.
const PRAZO_MINIMO: Duration = Duration::from_millis(150);

/// Tamanho mínimo de uma sonda (o cabeçalho sozinho).
const SONDA_MINIMA: usize = 12;

pub struct MediaSender {
    pub(crate) conn: quinn::Connection,
    pub(crate) clock: Clock,
    /// Bytes de vídeo aceitos e ainda não confirmados pelo par (fila do emissor).
    pub(crate) backlog: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl MediaSender {
    pub fn new(conn: quinn::Connection, clock: Clock) -> Self {
        Self {
            conn,
            clock,
            backlog: Default::default(),
        }
    }

    pub fn rtt(&self) -> Duration {
        self.conn.rtt()
    }

    /// `max(150 ms, 2 × RTT)`.
    pub fn deadline(&self) -> Duration {
        PRAZO_MINIMO.max(2 * self.rtt())
    }

    /// Envia um pacote de áudio em datagrama. `send_us` é preenchido aqui, sobrescrevendo o valor
    /// recebido, para medir o tempo de envio no mesmo relógio.
    pub fn send_audio(&self, mut header: AudioHeader, opus: &[u8]) -> Result<(), NetError> {
        header.send_us = self.clock.send_us();
        let buf = encode_audio(&header, opus);
        self.enviar_datagrama(buf)
    }

    /// Envia uma sonda de `total_len` bytes (preenchida com zeros). Tamanhos acima do que cabe
    /// num datagrama são cortados no limite, para a sonda continuar válida.
    pub fn send_probe(&self, cluster: u16, seq: u32, total_len: usize) -> Result<(), NetError> {
        let h = ProbeHeader {
            cluster,
            seq,
            send_us: self.clock.send_us(),
        };
        let max = self.limite_datagrama()?;
        let len = total_len.min(max).max(SONDA_MINIMA);
        let buf = encode_probe(&h, len);
        self.enviar_datagrama(buf)
    }

    /// Tamanho máximo de datagrama aceito pela conexão. Sem suporte a datagrama é erro.
    fn limite_datagrama(&self) -> Result<usize, NetError> {
        self.conn
            .max_datagram_size()
            .ok_or_else(|| NetError::Other("datagrama grande".into()))
    }

    /// Confere o tamanho antes de enfileirar: datagrama grande demais é recusado sem enviar.
    fn enviar_datagrama(&self, buf: Vec<u8>) -> Result<(), NetError> {
        let max = self.limite_datagrama()?;
        if buf.len() > max {
            return Err(NetError::Other("datagrama grande".into()));
        }
        self.conn.send_datagram(Bytes::from(buf))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use zap_proto::media::{decode_datagram, Datagram};

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

    /// Lê um datagrama do outro lado, com limite para o teste não travar.
    async fn recebe(conn: &quinn::Connection) -> Result<Bytes, Erro> {
        let buf = tokio::time::timeout(Duration::from_secs(5), conn.read_datagram()).await??;
        Ok(buf)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deadline_minimo_150ms() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        assert_eq!(tx.deadline(), Duration::from_millis(150));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn audio_chega_com_send_us_preenchido() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let relogio = Clock::new();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let tx = MediaSender::new(cli, relogio);
        let opus = [9u8, 8, 7, 6, 5];
        let h = AudioHeader {
            track: 3,
            seq: 42,
            capture_us: 1_000,
            send_us: 0,
        };
        tx.send_audio(h, &opus)?;

        let buf = recebe(&srv).await?;
        match decode_datagram(&buf)? {
            Datagram::Audio(rec, dados) => {
                assert_eq!(dados, &opus[..]);
                assert_eq!(rec.track, 3);
                assert_eq!(rec.seq, 42);
                assert_eq!(rec.capture_us, 1_000);
                // O relógio foi criado ~20 ms antes do envio: send_us precisa refletir isso.
                assert!(rec.send_us >= 15_000, "send_us {} não veio do relógio", rec.send_us);
            }
            outro => return Err(format!("esperava áudio, veio {outro:?}").into()),
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sonda_tamanho() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let tx = MediaSender::new(cli, Clock::new());
        tx.send_probe(7, 1, 1000)?;

        let buf = recebe(&srv).await?;
        assert_eq!(buf.len(), 1000);
        match decode_datagram(&buf)? {
            Datagram::Probe(h, len) => {
                assert_eq!(h.cluster, 7);
                assert_eq!(h.seq, 1);
                assert_eq!(len, 1000);
            }
            outro => return Err(format!("esperava sonda, veio {outro:?}").into()),
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sonda_acima_do_mtu_e_cortada() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let max = cli.max_datagram_size().ok_or("sem datagramas")?;
        let tx = MediaSender::new(cli, Clock::new());
        tx.send_probe(2, 5, 100_000)?;

        let buf = recebe(&srv).await?;
        assert!(buf.len() <= max, "{} > {max}", buf.len());
        assert!(matches!(decode_datagram(&buf)?, Datagram::Probe(_, _)));
        Ok(())
    }
}
