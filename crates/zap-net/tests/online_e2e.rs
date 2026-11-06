#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fluxo completo do modo Online contra um servidor axum local: info -> login -> criar sala ->
//! entrar -> WebSocket -> trocar mensagens. Usa só a API pública `zap_net::online::*`.
mod support;

use axum::{
    extract::{ws::Message, ws::WebSocketUpgrade, Path, Query},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::net::TcpListener;
use zap_net::online::{CreateRoomRequest, LoginRequest, ServerClient};
use zap_proto::{Access, ControlMsg};

const TOKEN: &str = "tok_e2e";
const SALA_ID: &str = "s_e2e01";
const CODIGO: &str = "ZAP-7K2Q";

/// Sobe o app numa porta livre de loopback e devolve a base `http://127.0.0.1:porta`.
async fn sobe(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Confere `Authorization: Bearer tok_e2e`, como o servidor real faria nas rotas autenticadas.
fn autorizado(headers: &HeaderMap) -> bool {
    headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) == Some("Bearer tok_e2e")
}

fn nao_autenticado() -> Response {
    let corpo = json!({"erro": "nao_autenticado", "mensagem": "sessão inválida"});
    (StatusCode::UNAUTHORIZED, Json(corpo)).into_response()
}

async fn info() -> Json<Value> {
    Json(json!({
        "nome": "Servidor E2E",
        "versao_servidor": "1.0.0",
        "protocolo_min": 4,
        "protocolo_max": 4,
        "requer_senha": false,
        "ponte": false,
        "quic": { "porta": 7443, "alpn": "zap/4" },
        "limites": { "participantes_por_sala": 16, "trilhas_video_por_sala": 8, "bitrate_max_kbps": 12000 },
        "agora": "2026-11-04T21:30:00Z"
    }))
}

async fn login() -> (StatusCode, Json<Value>) {
    let corpo = json!({
        "token": TOKEN,
        "expira_em": "2026-11-05T21:30:00Z",
        "participante_id": "p_e2e"
    });
    (StatusCode::CREATED, Json(corpo))
}

fn sala_json() -> Value {
    json!({
        "id": SALA_ID,
        "codigo": CODIGO,
        "nome": "Sexta do Rush",
        "dono": { "id": "p_e2e", "nome": "Heitor", "cor": "#3BF08A" },
        "acesso": "aberta",
        "criada_em": "2026-11-04T21:30:00Z",
        "participantes": 1
    })
}

async fn criar_sala(headers: HeaderMap) -> Response {
    if !autorizado(&headers) {
        return nao_autenticado();
    }
    (StatusCode::CREATED, Json(json!({ "sala": sala_json() }))).into_response()
}

async fn entrar(headers: HeaderMap, Path(codigo): Path<String>) -> Response {
    if !autorizado(&headers) {
        return nao_autenticado();
    }
    if codigo != CODIGO {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"erro": "sala_inexistente", "mensagem": "x"})),
        )
            .into_response();
    }
    let corpo = json!({
        "sala": sala_json(),
        "participante_id": "p_e2e",
        "ingresso": "ingresso-e2e",
        "ingresso_expira_em": "2026-11-04T22:30:00Z",
        "midia": { "direta_permitida": false }
    });
    Json(corpo).into_response()
}

/// WebSocket: o token vai na query. Responde `sala.entrar` com `salas.lista` contendo a sala
/// pedida, para o teste conferir que a mensagem chegou e voltou pelo fio.
async fn ws(ws: WebSocketUpgrade, Query(q): Query<HashMap<String, String>>) -> Response {
    if q.get("token").map(String::as_str) != Some(TOKEN) {
        return nao_autenticado();
    }
    ws.on_upgrade(|mut socket| async move {
        while let Some(Ok(Message::Text(texto))) = socket.recv().await {
            let Ok(msg) = serde_json::from_str::<ControlMsg>(texto.as_str()) else {
                continue;
            };
            if let ControlMsg::RoomJoin { room_id } = msg {
                let resposta = ControlMsg::RoomList {
                    salas: vec![json!({ "id": room_id })],
                };
                let Ok(texto) = serde_json::to_string(&resposta) else {
                    break;
                };
                if socket.send(Message::Text(texto.into())).await.is_err() {
                    break;
                }
            }
        }
    })
    .into_response()
}

fn app() -> Router {
    Router::new()
        .route("/v1/info", get(info))
        .route("/v1/sessoes", post(login))
        .route("/v1/salas", post(criar_sala))
        .route("/v1/salas/{codigo}/entrar", post(entrar))
        .route("/v1/ws", get(ws))
}

fn pedido() -> LoginRequest {
    LoginRequest {
        name: "Heitor".into(),
        color: "#3BF08A".into(),
        install_id: "i-e2e".into(),
        protocol: 4,
        app_version: "3.0.0".into(),
        server_password: None,
        account_token: None,
    }
}

#[tokio::test]
async fn fluxo_completo_info_login_sala_ws() {
    let _timer = support::timer();
    let base = sobe(app()).await;

    let info = ServerClient::info(&base).await.unwrap();
    assert_eq!(info.name, "Servidor E2E");
    assert_eq!((info.protocol_min, info.protocol_max), (4, 4));

    let cliente = ServerClient::login(&base, pedido()).await.unwrap();
    assert_eq!(cliente.participant_id(), "p_e2e");

    let sala = cliente
        .create_room(CreateRoomRequest {
            name: "Sexta do Rush".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    assert_eq!(sala.code.as_str(), CODIGO);

    let entrada = cliente.join(&sala.code, None).await.unwrap();
    assert_eq!(entrada.ticket, "ingresso-e2e");
    assert_eq!(entrada.room.id, SALA_ID);

    let (tx, mut rx) = cliente.connect_ws().await.unwrap();
    tx.send(ControlMsg::RoomJoin {
        room_id: sala.id.clone(),
    })
    .await
    .unwrap();

    // A resposta do servidor chega decodificada, com o id da sala ecoado.
    match rx.recv().await {
        Some(Ok(ControlMsg::RoomList { salas })) => {
            assert_eq!(salas.len(), 1);
            assert_eq!(salas[0]["id"], SALA_ID);
        }
        outra => panic!("esperava salas.lista, veio {outra:?}"),
    }
}

#[test]
fn cliente_e_send_sync() {
    let _timer = support::timer();
    fn exige<T: Send + Sync + Clone>() {}
    exige::<ServerClient>();
}
