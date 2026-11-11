//! Testes de integracao do plano (fase 3) com o zap-test-server: a transmissao confere o plano
//! antes de mudar qualquer coisa.

use std::time::{Duration, Instant};

use zap_proto::plan::PlanId;
use zap_proto::RoomMode;
use zap_test_server::{Config, Handle};

use crate::engine::{Core, CoreOptions};
use crate::plan_gate::PlanAction;
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

/// Nucleo apontado para `url`, com a sessao ligada (e o token, se houver).
async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
    let token = token.map(String::from);
    let (core, d) = tokio::task::spawn_blocking(move || {
        let d = std::env::temp_dir().join(format!("zapcore-plshare-{}", uuid::Uuid::new_v4()));
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

fn sala(modo: RoomMode) -> CreateRoom {
    CreateRoom {
        name: "Sala".into(),
        mode: modo,
        password: None,
    }
}

fn pedido(preset: Preset) -> ShareRequest {
    ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset,
        encoder: "auto".into(),
    }
}

fn so_som() -> ShareRequest {
    ShareRequest {
        source: crate::share::SO_AUDIO.into(),
        audio: AudioChoice::Keyword("all".into()),
        preset: Preset::P720p30,
        encoder: "auto".into(),
    }
}

fn camadas_no_servidor(srv: &Handle) -> Vec<zap_proto::LayerSpec> {
    let g = srv.hub.inner.lock();
    g.rooms
        .values()
        .flat_map(|r| r.tracks.iter())
        .flat_map(|t| t.layers.clone())
        .collect()
}

fn n_trilhas(srv: &Handle) -> usize {
    srv.hub.inner.lock().rooms.values().map(|r| r.tracks.len()).sum()
}

fn campo(e: &Result<(), CoreError>) -> Option<String> {
    match e {
        Err(CoreError::Plan(r)) => r.field.clone(),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hd_recusa_1080p60_e_aceita_720p60() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();

    let t0 = Instant::now();
    let r = core.start_share(pedido(Preset::P1080p60)).await;
    assert!(t0.elapsed() < Duration::from_millis(200), "demorou {:?}", t0.elapsed());
    assert_eq!(campo(&r).as_deref(), Some("camadas"), "{r:?}");
    assert_eq!(n_trilhas(&srv), 0);
    let snap = core.snapshot();
    let aviso = snap.plan_notice.as_ref().expect("aviso de plano");
    assert_eq!(aviso.seq, 1);
    assert_eq!(aviso.plan, PlanId::Hd);
    assert_eq!(aviso.action, PlanAction::Publish);
    assert_eq!(aviso.field.as_deref(), Some("camadas"));
    assert!(snap.share.is_none());

    core.start_share(pedido(Preset::P720p60)).await.expect("720p60 passa");
    assert!(ate(|| n_trilhas(&srv) == 1).await);
    let camadas = camadas_no_servidor(&srv);
    assert_eq!(camadas[0].layer, 0);
    assert_eq!((camadas[0].width, camadas[0].height, camadas[0].fps), (1280, 720, 60));
    assert_eq!(camadas[0].kbps, 4000);
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn espectador_nao_transmite_nem_so_o_som() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:espectador")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();

    let r = core.start_share(pedido(Preset::P720p30)).await;
    assert_eq!(campo(&r).as_deref(), Some("tipo"), "{r:?}");
    let r = core.start_share(so_som()).await;
    assert_eq!(campo(&r).as_deref(), Some("tipo"), "{r:?}");
    assert_eq!(n_trilhas(&srv), 0);
    assert!(core.snapshot().share.is_none());
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sem_plano_nada_e_conferido() {
    // Sala Local com sessao hd ligada.
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Local)).await.unwrap();
    core.start_share(pedido(Preset::P1080p60)).await.expect("Local passa");
    core.stop_share().await;
    core.leave_room().await;
    encerrar(core, d).await;

    // Online com servidor sem conta.
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();
    core.start_share(pedido(Preset::P1080p60))
        .await
        .expect("sem conta passa");
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_acima_do_plano_mantem_escada_e_pedido() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();
    core.start_share(pedido(Preset::P720p30)).await.expect("720p30 passa");
    assert!(ate(|| n_trilhas(&srv) == 1).await);
    let antes = camadas_no_servidor(&srv);

    let r = core.update_share(pedido(Preset::P1080p60)).await;
    assert_eq!(campo(&r).as_deref(), Some("camadas"), "{r:?}");
    assert_eq!(camadas_no_servidor(&srv), antes);
    let share = core.snapshot().share.clone().expect("continua transmitindo");
    assert_eq!(share.request.map(|q| q.preset), Some(Preset::P720p30));
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn qualidade_da_camera_acima_do_plano_e_recusada() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    core.create_room(sala(RoomMode::Online)).await.unwrap();
    let antes = core.settings().video.camera_preset;

    let r = core.set_camera_quality(Preset::P1080p60).await;
    assert_eq!(campo(&r).as_deref(), Some("camadas"), "{r:?}");
    assert_eq!(core.settings().video.camera_preset, antes);
    assert_eq!(core.snapshot().plan_notice.as_ref().map(|n| n.seq), Some(1));

    core.set_camera_quality(Preset::P720p30).await.expect("720p30 passa");
    assert_eq!(core.settings().video.camera_preset, Preset::P720p30);
    core.leave_room().await;
    encerrar(core, d).await;
}
