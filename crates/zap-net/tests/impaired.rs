//! Teste de aceite em rede ruim: proxy limitado a 3 Mbps, fonte de 6 Mbps.
//!
//! Margens largas de propósito; o objetivo é pegar regressões grosseiras do controle de
//! congestionamento e do descarte por prazo, não medir precisão fina.

mod support;

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use parking_lot::Mutex;
use support::proxy::{Proxy, ProxyConfig};
use tokio::{sync::mpsc, time::interval};
use zap_net::{
    endpoint,
    handshake::{accept_handshake, connect_handshake, Hello, ServerAuth},
    media_rx::{spawn_receiver, MediaEvent},
    media_tx::MediaSender,
    report::{spawn_reporter, ReportBuilder},
    tls::{self, Identity},
    Clock,
};
use zap_proto::{
    media::{VideoFlags, VideoHeader},
    types::Access,
    ControlMsg, Report, Room, RoomCode, RoomMode,
};

type Res = Result<(), Box<dyn std::error::Error>>;

const FPS_INTERVALO: Duration = Duration::from_micros(16_667);
/// 12,5 KB por quadro a 60 fps = 6 Mbps.
const TAM_VIDEO: usize = 12_500;
const DURACAO: Duration = Duration::from_secs(20);

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
async fn rede_ruim_3mbps_fonte_6mbps() -> Res {
    let _timer = support::timer();
    let id = Identity::generate("zap")?;
    let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
    let addr_srv: SocketAddr = format!("127.0.0.1:{porta}").parse()?;

    let proxy = Proxy::start(
        addr_srv,
        ProxyConfig {
            delay: Duration::from_millis(15),
            loss: 0.0,
            rate_kbps: Some(3000),
            queue_ms: 50,
            seed: 1,
        },
    )
    .await?;

    let cliente = endpoint::client_endpoint()?;
    let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, proxy.listen_addr, "zap")?;
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

    // Relógio único compartilhado entre as duas pontas (mesmo processo).
    let clock = Clock::new();
    let rb = Arc::new(Mutex::new(ReportBuilder::new(true)));
    let (ev_tx, mut ev_rx) = mpsc::channel::<MediaEvent>(4096);
    let _rx = spawn_receiver(conn_srv.clone(), clock.clone(), ev_tx, rb.clone());
    let _rep = spawn_reporter(srv_tx, rb, clock.clone());

    let coletor = tokio::spawn(async move {
        let mut eventos = Vec::new();
        while let Some(e) = ev_rx.recv().await {
            eventos.push(e);
        }
        eventos
    });

    let inicio = Instant::now();
    let leitor = tokio::spawn(async move {
        let mut rels: Vec<(Duration, Report)> = Vec::new();
        while let Ok(Some(msg)) = cli_rx.recv().await {
            if let ControlMsg::Report(r) = msg {
                rels.push((inicio.elapsed(), r));
            }
        }
        rels
    });

    let sender = Arc::new(MediaSender::new(conn_cli.clone(), clock.clone()));
    let prazo = sender.deadline();
    let mut t_video = interval(FPS_INTERVALO);
    t_video.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut seq = 0u32;
    // Uma tarefa curta por quadro (como em publish.rs): a fonte mantém 60 fps mesmo com a rede lenta.
    let mut envios = tokio::task::JoinSet::new();
    while inicio.elapsed() < DURACAO {
        t_video.tick().await;
        let h = VideoHeader {
            flags: VideoFlags(if seq % 60 == 0 { VideoFlags::KEYFRAME } else { 0 }),
            track: 1,
            layer: 0,
            width: 1280,
            height: 720,
            seq,
            capture_us: clock.now_us(),
            send_us: 0,
            size: 0,
        };
        let dados = Bytes::from(vec![seq as u8; TAM_VIDEO]);
        let s = sender.clone();
        envios.spawn(async move { s.send_video(h, dados, prazo).await });
        seq += 1;
    }
    let mut descartados = 0u32;
    while let Some(r) = envios.join_next().await {
        if !r?? {
            descartados += 1;
        }
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    conn_cli.close(0u32.into(), b"fim");
    conn_srv.close(0u32.into(), b"fim");
    let rels = tokio::time::timeout(Duration::from_secs(3), leitor).await??;
    let eventos = tokio::time::timeout(Duration::from_secs(3), coletor).await??;

    // Atraso de cada quadro entregue e contagem de perdas vistas pelo receptor.
    let mut max_atraso_ms = 0u64;
    let mut recebidos = 0u32;
    let mut perdidos = 0u32;
    for e in &eventos {
        match e {
            MediaEvent::Video { header, arrival_us, .. } => {
                recebidos += 1;
                let atraso = arrival_us.saturating_sub(header.capture_us) / 1000;
                max_atraso_ms = max_atraso_ms.max(atraso);
            }
            MediaEvent::Lost { .. } => perdidos += 1,
            MediaEvent::Audio { .. } => {}
        }
    }
    let bandas: Vec<u32> = rels
        .iter()
        .filter(|(t, _)| *t > Duration::from_secs(10))
        .filter_map(|(_, r)| r.bandwidth_kbps)
        .collect();
    let media_banda = if bandas.is_empty() {
        0
    } else {
        bandas.iter().map(|&b| b as u64).sum::<u64>() / bandas.len() as u64
    };
    let st = proxy.stats();
    eprintln!(
        "enviados={seq} descartados_pub={descartados} recebidos={recebidos} perdidos_rx={perdidos} \
         atraso_max_ms={max_atraso_ms} banda_media_kbps={media_banda} (n={}) proxy={st:?}",
        bandas.len()
    );

    assert!(max_atraso_ms <= 400, "atraso máximo {max_atraso_ms} ms > 400 ms");
    assert!(!bandas.is_empty(), "sem relatórios com banda após 10 s");
    assert!(
        (2200..=3600).contains(&media_banda),
        "banda média {media_banda} kbps fora de 2200..3600"
    );
    assert!(descartados > 0, "o publicador não descartou nenhum quadro");
    assert!(perdidos > 0, "o receptor não viu eventos Lost");
    Ok(())
}
