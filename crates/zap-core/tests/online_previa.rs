#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Prévia na lista de salas: um observador que NÃO entrou na sala recebe a camada 2, sem áudio.

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{core_online, desligar, espera, servidor};
use zap_core::{AudioChoice, Core, CreateRoom, Preset, ShareRequest};
use zap_proto::RoomMode;

const TILE: u32 = 1_000_001;

/// P cria a sala Online (com ou sem senha) e devolve o código.
async fn criar_sala(p: &Core, senha: Option<&str>) -> String {
    p.create_room(CreateRoom {
        name: "Previa".into(),
        mode: RoomMode::Online,
        password: senha.map(|s| s.to_string()),
    })
    .await
    .expect("criar sala");
    p.snapshot()
        .room
        .as_ref()
        .and_then(|r| r.code.clone())
        .expect("código da sala")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn previa_entrega_camada_2() {
    let srv = servidor().await;
    let (p, _sp) = core_online("p", &srv, true).await;
    let (o, sink_o) = core_online("o", &srv, true).await;
    let code = criar_sala(&p, None).await;
    p.start_share(ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P1080p60,
        encoder: "software".into(),
    })
    .await
    .expect("start_share");

    // A trilha precisa estar publicada na sala antes da prévia.
    let ok = espera(Duration::from_secs(10), || {
        p.snapshot().room.as_ref().is_some_and(|r| !r.tracks.is_empty())
    })
    .await;
    assert!(ok, "P não publicou a trilha");

    o.room_preview(code, TILE).await.expect("room_preview");
    let ok = sink_o
        .wait_for(Duration::from_secs(15), |s| s.video_count(TILE) >= 5)
        .await;
    assert!(ok, "prévia sem quadros: {}", sink_o.video_count(TILE));
    assert!(o.snapshot().room.is_none(), "O não deveria estar em sala");
    {
        let v = sink_o.videos.lock();
        assert!(
            v.iter().filter(|(_, t, _)| *t == TILE).all(|(_, _, p)| p.layer == 2),
            "todos os quadros deveriam ser da camada 2"
        );
    }

    // Parar a prévia: a contagem estabiliza (tolera quadros já em voo no instante do stop).
    o.room_preview_stop(TILE);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let n = sink_o.video_count(TILE);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(sink_o.video_count(TILE), n, "quadros continuam chegando após o stop");

    p.stop_share().await;
    p.leave_room().await;
    desligar(p).await;
    desligar(o).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn previa_sala_com_senha_proibida() {
    let srv = servidor().await;
    let (p, _sp) = core_online("p", &srv, true).await;
    let (o, _so) = core_online("o", &srv, true).await;
    let code = criar_sala(&p, Some("segredo")).await;
    let r = o.room_preview(code, TILE).await;
    match r {
        Err(e) => assert_eq!(e.code(), "proibido", "{e:?}"),
        Ok(()) => panic!("prévia de sala com senha deveria falhar"),
    }
    p.leave_room().await;
    desligar(p).await;
    desligar(o).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn previa_sem_transmissao() {
    let srv = servidor().await;
    let (p, _sp) = core_online("p", &srv, true).await;
    let (o, _so) = core_online("o", &srv, true).await;
    let code = criar_sala(&p, None).await;
    let r = o.room_preview(code, TILE).await;
    assert!(
        matches!(r, Err(zap_core::CoreError::Invalid(ref m)) if m == "Ninguém está transmitindo"),
        "{r:?}"
    );
    p.leave_room().await;
    desligar(p).await;
    desligar(o).await;
}
