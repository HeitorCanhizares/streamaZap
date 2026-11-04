#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Reconexão Online: a ponte QUIC cai e volta (nos dois lados) e a sessão reinicia (WebSocket).

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{core_online, desligar, espera, servidor};
use zap_core::testutil::TestSink;
use zap_core::{AudioChoice, Core, CreateRoom, JoinTarget, Preset, ServerState, ShareRequest};
use zap_proto::{RoomMode, TileSize};

fn participantes(c: &Core) -> Option<usize> {
    c.snapshot().room.as_ref().map(|r| r.participants.len())
}

/// Sala com P publicando e V assistindo no tile 1, vídeo já fluindo.
async fn cenario() -> (zap_test_server::Handle, (Core, Arc<TestSink>), (Core, Arc<TestSink>)) {
    let srv = servidor().await;
    let (p, sink_p) = core_online("p", &srv, true).await;
    let (v, sink_v) = core_online("v", &srv, true).await;
    p.create_room(CreateRoom {
        name: "Reconexao".into(),
        mode: RoomMode::Online,
        password: None,
    })
    .await
    .expect("criar sala");
    let code = p.snapshot().room.as_ref().and_then(|r| r.code.clone()).expect("código");
    v.join_room(JoinTarget::Code { code }, None).await.expect("entrar");
    for c in [&p, &v] {
        assert!(
            espera(Duration::from_secs(5), || participantes(c) == Some(2)).await,
            "esperava 2 participantes"
        );
    }
    p.start_share(ShareRequest {
        source: "synthetic:1280x720".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P720p30,
        encoder: "software".into(),
    })
    .await
    .expect("start_share");
    let mut track_id = None;
    let ok = sink_v
        .wait_for(Duration::from_secs(5), |_| {
            track_id = v
                .snapshot()
                .room
                .as_ref()
                .and_then(|r| r.tracks.first().map(|t| t.id.clone()));
            track_id.is_some()
        })
        .await;
    assert!(ok, "V não viu a trilha de P");
    v.watch(&track_id.unwrap(), 1, TileSize::Main, None).expect("watch");
    assert!(
        sink_v
            .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= 30)
            .await,
        "vídeo inicial não fluiu"
    );
    (srv, (p, sink_p), (v, sink_v))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ponte_cai_e_volta() {
    let (_srv, (p, _sp), (v, sink_v)) = cenario().await;

    // Cai a ponte de V: o vídeo volta a crescer e a sala segue com 2.
    let antes = sink_v.video_count(1);
    v.online_drop_bridge();
    let ok = sink_v
        .wait_for(Duration::from_secs(15), |s| s.video_count(1) >= antes + 20)
        .await;
    assert!(
        ok,
        "vídeo não voltou após queda da ponte de V: {} (antes {antes})",
        sink_v.video_count(1)
    );
    assert_eq!(participantes(&v), Some(2));
    assert_eq!(participantes(&p), Some(2));

    // Cai a ponte de P: a trilha fica na sala (tolerância do servidor), o servidor repete
    // `camadas.pedidas` na conexão nova e V volta a receber.
    let antes = sink_v.video_count(1);
    p.online_drop_bridge();
    let ok = sink_v
        .wait_for(Duration::from_secs(15), |s| s.video_count(1) >= antes + 20)
        .await;
    assert!(
        ok,
        "vídeo não voltou após queda da ponte de P: {} (antes {antes})",
        sink_v.video_count(1)
    );
    assert_eq!(participantes(&v), Some(2));
    assert_eq!(participantes(&p), Some(2));

    v.unwatch(1);
    p.stop_share().await;
    for c in [p, v] {
        desligar(c).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sessao_reinicia_e_sala_volta() {
    let (_srv, (p, _sp), (v, sink_v)) = cenario().await;

    v.online_restart_session();
    // Aceita só dois resultados: (a) servidor Online e V ainda em sala; (b) sala encerrada limpa.
    let ok = espera(Duration::from_secs(20), || {
        let s = v.snapshot();
        s.server.as_ref().is_some_and(|x| x.state == ServerState::Online)
    })
    .await;
    assert!(ok, "servidor não voltou a Online: {:?}", v.snapshot().server);
    // Dá um tempo para a sala assentar (ficar ou fechar).
    tokio::time::sleep(Duration::from_secs(2)).await;
    let em_sala = v.snapshot().room.is_some();
    if em_sala {
        println!("RESULTADO: V continuou em sala após reiniciar a sessão");
        p.send_chat("depois da reconexao".into()).await.expect("chat P");
        assert!(
            sink_v
                .wait_for(Duration::from_secs(5), |s| s
                    .chats
                    .lock()
                    .iter()
                    .any(|c| c.text == "depois da reconexao"))
                .await,
            "chat de P não chegou a V"
        );
    } else {
        println!("RESULTADO: V recebeu Closed limpo (room = None) após reiniciar a sessão");
    }

    v.unwatch(1);
    p.stop_share().await;
    for c in [p, v] {
        desligar(c).await;
    }
}
