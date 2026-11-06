//! Conta falsa, plano e renovação sem reconectar, pelo HTTP/WebSocket de verdade.
//! O token de conta é `teste:<sub>:<plano>` (ver `src/account.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use serde_json::{json, Value};
use zap_net::online::{AccountProvider, CreateRoomRequest, LoginRequest, ServerClient, WsRx};
use zap_proto::plan::PlanId;
use zap_proto::{Access, ControlMsg};
use zap_test_server::{spawn, Config};

/// Prazo de cada espera por mensagem do WebSocket.
const ESPERA: Duration = Duration::from_secs(3);

fn pedido(nome: &str, senha: Option<&str>, conta: Option<&str>) -> LoginRequest {
    LoginRequest {
        name: nome.into(),
        color: "#3BF08A".into(),
        install_id: format!("inst-{nome}"),
        protocol: 4,
        app_version: "3.0.0".into(),
        server_password: senha.map(String::from),
        account_token: conta.map(String::from),
    }
}

/// `POST /v1/sessoes` cru: devolve status e corpo JSON.
async fn login_cru(base: &str, req: &LoginRequest) -> (u16, Value) {
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/sessoes"))
        .json(req)
        .send()
        .await
        .unwrap();
    (r.status().as_u16(), r.json().await.unwrap())
}

/// `POST /v1/sessoes/renovar` cru. `corpo` `None` = sem corpo; `token` `None` = sem autenticação.
async fn renovar_cru(base: &str, token: Option<&str>, corpo: Option<Value>) -> (u16, Value) {
    let mut rb = reqwest::Client::new().post(format!("{base}/v1/sessoes/renovar"));
    if let Some(t) = token {
        rb = rb.bearer_auth(t);
    }
    if let Some(c) = corpo {
        rb = rb.json(&c);
    }
    let r = rb.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap())
}

