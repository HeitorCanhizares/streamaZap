#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Teste de integração de `ServerClient::info` contra um servidor axum local (`docs/api/rest.md`).
use axum::{http::StatusCode, routing::get, Router};
use tokio::net::TcpListener;
use zap_net::online::{ApiError, ServerClient};

/// Corpo de `GET /v1/info` do exemplo em `docs/api/rest.md`.
const INFO: &str = r#"{
  "nome": "Servidor do Heitor",
  "versao_servidor": "1.0.0",
  "protocolo_min": 4,
  "protocolo_max": 4,
  "regiao": "sa-east",
  "requer_senha": true,
  "ponte": true,
  "quic": { "porta": 7443, "alpn": "zap/4" },
  "limites": { "participantes_por_sala": 16, "trilhas_video_por_sala": 8, "bitrate_max_kbps": 12000 },
  "agora": "2026-11-04T21:30:00Z"
}"#;

/// Sobe o app em 127.0.0.1 com porta livre e devolve a base `http://127.0.0.1:porta`.
async fn sobe(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn servir_info() -> &'static str {
    INFO
}

async fn info_recusado() -> (StatusCode, &'static str) {
    (StatusCode::FORBIDDEN, r#"{"erro":"senha_sala","mensagem":"x"}"#)
}

#[tokio::test]
async fn info_ok() {
    let base = sobe(Router::new().route("/v1/info", get(servir_info))).await;
    // Barra final na base não pode quebrar a URL.
    let info = ServerClient::info(&format!("{base}/")).await.unwrap();
    assert_eq!(info.name, "Servidor do Heitor");
    assert_eq!(info.server_version, "1.0.0");
    assert_eq!((info.protocol_min, info.protocol_max), (4, 4));
    assert_eq!(info.region.as_deref(), Some("sa-east"));
    assert!(info.requires_password);
    assert!(info.relay);
    assert_eq!(info.quic.port, 7443);
    assert_eq!(info.quic.alpn, "zap/4");
    assert_eq!(info.limits.participants_per_room, 16);
    assert_eq!(info.now, "2026-11-04T21:30:00Z");
}

#[tokio::test]
async fn info_erro_http_mapeia_codigo() {
    let base = sobe(Router::new().route("/v1/info", get(info_recusado))).await;
    let r = ServerClient::info(&base).await;
    assert_eq!(
        r.err(),
        Some(ApiError::Http {
            status: 403,
            code: "senha_sala".into(),
            message: "x".into(),
        })
    );
}

#[tokio::test]
async fn info_servidor_fora_do_ar_e_network() {
    // Pega uma porta livre e a solta: nada escuta nela, então a conexão é recusada.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let r = ServerClient::info(&format!("http://{addr}")).await;
    assert!(matches!(r, Err(ApiError::Network(_))), "esperava Network, veio {r:?}");
}
