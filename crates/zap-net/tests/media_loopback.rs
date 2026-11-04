//! Teste de aceite da mídia em loopback, pela API pública de `zap_net`.
//!
//! Um publicador envia 300 quadros de vídeo a 60 fps e áudio a 50/s por ~5 s; o receptor
//! devolve relatórios a cada ~100 ms. Margens largas: sem asserts de latência fina.

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use parking_lot::Mutex;
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
    media::{AudioHeader, VideoFlags, VideoHeader},
    types::Access,
    ControlMsg, Report, Room, RoomCode, RoomMode,
};

type Res = Result<(), Box<dyn std::error::Error>>;

const QUADROS: u32 = 300;
const FPS_INTERVALO: Duration = Duration::from_micros(16_667);
const TAM_VIDEO: usize = 8 * 1024;
const TAM_AUDIO: usize = 80;

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

/// Conteúdo de um quadro de vídeo, determinístico por `seq`.
fn payload_video(seq: u32) -> Vec<u8> {
    (0..TAM_VIDEO).map(|i| (i as u32).wrapping_add(seq) as u8).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loopback_300_quadros_e_audio() -> Res {
    // Par QUIC: o cliente publica, o servidor recebe.
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

    // Receptor.
    let clock_rx = Clock::new();
    let rb = Arc::new(Mutex::new(ReportBuilder::new(true)));
    let (ev_tx, mut ev_rx) = mpsc::channel::<MediaEvent>(1024);
    let _rx = spawn_receiver(conn_srv.clone(), clock_rx.clone(), ev_tx, rb.clone());
    let _rep = spawn_reporter(srv_tx, rb, clock_rx);

    let coletor = tokio::spawn(async move {
        let mut eventos = Vec::new();
        while let Some(e) = ev_rx.recv().await {
            eventos.push(e);
        }
        eventos
    });

    // Publicador lê os relatórios, anotando quando chegaram.
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

    // Envio.
    let clock = Clock::new();
    let sender = MediaSender::new(conn_cli.clone(), clock.clone());
    // Este teste confere ordem e integridade, não prazo (isso é do `impaired.rs`). Como o quadro
    // só termina com a confirmação do par, um soluço de CPU no CI estouraria os 150 ms padrão.
    let prazo = Duration::from_secs(2);
    let mut t_video = interval(FPS_INTERVALO);
    let mut t_audio = interval(Duration::from_millis(20));
    let mut seq_v = 0u32;
    let mut seq_a = 0u32;
    let mut capturas = Vec::new();
    while seq_v < QUADROS {
        tokio::select! {
            _ = t_video.tick() => {
                let capture_us = clock.now_us();
                capturas.push(capture_us);
                let h = VideoHeader {
                    flags: VideoFlags(if seq_v % 60 == 0 { VideoFlags::KEYFRAME } else { 0 }),
                    track: 1,
                    layer: 0,
                    width: 1280,
                    height: 720,
                    seq: seq_v,
                    capture_us,
                    send_us: 0,
                    size: 0,
                };
                let ok = sender.send_video(h, Bytes::from(payload_video(seq_v)), prazo).await?;
                assert!(ok, "quadro {seq_v} atrasado em loopback");
                seq_v += 1;
            }
            _ = t_audio.tick() => {
                let h = AudioHeader { track: 2, seq: seq_a, capture_us: clock.now_us(), send_us: 0 };
                sender.send_audio(h, &[seq_a as u8; TAM_AUDIO])?;
                seq_a += 1;
            }
        }
    }

    // Folga para as últimas chegadas e o último relatório.
    tokio::time::sleep(Duration::from_millis(250)).await;
    conn_cli.close(0u32.into(), b"fim");
    conn_srv.close(0u32.into(), b"fim");
    let rels = tokio::time::timeout(Duration::from_secs(3), leitor).await??;
    let eventos = tokio::time::timeout(Duration::from_secs(3), coletor).await??;

    // Vídeo: todos chegam, em ordem, idênticos.
    let mut videos = Vec::new();
    let mut audios = 0u32;
    for e in &eventos {
        match e {
            MediaEvent::Video { header, data, .. } => videos.push((header, data)),
            MediaEvent::Audio { header, opus, .. } => {
                if header.track == 2 && opus.len() == TAM_AUDIO {
                    audios += 1;
                }
            }
            MediaEvent::Lost { track, layer, seq } => panic!("perdido: {track}/{layer}/{seq}"),
        }
    }
    assert_eq!(videos.len(), QUADROS as usize, "quadros recebidos");
    for (i, (h, data)) in videos.iter().enumerate() {
        let i = i as u32;
        assert_eq!(h.seq, i, "ordem de seq");
        assert_eq!(h.capture_us, capturas[i as usize], "capture_us do quadro {i}");
        assert_eq!(h.flags.is_keyframe(), i % 60 == 0, "keyframe do quadro {i}");
        assert_eq!(data.as_ref(), payload_video(i).as_slice(), "payload do quadro {i}");
    }

    // Áudio: pelo menos 95% dos pacotes enviados.
    assert!(audios * 100 >= seq_a * 95, "áudio chegou {audios} de {seq_a}");

    // Relatórios: um a cada ~100 ms e cobrindo todos os quadros de vídeo.
    assert!((35..=60).contains(&rels.len()), "relatórios: {}", rels.len());
    let chegadas: usize = rels
        .iter()
        .map(|(_, r)| r.arrivals.iter().filter(|a| a.0 == 1).count())
        .sum();
    assert_eq!(chegadas, QUADROS as usize, "chegadas de vídeo nos relatórios");
    assert!(
        rels.iter()
            .any(|(t, r)| *t > Duration::from_secs(2) && r.bandwidth_kbps.is_some_and(|b| b > 0)),
        "nenhum relatório com banda_kbps > 0 após 2 s"
    );
    Ok(())
}