/// `GET /v1/salas` com o token: serve para provar que ele autentica.
async fn status_salas(base: &str, token: &str) -> u16 {
    reqwest::Client::new()
        .get(format!("{base}/v1/salas"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn esperar(rx: &mut WsRx, descricao: &str, pred: impl Fn(&ControlMsg) -> bool) -> ControlMsg {
    tokio::time::timeout(ESPERA, async {
        loop {
            match rx.recv().await {
                Some(Ok(m)) if pred(&m) => return m,
                Some(Ok(_)) => {}
                outro => panic!("WebSocket terminou esperando {descricao}: {outro:?}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("tempo esgotado esperando {descricao}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn info_anuncia_conta() {
    let provedor = AccountProvider {
        provider: "supabase".into(),
        url: "https://x.supabase.co".into(),
        key: "sb_publishable_x".into(),
    };
    let h = spawn(Config {
        requires_account: true,
        account: Some(provedor.clone()),
        ..Config::default()
    })
    .await
    .unwrap();
    let info = ServerClient::info(&h.base_url).await.unwrap();
    assert!(info.requires_account);
    assert_eq!(info.account, Some(provedor));

    let h2 = spawn(Config::default()).await.unwrap();
    let info = ServerClient::info(&h2.base_url).await.unwrap();
    assert!(!info.requires_account);
    assert_eq!(info.account, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn login_com_conta_traz_plano() {
    let h = spawn(Config::default()).await.unwrap();
    let (status, v) = login_cru(&h.base_url, &pedido("Ana", None, Some("teste:u1:fullhd"))).await;
    assert_eq!(status, 201, "{v}");
    assert_eq!(v["conta_id"], "u1");
    assert_eq!(v["plano"]["id"], "fullhd");
    assert_eq!(v["plano"]["pode_entrar"], true);
    assert_eq!(v["plano"]["transmitir"]["altura_max"], 1080);

    let (status, v) = login_cru(&h.base_url, &pedido("Bia", None, None)).await;
    assert_eq!(status, 201, "{v}");
    assert!(v.get("conta_id").is_none(), "{v}");
    assert!(v.get("plano").is_none(), "{v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn requer_conta_401() {
    let h = spawn(Config {
        requires_account: true,
        ..Config::default()
    })
    .await
    .unwrap();
    for token in [None, Some("lixo")] {
        let (status, v) = login_cru(&h.base_url, &pedido("Ana", None, token)).await;
        assert_eq!(status, 401, "{v}");
        assert_eq!(v["erro"], "conta_necessaria");
    }
    let (status, _) = login_cru(&h.base_url, &pedido("Ana", None, Some("teste:u1:hd"))).await;
    assert_eq!(status, 201);

    // A senha do servidor vem antes da conta.
    let h = spawn(Config {
        requires_account: true,
        server_password: Some("x".into()),
        ..Config::default()
    })
    .await
    .unwrap();
    let (status, v) = login_cru(&h.base_url, &pedido("Ana", None, None)).await;
    assert_eq!(status, 401, "{v}");
    assert_eq!(v["erro"], "senha_servidor");
}

#[tokio::test(flavor = "multi_thread")]
async fn renovar_atualiza_plano_sem_reconectar() {
    let h = spawn(Config::default()).await.unwrap();
    let a = ServerClient::login(&h.base_url, pedido("Ana", None, Some("teste:u1:hd")))
        .await
        .unwrap();
    let token_antigo = a.token();
    let pid = a.participant_id();
    let sala = a
        .create_room(CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    let (ta, mut ra) = a.connect_ws().await.unwrap();
    ta.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();
    esperar(&mut ra, "sala.estado", |m| matches!(m, ControlMsg::RoomState { .. })).await;
    assert_eq!(h.hub.account_of(&pid).unwrap().renew_count, 0);

    let (status, v) = renovar_cru(
        &h.base_url,
        Some(&token_antigo),
        Some(json!({"acesso_conta": "teste:u1:fullhd"})),
    )
    .await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["plano"]["id"], "fullhd");
    assert!(!v["token"].as_str().unwrap().is_empty());

    assert_eq!(
        status_salas(&h.base_url, &token_antigo).await,
        200,
        "o token antigo ainda autentica"
    );

    // O WebSocket (aberto antes da renovação) continua recebendo.
    ta.send(ControlMsg::ChatSend {
        text: "oi".into(),
        client_id: "c1".into(),
    })
    .await
    .unwrap();
    esperar(
        &mut ra,
        "chat depois de renovar",
        |m| matches!(m, ControlMsg::ChatMessage { text, client_id: Some(c), .. } if text == "oi" && c == "c1"),
    )
    .await;

    let conta = h.hub.account_of(&pid).unwrap();
    assert_eq!(conta.renew_count, 1);
    assert_eq!(conta.plan, Some(PlanId::FullHd));
}

#[tokio::test(flavor = "multi_thread")]
async fn renovar_sem_corpo_e_invalido() {
    let h = spawn(Config::default()).await.unwrap();
    let (_, s) = login_cru(&h.base_url, &pedido("Ana", None, Some("teste:u1:hd"))).await;
    let token = s["token"].as_str().unwrap().to_string();
    let pid = s["participante_id"].as_str().unwrap().to_string();

    let (status, v) = renovar_cru(&h.base_url, Some(&token), None).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["plano"]["id"], "hd");
    assert_eq!(h.hub.account_of(&pid).unwrap().renew_count, 1);

    let (status, v) = renovar_cru(&h.base_url, Some(&token), Some(json!({"acesso_conta": "lixo"}))).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["plano"]["id"], "hd");
    let conta = h.hub.account_of(&pid).unwrap();
    assert_eq!(conta.renew_count, 2);
    assert_eq!(conta.plan, Some(PlanId::Hd));
}

#[tokio::test(flavor = "multi_thread")]
async fn renovar_liga_conta() {
    let h = spawn(Config::default()).await.unwrap();
    let (_, s) = login_cru(&h.base_url, &pedido("Ana", None, None)).await;
    assert!(s.get("conta_id").is_none());
    let token = s["token"].as_str().unwrap().to_string();

    let (status, v) = renovar_cru(&h.base_url, Some(&token), Some(json!({"acesso_conta": "teste:u7:hd"}))).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["conta_id"], "u7");
    assert_eq!(v["plano"]["id"], "hd");
}

#[tokio::test(flavor = "multi_thread")]
async fn renovar_outra_conta_403() {
    let h = spawn(Config::default()).await.unwrap();
    let (_, s) = login_cru(&h.base_url, &pedido("Ana", None, Some("teste:u1:hd"))).await;
    let token = s["token"].as_str().unwrap().to_string();

    let (status, v) = renovar_cru(&h.base_url, Some(&token), Some(json!({"acesso_conta": "teste:u2:hd"}))).await;
    assert_eq!(status, 403, "{v}");
    assert_eq!(v["erro"], "proibido");
    assert_eq!(
        status_salas(&h.base_url, &token).await,
        200,
        "o token da sessão continua valendo"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn renovar_sem_autenticacao_401() {
    let h = spawn(Config::default()).await.unwrap();
    let (status, v) = renovar_cru(&h.base_url, None, None).await;
    assert_eq!(status, 401, "{v}");
    assert_eq!(v["erro"], "nao_autenticado");
}
