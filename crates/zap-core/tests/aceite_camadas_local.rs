#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite: camadas numa sala Local no loopback. P publica 1080p60 sintético; V1 assiste em
//! `Main` e deve ficar na camada 0, V2 assiste em `Thumbnail` e deve ficar na camada 2. As
//! camadas não podem oscilar por 3 s. Prova a sondagem do publicador.

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
                default_server: None,
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

fn camada(c: &Core) -> Option<u8> {
    c.snapshot().tiles.iter().find(|t| t.tile == 1).map(|t| t.layer)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn camadas_local_v1_na_0_v2_na_2() {
    let (p, _sp) = novo_core("p").await;
    let (v1, s1) = novo_core("v1").await;
    let (v2, s2) = novo_core("v2").await;

    p.create_room(CreateRoom {
        name: "CamadasLocal".into(),
        mode: RoomMode::Local,
        password: None,
    })
    .await
    .expect("criar sala");
    let porta = p.control_port().expect("porta");
    let fp = p.control_fingerprint().expect("fingerprint");
    let addr = SocketAddr::from(([127, 0, 0, 1], porta));
    v1.join_room_pinned(addr, fp.clone(), None).await.expect("V1 entra");
    v2.join_room_pinned(addr, fp, None).await.expect("V2 entra");

    p.start_share(ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P1080p60,
        encoder: "auto".into(),
    })
    .await
    .expect("start_share");

    let mut track_id = None;
    let ok = s1
        .wait_for(Duration::from_secs(5), |s| {
            track_id = s
                .last_snapshot()
                .and_then(|sn| sn.room.as_ref().and_then(|r| r.tracks.first().map(|t| t.id.clone())));
            track_id.is_some()
        })
        .await;
    assert!(ok, "V1 não viu a trilha de P");
    let track_id = track_id.unwrap();

    // V2 também precisa ver a trilha antes do watch.
    let ok = s2
        .wait_for(Duration::from_secs(5), |s| {
            s.last_snapshot()
                .is_some_and(|sn| sn.room.as_ref().is_some_and(|r| !r.tracks.is_empty()))
        })
        .await;
    assert!(ok, "V2 não viu a trilha de P");

    v1.watch(&track_id, 1, TileSize::Main, None).expect("watch V1");
    v2.watch(&track_id, 1, TileSize::Thumbnail, None).expect("watch V2");

    let alvo = |a: &Core, b: &Core| camada(a) == Some(0) && camada(b) == Some(2);
    let fim = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut ok = false;
    while tokio::time::Instant::now() < fim {
        if alvo(&v1, &v2) && s1.video_count(1) >= 10 && s2.video_count(1) >= 10 {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Falhas viram mensagem e o pânico só vem depois de desligar os Cores (soltar um `Core`
    // dentro do runtime durante o pânico aborta o processo e esconde a mensagem).
    let falha = if !ok {
        Some(format!(
            "camadas V1/V2 = {:?}/{:?}, pacotes {}/{}",
            camada(&v1),
            camada(&v2),
            s1.video_count(1),
            s2.video_count(1)
        ))
    } else {
        // Estabilidade: as camadas não podem mudar por 3 s.
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut oscilou = false;
        while tokio::time::Instant::now() < fim {
            if !alvo(&v1, &v2) {
                oscilou = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        oscilou.then(|| format!("camadas oscilaram: V1={:?} V2={:?}", camada(&v1), camada(&v2)))
    };

    v1.unwatch(1);
    v2.unwatch(1);
    p.stop_share().await;
    v1.leave_room().await;
    v2.leave_room().await;
    p.leave_room().await;
    for c in [v1, v2, p] {
        desligar(c).await;
    }
    if let Some(m) = falha {
        panic!("{m}");
    }
}
