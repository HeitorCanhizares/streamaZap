#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 1: dono + 2 membros, chat, publicação sintética 1080p60, atraso < 120 ms e miniatura
//! na camada 2.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use zap_core::testutil::TestSink;
use zap_core::{AudioChoice, Core, CoreOptions, CreateRoom, Preset, ShareRequest};
use zap_proto::{RoomMode, TileSize};

async fn novo_core(nome: &str) -> (Core, Arc<TestSink>) {
    let sink = TestSink::new();
    let dir = std::env::temp_dir().join(format!("zapcore-{}-{}", nome, uuid::Uuid::new_v4()));
    let s = sink.clone();
    let core = tokio::task::spawn_blocking(move || {
        Core::start_with(
            dir,
            s,
            CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                ..Default::default()
            },
        )
    })
    .await
    .unwrap()
    .expect("core");
    (core, sink)
}

async fn desligar(mut c: Core) {
    tokio::task::spawn_blocking(move || c.shutdown()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aceite_dono_e_dois_membros() {
    let (d, sink_d) = novo_core("d").await;
    let (a, _) = novo_core("a").await;
    let (b, sink_b) = novo_core("b").await;

    d.create_room(CreateRoom {
        name: "Teste".into(),
        mode: RoomMode::Local,
        password: None,
    })
    .await
    .expect("criar sala");
    let porta = d.control_port().expect("porta");
    let fp = d.control_fingerprint().expect("fingerprint");
    let addr = SocketAddr::from(([127, 0, 0, 1], porta));
    a.join_room_pinned(addr, fp.clone(), None).await.expect("A entra");
    b.join_room_pinned(addr, fp, None).await.expect("B entra");

    // Chat de A chega a D e B (e ao próprio A) em até 2 s.
    a.send_chat("olá, sala".into()).await.expect("chat");
    for (nome, s) in [("D", &sink_d), ("B", &sink_b)] {
        let ok = s
            .wait_for(Duration::from_secs(2), |s| {
                s.chats.lock().iter().any(|c| c.text == "olá, sala")
            })
            .await;
        assert!(ok, "chat não chegou em {nome}");
    }

    // A publica 1080p60 sintético.
    a.start_share(ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P1080p60,
        encoder: "auto".into(),
    })
    .await
    .expect("start_share");

    let mut track_id = None;
    let ok = sink_b
        .wait_for(Duration::from_secs(2), |s| {
            track_id = s
                .last_snapshot()
                .and_then(|sn| sn.room.as_ref().and_then(|r| r.tracks.first().map(|t| t.id.clone())));
            track_id.is_some()
        })
        .await;
    assert!(ok, "B não viu a trilha de A");
    let track_id = track_id.unwrap();

    b.watch(&track_id, 1, TileSize::Main, None).expect("watch B");
    d.watch(&track_id, 7, TileSize::Thumbnail, None).expect("watch D");

    assert!(
        sink_b
            .wait_for(Duration::from_secs(5), |s| s.video_count(1) >= 60)
            .await,
        "B recebeu poucos pacotes: {}",
        sink_b.video_count(1)
    );
    assert!(
        sink_d
            .wait_for(Duration::from_secs(5), |s| s.video_count(7) >= 30)
            .await,
        "D recebeu poucos pacotes: {}",
        sink_d.video_count(7)
    );

    // `seq` estritamente crescente por camada.
    {
        let v = sink_b.videos.lock();
        let mut ultimo: std::collections::HashMap<u8, u32> = Default::default();
        for (_, t, p) in v.iter().filter(|(_, t, _)| *t == 1) {
            let _ = t;
            if let Some(&u) = ultimo.get(&p.layer) {
                assert!(
                    p.seq > u,
                    "seq não crescente na camada {}: {} após {}",
                    p.layer,
                    p.seq,
                    u
                );
            }
            ultimo.insert(p.layer, p.seq);
        }
    }

    // Atraso de reprodução: o relógio de captura é o do processo, então
    // atraso = (entrega - entrega do 1º pacote) - (capture_us - capture_us do 1º).
    // O 1º pacote fica como referência (atraso 0); a mediana mede a deriva relativa.
    {
        let v = sink_b.videos.lock();
        let pk: Vec<_> = v.iter().filter(|(_, t, _)| *t == 1).collect();
        let (i0, _, p0) = pk[0];
        let mut atrasos: Vec<i64> = pk
            .iter()
            .map(|(i, _, p)| {
                let ent = i.duration_since(*i0).as_micros() as i64;
                let cap = p.capture_us as i64 - p0.capture_us as i64;
                ent - cap
            })
            .collect();
        atrasos.sort();
        let mediana = atrasos[atrasos.len() / 2];
        println!("atraso mediano: {} us", mediana);
        assert!(mediana < 120_000, "atraso mediano {} us", mediana);
    }

    // Miniatura de D vem da camada 2.
    {
        let v = sink_d.videos.lock();
        let n = v.iter().filter(|(_, t, p)| *t == 7 && p.layer == 2).count();
        let total = v.iter().filter(|(_, t, _)| *t == 7).count();
        assert!(n >= 30 && n == total, "camada 2: {n} de {total}");
    }

    b.unwatch(1);
    d.unwatch(7);
    a.leave_room().await;
    b.leave_room().await;
    let ok = sink_d
        .wait_for(Duration::from_secs(3), |_| {
            d.snapshot().room.as_ref().map(|r| r.participants.len()) == Some(1)
        })
        .await;
    assert!(ok, "dono deveria ter 1 participante");
    d.leave_room().await;
    for c in [a, b, d] {
        desligar(c).await;
    }
}
