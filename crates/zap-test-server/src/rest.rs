//! REST (`docs/api/rest.md`): roteador, autenticação, erros, `info` e sessões.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{rejection::JsonRejection, ConnectInfo, FromRequestParts, State},
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use zap_net::online::{LoginRequest, QuicInfo, ServerInfo, ServerLimits, SessionResponse};

use crate::{
    state::{Hub, HubError},
    util::now_iso,
};

/// Protocolo aceito (`protocolo_min` = `protocolo_max`).
const PROTOCOLO: u32 = 4;
/// Limites anunciados em `GET /v1/info` (o servidor de teste não os impõe, exceto participantes).
const TRILHAS_VIDEO_POR_SALA: u32 = 8;
const BITRATE_MAX_KBPS: u32 = 12_000;

/// Resposta de erro: status HTTP + `{"erro": codigo, "mensagem": texto}`.
/// Para `protocolo_incompativel` (426) acrescenta `protocolo_min` e `protocolo_max`.
impl IntoResponse for HubError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut corpo = json!({ "erro": self.code, "mensagem": self.message });
        if self.code == "protocolo_incompativel" {
            corpo["protocolo_min"] = json!(PROTOCOLO);
            corpo["protocolo_max"] = json!(PROTOCOLO);
        }
        (status, Json(corpo)).into_response()
    }
}

/// Corpo JSON fora do formato vira `400 invalido` com o corpo de erro do contrato (e não o texto
/// puro que o axum devolveria), para o cliente receber um `ApiError::Http` como em qualquer erro.
pub(crate) fn corpo_invalido(rejeicao: JsonRejection) -> HubError {
    HubError::new(400, "invalido", rejeicao.body_text())
}

/// Token de `Authorization: Bearer <token>`; ausente ou em outro esquema = `nao_autenticado`.
fn bearer(headers: &HeaderMap) -> Result<String, HubError> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .ok_or_else(|| HubError::new(401, "nao_autenticado", "Token ausente"))
}

/// Extrator de autenticação: lê `Authorization: Bearer <token>` e devolve o id do participante.
/// Falha com `nao_autenticado` (401).
pub struct Auth(pub String);

impl FromRequestParts<Arc<Hub>> for Auth {
    type Rejection = HubError;

    async fn from_request_parts(parts: &mut Parts, hub: &Arc<Hub>) -> Result<Self, Self::Rejection> {
        let HeaderToken(token) = HeaderToken::from_request_parts(parts, hub).await?;
        hub.authenticate(&token).map(Auth)
    }
}

/// Todas as rotas `/v1/...`: `info`, `sessoes`, `sessoes/renovar`, as de `rest_rooms::routes()`
/// e a de `ws::routes()`. O estado é `Arc<Hub>`.
pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/v1/info", get(info))
        .route("/v1/sessoes", post(login))
        .route("/v1/sessoes/renovar", post(renew))
        .merge(crate::rest_rooms::routes())
        .merge(crate::ws::routes())
        .fallback(nao_encontrado)
        .with_state(hub)
}

/// Caminho inexistente: `404 {"erro": "nao_encontrado"}` (como o servidor de produção).
async fn nao_encontrado() -> HubError {
    HubError::new(404, "nao_encontrado", "Caminho inexistente")
}

/// Porta do texto `host:porta` (0 se não houver).
fn porta_de(endereco: Option<&String>) -> u16 {
    endereco
        .and_then(|a| a.rsplit_once(':'))
        .and_then(|(_, p)| p.parse().ok())
        .unwrap_or(0)
}

