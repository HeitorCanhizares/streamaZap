//! REST de salas e diagnóstico (`docs/api/rest.md`). Cada handler só converte o pedido,
//! chama o método de `Hub` (`rooms.rs`/`rooms_join.rs`) e serializa a resposta.

use std::sync::Arc;

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use zap_net::online::{
    CreateRoomRequest, CreateRoomResponse, JoinRequest, JoinResponse, PreviewResponse, RoomDetails, RoomListResponse,
    ServerDiagnostics,
};
use zap_proto::RoomCode;

use crate::{
    rest::{corpo_invalido, Auth},
    state::{Hub, HubError},
};

/// Rotas: `GET/POST /v1/salas`, `GET/DELETE /v1/salas/{codigo}`, `POST …/entrar`, `…/sair`,
/// `…/previa` e `GET /v1/diagnostico`. Sintaxe de path do axum 0.8: `/{codigo}`.
pub fn routes() -> Router<Arc<Hub>> {
    Router::new()
        .route("/v1/salas", get(list).post(create))
        .route("/v1/salas/{codigo}", get(details).delete(remove))
        .route("/v1/salas/{codigo}/entrar", post(join))
        .route("/v1/salas/{codigo}/sair", post(leave))
        .route("/v1/salas/{codigo}/previa", post(preview))
        .route("/v1/diagnostico", get(diagnostics))
}

/// `{codigo}` inválido ou inexistente = `sala_nao_encontrada` (404). Aceita `ZAP-7K2Q`,
/// `zap-7k2q` e `7K2Q` (`RoomCode::parse` normaliza). O id `s_…` que o servidor de produção
/// também aceita fica de fora: o app sempre usa o código.
pub fn parse_code(text: &str) -> Result<RoomCode, HubError> {
    RoomCode::parse(text).map_err(|_| HubError::new(404, "sala_nao_encontrada", "Sala não encontrada"))
}

pub async fn list(State(hub): State<Arc<Hub>>, Auth(who): Auth) -> Json<RoomListResponse> {
    Json(RoomListResponse {
        rooms: hub.list_rooms(&who),
    })
}

/// `201` com `{ "sala": ... }`; depois `broadcast_room_list`.
pub async fn create(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    corpo: Result<Json<CreateRoomRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateRoomResponse>), HubError> {
    let Json(req) = corpo.map_err(corpo_invalido)?;
    let room = hub.create_room(&who, req)?;
    hub.inner.lock().broadcast_room_list();
    Ok((StatusCode::CREATED, Json(CreateRoomResponse { room })))
}

pub async fn details(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<Json<RoomDetails>, HubError> {
    let code = parse_code(&code)?;
    hub.room_details(&who, &code).map(Json)
}

/// Corpo opcional: `POST …/entrar` sem corpo equivale a `{}` (sem senha).
pub async fn join(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
    body: Option<Json<JoinRequest>>,
) -> Result<Json<JoinResponse>, HubError> {
    let code = parse_code(&code)?;
    let password = body.and_then(|Json(b)| b.password);
    hub.join_room(&who, &code, password.as_deref()).map(Json)
}

/// `204`.
pub async fn leave(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<StatusCode, HubError> {
    let code = parse_code(&code)?;
    hub.leave_room(&who, &code)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `204` (só o dono).
pub async fn remove(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<StatusCode, HubError> {
    let code = parse_code(&code)?;
    hub.delete_room(&who, &code)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn preview(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<Json<PreviewResponse>, HubError> {
    let code = parse_code(&code)?;
    hub.preview_room(&who, &code).map(Json)
}

pub async fn diagnostics(State(hub): State<Arc<Hub>>, Auth(who): Auth) -> Result<Json<ServerDiagnostics>, HubError> {
    hub.diagnostics(&who).map(Json)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Config;
    use zap_net::online::LoginRequest;
    use zap_proto::Access;

    fn entrar(hub: &Hub, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: format!("inst-{nome}"),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

    #[test]
    fn parse_code_ok() {
        assert_eq!(parse_code("zap-7k2q").unwrap().as_str(), "ZAP-7K2Q");
        assert_eq!(parse_code("7K2Q").unwrap().as_str(), "ZAP-7K2Q");
    }

    #[test]
    fn parse_code_invalido() {
        // `O` não faz parte do alfabeto dos códigos; e o tamanho precisa ser 4.
        for texto in ["ZAP-7K2O", "abc", ""] {
            let e = parse_code(texto).unwrap_err();
            assert_eq!((e.status, e.code), (404, "sala_nao_encontrada"));
        }
    }

    #[tokio::test]
    async fn handlers_fluxo() {
        let hub = Arc::new(Hub::new(Config::default()));
        let ana = entrar(&hub, "Ana");
        let bia = entrar(&hub, "Bia");

        let pedido = CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        };
        let (status, Json(criada)) = create(State(hub.clone()), Auth(ana.clone()), Ok(Json(pedido)))
            .await
            .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        let codigo = criada.room.code.as_str().to_string();

        let Json(lista) = list(State(hub.clone()), Auth(bia.clone())).await;
        assert_eq!(lista.rooms.len(), 1);

        let Json(det) = details(State(hub.clone()), Auth(bia.clone()), Path(codigo.clone()))
            .await
            .unwrap();
        assert_eq!(det.room.code.as_str(), codigo);

        let Json(entrou) = join(State(hub.clone()), Auth(bia.clone()), Path(codigo.clone()), None)
            .await
            .unwrap();
        assert!(!entrou.ticket.is_empty());

        let Json(previa) = preview(State(hub.clone()), Auth(bia.clone()), Path(codigo.clone()))
            .await
            .unwrap();
        assert!(!previa.ticket.is_empty());

        let Json(diag) = diagnostics(State(hub.clone()), Auth(bia.clone())).await.unwrap();
        assert_eq!(diag.public_ip, "203.0.113.7");

        let st = leave(State(hub.clone()), Auth(bia.clone()), Path(codigo.clone()))
            .await
            .unwrap();
        assert_eq!(st, StatusCode::NO_CONTENT);

        // Só o dono encerra.
        let e = remove(State(hub.clone()), Auth(bia.clone()), Path(codigo.clone()))
            .await
            .unwrap_err();
        assert_eq!(e.code, "proibido");
        let st = remove(State(hub.clone()), Auth(ana.clone()), Path(codigo.clone()))
            .await
            .unwrap();
        assert_eq!(st, StatusCode::NO_CONTENT);

        // Sala inexistente.
        let e = join(State(hub.clone()), Auth(bia.clone()), Path("ZAP-2345".into()), None)
            .await
            .unwrap_err();
        assert_eq!((e.status, e.code), (404, "sala_nao_encontrada"));
    }
}
