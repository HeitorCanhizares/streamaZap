//! Task que envia o relatório a cada 100 ms.

use std::time::Duration;

use super::{ReportBuilderHandle, REPORT_INTERVAL};
use crate::{control::ControlTx, Clock};
use tokio::time::{interval, MissedTickBehavior};
use zap_proto::ControlMsg;

/// Período em que se confere o estado `Overuse` para enviar na hora.
const CHECA_OVERUSE: Duration = Duration::from_millis(20);

/// A cada 100 ms envia `relatorio`; também na hora quando vira `Overuse`. Termina se o envio falha.
pub fn spawn_reporter(ctrl: ControlTx, rb: ReportBuilderHandle, clock: Clock) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut periodico = interval(REPORT_INTERVAL);
        periodico.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut vigia = interval(CHECA_OVERUSE);
        vigia.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut estava_overuse = false;

        loop {
            // Só envia quando há tick periódico ou virada false -> true de `Overuse`.
            let enviar = tokio::select! {
                _ = periodico.tick() => true,
                _ = vigia.tick() => {
                    // O guard do lock é solto no fim do `let`, antes de qualquer await.
                    let overuse = rb.lock().is_overuse();
                    let virou = overuse && !estava_overuse;
                    estava_overuse = overuse;
                    virou
                }
            };
            if enviar && !send_report(&ctrl, &rb, &clock).await {
                break;
            }
        }
    })
}

/// Monta e envia um relatório. `false` = conexão de controle caiu (a task deve terminar).
async fn send_report(ctrl: &ControlTx, rb: &ReportBuilderHandle, clock: &Clock) -> bool {
    // Escopo fechado: o guard (parking_lot, não é Send) não pode atravessar o `.await`.
    let report = {
        let mut g = rb.lock();
        g.take(clock.now_us())
    };
    match ctrl.send(&ControlMsg::Report(report)).await {
        Ok(()) => true,
        Err(erro) => {
            tracing::debug!(erro = %erro, "relatório não enviado; reporter encerrado");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        control::ControlRx,
        endpoint,
        report::ReportBuilder,
        tls::{self, Identity},
    };
    use parking_lot::Mutex;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use zap_proto::media::{VideoFlags, VideoHeader};

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

    /// Cliente abre o stream de controle com o reporter; servidor recebe com `ControlRx`.
    /// Devolve também a metade de leitura do cliente, que precisa ficar viva.
    async fn liga(
        p: &Par,
        rb: ReportBuilderHandle,
    ) -> Result<(tokio::task::JoinHandle<()>, ControlRx, quinn::RecvStream), Box<dyn std::error::Error>> {
        let (send, rd_cli) = p.cli.open_bi().await?;
        let h = spawn_reporter(ControlTx::new(send), rb, Clock::new());
        let (_wr, recv) = p.srv.accept_bi().await?;
        Ok((h, ControlRx::new(recv), rd_cli))
    }

    /// Recebe um `Report` ignorando outras mensagens de controle.
    async fn proximo_relatorio(rx: &mut ControlRx) -> Result<zap_proto::Report, Box<dyn std::error::Error>> {
        loop {
            match rx.recv().await? {
                Some(ControlMsg::Report(r)) => return Ok(r),
                Some(_) => continue,
                None => return Err("stream de controle terminou".into()),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chega_a_cada_100ms() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let rb: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(false)));
        let (_h, mut rx, _rd) = liga(&p, rb).await?;

        let fim = tokio::time::Instant::now() + Duration::from_secs(1);
        let mut recebidos = 0usize;
        while tokio::time::Instant::now() < fim {
            let restante = fim.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(restante, proximo_relatorio(&mut rx)).await {
                Ok(r) => {
                    r?;
                    recebidos += 1;
                }
                Err(_) => break,
            }
        }
        assert!((7..=12).contains(&recebidos), "relatórios em ~1 s: {recebidos}");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn leva_chegadas() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let rb: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(false)));
        let (_h, mut rx, _rd) = liga(&p, rb.clone()).await?;

        // Um relatório pode já estar em trânsito; o próximo que contiver a chegada serve.
        rb.lock().on_video(&vh(42, 1000), 2000, 1200);
        let achou = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let r = proximo_relatorio(&mut rx).await?;
                if r.arrivals.iter().any(|a| a.0 == 3 && a.2 == 42) {
                    return Ok::<bool, Box<dyn std::error::Error>>(true);
                }
            }
        })
        .await??;
        assert!(achou);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn termina_se_conexao_cai() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let rb: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(false)));
        let (h, _rx, _rd) = liga(&p, rb).await?;

        p.cli.close(0u32.into(), b"fim");
        p.srv.close(0u32.into(), b"fim");
        tokio::time::timeout(Duration::from_secs(1), h).await??;
        Ok(())
    }
}
