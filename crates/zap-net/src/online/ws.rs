//! WebSocket de sinalização: mensagens `zap_proto::ControlMsg`, ping/pong de 15 s.
use super::error::ApiError;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::{self, Message};
use zap_proto::ControlMsg;

/// Intervalo do `ping` enviado pelo cliente.
pub(crate) const PING_INTERVAL_SECS: u64 = 15;
/// Sem nenhum frame por este tempo = conexão morta.
pub(crate) const DEAD_AFTER_SECS: u64 = 45;

/// Lado de envio.
#[derive(Debug, Clone)]
pub struct WsTx {
    pub(crate) tx: mpsc::Sender<ControlMsg>,
}

/// Lado de recepção. `Ping` do servidor é respondido internamente e não chega aqui;
/// `Pong` também é consumido.
#[derive(Debug)]
pub struct WsRx {
    pub(crate) rx: mpsc::Receiver<Result<ControlMsg, ApiError>>,
}

impl WsTx {
    /// Erro `Network("websocket fechado")` se a conexão caiu.
    pub async fn send(&self, msg: ControlMsg) -> Result<(), ApiError> {
        self.tx
            .send(msg)
            .await
            .map_err(|_| ApiError::Network("websocket fechado".into()))
    }
}

impl WsRx {
    /// Próxima mensagem. `None` = conexão encerrada. `Some(Err(_))` = falha (rede ou JSON
    /// inválido; depois de falha de rede vem `None`).
    pub async fn recv(&mut self) -> Option<Result<ControlMsg, ApiError>> {
        self.rx.recv().await
    }
}

/// Percent-encoding manual: mantém `A-Za-z0-9-._~`, o resto vira `%XX`.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn agora_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Conecta em `ws(s)://<base>/v1/ws?token=<token>` (https→wss, http→ws; token com
/// percent-encoding) e inicia a tarefa de fundo (ping a cada 15 s, pong automático, morte
/// em 45 s sem frames).
pub(crate) async fn connect(base_url: &str, token: &str) -> Result<(WsTx, WsRx), ApiError> {
    connect_with(
        base_url,
        token,
        Duration::from_secs(PING_INTERVAL_SECS),
        Duration::from_secs(DEAD_AFTER_SECS),
    )
    .await
}

