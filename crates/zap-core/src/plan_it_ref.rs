//! Testes de integracao do plano (fase 3) com o zap-test-server.

use std::sync::Arc;
use std::time::{Duration, Instant};

use zap_proto::{RoomMode, TrackKind};
use zap_rate::ladder::{screen_ladder, Preset as RatePreset};
use zap_test_server::{Config, Handle};

use crate::engine::{Core, CoreOptions};
use crate::online::OnlineRoom;
use crate::room::{ActiveRoom, TrackSpec};
use crate::testutil::TestSink;
use crate::types::{AudioChoice, CreateRoom, Preset, ShareRequest};
use crate::CoreError;

fn conta_cfg() -> Config {
    Config {
        requires_account: true,
        account: Some(zap_net::online::AccountProvider {
            provider: "supabase".into(),
            url: "https://x.supabase.co".into(),
            key: "k".into(),
        }),
        ..Default::default()
    }
}

async fn ate(mut ok: impl FnMut() -> bool) -> bool {
    let fim = Instant::now() + Duration::from_secs(5);
    while Instant::now() < fim {
        if ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    ok()
}

async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
    let token = token.map(String::from);
    let (core, d) = tokio::task::spawn_blocking(move || {
        let d = std::env::temp_dir().join(format!("zapcore-plref-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            online: false,
            ..Default::default()
        };
        (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
    })
    .await
    .unwrap();
    core.update_settings(|s| {
        s.server.address = url;
        s.server.enabled = true;
        s.server.use_relay = false;
    });
    *core.shared.online.account_token.lock() = token;
    core.shared.online.start(&core.shared);
    assert!(ate(|| core.shared.online.ws().is_some()).await, "sem websocket");
    (core, d)
}

async fn encerrar(core: Core, d: std::path::PathBuf) {
    tokio::task::spawn_blocking(move || {
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    })
    .await
    .unwrap();
}

async fn sala_online(core: &Core) -> Arc<OnlineRoom> {
    core.create_room(CreateRoom {
        name: "Sala".into(),
        mode: RoomMode::Online,
        password: None,
    })
    .await
    .unwrap();
    match core.shared.current_room().as_deref() {
        Some(ActiveRoom::Online(o)) => o.clone(),
        _ => panic!("sala nao e Online"),
    }
}

fn spec(local: &str, layers: Vec<zap_proto::LayerSpec>) -> TrackSpec {
    TrackSpec {
        local_id: local.into(),
        kind: TrackKind::Screen,
        title: "Tela".into(),
        layers,
        audio: None,
        address: None,
        camera_layout: None,
    }
}

fn n_trilhas(srv: &Handle) -> usize {
    srv.hub.inner.lock().rooms.values().map(|r| r.tracks.len()).sum()
}

fn camadas_no_servidor(srv: &Handle) -> Vec<zap_proto::LayerSpec> {
    let g = srv.hub.inner.lock();
    g.rooms
        .values()
        .flat_map(|r| r.tracks.iter())
        .flat_map(|t| t.layers.clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erro_do_servidor_volta_com_ref_sem_esperar_timeout() {
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    let room = sala_online(&core).await;

    let t0 = Instant::now();
    let r = room.publish_track(spec("l-1", vec![])).await;
    assert!(t0.elapsed() < Duration::from_secs(1), "demorou {:?}", t0.elapsed());
    assert!(
        matches!(r, Err(CoreError::Server { ref code, .. }) if code == "invalido"),
        "{r:?}"
    );
    assert!(room.pending.lock().is_empty());
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn publish_acima_do_plano_nao_envia_nada() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    let room = sala_online(&core).await;

    let escada = screen_ladder(RatePreset::P1080p60, 1920, 1080);
    let r = room.publish_track(spec("l-2", escada)).await;
    assert!(matches!(r, Err(CoreError::Plan(_))), "{r:?}");
    assert_eq!(n_trilhas(&srv), 0);
    assert!(room.pending.lock().is_empty());
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_acima_do_plano_nao_muda_a_escada() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    let room = sala_online(&core).await;
    core.start_share(ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P720p30,
        encoder: "auto".into(),
    })
    .await
    .expect("720p30 passa");
    assert!(ate(|| n_trilhas(&srv) == 1).await);
    let antes = camadas_no_servidor(&srv);
    let id = room
        .model
        .lock()
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Screen)
        .map(|t| t.id.clone())
        .expect("trilha no modelo");

    let escada = screen_ladder(RatePreset::P1080p60, 1920, 1080);
    let r = room.update_track(&id, None, Some(escada), None).await;
    assert!(matches!(r, Err(CoreError::Plan(_))), "{r:?}");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(camadas_no_servidor(&srv), antes);

    // Sem camadas, o plano nao entra na conta.
    room.update_track(&id, Some("Novo".into()), None, None)
        .await
        .expect("titulo passa");
    core.stop_share().await;
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sem_plano_publish_acima_passa() {
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    let room = sala_online(&core).await;
    let escada = screen_ladder(RatePreset::P1080p60, 1920, 1080);
    room.publish_track(spec("l-3", escada)).await.expect("sem plano passa");
    assert_eq!(n_trilhas(&srv), 1);
    core.leave_room().await;
    encerrar(core, d).await;
}
