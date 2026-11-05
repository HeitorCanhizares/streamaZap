//! Sondagem de banda de um espectador: junta a necessidade das trilhas, o planejador e o envio
//! das rajadas de sondas (datagramas) com ritmo por crédito.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::time::MissedTickBehavior;
use zap_rate::probe::{ProbeBurst, ProbeNeed, ProbePacer, ProbePlanner};

use crate::{media_tx::MediaSender, NetError};

/// Sondagem de um espectador (uma por conexão). As trilhas informam a necessidade; quem chama
/// `poll` a cada ~100 ms e dispara `run_burst` numa tarefa quando ele devolve uma rajada.
pub struct Prober {
    sender: Arc<MediaSender>,
    planner: Mutex<ProbePlanner>,
    needs: Mutex<HashMap<u16, ProbeNeed>>,
}

impl Prober {
    pub fn new(sender: Arc<MediaSender>) -> Arc<Self> {
        Arc::new(Self {
            sender,
            planner: Mutex::new(ProbePlanner::new()),
            needs: Mutex::new(HashMap::new()),
        })
    }
    /// Necessidade da trilha `track` (`None` = nada a testar nela). Vale a de menor `next_kbps`.
    pub fn set_need(&self, track: u16, need: Option<ProbeNeed>) {
        let mut needs = self.needs.lock();
        match need {
            Some(n) => {
                needs.insert(track, n);
            }
            None => {
                needs.remove(&track);
            }
        }
    }
    /// Esquece a trilha (assinatura cancelada).
    pub fn clear_track(&self, track: u16) {
        self.needs.lock().remove(&track);
    }
    /// `ProbePlanner::poll(now_us, menor necessidade, bandwidth_kbps, sender.sent_bytes())`.
    pub fn poll(&self, now_us: u64, bandwidth_kbps: u32) -> Option<ProbeBurst> {
        // Copia a necessidade e solta o lock de `needs` antes de chamar o planejador.
        let need = menor_necessidade(&self.needs.lock());
        self.planner
            .lock()
            .poll(now_us, need, bandwidth_kbps, self.sender.sent_bytes())
    }
    /// Manda a rajada: `ProbePacer` com o relógio do `MediaSender`, tique `interval(5 ms)`
    /// (`MissedTickBehavior::Skip`), `send_probe(cluster, seq, probe_size)`; `seq` de 0 em diante.
    /// Erro de datagrama encerra a rajada. Devolve quantos datagramas saíram.
    pub async fn run_burst(&self, burst: ProbeBurst) -> Result<u32, NetError> {
        let size = self
            .sender
            .probe_size()
            .ok_or_else(|| NetError::Other("datagrama grande".into()))?;
        let clock = &self.sender.clock;
        let mut pacer = ProbePacer::new(clock.now_us(), &burst, size);
        let mut tick = tokio::time::interval(Duration::from_millis(5));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut seq: u32 = 0;
        loop {
            tick.tick().await;
            let now = clock.now_us();
            for _ in 0..pacer.due(now) {
                match self.sender.send_probe(burst.cluster, seq, size) {
                    Ok(()) => seq += 1,
                    Err(e) => {
                        // Erro de datagrama encerra a rajada; o que já saiu conta.
                        tracing::debug!("rajada de sondas encerrada: {e}");
                        return Ok(seq);
                    }
                }
            }
            // Só sai depois de enviar o crédito deste tique.
            if pacer.finished(now) {
                break;
            }
        }
        Ok(seq)
    }
}

/// Necessidade de menor `next_kbps` (empate: a de menor trilha). `None` se não há nenhuma.
fn menor_necessidade(needs: &HashMap<u16, ProbeNeed>) -> Option<ProbeNeed> {
    needs
        .iter()
        .min_by_key(|(track, n)| (n.next_kbps, **track))
        .map(|(_, n)| *n)
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;
    use crate::{
        endpoint::{bind_server, client_endpoint},
        tls::{self, Identity},
        Clock,
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

    fn necessidade(next_kbps: u32, reserve_kbps: u32) -> ProbeNeed {
        ProbeNeed {
            next_kbps,
            reserve_kbps,
        }
    }

    #[test]
    fn necessidade_efetiva_e_a_menor() {
        let mut needs = HashMap::new();
        assert_eq!(menor_necessidade(&needs), None);

        needs.insert(1, necessidade(2500, 0));
        needs.insert(2, necessidade(1000, 0));
        assert_eq!(menor_necessidade(&needs), Some(necessidade(1000, 0)));

        needs.remove(&2);
        assert_eq!(menor_necessidade(&needs), Some(necessidade(2500, 0)));

        // Empate em next_kbps: vale a trilha de menor número.
        needs.insert(7, necessidade(2500, 99));
        assert_eq!(menor_necessidade(&needs), Some(necessidade(2500, 0)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sem_necessidade_nao_sonda() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        assert!(p.poll(0, 5000).is_none());
        assert!(p.poll(10_000_000, 5000).is_none());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn clear_track_esquece() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        p.set_need(4, Some(necessidade(1000, 0)));
        assert_eq!(menor_necessidade(&p.needs.lock()), Some(necessidade(1000, 0)));
        p.clear_track(4);
        assert_eq!(menor_necessidade(&p.needs.lock()), None);
        Ok(())
    }

    fn rajada(duration_us: u64) -> ProbeBurst {
        ProbeBurst {
            cluster: 3,
            kbps: 3300,
            duration_us,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rajada_envia_quantidade_esperada() -> Result<(), Erro> {
        let (cli, srv, _e1, _e2) = par_loopback().await?;
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        let n = p.run_burst(rajada(200_000)).await?;
        assert!((50..=90).contains(&n), "n = {n}");

        let mut esperado = 0u32;
        while esperado < n {
            let buf = tokio::time::timeout(Duration::from_secs(1), srv.read_datagram()).await??;
            match zap_proto::media::decode_datagram(&buf)? {
                zap_proto::media::Datagram::Probe(h, _) => {
                    assert_eq!(h.cluster, 3);
                    assert_eq!(h.seq, esperado);
                }
                outro => return Err(format!("datagrama inesperado: {outro:?}").into()),
            }
            esperado += 1;
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rajada_dura_cerca_de_200ms() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        let t0 = std::time::Instant::now();
        p.run_burst(rajada(200_000)).await?;
        let d = t0.elapsed();
        assert!(
            d >= Duration::from_millis(190) && d <= Duration::from_millis(600),
            "{d:?}"
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conexao_fechada_encerra_sem_travar() -> Result<(), Erro> {
        let (cli, _srv, _e1, _e2) = par_loopback().await?;
        cli.close(0u32.into(), b"");
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        let t0 = std::time::Instant::now();
        let _ = p.run_burst(rajada(200_000)).await;
        assert!(t0.elapsed() < Duration::from_secs(1));
        Ok(())
    }
}
