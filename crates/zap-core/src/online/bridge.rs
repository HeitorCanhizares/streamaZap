//! Conexão QUIC `zap/4` com a ponte do servidor (docs/api/midia.md).

use std::sync::Arc;

use zap_net::control::{ControlRx, ControlTx};
use zap_net::handshake::Welcome;

use crate::engine::Shared;
use crate::CoreError;

/// Conexão já autenticada (depois do `bem_vindo`).
pub struct BridgeConnected {
    pub conn: quinn::Connection,
    pub ctrl: ControlTx,
    pub crx: ControlRx,
    pub welcome: Welcome,
}

/// Conecta na ponte e faz o aperto de mão:
/// - `address` = `"host:porta"` (ou `options.dev_bridge_addr` se definido); resolve com
///   `tokio::net::lookup_host` (primeiro endereço); nome do servidor TLS = parte `host`
///   (ou `"zap"` se o host for um IP);
/// - `zap_net::endpoint::client_endpoint()` + `zap_net::tls::client_config(
///   options.dev_cert_fp.as_deref())`; timeout de 10 s para conectar;
/// - `connect_handshake(&conn, Hello{app_version, install_id, name, color (das settings),
///   ticket: Some(ingresso), room_id: Some(room_id), preview: preview.then_some(true)}, None)`;
/// - o `Endpoint` precisa viver tanto quanto a conexão (guarde-o numa tarefa que espera
///   `conn.closed()`).
/// Erros: `NetError::Rejected(codigo)` → `CoreError::Server{code: codigo, message: <texto em
/// português; `nao_autenticado` → "Ingresso recusado pelo servidor", `protocolo_incompativel`
/// → "Atualize o StreamaZap para entrar neste servidor", outros → o próprio código>}`;
/// tempo esgotado/rede → `CoreError::Connect`.
pub async fn connect_bridge(
    shared: &Arc<Shared>,
    address: &str,
    ticket: &str,
    room_id: &str,
    preview: bool,
) -> Result<BridgeConnected, CoreError> {
    let _ = (shared, address, ticket, room_id, preview);
    todo!("tarefa core-online-07")
}