/// `GET /v1/info`: `protocolo_min = protocolo_max = 4`, `requer_senha` pela config, `ponte` pela
/// config, `quic.porta` = porta de `Hub::bridge_addr` (0 se ainda não definido), `alpn: "zap/4"`,
/// limites (`participantes_por_sala` da config, 8 trilhas, 12000 kbps), `agora` ISO.
pub async fn info(State(hub): State<Arc<Hub>>) -> Json<ServerInfo> {
    let config = &hub.config;
    Json(ServerInfo {
        name: config.name.clone(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_min: PROTOCOLO,
        protocol_max: PROTOCOLO,
        region: Some("local".to_string()),
        requires_password: config.server_password.is_some(),
        requires_account: false,
        account: None,
        relay: config.relay,
        quic: QuicInfo {
            port: porta_de(hub.bridge_addr.get()),
            alpn: "zap/4".to_string(),
        },
        limits: ServerLimits {
            participants_per_room: config.participants_per_room,
            video_tracks_per_room: TRILHAS_VIDEO_POR_SALA,
            max_bitrate_kbps: BITRATE_MAX_KBPS,
        },
        now: now_iso(),
    })
}

/// `POST /v1/sessoes`: `201` com `SessionResponse`. IP remoto vem de `ConnectInfo`.
pub async fn login(
    State(hub): State<Arc<Hub>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    corpo: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SessionResponse>), HubError> {
    let Json(req) = corpo.map_err(corpo_invalido)?;
    let sessao = hub.login(req, &addr.ip().to_string())?;
    Ok((StatusCode::CREATED, Json(sessao)))
}

/// `POST /v1/sessoes/renovar` (autenticado): `200` com token novo.
pub async fn renew(State(hub): State<Arc<Hub>>, parts: HeaderToken) -> Result<Json<SessionResponse>, HubError> {
    let HeaderToken(token) = parts;
    hub.renew(&token).map(Json)
}

/// Token bruto do cabeçalho `Authorization: Bearer` (a renovação precisa do token, não do id).
pub struct HeaderToken(pub String);

impl FromRequestParts<Arc<Hub>> for HeaderToken {
    type Rejection = HubError;

    async fn from_request_parts(parts: &mut Parts, _hub: &Arc<Hub>) -> Result<Self, Self::Rejection> {
        bearer(&parts.headers).map(HeaderToken)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Config;
    use axum::http::HeaderValue;
    use serde_json::Value;

    async fn corpo_json(resp: Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn erro_json() {
        let resp = HubError::new(404, "sala_nao_encontrada", "x").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let v = corpo_json(resp).await;
        assert_eq!(v["erro"], "sala_nao_encontrada");
        assert_eq!(v["mensagem"], "x");
        assert!(v.get("protocolo_min").is_none());
    }

    #[tokio::test]
    async fn erro_protocolo_traz_min_max() {
        let resp = HubError::new(426, "protocolo_incompativel", "atualize").into_response();
        assert_eq!(resp.status().as_u16(), 426);
        let v = corpo_json(resp).await;
        assert_eq!(v["protocolo_min"], 4);
        assert_eq!(v["protocolo_max"], 4);
    }

    #[tokio::test]
    async fn status_desconhecido_vira_500() {
        let resp = HubError::new(1000, "interno", "x").into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn info_handler() {
        let hub = Arc::new(Hub::new(Config::default()));
        let Json(i) = info(State(hub.clone())).await;
        assert_eq!((i.protocol_min, i.protocol_max), (4, 4));
        assert!(i.relay);
        assert_eq!(i.quic.alpn, "zap/4");
        assert_eq!(i.quic.port, 0, "sem ponte ainda: porta 0");
        assert!(!i.requires_password);
        assert_eq!(i.limits.participants_per_room, 16);

        hub.bridge_addr.set("127.0.0.1:47999".into()).unwrap();
        let Json(i) = info(State(hub)).await;
        assert_eq!(i.quic.port, 47999);
    }

    #[test]
    fn bearer_formatos() {
        let mut h = HeaderMap::new();
        assert_eq!(bearer(&h).unwrap_err().code, "nao_autenticado");
        h.insert("authorization", HeaderValue::from_static("Basic x"));
        assert_eq!(bearer(&h).unwrap_err().code, "nao_autenticado");
        h.insert("authorization", HeaderValue::from_static("Bearer "));
        assert_eq!(bearer(&h).unwrap_err().code, "nao_autenticado");
        h.insert("authorization", HeaderValue::from_static("Bearer abc"));
        assert_eq!(bearer(&h).unwrap(), "abc");
    }

    #[test]
    fn porta_do_endereco() {
        assert_eq!(porta_de(None), 0);
        assert_eq!(porta_de(Some(&"127.0.0.1:7443".to_string())), 7443);
        assert_eq!(porta_de(Some(&"sem-porta".to_string())), 0);
    }
}
