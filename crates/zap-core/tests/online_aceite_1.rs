#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 1 Online: dois apps logam no servidor de teste, um cria sala com senha, o outro entra
//! pelo código, e chat e mídia sintética passam pela ponte.

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{core_online, desligar, espera, servidor};
use zap_core::{AudioChoice, CreateRoom, JoinTarget, Preset, ServerState, ShareRequest};
use zap_proto::control::Route;
use zap_proto::{RoomMode, TileSize};

fn online_com_ping(c: &zap_core::Core) -> bool {
    c.snapshot()
        .server
        .as_ref()
        .is_some_and(|s| s.state == ServerState::Online && s.ping_ms.is_some())
}

fn participantes(c: &zap_core::Core) -> Option<usize> {
    c.snapshot().room.as_ref().map(|r| r.participants.len())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aceite_online_sala_com_senha_chat_e_midia() {
    // (1) Servidor e dois apps online, com ping.
    let srv = servidor().await;
    let (p, sink_p) = core_online("p", &srv, true).await;
    let (v, sink_v) = core_online("v", &srv, true).await;
    for (nome, c) in [("P", &p), ("V", &v)] {
        assert!(
            espera(Duration::from_secs(5), || online_com_ping(c)).await,
            "{nome} sem ping: {:?}",
            c.snapshot().server
        );
    }

    // (2) P cria a sala Online com senha.
    p.create_room(CreateRoom {
        name: "Aceite".into(),
        mode: RoomMode::Online,
        password: Some("abc".into()),
    })
    .await
    .expect("criar sala online");
    let snap = p.snapshot();
    let sala = snap.room.as_ref().expect("P deveria ter sala");
    assert_eq!(sala.mode, RoomMode::Online);
    let code = sala.code.clone().expect("sala sem código");
    assert!(code.starts_with("ZAP-") && code.len() == 8, "código: {code}");

    // (3) Um terceiro app enxerga a sala na lista do servidor.
    let (o, _sink_o) = core_online("o", &srv, true).await;
    let ok = espera(Duration::from_secs(5), || {
        o.snapshot()
            .rooms
            .iter()
            .any(|r| r.mode == RoomMode::Online && r.has_password && r.addr.as_deref() == Some(code.as_str()))
    })
    .await;
    assert!(ok, "O não viu a sala: {:?}", o.snapshot().rooms);

    // (4) V tenta entrar com senha errada e depois com a certa (código digitado "sujo").
    let sujo = format!("  {} ", code.to_lowercase().replace('-', " - "));
    let e = v
        .join_room(JoinTarget::Code { code: sujo.clone() }, Some("errada".into()))
        .await
        .expect_err("senha errada deveria falhar");
    assert_eq!(e.code(), "senha_sala", "senha errada: {e:?}");
    v.join_room(JoinTarget::Code { code: sujo }, Some("abc".into()))
        .await
        .expect("senha certa entra");
    for (nome, c) in [("P", &p), ("V", &v)] {
        assert!(
            espera(Duration::from_secs(5), || participantes(c) == Some(2)).await,
            "{nome} deveria ver 2 participantes: {:?}",
            participantes(c)
        );
    }

    // (5) Chat nos dois sentidos.
    v.send_chat("oi do V".into()).await.expect("chat V");
    assert!(
        sink_p
            .wait_for(Duration::from_secs(3), |s| s
                .chats
                .lock()
                .iter()
                .any(|c| c.text == "oi do V"))
            .await,
        "chat de V não chegou em P"
    );
    p.send_chat("oi do P".into()).await.expect("chat P");
    assert!(
        sink_v
            .wait_for(Duration::from_secs(3), |s| s
                .chats
                .lock()
                .iter()
                .any(|c| c.text == "oi do P"))
            .await,
        "chat de P não chegou em V"
    );

    // (6) P publica vídeo sintético; V assiste pela ponte.
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
    let track_id = track_id.unwrap();

    v.watch(&track_id, 1, TileSize::Main, None).expect("watch");
    let ok_v = sink_v
        .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= 30)
        .await;
    assert!(ok_v, "V recebeu poucos pacotes: {}", sink_v.video_count(1));
    {
        let vs = sink_v.videos.lock();
        let pk: Vec<_> = vs.iter().filter(|(_, t, _)| *t == 1).collect();
        assert!(pk.iter().all(|(_, _, p)| !p.data.is_empty()), "pacote vazio");
        assert!(pk[0].2.keyframe, "o primeiro pacote deveria ser keyframe");
        // Annex B: começa com start code 00 00 01 ou 00 00 00 01.
        let d = &pk[0].2.data;
        assert!(
            d.starts_with(&[0, 0, 1]) || d.starts_with(&[0, 0, 0, 1]),
            "não parece H.264 Annex B"
        );
    }
    assert_eq!(v.online_route(&track_id), Some(Route::Relay));

    // (7) V sai; P vê só ele mesmo, para de publicar e sai.
    v.unwatch(1);
    v.leave_room().await;
    assert!(
        espera(Duration::from_secs(5), || participantes(&p) == Some(1)).await,
        "P deveria ver 1 participante: {:?}",
        participantes(&p)
    );
    p.stop_share().await;
    p.leave_room().await;

    // (8) Desliga tudo sem pânico.
    for c in [p, v, o] {
        desligar(c).await;
    }
}
