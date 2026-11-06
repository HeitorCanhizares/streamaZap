//! Testes de integração de conta e plano contra o `zap-test-server` (tarefa core-07).

use std::time::{Duration, Instant};

use zap_net::online::AccountProvider;
use zap_proto::plan::PlanId;
use zap_test_server::Config;

use crate::engine::{Core, CoreOptions};
use crate::online::ServerState;
use crate::testutil::TestSink;
use crate::types::{CreateRoom, RoomMode};

async fn novo_core() -> (Core, std::path::PathBuf) {
    tokio::task::spawn_blocking(|| {
        let d = std::env::temp_dir().join(format!("zapcore-conta-{}", uuid::Uuid::new_v4()));
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

async fn encerrar(core: Core, d: std::path::PathBuf) {
    tokio::task::spawn_blocking(move || {
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    })
    .await
    .unwrap();
}

async fn ate(limite: u64, mut ok: impl FnMut() -> bool) -> bool {
    let fim = Instant::now() + Duration::from_secs(limite);
    while Instant::now() < fim {
        if ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    ok()
}

fn iniciar(core: &Core, url: String) {
    core.update_settings(|s| {
        s.server.address = url;
        s.server.enabled = true;
    });
    core.shared.online.start(&core.shared);
}

fn config_conta(requires_account: bool) -> Config {
    Config {
        requires_account,
        account: Some(AccountProvider {
            provider: "supabase".into(),
            url: "https://x.supabase.co".into(),
            key: "sb_publishable_x".into(),
        }),
        ..Default::default()
    }
}

fn esta_online(core: &Core) -> bool {
    core.snapshot()
        .server
        .as_ref()
        .is_some_and(|s| s.state == ServerState::Online)
}

fn account_id(core: &Core) -> Option<String> {
    core.snapshot().account.as_ref().and_then(|a| a.account_id.clone())
}

fn plano(core: &Core) -> Option<PlanId> {
    core.snapshot()
        .account
        .as_ref()
        .and_then(|a| a.plan.as_ref().map(|p| p.id))
}

fn login_error(core: &Core) -> Option<String> {
    core.snapshot().account.as_ref().and_then(|a| a.login_error.clone())
}

/// Entrega o token como o app faz com o Online ligado. `online_set_account` só guarda o token
/// quando `CoreOptions::online` é falso (caso destes testes), então chama a reação direto.
async fn definir_conta(core: &Core, token: Option<&str>) {
    core.shared
        .online
        .apply_account(&core.shared, token.map(String::from))
        .await;
}

fn epoch(core: &Core) -> u64 {
    *core.shared.online.epoch_rx().borrow()
}

fn renovacoes(srv: &zap_test_server::Handle, core: &Core) -> u32 {
    let pid = core.shared.online.client().unwrap().participant_id();
    srv.hub.account_of(&pid).unwrap().renew_count
}

fn ws_aberto(srv: &zap_test_server::Handle, core: &Core) -> bool {
    let Some(client) = core.shared.online.client() else {
        return false;
    };
    let pid = client.participant_id();
    srv.hub
        .inner
        .lock()
        .participants
        .get(&pid)
        .is_some_and(|p| p.ws.is_some())
}

fn sala_online() -> CreateRoom {
    CreateRoom {
        name: "Sala da conta".into(),
        mode: RoomMode::Online,
        password: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn login_com_token_traz_conta() {
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:fullhd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || esta_online(&core)).await, "{:?}", core.snapshot().server);
    let snap = core.snapshot();
    let conta = snap.account.as_ref().expect("account");
    assert_eq!(conta.account_id.as_deref(), Some("u1"));
    assert_eq!(conta.plan.as_ref().map(|p| p.id), Some(PlanId::FullHd));
    assert!(conta.requires_account);
    assert_eq!(
        conta.account_provider.as_ref().map(|p| p.url.as_str()),
        Some("https://x.supabase.co")
    );
    assert_eq!(conta.login_error, None);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn renovar_sem_derrubar_a_sala() {
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:hd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || esta_online(&core)).await);
    assert_eq!(plano(&core), Some(PlanId::Hd));
    core.create_room(sala_online()).await.unwrap();
    assert!(core.snapshot().room.is_some());
    assert!(ate(3, || ws_aberto(&srv, &core)).await);
    let epoch_antes = epoch(&core);
    let renew_antes = renovacoes(&srv, &core);

    definir_conta(&core, Some("teste:u1:fullhd")).await;
    assert!(
        ate(3, || plano(&core) == Some(PlanId::FullHd)).await,
        "{:?}",
        core.snapshot().account
    );

    assert!(core.snapshot().room.is_some(), "a sala continua");
    assert_eq!(epoch(&core), epoch_antes, "sessão não reiniciou");
    assert_eq!(renovacoes(&srv, &core), renew_antes + 1);
    assert!(ws_aberto(&srv, &core), "o WebSocket não caiu");
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mesmo_token_nao_renova_de_novo() {
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:hd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || esta_online(&core)).await);
    definir_conta(&core, Some("teste:u1:fullhd")).await;
    assert!(ate(3, || plano(&core) == Some(PlanId::FullHd)).await);
    let n = renovacoes(&srv, &core);
    definir_conta(&core, Some("teste:u1:fullhd")).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(renovacoes(&srv, &core), n);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sair_da_conta_reinicia_sem_conta() {
    // Servidor que exige conta: sem token o login falha.
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:hd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || esta_online(&core)).await);
    definir_conta(&core, None).await;
    let ok = ate(5, || {
        login_error(&core).as_deref() == Some("conta_necessaria")
            && core
                .snapshot()
                .server
                .as_ref()
                .is_some_and(|s| s.state == ServerState::Error)
    })
    .await;
    assert!(ok, "{:?} / {:?}", core.snapshot().account, core.snapshot().server);
    encerrar(core, d).await;

    // Servidor que não exige: sobe online sem conta.
    let srv = zap_test_server::spawn(config_conta(false)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:hd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || account_id(&core).as_deref() == Some("u1")).await);
    definir_conta(&core, None).await;
    let ok = ate(5, || esta_online(&core) && account_id(&core).is_none()).await;
    assert!(ok, "{:?} / {:?}", core.snapshot().account, core.snapshot().server);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requires_account_sem_token_depois_sobe_em_2s() {
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    iniciar(&core, srv.base_url.clone());
    assert!(
        ate(5, || login_error(&core).as_deref() == Some("conta_necessaria")).await,
        "{:?}",
        core.snapshot().account
    );
    definir_conta(&core, Some("teste:u1:hd")).await;
    let ok = ate(2, || esta_online(&core) && account_id(&core).as_deref() == Some("u1")).await;
    assert!(ok, "{:?} / {:?}", core.snapshot().account, core.snapshot().server);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn outra_conta_reinicia_a_sessao() {
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some("teste:u1:hd".into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || account_id(&core).as_deref() == Some("u1")).await);
    definir_conta(&core, Some("teste:u2:hd")).await;
    let ok = ate(5, || esta_online(&core) && account_id(&core).as_deref() == Some("u2")).await;
    assert!(ok, "{:?} / {:?}", core.snapshot().account, core.snapshot().server);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn senha_servidor_errada_e_depois_certa() {
    let cfg = Config {
        server_password: Some("abc".into()),
        ..config_conta(false)
    };
    let srv = zap_test_server::spawn(cfg).await.unwrap();
    let (core, d) = novo_core().await;
    iniciar(&core, srv.base_url.clone());
    let ok = ate(5, || login_error(&core).as_deref() == Some("senha_servidor")).await;
    assert!(ok, "{:?}", core.snapshot().account);
    assert!(core.snapshot().account.as_ref().is_some_and(|a| a.requires_password));
    core.update_settings(|s| s.server.password = "abc".into());
    let ok = ate(5, || esta_online(&core) && login_error(&core).is_none()).await;
    assert!(ok, "{:?} / {:?}", core.snapshot().account, core.snapshot().server);
    encerrar(core, d).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn token_nao_vai_para_o_settings_json() {
    let token = "teste:u1:fullhd";
    let srv = zap_test_server::spawn(config_conta(true)).await.unwrap();
    let (core, d) = novo_core().await;
    core.online_set_account(Some(token.into()));
    iniciar(&core, srv.base_url.clone());
    assert!(ate(5, || esta_online(&core)).await);
    // Força uma gravação do settings depois do login.
    core.update_settings(|s| s.server.enabled = true);
    let texto = std::fs::read_to_string(core.shared.data_dir.join("settings.json")).unwrap();
    assert!(!texto.contains(token), "token em disco");
    assert!(!format!("{:?}", core.snapshot()).contains(token), "token no snapshot");
    encerrar(core, d).await;
}
