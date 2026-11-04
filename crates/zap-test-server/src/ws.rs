//! WebSocket de sinalização `GET /v1/ws?token=…` (`docs/api/rest.md` e `mensagens.md`).

use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, Query, State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use serde::Deserialize;

use crate::state::{Hub, HubError};

/// Rota `GET /v1/ws`.
pub fn routes() -> Router<Arc<Hub>> {
    let _ = get(upgrade);
    todo!("tarefa zt-13")
}

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// Valida o token (`Hub::authenticate`; erro = 401 `nao_autenticado` antes do upgrade) e faz o
/// upgrade chamando `run_socket`.
pub async fn upgrade(
    State(hub): State<Arc<Hub>>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, HubError> {
    let _ = (hub, q, ws);
    todo!("tarefa zt-13")
}

/// Laço de um WebSocket. Registra o canal de saída com `Hub::ws_connected` (que manda
/// `salas.lista`), manda `ping` a cada 15 s, considera a conexão morta após 45 s sem frames,
/// responde `ping` com `pong` (mesmo `ts_ms`), repassa cada `ControlMsg` de texto a
/// `Hub::handle_ws_message` e, ao terminar, chama `Hub::ws_disconnected`. JSON inválido gera
/// mensagem `erro` com código `invalido` e a conexão segue.
pub async fn run_socket(hub: Arc<Hub>, socket: WebSocket, participant_id: String) {
    let _ = (hub, socket, participant_id);
    todo!("tarefa zt-13")
}
