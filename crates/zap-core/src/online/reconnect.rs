//! Reconexão do WebSocket e da ponte durante uma sala Online.

use std::sync::Arc;

use super::room::OnlineRoom;
use crate::CoreError;

/// Pede um ingresso novo (`client.join`), conecta a ponte (`bridge::connect_bridge`), monta a
/// `PubConn` da ponte (`PubConn::connect_online`) e a guarda em `room.bridge` e no `ConnPool`;
/// `publish.reattach_all` (ou `attach` das trilhas guardadas) na conexão nova. Usada na entrada
/// e na reconexão. Sem `use_relay` ou sem `media.bridge`: `Ok(())` sem fazer nada.
pub(crate) async fn connect_bridge_for(room: &Arc<OnlineRoom>) -> Result<(), CoreError> {
    let _ = room;
    todo!("tarefa core-online-11")
}

/// Tarefa da sala: espera `session.epoch_rx()` mudar (WebSocket voltou depois de cair).
/// Então: `ws.send(RoomJoin{room_id})`, espera `RoomState` pelo canal da sala (`set_route` com
/// canal novo se o antigo fechou) e substitui o modelo (`replace_state`, mantém o chat);
/// `RoomEvent::Reconnected`; manda `p2p.meus_enderecos` de novo; reassina todas as trilhas
/// assistidas (`Subscription::update` de cada tile). Sala inexistente/encerrada
/// (`sala_nao_encontrada`, `sala.encerrada`) → `RoomEvent::Closed`. Sai se `room.left`.
pub(crate) async fn watch_ws(room: Arc<OnlineRoom>) {
    let _ = room;
    todo!("tarefa core-online-11")
}

/// Tarefa da sala: espera `PubConn::conn.closed()` da ponte; se não foi `leave`, mostra
/// `RoomEvent::Reconnecting`, e tenta `connect_bridge_for` com `Backoff` (1, 2, 4, 8, 15 s) até
/// conseguir; ao voltar, reassina as trilhas assistidas e emite `RoomEvent::Reconnected`.
pub(crate) async fn watch_bridge(room: Arc<OnlineRoom>) {
    let _ = room;
    todo!("tarefa core-online-11")
}
