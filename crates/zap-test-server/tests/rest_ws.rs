//! REST + WebSocket do servidor de teste com o cliente de verdade do app (`zap_net::online`).
//! Aceites 1 e 3 de `docs/planos/m4-online.md` (a parte sem mídia).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use zap_net::online::{ApiError, CreateRoomRequest, LoginRequest, ServerClient, WsRx};
use zap_proto::{Access, ControlMsg};
use zap_test_server::{spawn, Config};

/// Prazo de cada espera por mensagem do WebSocket.
const ESPERA: Duration = Duration::from_secs(3);

fn pedido(nome: &str, protocolo: u32, senha: Option<&str>) -> LoginRequest {
    LoginRequest {
        name: nome.into(),
        color: "#3BF08A".into(),
        install_id: format!("inst-{nome}"),
        protocol: protocolo,
        app_version: "3.0.0".into(),
        server_password: senha.map(String::from),
        account_token: None,
    }
}

async fn login(base: &str, nome: &str) -> ServerClient {
    ServerClient::login(base, pedido(nome, 4, None)).await.unwrap()
}

/// Lê o WebSocket até chegar uma mensagem que satisfaça `pred` (as outras são puladas).
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

fn sala_com_senha() -> CreateRoomRequest {
    CreateRoomRequest {
        name: "Sala".into(),
        access: Access::Password,
        password: Some("segredo".into()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fluxo_sala_com_senha() {
    let h = spawn(Config::default()).await.unwrap();
    let a = login(&h.base_url, "Ana").await;
    let sala = a.create_room(sala_com_senha()).await.unwrap();

    let b = login(&h.base_url, "Bia").await;
    let det = b.room(&sala.code).await.unwrap();
    assert!(
        det.participant_list.is_none(),
        "sala com senha esconde a lista de quem não entrou"
    );

    let erro = b.join(&sala.code, Some("errada")).await.unwrap_err();
    assert!(
        matches!(&erro, ApiError::Http { code, status: 403, .. } if code == "senha_sala"),
        "{erro:?}"
    );

    let entrou = b.join(&sala.code, Some("segredo")).await.unwrap();
    assert!(!entrou.ticket.is_empty());
    let ponte = entrou.media.bridge.expect("servidor com ponte");
    assert_eq!(ponte.alpn, "zap/4");
    assert_eq!(ponte.address, h.quic_addr.to_string());

    let salas = a.rooms().await.unwrap();
    assert_eq!(salas.len(), 1);
    assert_eq!(salas[0].participants, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn websocket_e_chat() {
    let h = spawn(Config::default()).await.unwrap();
    let a = login(&h.base_url, "Ana").await;
    let b = login(&h.base_url, "Bia").await;
    let sala = a
        .create_room(CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    b.join(&sala.code, None).await.unwrap();

    let (ta, mut ra) = a.connect_ws().await.unwrap();
    let (tb, mut rb) = b.connect_ws().await.unwrap();
    esperar(&mut ra, "salas.lista de Ana", |m| {
        matches!(m, ControlMsg::RoomList { .. })
    })
    .await;
    esperar(&mut rb, "salas.lista de Bia", |m| {
        matches!(m, ControlMsg::RoomList { .. })
    })
    .await;

    // Quem criou a sala já está autorizado: `sala.entrar` sem `POST …/entrar`.
    ta.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();
    tb.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();
    esperar(&mut ra, "sala.estado de Ana", |m| {
        matches!(m, ControlMsg::RoomState { .. })
    })
    .await;
    esperar(&mut rb, "sala.estado de Bia", |m| {
        matches!(m, ControlMsg::RoomState { .. })
    })
    .await;

    ta.send(ControlMsg::ChatSend {
        text: "oi".into(),
        client_id: "c1".into(),
    })
    .await
    .unwrap();
    esperar(
        &mut rb,
        "chat de Ana para Bia",
        |m| matches!(m, ControlMsg::ChatMessage { text, client_id: None, .. } if text == "oi"),
    )
    .await;
    esperar(
        &mut ra,
        "confirmação do chat para Ana",
        |m| matches!(m, ControlMsg::ChatMessage { text, client_id: Some(c), .. } if text == "oi" && c == "c1"),
    )
    .await;

    // JSON que não é `ControlMsg` vira `erro` `invalido` e a conexão segue: um tipo que o servidor
    // não aceita do app também.
    tb.send(ControlMsg::Refused {
        code: "x".into(),
        message: "y".into(),
    })
    .await
    .unwrap();
    esperar(
        &mut rb,
        "erro invalido",
        |m| matches!(m, ControlMsg::Error { code, .. } if code == "invalido"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn protocolo_incompativel() {
    let h = spawn(Config::default()).await.unwrap();
    let erro = ServerClient::login(&h.base_url, pedido("Ana", 3, None))
        .await
        .err()
        .unwrap();
    assert!(
        matches!(&erro, ApiError::Http { status: 426, code, .. } if code == "protocolo_incompativel"),
        "{erro:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn senha_do_servidor() {
    let h = spawn(Config {
        server_password: Some("x".into()),
        ..Config::default()
    })
    .await
    .unwrap();
    let info = ServerClient::info(&h.base_url).await.unwrap();
    assert!(info.requires_password);

    let erro = ServerClient::login(&h.base_url, pedido("Ana", 4, None))
        .await
        .err()
        .unwrap();
    assert!(
        matches!(&erro, ApiError::Http { status: 401, code, .. } if code == "senha_servidor"),
        "{erro:?}"
    );
    assert!(ServerClient::login(&h.base_url, pedido("Ana", 4, Some("x")))
        .await
        .is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn saida_notifica() {
    let h = spawn(Config::default()).await.unwrap();
    let a = login(&h.base_url, "Ana").await;
    let b = login(&h.base_url, "Bia").await;
    let sala = a
        .create_room(CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    b.join(&sala.code, None).await.unwrap();
    let (ta, mut ra) = a.connect_ws().await.unwrap();
    ta.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();
    esperar(&mut ra, "sala.estado", |m| matches!(m, ControlMsg::RoomState { .. })).await;

    b.leave(&sala.code).await.unwrap();
    esperar(
        &mut ra,
        "participante.saiu",
        |m| matches!(m, ControlMsg::ParticipantLeft { reason, .. } if reason == "saiu"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn encerrar_sala() {
    let h = spawn(Config::default()).await.unwrap();
    let a = login(&h.base_url, "Ana").await;
    let b = login(&h.base_url, "Bia").await;
    let sala = a
        .create_room(CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    b.join(&sala.code, None).await.unwrap();
    let (tb, mut rb) = b.connect_ws().await.unwrap();
    tb.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();
    esperar(&mut rb, "sala.estado", |m| matches!(m, ControlMsg::RoomState { .. })).await;

    let erro = b.delete_room(&sala.code).await.unwrap_err();
    assert_eq!(erro.code(), Some("proibido"));
    a.delete_room(&sala.code).await.unwrap();
    esperar(
        &mut rb,
        "sala.encerrada",
        |m| matches!(m, ControlMsg::RoomClosed { reason } if reason == "dono"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn caminho_inexistente_e_corpo_invalido() {
    let h = spawn(Config::default()).await.unwrap();
    let http = reqwest_simples();
    let r = http.get(format!("{}/v1/nada", h.base_url)).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 404);
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["erro"], "nao_encontrado");

    let r = http
        .post(format!("{}/v1/sessoes", h.base_url))
        .header("content-type", "application/json")
        .body("{\"nome\": 1}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 400);
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["erro"], "invalido");
}

fn reqwest_simples() -> reqwest::Client {
    reqwest::Client::new()
}
