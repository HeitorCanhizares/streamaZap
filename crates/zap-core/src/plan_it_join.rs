//! Testes de integracao do plano (fase 3) com o zap-test-server.

use std::time::{Duration, Instant};

use zap_proto::plan::PlanId;
use zap_proto::RoomMode;
use zap_test_server::Config;

use crate::engine::{Core, CoreOptions};
use crate::plan_gate::{PlanAction, PlanRefusal};
use crate::room::RoomEvent;
use crate::testutil::TestSink;
use crate::types::{CreateRoom, JoinTarget};
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

/// Núcleo apontado para `url`, com a sessão ligada (e o token, se houver).
async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
    let token = token.map(String::from);
    let (core, d) = tokio::task::spawn_blocking(move || {
        let d = std::env::temp_dir().join(format!("zapcore-pljoin-{}", uuid::Uuid::new_v4()));
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

fn sala() -> CreateRoom {
    CreateRoom {
        name: "Sala".into(),
        mode: RoomMode::Online,
        password: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plano_nenhum_recusa_criar_e_entrar() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:nenhum")).await;

    let r = core.create_room(sala()).await;
    match r {
        Err(CoreError::Plan(ref p)) => {
            assert_eq!(p.action, PlanAction::Join);
            assert_eq!(p.field, None);
            assert_eq!(p.message, "Seu plano não permite entrar em salas Online");
        }
        outro => panic!("esperava recusa por plano: {outro:?}"),
    }
    assert!(srv.hub.inner.lock().rooms.is_empty(), "o servidor criou sala");
    let n = core.snapshot().plan_notice.clone().expect("sem plan_notice");
    assert_eq!(n.action, PlanAction::Join);
    assert_eq!(n.seq, 1);

    let r = core
        .join_room(
            JoinTarget::Code {
                code: "ZAP-ABCD".into(),
            },
            None,
        )
        .await;
    assert!(matches!(r, Err(CoreError::Plan(_))), "{r:?}");
    assert_eq!(core.snapshot().plan_notice.as_ref().map(|n| n.seq), Some(2));
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plano_espectador_cria_sala() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:espectador")).await;
    core.create_room(sala()).await.expect("espectador deve criar");
    assert!(core.snapshot().plan_notice.is_none());
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn servidor_sem_conta_nao_muda_nada() {
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    core.create_room(sala()).await.expect("criar");
    assert!(core.snapshot().plan_notice.is_none());
    core.leave_room().await;
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn evento_plan_refused_vira_aviso_e_erro() {
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    let msg = "Seu plano não permite essas camadas";
    let _ = core.shared.room_events.send(RoomEvent::PlanRefused(PlanRefusal {
        plan: PlanId::Hd,
        action: PlanAction::Publish,
        field: Some("camadas".into()),
        message: msg.into(),
    }));
    assert!(ate(|| core.snapshot().plan_notice.is_some()).await, "sem aviso");
    let snap = core.snapshot();
    assert_eq!(snap.plan_notice.as_ref().map(|n| n.seq), Some(1));
    assert!(snap.errors.iter().any(|e| e == msg), "{:?}", snap.errors);
    encerrar(core, d).await;
}
