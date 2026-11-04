//! Stream de controle: envio e recepção de `ControlMsg` com prefixo u32.

use std::sync::Arc;
use zap_proto::{framing::ControlDecoder, ControlMsg, ProtoError};

use crate::NetError;

/// Tamanho do buffer de leitura: cabe várias mensagens pequenas por chamada.
const TAMANHO_LEITURA: usize = 16 * 1024;

#[derive(Clone)]
pub struct ControlTx {
    pub(crate) inner: Arc<tokio::sync::Mutex<quinn::SendStream>>,
}

impl ControlTx {
    pub fn new(stream: quinn::SendStream) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(stream)),
        }
    }
    pub async fn send(&self, msg: &ControlMsg) -> Result<(), NetError> {
        let bytes = zap_proto::framing::encode_control(msg)?;
        // Um único write_all por mensagem: com a trava, mensagens de tasks diferentes
        // não se misturam no meio do quadro.
        let mut stream = self.inner.lock().await;
        stream.write_all(&bytes).await?;
        Ok(())
    }
}

pub struct ControlRx {
    pub(crate) stream: quinn::RecvStream,
    pub(crate) decoder: ControlDecoder,
}

impl ControlRx {
    pub fn new(stream: quinn::RecvStream) -> Self {
        Self {
            stream,
            decoder: ControlDecoder::new(),
        }
    }
    /// `None` = stream terminou. Mensagens desconhecidas são logadas e puladas.
    pub async fn recv(&mut self) -> Result<Option<ControlMsg>, NetError> {
        let mut buf = vec![0u8; TAMANHO_LEITURA];
        loop {
            // Primeiro o que já está no buffer: pode haver mensagem completa de leituras antigas.
            match self.decoder.next() {
                Ok(Some(msg)) => return Ok(Some(msg)),
                Ok(None) => {}
                Err(ProtoError::UnknownMessage(tipo)) => {
                    // O decoder já descartou o quadro; basta seguir para a próxima mensagem.
                    tracing::warn!(tipo = %tipo, "mensagem de controle desconhecida ignorada");
                    continue;
                }
                Err(outro) => return Err(NetError::Proto(outro)),
            }
            // Falta byte: lê mais do stream. Fim do stream sem mensagem pendente é `None`.
            match self.stream.read(&mut buf).await? {
                Some(n) => self.decoder.push(&buf[..n]),
                None => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        endpoint,
        tls::{self, Identity},
    };
    use std::net::SocketAddr;

    /// Par de conexões QUIC locais. Mantém endpoints e conexões vivos: soltá-los fecha o stream.
    struct Par {
        cli: quinn::Connection,
        srv: quinn::Connection,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn par() -> Result<Par, Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
        let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?;
        let inc = servidor.accept().await.ok_or("sem conexão")?;
        let srv = inc.await?;
        let cli = conectando.await?;
        Ok(Par {
            cli,
            srv,
            _eps: (servidor, cliente),
        })
    }

    /// Abre o stream de controle do cliente e devolve (tx, metade de leitura do cliente).
    /// A metade de leitura fica viva com o chamador para o stream não ser resetado.
    async fn abre(p: &Par) -> Result<(ControlTx, quinn::RecvStream), Box<dyn std::error::Error>> {
        let (send, recv) = p.cli.open_bi().await?;
        Ok((ControlTx::new(send), recv))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn roundtrip_pela_rede() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (tx, _rd) = abre(&p).await?;
        tx.send(&ControlMsg::Ping { ts_ms: 7 }).await?;
        let (_wr, recv) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv);
        assert_eq!(rx.recv().await?, Some(ControlMsg::Ping { ts_ms: 7 }));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn varias_mensagens_em_ordem() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (tx, _rd) = abre(&p).await?;
        for ts in 0..50u64 {
            tx.send(&ControlMsg::Ping { ts_ms: ts }).await?;
        }
        let (_wr, recv) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv);
        for ts in 0..50u64 {
            assert_eq!(rx.recv().await?, Some(ControlMsg::Ping { ts_ms: ts }));
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fim_do_stream() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (tx, _rd) = abre(&p).await?;
        tx.send(&ControlMsg::Ping { ts_ms: 1 }).await?;
        tx.inner.lock().await.finish()?;
        let (_wr, recv) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv);
        assert_eq!(rx.recv().await?, Some(ControlMsg::Ping { ts_ms: 1 }));
        assert_eq!(rx.recv().await?, None);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn desconhecida_e_pulada() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (tx, _rd) = abre(&p).await?;
        // Quadro com tipo que não existe, seguido de um Ping válido.
        let raw = br#"{"t":"zzz"}"#;
        let mut bruto = (raw.len() as u32).to_be_bytes().to_vec();
        bruto.extend_from_slice(raw);
        tx.inner.lock().await.write_all(&bruto).await?;
        tx.send(&ControlMsg::Ping { ts_ms: 9 }).await?;
        let (_wr, recv) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv);
        assert_eq!(rx.recv().await?, Some(ControlMsg::Ping { ts_ms: 9 }));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn envio_concorrente() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (tx, _rd) = abre(&p).await?;
        let mut tarefas = Vec::new();
        for _ in 0..4 {
            let tx = tx.clone();
            tarefas.push(tokio::spawn(async move {
                for ts in 0..25u64 {
                    tx.send(&ControlMsg::Ping { ts_ms: ts }).await?;
                }
                Ok::<(), NetError>(())
            }));
        }
        let (_wr, recv) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv);
        for _ in 0..100 {
            match rx.recv().await? {
                Some(ControlMsg::Ping { .. }) => {}
                outro => return Err(format!("mensagem inesperada: {outro:?}").into()),
            }
        }
        for t in tarefas {
            t.await??;
        }
        Ok(())
    }
}
