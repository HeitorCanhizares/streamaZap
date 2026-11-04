//! REST de salas e diagnóstico (`docs/api/rest.md`). Cada handler só converte o pedido,
//! chama o método de `Hub` (`rooms.rs`/`rooms_join.rs`) e serializa a resposta.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use zap_net::online::{
    CreateRoomRequest, CreateRoomResponse, JoinRequest, JoinResponse, PreviewResponse, RoomDetails, RoomListResponse,
    ServerDiagnostics,
};
use zap_proto::RoomCode;

use crate::{
    rest::Auth,
    state::{Hub, HubError},
};

/// Rotas: `GET/POST /v1/salas`, `GET/DELETE /v1/salas/{codigo}`, `POST …/entrar`, `…/sair`,
/// `…/previa` e `GET /v1/diagnostico`. Sintaxe de path do axum 0.8: `/{codigo}`.
pub fn routes() -> Router<Arc<Hub>> {
    let _ = (get(list), post(create), delete(remove), get(details));
    let _ = (post(join), post(leave), post(preview), get(diagnostics));
    todo!("tarefa zt-09")
}

/// `{codigo}` inválido ou inexistente = `sala_nao_encontrada` (404).
pub fn parse_code(text: &str) -> Result<RoomCode, HubError> {
    let _ = text;
    todo!("tarefa zt-09")
}

pub async fn list(State(hub): State<Arc<Hub>>, Auth(who): Auth) -> Json<RoomListResponse> {
    let _ = (hub, who);
    todo!("tarefa zt-09")
}

/// `201` com `{ "sala": ... }`; depois `broadcast_room_list`.
pub async fn create(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<CreateRoomResponse>), HubError> {
    let _ = (hub, who, req);
    todo!("tarefa zt-09")
}

pub async fn details(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<Json<RoomDetails>, HubError> {
    let _ = (hub, who, code);
    todo!("tarefa zt-09")
}

/// Corpo opcional: `POST …/entrar` sem corpo equivale a `{}` (sem senha).
pub async fn join(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
    body: Option<Json<JoinRequest>>,
) -> Result<Json<JoinResponse>, HubError> {
    let _ = (hub, who, code, body);
    todo!("tarefa zt-09")
}

/// `204`.
pub async fn leave(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<StatusCode, HubError> {
    let _ = (hub, who, code);
    todo!("tarefa zt-09")
}

/// `204` (só o dono).
pub async fn remove(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<StatusCode, HubError> {
    let _ = (hub, who, code);
    todo!("tarefa zt-09")
}

pub async fn preview(
    State(hub): State<Arc<Hub>>,
    Auth(who): Auth,
    Path(code): Path<String>,
) -> Result<Json<PreviewResponse>, HubError> {
    let _ = (hub, who, code);
    todo!("tarefa zt-09")
}

pub async fn diagnostics(State(hub): State<Arc<Hub>>, Auth(who): Auth) -> Result<Json<ServerDiagnostics>, HubError> {
    let _ = (hub, who);
    todo!("tarefa zt-09")
}
