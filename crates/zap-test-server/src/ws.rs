//! WebSocket de sinalização `GET /v1/ws?token=…` (`docs/api/rest.md` e `mensagens.md`).

use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{
    sync::mpsc,
    time::{interval, sleep_until, Instant, MissedTickBehavior},
};
use zap_proto::ControlMsg;

use crate::{
    state::{Hub, HubError},
    util::now_unix_ms,
};

/// O servidor manda `ping` a cada 15 s.
const INTERVALO_PING: Duration = Duration::from_secs(15);
/// Sem nenhum frame recebido por 45 s, a conexão é dada como morta.
const PRAZO_SILENCIO: Duration = Duration::from_secs(45);

/// Rota `GET /v1/ws`.
pub fn routes() -> Router<Arc<Hub>> {
    Router::new().route("/v1/ws", get(upgrade))
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
    let id = hub.authenticate(&q.token)?;
    Ok(ws.on_upgrade(move |socket| run_socket(hub, socket, id)))
}

/// Texto de um frame → `ControlMsg` (JSON inválido ou tipo desconhecido = `Err`).
fn parse_text(texto: &str) -> Result<ControlMsg, ()> {
    serde_json::from_str(texto).map_err(|_| ())
}

/// `ControlMsg` → texto do frame (`None` se não serializa, o que não deve acontecer).
fn encode(msg: &ControlMsg) -> Option<String> {
    serde_json::to_string(msg).ok()
}

/// Laço de um WebSocket. Registra o canal de saída com `Hub::ws_connected` (que manda
/// `salas.lista`), manda `ping` a cada 15 s, considera a conexão morta após 45 s sem frames,
/// responde `ping` com `pong` (mesmo `ts_ms`), repassa cada `ControlMsg` de texto a
/// `Hub::handle_ws_message` e, ao terminar, chama `Hub::ws_disconnected`. JSON inválido gera
/// mensagem `erro` com código `invalido` e a conexão segue.
pub async fn run_socket(hub: Arc<Hub>, socket: WebSocket, participant_id: String) {
    let (tx, mut out_rx) = mpsc::unbounded_channel::<ControlMsg>();
    hub.ws_connected(&participant_id, tx.clone());

    let (mut sink, mut stream) = socket.split();
    let mut ping = interval(INTERVALO_PING);
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // O primeiro tick do `interval` é imediato: pula, para o primeiro `ping` sair aos 15 s.
    ping.tick().await;
    let mut prazo = Instant::now() + PRAZO_SILENCIO;

    loop {
        tokio::select! {
            saida = out_rx.recv() => {
                // `None` não acontece enquanto `tx` existe aqui; trata como fim por segurança.
                let Some(msg) = saida else { break };
                let Some(texto) = encode(&msg) else { continue };
                if sink.send(Message::Text(texto.into())).await.is_err() {
                    break;
                }
            }
            entrada = stream.next() => {
                prazo = Instant::now() + PRAZO_SILENCIO;
                match entrada {
                    Some(Ok(Message::Text(t))) => match parse_text(t.as_str()) {
                        Ok(ControlMsg::Ping { ts_ms }) => {
                            let _ = tx.send(ControlMsg::Pong { ts_ms });
                        }
                        Ok(ControlMsg::Pong { .. }) => {}
                        Ok(msg) => hub.handle_ws_message(&participant_id, msg),
                        Err(()) => {
                            let _ = tx.send(ControlMsg::Error {
                                code: "invalido".into(),
                                message: "JSON inválido ou tipo de mensagem desconhecido".into(),
                                reference: None,
                            });
                        }
                    },
                    // Ping/Pong do próprio WebSocket (o axum responde sozinho) e binário: só
                    // contam como atividade.
                    Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Binary(_))) => {}
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                }
            }
            _ = ping.tick() => {
                let _ = tx.send(ControlMsg::Ping { ts_ms: now_unix_ms() });
            }
            _ = sleep_until(prazo) => {
                tracing::debug!(participante = %participant_id, "WebSocket sem tráfego por 45 s: encerrado");
                break;
            }
        }
    }

    hub.ws_disconnected(&participant_id, &tx);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_valido_e_lixo() {
        assert_eq!(
            parse_text(r#"{"t":"ping","ts_ms":7}"#),
            Ok(ControlMsg::Ping { ts_ms: 7 })
        );
        assert!(parse_text("não é json").is_err());
        assert!(parse_text(r#"{"t":"tipo.que.nao.existe"}"#).is_err());
    }

    #[test]
    fn encode_ida_e_volta() {
        let msg = ControlMsg::Pong { ts_ms: 9 };
        let texto = encode(&msg).unwrap();
        assert!(texto.contains("\"pong\""));
        assert_eq!(parse_text(&texto), Ok(msg));
    }
}