/// `connect` com intervalo de ping e limite de morte injetáveis (testes usam tempos curtos).
async fn connect_with(
    base_url: &str,
    token: &str,
    ping_every: Duration,
    dead_after: Duration,
) -> Result<(WsTx, WsRx), ApiError> {
    let base = base_url.trim_end_matches('/');
    let base = if let Some(r) = base.strip_prefix("https://") {
        format!("wss://{r}")
    } else if let Some(r) = base.strip_prefix("http://") {
        format!("ws://{r}")
    } else {
        base.to_string()
    };
    let url = format!("{base}/v1/ws?token={}", percent_encode(token));

    let (stream, _) = match tokio_tungstenite::connect_async(url).await {
        Ok(ok) => ok,
        Err(tungstenite::Error::Http(resp)) => {
            let status = resp.status().as_u16();
            return Err(match resp.body() {
                Some(corpo) if !corpo.is_empty() => ApiError::from_body(status, corpo),
                _ => ApiError::Network(format!("handshake rejeitado: HTTP {status}")),
            });
        }
        Err(e) => return Err(ApiError::Network(e.to_string())),
    };

    let (mut sink, mut stream) = stream.split();
    let (out_tx, mut out_rx) = mpsc::channel::<ControlMsg>(64);
    let (in_tx, in_rx) = mpsc::channel::<Result<ControlMsg, ApiError>>(64);

    tokio::spawn(async move {
        let mut ping = tokio::time::interval_at(Instant::now() + ping_every, ping_every);
        let dead = tokio::time::sleep(dead_after);
        tokio::pin!(dead);
        loop {
            tokio::select! {
                saida = out_rx.recv() => {
                    // Todos os WsTx descartados: encerra.
                    let Some(msg) = saida else { break };
                    let Ok(texto) = serde_json::to_string(&msg) else { continue };
                    if sink.send(Message::Text(texto)).await.is_err() {
                        let _ = in_tx.send(Err(ApiError::Network("websocket fechado".into()))).await;
                        break;
                    }
                }
                _ = ping.tick() => {
                    let Ok(texto) = serde_json::to_string(&ControlMsg::Ping { ts_ms: agora_ms() }) else { continue };
                    if sink.send(Message::Text(texto)).await.is_err() {
                        let _ = in_tx.send(Err(ApiError::Network("websocket fechado".into()))).await;
                        break;
                    }
                }
                _ = &mut dead => {
                    let _ = in_tx.send(Err(ApiError::Network("sem tráfego por 45 s".into()))).await;
                    break;
                }
                frame = stream.next() => {
                    // Qualquer frame prova que a conexão está viva.
                    dead.as_mut().reset(Instant::now() + dead_after);
                    match frame {
                        None | Some(Ok(Message::Close(_))) => break,
                        Some(Err(e)) => {
                            let _ = in_tx.send(Err(ApiError::Network(e.to_string()))).await;
                            break;
                        }
                        Some(Ok(Message::Text(texto))) => match serde_json::from_str::<ControlMsg>(&texto) {
                            Ok(ControlMsg::Ping { ts_ms }) => {
                                let Ok(r) = serde_json::to_string(&ControlMsg::Pong { ts_ms }) else { continue };
                                if sink.send(Message::Text(r)).await.is_err() {
                                    let _ = in_tx.send(Err(ApiError::Network("websocket fechado".into()))).await;
                                    break;
                                }
                            }
                            Ok(ControlMsg::Pong { .. }) => {}
                            Ok(outra) => {
                                if in_tx.send(Ok(outra)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                if in_tx.send(Err(ApiError::Protocol(e.to_string()))).await.is_err() {
                                    break;
                                }
                            }
                        },
                        Some(Ok(Message::Ping(dados))) => {
                            if sink.send(Message::Pong(dados)).await.is_err() {
                                break;
                            }
                        }
                        // Binary, Pong e Frame crus: ignorados.
                        Some(Ok(_)) => {}
                    }
                }
            }
        }
        let _ = sink.close().await;
    });

    Ok((WsTx { tx: out_tx }, WsRx { rx: in_rx }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ws::{Message as AMsg, WebSocket, WebSocketUpgrade};
    use axum::extract::Query;
    use axum::routing::get;
    use axum::Router;
    use std::collections::HashMap;
    use tokio::net::TcpListener;

    /// Sobe um servidor com o handler dado e devolve a base `http://127.0.0.1:porta`.
    async fn servidor<F, Fut>(f: F) -> String
    where
        F: Fn(WebSocket, HashMap<String, String>) -> Fut + Clone + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let app = Router::new().route(
            "/v1/ws",
            get(move |ws: WebSocketUpgrade, Query(q): Query<HashMap<String, String>>| {
                let f = f.clone();
                async move { ws.on_upgrade(move |s| f(s, q)) }
            }),
        );
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(l, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn texto(s: &str) -> AMsg {
        AMsg::Text(s.into())
    }

    #[tokio::test]
    async fn recebe_mensagem_do_servidor() {
        let base = servidor(|mut s, _| async move {
            s.send(texto(r#"{"t":"salas.lista","salas":[]}"#)).await.unwrap();
            let _ = s.recv().await;
        })
        .await;
        let (_tx, mut rx) = connect(&base, "tok").await.unwrap();
        assert_eq!(rx.recv().await, Some(Ok(ControlMsg::RoomList { salas: vec![] })));
    }

    #[tokio::test]
    async fn envia_mensagem_json_correto() {
        let base = servidor(|mut s, _| async move {
            while let Some(Ok(AMsg::Text(t))) = s.recv().await {
                // Ecoa como Text; o cliente reinterpreta como ControlMsg.
                s.send(AMsg::Text(t)).await.unwrap();
            }
        })
        .await;
        let (tx, mut rx) = connect(&base, "tok").await.unwrap();
        tx.send(ControlMsg::RoomJoin { room_id: "abc".into() }).await.unwrap();
        // O eco volta como ControlMsg; conferimos também o JSON bruto.
        let got = rx.recv().await.unwrap().unwrap();
        assert_eq!(got, ControlMsg::RoomJoin { room_id: "abc".into() });
        let v = serde_json::to_value(&got).unwrap();
        assert_eq!(v["t"], "sala.entrar");
    }

    #[tokio::test]
    async fn ping_do_servidor_respondido_com_pong_e_nao_vaza() {
        let base = servidor(|mut s, _| async move {
            s.send(texto(r#"{"t":"ping","ts_ms":77}"#)).await.unwrap();
            // Espera o pong e só então manda uma mensagem comum.
            if let Some(Ok(AMsg::Text(t))) = s.recv().await {
                assert!(t.contains("pong") && t.contains("77"), "{t}");
                s.send(texto(r#"{"t":"salas.lista","salas":[]}"#)).await.unwrap();
            }
            let _ = s.recv().await;
        })
        .await;
        let (_tx, mut rx) = connect(&base, "tok").await.unwrap();
        // A primeira mensagem repassada é a lista: o ping não vazou.
        assert_eq!(rx.recv().await, Some(Ok(ControlMsg::RoomList { salas: vec![] })));
    }

    #[tokio::test]
    async fn cliente_manda_ping_a_cada_15s() {
        let (ptx, mut prx) = mpsc::unbounded_channel::<String>();
        let base = servidor(move |mut s, _| {
            let ptx = ptx.clone();
            async move {
                while let Some(Ok(m)) = s.recv().await {
                    if let AMsg::Text(t) = m {
                        let _ = ptx.send(t.to_string());
                    }
                }
            }
        })
        .await;
        // Auditoria (net-12): decisão (b). Havia E/S real com o tempo pausado; agora tempo real
        // com ping de 100 ms (proporção de 15 s) e morte longa para não interferir.
        let (_tx, _rx) = connect_with(&base, "tok", Duration::from_millis(100), Duration::from_secs(30))
            .await
            .unwrap();
        for _ in 0..2 {
            let t = tokio::time::timeout(Duration::from_secs(10), prx.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(t.contains(r#""t":"ping""#), "{t}");
        }
    }

    #[tokio::test]
    async fn morre_apos_45s_sem_trafego() {
        let base = servidor(|mut s, _| async move { while s.recv().await.is_some() {} }).await;
        // Auditoria (net-12): decisão (b). Tempo real com morte em 400 ms (proporção de 45 s)
        // em vez de relógio pausado com socket real.
        let (tx, mut rx) = connect_with(&base, "tok", Duration::from_secs(30), Duration::from_millis(400))
            .await
            .unwrap();
        assert!(matches!(rx.recv().await, Some(Err(ApiError::Network(_)))));
        assert!(rx.recv().await.is_none());
        assert_eq!(
            tx.send(ControlMsg::Ping { ts_ms: 1 }).await,
            Err(ApiError::Network("websocket fechado".into()))
        );
    }

    #[tokio::test]
    async fn json_invalido_vira_protocol_e_continua() {
        let base = servidor(|mut s, _| async move {
            s.send(texto("isto não é json")).await.unwrap();
            s.send(texto(r#"{"t":"salas.lista","salas":[]}"#)).await.unwrap();
            let _ = s.recv().await;
        })
        .await;
        let (_tx, mut rx) = connect(&base, "tok").await.unwrap();
        assert!(matches!(rx.recv().await, Some(Err(ApiError::Protocol(_)))));
        assert_eq!(rx.recv().await, Some(Ok(ControlMsg::RoomList { salas: vec![] })));
    }

    #[tokio::test]
    async fn token_com_caracteres_especiais_codificado() {
        assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
        assert_eq!(percent_encode("a b&c=é"), "a%20b%26c%3D%C3%A9");
        let (ttx, mut trx) = mpsc::unbounded_channel::<String>();
        let base = servidor(move |mut s, q| {
            let ttx = ttx.clone();
            async move {
                let _ = ttx.send(q.get("token").cloned().unwrap_or_default());
                let _ = s.recv().await;
            }
        })
        .await;
        let (_tx, _rx) = connect(&base, "a b&c=é").await.unwrap();
        assert_eq!(trx.recv().await.unwrap(), "a b&c=é");
    }

    #[tokio::test]
    async fn send_apos_fechar_devolve_erro() {
        let base = servidor(|mut s, _| async move {
            let _ = s.close().await;
        })
        .await;
        let (tx, mut rx) = connect(&base, "tok").await.unwrap();
        assert!(rx.recv().await.is_none());
        assert_eq!(
            tx.send(ControlMsg::Ping { ts_ms: 1 }).await,
            Err(ApiError::Network("websocket fechado".into()))
        );
    }
}
