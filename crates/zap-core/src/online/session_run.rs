//! Laço de fundo da sessão Online: login, ping, leitura do WebSocket e reconexão.

use std::sync::Arc;

use crate::engine::Shared;

/// Roda até `shared.stop` ou a tarefa ser abortada (`OnlineSession::restart/stop`).
/// A cada volta:
/// 1. endereço vazio → `session.status = None`, `state.set_server(None)`, `notifier.touch()` e
///    espera `session.wake` (sem sondar);
/// 2. senão status `connecting`; `session.connect(&shared)`; erro → status `error(mensagem)`
///    (+ `push_error` só na primeira falha seguida), espera `Backoff::next_delay` (ou `wake`)
///    e tenta de novo; sucesso → `Backoff::reset`, `epoch += 1`, status `online(nome, ping)`;
/// 3. com o WebSocket aberto: lê `WsRx::recv` e despacha — `RoomList{salas}` →
///    `convert::parse_ws_rooms` → `session.rooms` e `state.set_online_rooms(listings
///    convertidas com listing_to_ui)` + `touch`; `Error` → `push_error`; demais mensagens →
///    `session.route` (`try_send`; cheio = descarta com `tracing::warn!`);
///    em paralelo, a cada 5 s `measure_ping` atualiza o status (`state.set_server`, `touch`);
/// 4. `recv` devolver `None` (ou erro): status `connecting`, zera `ws`, volta ao passo 2.
/// Toda mudança de status faz `shared.state.lock().set_server(status.clone())`.
pub(crate) async fn run(shared: Arc<Shared>) {
    let _ = shared;
    todo!("tarefa core-online-06")
}
