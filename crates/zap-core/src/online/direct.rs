//! Rota direta (`p2p.*`): tenta ligar espectador e publicador sem passar pela ponte.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use zap_proto::control::{P2pRole, Route};
use zap_proto::MediaAddress;

use super::room::OnlineRoom;
use crate::CoreError;

/// Tempo da tentativa direta (docs/api/mensagens.md, "Rota direta").
pub const DIRECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Default)]
pub struct DirectState {
    /// Rota em uso por trilha assistida (ausente = ponte).
    pub(crate) routes: Mutex<HashMap<String, Route>>,
    /// Trilhas para as quais já mandei `midia.direta_ok` (visível aos testes).
    pub(crate) ok_sent: Mutex<std::collections::HashSet<String>>,
    /// Trilhas com tentativa direta em andamento (chave: `"<trilha>|<com>"`).
    pub(crate) trying: Mutex<std::collections::HashSet<String>>,
}

impl DirectState {
    pub fn route_of(&self, track_id: &str) -> Route {
        self.routes.lock().get(track_id).copied().unwrap_or(Route::Relay)
    }
}

/// Depois de entrar (e a cada reconexão): `P2pMyAddresses{addresses: "ip:porta" do
/// `MediaServer::address` (interfaces locais; `include_loopback` das opções), fingerprint}`
/// pelo WebSocket. Só se `media.direct_allowed`.
pub(crate) async fn send_my_addresses(room: &Arc<OnlineRoom>) -> Result<(), CoreError> {
    let _ = room;
    todo!("tarefa core-online-13")
}

/// `p2p.candidatos`:
/// - `Client` (eu assisto): `PubConn::connect_direct(shared, addresses, fingerprint,
///   ticket = make_ticket(media_secret, my_id), room_id, router da ponte)` (3 s, todos os
///   endereços em paralelo). Sucesso: `bridge.direct.insert(track_id, conn_direta)`,
///   `conn_direta.subscribe(...)` com os parâmetros da assinatura atual, `cancelar` da trilha
///   no servidor (WebSocket), `routes[track] = Direct`, `ok_sent.insert(track)` e `DirectOk`
///   pelo WebSocket. Falha: `DirectFailed{track_id}` e segue pela ponte. Uma tentativa por vez
///   por `"<trilha>|<com>"` (`trying`). Só se `media.direct_allowed` e a trilha é assistida;
///   a direta que cair some de `bridge.direct` e a trilha volta para a ponte (`assinar` de novo);
/// - `Server` (eu publico): para cada endereço, `MediaServer::endpoint.connect(addr, "zap")` e
///   solta (só para abrir o NAT; o espectador é quem conecta de verdade).
pub(crate) fn on_candidates(
    room: &Arc<OnlineRoom>,
    track_id: String,
    with: String,
    role: P2pRole,
    addresses: Vec<String>,
    fingerprint: String,
) {
    let _ = (room, track_id, with, role, addresses, fingerprint);
    todo!("tarefa core-online-13")
}

/// `midia.rota`: guarda a rota (`ponte`/`direta`) em `routes`. Com `direta` e `address`, o
/// servidor está mandando usar a rota direta já negociada: não faz nada além de guardar.
pub(crate) fn on_media_route(room: &Arc<OnlineRoom>, track_id: &str, route: Route, address: Option<MediaAddress>) {
    let _ = (room, track_id, route, address);
    todo!("tarefa core-online-13")
}
