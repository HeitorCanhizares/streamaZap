#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 4 Online: a rota direta funciona no loopback (e `midia.direta_ok` é enviado) e, se ela
//! falha, o vídeo segue pela ponte.
//!
//! O `zap-test-server` ignora `p2p.*`; por isso o teste injeta `p2p.candidatos` com
//! `Core::online_inject`.

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{core_online, desligar, espera, servidor};
use zap_core::testutil::TestSink;
use zap_core::{AudioChoice, Core, CreateRoom, JoinTarget, Preset, ShareRequest};
use zap_proto::control::{P2pRole, Route};
use zap_proto::{ControlMsg, RoomMode, TileSize};

struct Cena {
    p: Core,
    v: Core,
    sink_v: Arc<TestSink>,
    track_id: String,
    /// Participante dono da trilha (P) e o espectador (V).
    id_p: String,
    id_v: String,
}

/// Sala Online aberta criada por P, V entra, P publica vídeo sintético e V assiste (tile 1).
async fn montar(srv: &zap_test_server::Handle, use_relay: bool) -> Cena {
    let (p, _sink_p) = core_online("p", srv, use_relay).await;
    let (v, sink_v) = core_online("v", srv, use_relay).await;
    p.create_room(CreateRoom {
        name: "Aceite4".into(),
        mode: RoomMode::Online,
        password: None,
    })
    .await
    .expect("criar sala");
    let code = p.snapshot().room.as_ref().and_then(|r| r.code.clone()).expect("código");
    v.join_room(JoinTarget::Code { code }, None).await.expect("entrar");
    assert!(
        espera(Duration::from_secs(5), || {
            [&p, &v]
                .iter()
                .all(|c| c.snapshot().room.as_ref().map(|r| r.participants.len()) == Some(2))
        })
        .await,
        "os dois deveriam ver 2 participantes"
    );

    p.start_share(ShareRequest {
        source: "synthetic:1280x720".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P720p30,
        encoder: "software".into(),
    })
    .await
    .expect("start_share");

    let mut achado = None;
    let ok = espera(Duration::from_secs(5), || {
        achado = v
            .snapshot()
            .room
            .as_ref()
            .and_then(|r| r.tracks.first().map(|t| (t.id.clone(), t.owner_id.clone())));
        achado.is_some()
    })
    .await;
    assert!(ok, "V não viu a trilha de P");
    let (track_id, id_p) = achado.unwrap();
    let id_v = v
        .snapshot()
        .room
        .as_ref()
        .and_then(|r| r.participants.iter().find(|x| x.id != id_p).map(|x| x.id.clone()))
        .expect("id de V");

    v.watch(&track_id, 1, TileSize::Main, None).expect("watch");
    Cena {
        p,
        v,
        sink_v,
        track_id,
        id_p,
        id_v,
    }
}

async fn encerrar(c: Cena) {
    c.v.unwatch(1);
    c.v.leave_room().await;
    c.p.stop_share().await;
    c.p.leave_room().await;
    desligar(c.p).await;
    desligar(c.v).await;
}

/// Injeta em V os candidatos do `MediaServer` de P e em P os de V (papel servidor).
async fn injetar_candidatos_reais(c: &Cena) {
    let mut addr = None;
    assert!(
        espera(Duration::from_secs(3), || {
            addr = c.p.online_media_address();
            addr.is_some()
        })
        .await,
        "P sem endereço de mídia"
    );
    let addr = addr.unwrap();
    c.v.online_inject(ControlMsg::P2pCandidates {
        track_id: c.track_id.clone(),
        with: c.id_p.clone(),
        role: P2pRole::Client,
        addresses: addr.addresses.iter().map(|ip| format!("{ip}:{}", addr.port)).collect(),
        fingerprint: addr.fingerprint.clone(),
    });
    c.p.online_inject(ControlMsg::P2pCandidates {
        track_id: c.track_id.clone(),
        with: c.id_v.clone(),
        role: P2pRole::Server,
        addresses: vec!["127.0.0.1:9".into()],
        fingerprint: addr.fingerprint,
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direta_funciona() {
    let srv = servidor().await;
    let c = montar(&srv, true).await;

    assert!(
        c.sink_v
            .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= 10)
            .await,
        "sem vídeo pela ponte: {}",
        c.sink_v.video_count(1)
    );
    assert_eq!(c.v.online_route(&c.track_id), Some(Route::Relay));

    injetar_candidatos_reais(&c).await;
    let ok = espera(Duration::from_secs(8), || {
        c.v.online_route(&c.track_id) == Some(Route::Direct) && c.v.online_direct_ok_sent(&c.track_id)
    })
    .await;
    assert!(
        ok,
        "rota = {:?}, direta_ok = {}",
        c.v.online_route(&c.track_id),
        c.v.online_direct_ok_sent(&c.track_id)
    );

    // O vídeo continua depois da troca.
    let antes = c.sink_v.video_count(1);
    let ok = c
        .sink_v
        .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= antes + 30)
        .await;
    assert!(
        ok,
        "vídeo parou após a troca: antes {antes}, depois {}",
        c.sink_v.video_count(1)
    );
    encerrar(c).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direta_falha_e_fica_na_ponte() {
    let srv = servidor().await;
    let c = montar(&srv, true).await;
    assert!(
        c.sink_v
            .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= 10)
            .await,
        "sem vídeo pela ponte"
    );

    c.v.online_inject(ControlMsg::P2pCandidates {
        track_id: c.track_id.clone(),
        with: c.id_p.clone(),
        role: P2pRole::Client,
        addresses: vec!["127.0.0.1:1".into()],
        fingerprint: "00".repeat(32),
    });
    // Dá tempo para a tentativa terminar (timeout da direta) sem trocar de rota.
    tokio::time::sleep(Duration::from_secs(8)).await;
    assert_eq!(c.v.online_route(&c.track_id), Some(Route::Relay));
    assert!(!c.v.online_direct_ok_sent(&c.track_id));

    let antes = c.sink_v.video_count(1);
    let ok = c
        .sink_v
        .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= antes + 30)
        .await;
    assert!(
        ok,
        "vídeo parou na ponte: antes {antes}, depois {}",
        c.sink_v.video_count(1)
    );
    encerrar(c).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sem_use_relay_so_direta() {
    let srv = servidor().await;
    let c = montar(&srv, false).await;

    // Sem candidatos e sem ponte: nada de vídeo. (`online_route` devolve `Relay` como padrão da
    // trilha, então a prova de que não há ponte é não chegar vídeo algum.)
    let chegou = c
        .sink_v
        .wait_for(Duration::from_secs(3), |s| s.video_count(1) > 0)
        .await;
    assert!(!chegou, "chegou vídeo sem ponte e sem candidatos");

    // Com candidatos injetados, o vídeo chega pela direta (sem ponte).
    injetar_candidatos_reais(&c).await;
    let ok = c
        .sink_v
        .wait_for(Duration::from_secs(10), |s| s.video_count(1) >= 10)
        .await;
    assert!(
        ok,
        "sem vídeo pela direta: {}, rota = {:?}",
        c.sink_v.video_count(1),
        c.v.online_route(&c.track_id)
    );
    assert_eq!(c.v.online_route(&c.track_id), Some(Route::Direct));
    encerrar(c).await;
}
