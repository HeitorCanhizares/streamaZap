//! REST (`docs/api/rest.md`): roteador, autenticação, erros, `info` e sessões.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, FromRequestParts, State},
    http::request::Parts,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use zap_net::online::{LoginRequest, ServerInfo, SessionResponse};

use crate::state::{Hub, HubError};

/// Resposta de erro: status HTTP + `{"erro": codigo, "mensagem": texto}`.
/// Para `protocolo_incompativel` (426) acrescenta `protocolo_min` e `protocolo_max`.
impl IntoResponse for HubError {
    fn into_response(self) -> Response {
        todo!("tarefa zt-08")
    }
}

/// Extrator de autenticação: lê `Authorization: Bearer <token>` e devolve o id do participante.
/// Falha com `nao_autenticado` (401).
pub struct Auth(pub String);

impl FromRequestParts<Arc<Hub>> for Auth {
    type Rejection = HubError;

    async fn from_request_parts(parts: &mut Parts, hub: &Arc<Hub>) -> Result<Self, Self::Rejection> {
        let _ = (parts, hub);
        todo!("tarefa zt-08")
    }
}

/// Todas as rotas `/v1/...`: `info`, `sessoes`, `sessoes/renovar`, as de `rest_rooms::routes()`
/// e a de `ws::routes()`. O estado é `Arc<Hub>`.
pub fn router(hub: Arc<Hub>) -> Router {
    let _ = (get(info), post(login), post(renew));
    let _ = hub;
    todo!("tarefa zt-08")
}

/// `GET /v1/info`: `protocolo_min = protocolo_max = 4`, `requer_senha` pela config, `ponte` pela
/// config, `quic.porta` = porta de `Hub::bridge_addr` (0 se ainda não definido), `alpn: "zap/4"`,
/// limites (`participantes_por_sala` da config, 8 trilhas, 12000 kbps), `agora` ISO.
pub async fn info(State(hub): State<Arc<Hub>>) -> Json<ServerInfo> {
    let _ = hub;
    todo!("tarefa zt-08")
}

/// `POST /v1/sessoes`: `201` com `SessionResponse`. IP remoto vem de `ConnectInfo`.
pub async fn login(
    State(hub): State<Arc<Hub>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<(axum::http::StatusCode, Json<SessionResponse>), HubError> {
    let _ = (hub, addr, req);
    todo!("tarefa zt-08")
}

/// `POST /v1/sessoes/renovar` (autenticado): `200` com token novo.
pub async fn renew(State(hub): State<Arc<Hub>>, parts: HeaderToken) -> Result<Json<SessionResponse>, HubError> {
    let _ = (hub, parts);
    todo!("tarefa zt-08")
}

/// Token bruto do cabeçalho `Authorization: Bearer` (a renovação precisa do token, não do id).
pub struct HeaderToken(pub String);

impl FromRequestParts<Arc<Hub>> for HeaderToken {
    type Rejection = HubError;

    async fn from_request_parts(parts: &mut Parts, hub: &Arc<Hub>) -> Result<Self, Self::Rejection> {
        let _ = (parts, hub);
        todo!("tarefa zt-08")
    }
}
