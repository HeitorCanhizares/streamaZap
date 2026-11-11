//! Testes de integracao do plano (fase 3) com o zap-test-server.

use std::time::{Duration, Instant};

use zap_proto::RoomMode;
use zap_test_server::{Config, Handle};

use crate::engine::{Core, CoreOptions};
use crate::testutil::TestSink;
use crate::types::{CreateRoom, Preset};
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

/// Nucleo apontado para `url`, com a sessao ligada e a camera sintetica 1920x1080.
async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
    let token = token.map(String::from);
    let (core, d) = tokio::task::spawn_blocking(move || {
        let d = std::env::temp_dir().join(format!("zapcore-plcam-{}", uuid::Uuid::new_v4()));
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
        s.video.camera_id = Some("synthetic:1920x1080".into());
        s.video.camera_preset = Preset::P1080p60;
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

fn sala(modo: RoomMode) -> CreateRoom {
    CreateRoom {
        name: "Sala".into(),
        mode: modo,
        password: None,
    }
}

fn trilhas(srv: &Handle) -> Vec<(String, Vec<zap_proto::LayerSpec>)> {
    let g = srv.hub.inner.lock();
    g.rooms
        .values()
        .flat_map(|r| r.tracks.iter())
        .map(|t| (t.id.clone(), t.layers.clone()))
        .collect()
}

fn campo(e: &Result<(), CoreError>) -> Option<String> {
    match e {
        Err(CoreError::Plan(r)) => r.field.clone(),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hd_liga_limitada_e_recusa_qualidade_acima() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();

    core.set_camera(true).await.expect("liga limitada");
    assert!(ate(|| trilhas(&srv).len() == 1).await);
    let antes = trilhas(&srv);
    let c = &antes[0].1[0];
    assert_eq!(c.layer, 0);
    assert_eq!((c.width, c.height, c.fps, c.kbps), (1280, 720, 60, 2500));
    assert_eq!(core.settings().video.camera_preset, Preset::P1080p60);

    let r = core.set_camera_quality(Preset::P1080p30).await;
    assert_eq!(campo(&r).as_deref(), Some("camadas"), "{r:?}");
    assert_eq!(trilhas(&srv), antes);
    assert_eq!(core.settings().video.camera_preset, Preset::P1080p60);
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fullhd_liga_em_1080p60() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:fullhd")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();
    core.set_camera(true).await.expect("liga");
    assert!(ate(|| trilhas(&srv).len() == 1).await);
    let c = trilhas(&srv)[0].1[0].clone();
    assert_eq!((c.width, c.height, c.fps, c.kbps), (1920, 1080, 60, 4500));
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_sem_limite() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Local)).await.unwrap();
    core.set_camera(true).await.expect("liga");
    let camada = core
        .shared
        .share
        .lock()
        .await
        .camera_info
        .as_ref()
        .map(|i| i.plan.ladder[0].clone())
        .expect("camera ligada");
    assert_eq!((camada.width, camada.height, camada.fps), (1920, 1080, 60));
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn espectador_nao_liga_camera() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:espectador")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();
    let r = core.set_camera(true).await;
    assert_eq!(campo(&r).as_deref(), Some("tipo"), "{r:?}");
    assert!(trilhas(&srv).is_empty());
    core.leave_room().await;
    encerrar(core, d).await;
}
