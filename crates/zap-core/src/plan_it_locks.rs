//! Testes de integracao do plano (fase 3) com o zap-test-server: travas de qualidade no
//! snapshot por plano e desligar o Online (tarefa core-10).

use std::time::{Duration, Instant};

use serde_json::json;
use zap_proto::plan::PlanId;
use zap_proto::RoomMode;
use zap_test_server::Config;

use crate::engine::{Core, CoreOptions};
use crate::plan_gate::{PresetLock, QualityLocks};
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

/// Nucleo sem sessao (Online desligado nas opcoes, como nos outros testes de conta).
async fn core_local() -> (Core, std::path::PathBuf) {
    tokio::task::spawn_blocking(|| {
        let d = std::env::temp_dir().join(format!("zapcore-pllocks-{}", uuid::Uuid::new_v4()));
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
    .unwrap()
}

/// Nucleo apontado para `url`, com a sessao ligada (e o token, se houver).
async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
    let (core, d) = core_local().await;
    core.update_settings(|s| {
        s.server.address = url;
        s.server.enabled = true;
        s.server.use_relay = false;
    });
    *core.shared.online.account_token.lock() = token.map(String::from);
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

fn trava(preset: Preset, needs: PlanId) -> PresetLock {
    PresetLock {
        preset,
        needs: Some(needs),
    }
}

fn travas_1080(needs: PlanId) -> Vec<PresetLock> {
    vec![trava(Preset::P1080p30, needs), trava(Preset::P1080p60, needs)]
}

fn locks(core: &Core) -> Option<QualityLocks> {
    core.snapshot().quality_locks.clone()
}

fn sala() -> CreateRoom {
    CreateRoom {
        name: "Sala".into(),
        mode: RoomMode::Online,
        password: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plano_hd_trava_os_1080() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    assert!(ate(|| locks(&core).is_some()).await, "{:?}", core.snapshot().account);
    let q = locks(&core).expect("travas");
    assert!(q.can_publish);
    assert_eq!(q.screen, travas_1080(PlanId::FullHd));
    assert_eq!(q.camera, travas_1080(PlanId::FullHd));
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plano_fullhd_nao_trava_nada() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:fullhd")).await;
    assert!(ate(|| locks(&core).is_some()).await, "{:?}", core.snapshot().account);
    let q = locks(&core).expect("travas");
    assert!(q.can_publish);
    assert!(q.screen.is_empty());
    assert!(q.camera.is_empty());
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn espectador_nao_publica_e_trava_os_quatro() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:espectador")).await;
    assert!(ate(|| locks(&core).is_some()).await, "{:?}", core.snapshot().account);
    let q = locks(&core).expect("travas");
    assert!(!q.can_publish);
    let todos = vec![
        trava(Preset::P720p30, PlanId::Hd),
        trava(Preset::P720p60, PlanId::Hd),
        trava(Preset::P1080p30, PlanId::FullHd),
        trava(Preset::P1080p60, PlanId::FullHd),
    ];
    assert_eq!(q.screen, todos);
    assert_eq!(q.camera, todos);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn servidor_sem_conta_nao_tem_travas() {
    let srv = zap_test_server::spawn(Config::default()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), None).await;
    // Sessao pronta (ws aberto): da um tempo para um eventual snapshot tardio.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(locks(&core).is_none());
    let valor = serde_json::to_value(core.snapshot().as_ref()).expect("serializa");
    assert!(!valor.as_object().expect("objeto").contains_key("quality_locks"));
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trocar_a_conta_atualiza_as_travas() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    assert!(ate(|| locks(&core).is_some_and(|q| !q.screen.is_empty())).await);

    core.shared
        .online
        .apply_account(&core.shared, Some("teste:u1:fullhd".into()))
        .await;
    assert!(
        ate(|| locks(&core).is_some_and(|q| q.screen.is_empty() && q.camera.is_empty())).await,
        "{:?}",
        core.snapshot().quality_locks
    );

    core.shared
        .online
        .apply_account(&core.shared, Some("teste:u1:hd".into()))
        .await;
    assert!(
        ate(|| locks(&core).is_some_and(|q| q.screen == travas_1080(PlanId::FullHd))).await,
        "{:?}",
        core.snapshot().quality_locks
    );
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn json_do_snapshot_tem_o_formato_do_plano() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
    assert!(ate(|| locks(&core).is_some()).await);
    let valor = serde_json::to_value(core.snapshot().as_ref()).expect("serializa");
    let p30 = json!({"preset": "1080p30", "needs": "fullhd"});
    let p60 = json!({"preset": "1080p60", "needs": "fullhd"});
    assert_eq!(
        valor["quality_locks"],
        json!({"can_publish": true, "screen": [p30, p60], "camera": [p30, p60]})
    );
    encerrar(core, d).await;

    // Local (sem sessao): a chave nao existe.
    let (core, d) = core_local().await;
    let valor = serde_json::to_value(core.snapshot().as_ref()).expect("serializa");
    let objeto = valor.as_object().expect("objeto");
    assert!(!objeto.contains_key("quality_locks"));
    assert!(!objeto.contains_key("plan_notice"));
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn desligar_o_online_zera_aviso_e_travas() {
    let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
    let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:nenhum")).await;
    assert!(ate(|| locks(&core).is_some()).await, "{:?}", core.snapshot().account);
    let r = core.create_room(sala()).await;
    assert!(matches!(r, Err(CoreError::Plan(_))), "{r:?}");
    assert!(core.snapshot().plan_notice.is_some());

    // Mesmo caminho do interruptor (`aplicar_online`), que nestas opcoes nao roda sozinho.
    core.update_settings(|s| s.server.enabled = false);
    core.shared.online.stop_and_clear(&core.shared);

    let snap = core.snapshot();
    assert!(snap.plan_notice.is_none(), "{:?}", snap.plan_notice);
    assert!(snap.quality_locks.is_none(), "{:?}", snap.quality_locks);
    let valor = serde_json::to_value(snap.as_ref()).expect("serializa");
    let objeto = valor.as_object().expect("objeto");
    assert!(!objeto.contains_key("quality_locks"));
    assert!(!objeto.contains_key("plan_notice"));
    encerrar(core, d).await;
}
