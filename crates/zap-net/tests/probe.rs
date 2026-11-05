//! Sondagem de banda em QUIC no loopback (tarefa net-06).
//!
//! Emissor com vídeo de ~400 kbps e um `Prober` querendo 2500 kbps: a rajada sai ~1 s depois do
//! início, o receptor mede a banda e o relatório seguinte a traz acima de 3195 kbps.
mod support;

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use parking_lot::Mutex;
use tokio::{sync::mpsc, task::JoinHandle, time::interval};
use zap_net::{
    endpoint,
    handshake::{accept_handshake, connect_handshake, Hello, ServerAuth},
    media_rx::{spawn_receiver, MediaEvent},
    media_tx::MediaSender,
    probe::Prober,
    report::{spawn_reporter, ReportBuilder},
    tls::{self, Identity},
    Clock, NetError,
};
use zap_proto::{
    media::{VideoFlags, VideoHeader},
    types::Access,
    ControlMsg, Room, RoomCode, RoomMode,
};
use zap_rate::probe::ProbeNeed;

type Res = Result<(), Box<dyn std::error::Error>>;
/// Relatórios recebidos: (instante, banda estimada).
type Relatorios = Arc<Mutex<Vec<(Duration, Option<u32>)>>>;

fn sala() -> Room {
    Room {
        id: "r1".into(),
        code: Some(RoomCode::from_entropy([2, 3, 4, 5])),
        name: "Sala".into(),
        owner_id: "p0".into(),
        access: Access::Open,
        mode: RoomMode::Local,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn hello() -> Hello {
    Hello {
        app_version: "1.0".into(),
        install_id: "inst".into(),
        name: "Ana".into(),
        color: "#fff".into(),
        ticket: None,
        room_id: None,
        preview: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sonda_sobe_banda_no_loopback() -> Res {
    let _timer = support::timer();
    tokio::time::timeout(Duration::from_secs(15), cenario(false)).await?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn on_report_mede_o_grupo() -> Res {
    let _timer = support::timer();
    tokio::time::timeout(Duration::from_secs(15), cenario(true)).await?
}

/// `medir`: os relatórios também alimentam `Prober::on_report` e o cenário espera a medida.
async fn cenario(medir: bool) -> Res {
    // Par QUIC: o cliente emite, o servidor recebe e reporta.
    let id = Identity::generate("zap")?;
    let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
    let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
    let cliente = endpoint::client_endpoint()?;
    let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?;
    let inc = servidor.accept().await.ok_or("sem conexão")?;
    let conn_srv = inc.await?;
    let conn_cli = conectando.await?;

    let s = sala();
    let (r, c) = tokio::join!(
        accept_handshake(&conn_srv, &ServerAuth::Open, &s, "p1".into()),
        connect_handshake(&conn_cli, hello(), None)
    );
    let (srv_tx, _srv_rx, _h) = r?;
    let (_cli_tx, mut cli_rx, _w) = c?;

    // Receptor com estimativa de banda ligada.
    let clock_rx = Clock::new();
    let rb = Arc::new(Mutex::new(ReportBuilder::new(true)));
    let (ev_tx, mut ev_rx) = mpsc::channel::<MediaEvent>(1024);
    let _rx = spawn_receiver(conn_srv.clone(), clock_rx.clone(), ev_tx, rb.clone());
    let _rep = spawn_reporter(srv_tx, rb, clock_rx);
    tokio::spawn(async move { while ev_rx.recv().await.is_some() {} });

    let clock = Clock::new();
    let sender = Arc::new(MediaSender::new(conn_cli.clone(), clock.clone()));
    let prober = Prober::new(sender.clone());
    prober.set_need(
        1,
        Some(ProbeNeed {
            next_kbps: 2500,
            reserve_kbps: 0,
        }),
    );

    // Emissor lê os relatórios: guarda (instante, banda) e a última banda para o `poll`.
    let inicio = Instant::now();
    let ultima = Arc::new(Mutex::new(0u32));
    let rels: Relatorios = Arc::new(Mutex::new(Vec::new()));
    {
        let (ultima, rels) = (ultima.clone(), rels.clone());
        let (prober, clock) = (prober.clone(), clock.clone());
        tokio::spawn(async move {
            while let Ok(Some(msg)) = cli_rx.recv().await {
                if let ControlMsg::Report(r) = msg {
                    if medir {
                        prober.on_report(clock.now_us(), &r);
                    }
                    if let Some(b) = r.bandwidth_kbps {
                        *ultima.lock() = b;
                    }
                    rels.lock().push((inicio.elapsed(), r.bandwidth_kbps));
                }
            }
        });
    }

    let tamanho_sonda = sender.probe_size();
    let quadro = Bytes::from(vec![0x5au8; 1666]);
    let prazo = Duration::from_secs(2);
    let mut t_video = interval(Duration::from_millis(33));
    let mut t_poll = interval(Duration::from_millis(100));
    let mut seq = 0u32;
    // (instante da saída, tarefa da rajada)
    let mut rajadas: Vec<(Duration, JoinHandle<Result<u32, NetError>>)> = Vec::new();
    let mut rajadas_3s = 0;

    let limite = Duration::from_secs(if medir { 8 } else { 5 });
    let mut medida = None;
    while inicio.elapsed() < limite {
        tokio::select! {
            _ = t_video.tick() => {
                let h = VideoHeader {
                    flags: VideoFlags(if seq == 0 { VideoFlags::KEYFRAME } else { 0 }),
                    track: 1,
                    layer: 0,
                    width: 1280,
                    height: 720,
                    seq,
                    capture_us: clock.now_us(),
                    send_us: 0,
                    size: 0,
                };
                sender.send_video(h, quadro.clone(), prazo).await?;
                seq += 1;
            }
            _ = t_poll.tick() => {
                if medir {
                    medida = prober.last_measure(clock.now_us());
                    if medida.is_some() {
                        break;
                    }
                }
                let banda = *ultima.lock();
                if let Some(b) = prober.poll(clock.now_us(), banda) {
                    let t = inicio.elapsed();
                    if t < Duration::from_secs(3) {
                        rajadas_3s += 1;
                    }
                    let p = prober.clone();
                    rajadas.push((t, tokio::spawn(async move { p.run_burst(b).await })));
                }
            }
        }
    }

    if medir {
        let (m, saturou) = medida.ok_or("timeout: last_measure não virou Some em 8 s")?;
        let total = *prober.bursts().first().ok_or("nenhuma rajada saiu")?;
        let (min, max) = (f64::from(total) * 0.9, f64::from(total) * 1.1);
        println!("medida={m} saturou={saturou} total_kbps={total}");
        assert!(
            (min..=max).contains(&f64::from(m)),
            "medida {m} kbps fora de ±10% de {total} kbps"
        );
        assert!(!saturou, "não devia saturar no loopback");
        conn_cli.close(0u32.into(), b"fim");
        conn_srv.close(0u32.into(), b"fim");
        return Ok(());
    }

    assert_eq!(rajadas_3s, 1, "rajadas nos primeiros 3 s");
    let (t_rajada, handle) = rajadas.remove(0);
    assert!(
        (Duration::from_millis(800)..=Duration::from_secs(3)).contains(&t_rajada),
        "rajada saiu em {t_rajada:?}"
    );
    let enviados = handle.await??;
    println!("probe_size={tamanho_sonda:?} datagramas={enviados} rajada_em={t_rajada:?}");
    assert!(
        (60..=80).contains(&enviados),
        "datagramas da rajada: {enviados} (probe_size {tamanho_sonda:?})"
    );

    // Relatórios entre o fim da rajada e 1,5 s depois dela: algum traz a banda medida.
    let melhor = rels
        .lock()
        .iter()
        .filter(|(t, _)| *t > t_rajada && *t < t_rajada + Duration::from_millis(1500))
        .filter_map(|(_, b)| *b)
        .max();
    println!("melhor banda apos rajada: {melhor:?}");
    assert!(melhor >= Some(3195), "banda após a rajada: {melhor:?}");

    conn_cli.close(0u32.into(), b"fim");
    conn_srv.close(0u32.into(), b"fim");
    Ok(())
}
