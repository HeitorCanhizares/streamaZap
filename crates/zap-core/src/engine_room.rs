//! `impl Core`: salas.

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::engine::{Core, Shared};
use crate::room::RoomEvent;
use crate::types::*;
use crate::CoreError;

impl Core {
    /// Salta para o runtime do núcleo. Já em sala → `AlreadyInRoom`. Dono: `OwnerRoom::start`,
    /// anuncia na descoberta (`settings.local.announce`), `ensure_media_server` +
    /// `set_auth`, `refresh_room_view`, `notifier.flush()`.
    pub async fn create_room(&self, req: CreateRoom) -> Result<(), CoreError> {
        todo!("tarefa core-39")
    }
    /// `Discovered{room_id}`: acha na descoberta (`RoomNotFound` se não); `Address{ip,port}`:
    /// impressão digital desconhecida → consulta a descoberta pelo endereço; sem anúncio →
    /// `RoomNotFound`. Então `MemberRoom::join`, `ensure_media_server`, `set_auth`,
    /// `refresh_room_view`, `flush`.
    pub async fn join_room(&self, target: JoinTarget, password: Option<String>) -> Result<(), CoreError> {
        todo!("tarefa core-39")
    }
    /// Para compartilhamento e assinaturas, `room.leave()`, remove o anúncio, limpa o estado,
    /// `flush`.
    pub async fn leave_room(&self) {
        todo!("tarefa core-39")
    }
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        todo!("tarefa core-39")
    }
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        todo!("tarefa core-39")
    }
}

/// Tarefa que consome `RoomEvent`: `Changed` → `refresh_room_view`; `Chat` → `ui.chat` +
/// `refresh`; `Closed` → limpa a sala como `leave_room` e `push_error` com o motivo;
/// `Reconnected` → `ShareSession::republish` e reassina os tiles; `TrackRemoved(id)` → encerra a
/// assinatura dessa trilha; `ReverseRequest` → `media_server.connect_back`; `Error` → `push_error`.
pub(crate) async fn pump_room_events(shared: Arc<Shared>, mut rx: UnboundedReceiver<RoomEvent>) {
    todo!("tarefa core-40")
}
