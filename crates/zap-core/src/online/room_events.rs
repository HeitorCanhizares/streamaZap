//! Tarefa que consome as mensagens do WebSocket destinadas à sala ativa.

use std::sync::Arc;

use tokio::sync::mpsc;
use zap_proto::ControlMsg;

use super::room::OnlineRoom;

/// Para cada mensagem de `rx` (até o canal fechar ou `room.left`):
/// 1. `TrackPublished{track, local_id: Some(l)}` com `l` em `room.pending` → completa o
///    `oneshot` (antes de aplicar);
/// 2. `ws_apply::apply_ws_msg(&mut model, msg.clone())` → cada `RoomEvent` vai para
///    `room.events` (`Closed` também marca `room.left`);
/// 3. mensagens de mídia: `LayersRequested` → `room.publish.on_layers_requested`;
///    `KeyframeRequest{track_id, layer: Some(l)}` → `room.publish.on_keyframe_request`;
///    `MediaRoute` → `direct::on_media_route`; `P2pCandidates` → `direct::on_candidates`;
///    `LayerCurrent` → guarda em `PubConn::last_layer` da ponte e atualiza o motivo do tile
///    (`TileMetrics::set_reason` da assinatura da trilha, se houver);
/// 4. `Error{code,message}` → `RoomEvent::Error(message)`.
/// Depois de aplicar `ParticipantLeft` de quem eu assistia, nada especial (o `Core` limpa pelo
/// `RoomEvent::TrackRemoved`).
pub(crate) async fn run(room: Arc<OnlineRoom>, rx: mpsc::Receiver<ControlMsg>) {
    let _ = (room, rx);
    todo!("tarefa core-online-10")
}
