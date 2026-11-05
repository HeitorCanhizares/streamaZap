//! Sondagem de banda de um espectador: junta a necessidade das trilhas, o planejador e o envio
//! das rajadas de sondas (datagramas) com ritmo por crédito.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::time::MissedTickBehavior;
use zap_rate::probe::{ProbeBurst, ProbeMeter, ProbeNeed, ProbePacer, ProbePlanner};

use crate::{media_tx::MediaSender, NetError};

/// Sondagem de um espectador (uma por conexão). As trilhas informam a necessidade; quem chama
/// `poll` a cada ~100 ms e dispara `run_burst` numa tarefa quando ele devolve uma rajada.
pub struct Prober {
    sender: Arc<MediaSender>,
    planner: Mutex<ProbePlanner>,
    needs: Mutex<HashMap<u16, ProbeNeed>>,
    /// Mede, com os relatórios do espectador, os grupos de sondas desta conexão.
    meter: Mutex<ProbeMeter>,
    /// `total_kbps` de cada rajada devolvida por `poll`, em ordem (para testes e diagnóstico).
    rajadas: Mutex<Vec<u32>>,
}

impl Prober {
    pub fn new(sender: Arc<MediaSender>) -> Arc<Self> {
        Arc::new(Self {
            sender,
            planner: Mutex::new(ProbePlanner::new()),
            needs: Mutex::new(HashMap::new()),
            meter: Mutex::new(ProbeMeter::new()),
            rajadas: Mutex::new(Vec::new()),
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
        let rajada = self
            .planner
            .lock()
            .poll(now_us, need, bandwidth_kbps, self.sender.sent_bytes());
        if let Some(b) = &rajada {
            self.rajadas.lock().push(b.total_kbps);
        }
        rajada
    }
    /// `total_kbps` de cada rajada que `poll` já devolveu, em ordem. `poll` grava aqui (tarefa net-08).
    pub fn bursts(&self) -> Vec<u32> {
        self.rajadas.lock().clone()
    }
    /// Última medida do caminho (`ProbePlanner::last_measure`): (kbps, saturou).
    pub fn last_measure(&self, now_us: u64) -> Option<(u32, bool)> {
        self.planner.lock().last_measure(now_us)
    }
    /// Relatório do espectador (relógio dele nos instantes): junta `report.probes` e
    /// `report.arrivals` por `chegada_us`, alimenta o `ProbeMeter` desta conexão
    /// (`on_probe`/`on_media`), fecha o grupo com `poll(maior chegada do relatório)` e, se o
    /// relatório não tem sondas e há grupo aberto, com `finish()`. Cada resultado vai para
    /// `ProbePlanner::on_result(now_us, &r)`.
    pub fn on_report(&self, now_us: u64, report: &zap_proto::Report) {
        enum Evento {
            Sonda(u16, u64, u32),
            Midia(u64, u32),
        }
        let mut eventos: Vec<(u64, Evento)> = Vec::new();
        for &(cluster, _seq, _envio, chegada, bytes) in &report.probes {
            eventos.push((chegada, Evento::Sonda(cluster, chegada, bytes)));
        }
        for &(_trilha, _camada, _seq, _envio, chegada, bytes) in &report.arrivals {
            eventos.push((chegada, Evento::Midia(chegada, bytes)));
        }
        // Ordenação estável: o medidor espera chegadas crescentes.
        eventos.sort_by_key(|(chegada, _)| *chegada);

        let mut resultados = Vec::new();
        {
            let mut meter = self.meter.lock();
            let maior = eventos.last().map(|(c, _)| *c);
            for (_, e) in &eventos {
                match *e {
                    Evento::Sonda(cluster, chegada, bytes) => {
                        if let Some(r) = meter.on_probe(cluster, chegada, bytes) {
                            resultados.push(r);
                        }
                    }
                    Evento::Midia(chegada, bytes) => meter.on_media(chegada, bytes),
                }
            }
            if let Some(maior) = maior {
                if let Some(r) = meter.poll(maior) {
                    resultados.push(r);
                }
            }
            if report.probes.is_empty() {
                if let Some(r) = meter.finish() {
                    resultados.push(r);
                }
            }
        }
        // Os dois locks nunca ficam presos juntos.
        if !resultados.is_empty() {
            let mut planner = self.planner.lock();
            for r in &resultados {
                planner.on_result(now_us, r);
            }
        }
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
            total_kbps: 3750,
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

    fn relatorio(
        probes: Vec<(u16, u32, u32, u64, u32)>,
        arrivals: Vec<(u16, u8, u32, u32, u64, u32)>,
    ) -> zap_proto::Report {
        zap_proto::Report {
            arrivals,
            probes,
            lost: vec![],
            loss: 0.0,
            bandwidth_kbps: None,
            state: None,
            buffer_ms: None,
        }
    }

    async fn prober_com_rajada() -> Result<
        (
            Arc<Prober>,
            ProbeBurst,
            quinn::Connection,
            quinn::Endpoint,
            quinn::Endpoint,
        ),
        Erro,
    > {
        let (cli, srv, e1, e2) = par_loopback().await?;
        let p = Prober::new(Arc::new(MediaSender::new(cli, Clock::new())));
        p.set_need(1, Some(necessidade(2500, 0)));
        // O planejador precisa de >= 0,5 s de janela de envio antes de sondar.
        assert!(p.poll(0, 400).is_none());
        let b = p.poll(1_100_000, 400).ok_or("sem rajada")?;
        Ok((p, b, srv, e1, e2))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn poll_registra_total_das_rajadas() -> Result<(), Erro> {
        let (p, b, _s, _e1, _e2) = prober_com_rajada().await?;
        assert_eq!(p.bursts(), vec![b.total_kbps]);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_report_vazio_nao_faz_nada() -> Result<(), Erro> {
        let (p, _b, _s, _e1, _e2) = prober_com_rajada().await?;
        p.on_report(2_000_000, &relatorio(vec![], vec![]));
        assert_eq!(p.last_measure(2_000_000), None);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_report_sem_rajada_conhecida_ignora() -> Result<(), Erro> {
        let (p, _b, _s, _e1, _e2) = prober_com_rajada().await?;
        let sondas = (0..60)
            .map(|i| (99u16, i, 0u32, 1_000_000 + i as u64 * 3000, 1200u32))
            .collect();
        p.on_report(2_000_000, &relatorio(sondas, vec![]));
        p.on_report(2_100_000, &relatorio(vec![], vec![]));
        assert_eq!(p.last_measure(2_100_000), None);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_report_mede_o_grupo() -> Result<(), Erro> {
        let (p, b, _s, _e1, _e2) = prober_com_rajada().await?;
        let sondas = (0..60)
            .map(|i| (b.cluster, i, 0u32, 1_000_000 + i as u64 * 3000, 1200u32))
            .collect();
        p.on_report(1_300_000, &relatorio(sondas, vec![]));
        p.on_report(1_400_000, &relatorio(vec![], vec![]));
        let (kbps, saturou) = p.last_measure(1_400_000).ok_or("sem medida")?;
        assert!((2880..=3520).contains(&kbps), "kbps = {kbps}");
        assert_eq!(saturou, f64::from(kbps) < 0.85 * f64::from(b.total_kbps));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_report_junta_chegadas_de_midia() -> Result<(), Erro> {
        let (p, b, _s, _e1, _e2) = prober_com_rajada().await?;
        // Sondas de 1200 B a cada 6 ms e vídeo de 1200 B entre elas: o total é o mesmo de 3 ms.
        let sondas = (0..60)
            .map(|i| (b.cluster, i, 0u32, 1_000_000 + i as u64 * 6000, 1200u32))
            .collect();
        let midia = (0..60)
            .map(|i| (1u16, 0u8, i, 0u32, 1_003_000 + i as u64 * 6000, 1200u32))
            .collect();
        p.on_report(1_500_000, &relatorio(sondas, midia));
        p.on_report(1_600_000, &relatorio(vec![], vec![]));
        let (kbps, _) = p.last_measure(1_600_000).ok_or("sem medida")?;
        assert!((2880..=3520).contains(&kbps), "kbps = {kbps}");
        Ok(())
    }
}
