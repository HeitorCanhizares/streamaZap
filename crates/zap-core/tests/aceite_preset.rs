#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 4: troca de preset ao vivo. O espectador continua recebendo pacotes depois da troca,
//! vê um keyframe e a altura da camada 0 cai de 1080 para 720, sem chamar `watch` de novo.

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

fn pedido(preset: Preset) -> ShareRequest {
    ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset,
        encoder: "auto".into(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn troca_de_preset_ao_vivo_nao_derruba_espectador() {
    let (d, _sink_d) = novo_core("d").await;
    let (a, _) = novo_core("a").await;
    let (b, sink_b) = novo_core("b").await;

    d.create_room(CreateRoom {
        name: "Preset".into(),
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

    // A publica 1080p60 sintético.
    a.start_share(pedido(Preset::P1080p60)).await.expect("start_share");

    let mut track_id = None;
    let ok = sink_b
        .wait_for(Duration::from_secs(5), |s| {
            track_id = s
                .last_snapshot()
                .and_then(|sn| sn.room.as_ref().and_then(|r| r.tracks.first().map(|t| t.id.clone())));
            track_id.is_some()
        })
        .await;
    assert!(ok, "B não viu a trilha de A");
    let track_id = track_id.unwrap();

    b.watch(&track_id, 1, TileSize::Main, None).expect("watch B");

    assert!(
        sink_b
            .wait_for(Duration::from_secs(5), |s| s.video_count(1) >= 30)
            .await,
        "B recebeu poucos pacotes antes da troca: {}",
        sink_b.video_count(1)
    );

    // Troca de preset ao vivo, sem novo `watch`.
    let n0 = sink_b.video_count(1);
    a.update_share(pedido(Preset::P720p30)).await.expect("update_share");

    let ok = sink_b
        .wait_for(Duration::from_secs(5), |s| s.video_count(1) >= n0 + 30)
        .await;
    assert!(
        ok,
        "B parou de receber após a troca: {} pacotes (antes {n0})",
        sink_b.video_count(1)
    );

    // Snapshot do espectador sem erro.
    let erros = sink_b.last_snapshot().map(|s| s.errors.clone()).unwrap_or_default();
    assert!(erros.is_empty(), "erros no snapshot de B: {erros:?}");

    // Snapshot do publicador reflete o novo preset.
    let preset_a = a.snapshot().share.as_ref().map(|s| s.preset);
    assert_eq!(preset_a, Some(Preset::P720p30), "preset em A");

    // Pacotes antes e depois da troca (separados pelo índice em `n0`).
    {
        let v = sink_b.videos.lock();
        let tile1: Vec<_> = v.iter().filter(|(_, t, _)| *t == 1).map(|(_, _, p)| p).collect();
        let (_antes, depois) = tile1.split_at(n0.min(tile1.len()));
        assert!(!depois.is_empty(), "sem pacotes pós-troca");

        // Sem `watch` novo, a entrega continua e traz um keyframe pós-troca. B pode ficar em
        // outra camada (o alocador decide), então a altura é conferida por teto.
        assert!(
            depois.iter().any(|p| p.keyframe),
            "nenhum keyframe depois da troca de preset"
        );

        // A altura máxima pós-troca não pode passar de 720.
        let altura_depois = depois.iter().map(|p| p.height).max().unwrap_or(0);
        assert!(altura_depois <= 720, "altura pós-troca {altura_depois} > 720");

        // Se o espectador está na camada 0, ela tem de vir em 720.
        if depois.iter().any(|p| p.layer == 0) {
            assert!(
                depois.iter().any(|p| p.layer == 0 && p.height == 720),
                "camada 0 pós-troca sem altura 720"
            );
        }
    }

    b.unwatch(1);
    a.stop_share().await;
    a.leave_room().await;
    b.leave_room().await;
    d.leave_room().await;
    for c in [a, b, d] {
        desligar(c).await;
    }
}
