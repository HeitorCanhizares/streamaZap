#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 3 Online: protocolo incompatível mostra erro para atualizar.
//!
//! O `zap-test-server` não deixa escolher a faixa de protocolo, então este teste sobe um servidor
//! HTTP falso que responde `GET /v1/info` com `protocolo_min = PROTOCOL_VERSION + 1`.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use zap_core::testutil::TestSink;
use zap_core::{Core, CoreOptions, CreateRoom, ServerState};
use zap_net::online::{QuicInfo, ServerInfo, ServerLimits};
use zap_proto::{RoomMode, PROTOCOL_VERSION};

/// `GET /v1/info` com a faixa de protocolo que não inclui este app.
fn info_falso() -> ServerInfo {
    ServerInfo {
        name: "Falso".into(),
        server_version: "0.0.0".into(),
        protocol_min: PROTOCOL_VERSION + 1,
        protocol_max: PROTOCOL_VERSION + 2,
        region: None,
        requires_password: false,
        relay: true,
        quic: QuicInfo {
            port: 4433,
            alpn: "zap/4".into(),
        },
        limits: ServerLimits {
            participants_per_room: 10,
            video_tracks_per_room: 2,
            max_bitrate_kbps: 10_000,
        },
        now: "2026-11-04T12:00:00Z".into(),
    }
}

/// Servidor HTTP mínimo: qualquer conexão recebe o mesmo `GET /v1/info`. Devolve o endereço e a
/// tarefa de aceitação (para ser abortada no fim do teste).
async fn servidor_falso() -> (SocketAddr, JoinHandle<()>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = ouvinte.local_addr().expect("endereço");
    let corpo = serde_json::to_string(&info_falso()).expect("json");
    let resposta = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let tarefa = tokio::spawn(async move {
        while let Ok((mut conexao, _)) = ouvinte.accept().await {
            let resposta = resposta.clone();
            tokio::spawn(async move {
                // Lê a requisição até a linha em branco final (o corpo do GET é vazio).
                let mut lido = Vec::new();
                let mut bloco = [0u8; 1024];
                while !lido.windows(4).any(|j| j == b"\r\n\r\n") {
                    match conexao.read(&mut bloco).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => lido.extend_from_slice(&bloco[..n]),
                    }
                }
                let _ = conexao.write_all(resposta.as_bytes()).await;
                let _ = conexao.shutdown().await;
            });
        }
    });
    (addr, tarefa)
}

/// Poll a cada 20 ms até `f` devolver true ou estourar o prazo.
async fn espera<F: FnMut() -> bool>(timeout: Duration, mut f: F) -> bool {
    let fim = tokio::time::Instant::now() + timeout;
    loop {
        if f() {
            return true;
        }
        if tokio::time::Instant::now() >= fim {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// `Core` com diretório temporário; `endereco` vai para `settings.server.address` (dispara a
/// sessão Online se `online` for true).
async fn novo_core(nome: &str, online: bool, endereco: Option<String>) -> Core {
    let sink = TestSink::new();
    let dir = std::env::temp_dir().join(format!("zapcore-{}-{}", nome, uuid::Uuid::new_v4()));
    let core = tokio::task::spawn_blocking(move || {
        Core::start_with(
            dir,
            sink,
            CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online,
                ..Default::default()
            },
        )
    })
    .await
    .unwrap()
    .expect("core");
    if let Some(e) = endereco {
        core.update_settings(|s| s.server.address = e);
    }
    core
}

/// Soltar um `Core` dentro do runtime aborta o processo: desliga numa thread bloqueante.
async fn desligar(mut c: Core) {
    tokio::task::spawn_blocking(move || c.shutdown()).await.unwrap();
}

/// Estado de erro do servidor com a mensagem para atualizar e a pista de protocolo mais novo.
fn erro_de_protocolo(c: &Core) -> bool {
    let snap = c.snapshot();
    snap.server
        .as_ref()
        .is_some_and(|s| s.state == ServerState::Error && s.message.as_deref().is_some_and(|m| m.contains("Atualize")))
        && snap.newer_protocol == Some(PROTOCOL_VERSION + 1)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn teste_de_servidor_mostra_protocolo_incompativel() {
    let (addr, srv) = servidor_falso().await;
    let c = novo_core("teste", false, None).await;
    let r = c.server_test(format!("http://{addr}"), String::new()).await;
    desligar(c).await;
    srv.abort();
    assert!(!r.ok, "não deveria passar: {r:?}");
    assert_eq!(r.code.as_deref(), Some("protocolo_incompativel"));
    assert!(r.message.contains("Atualize"), "mensagem: {}", r.message);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sessao_mostra_erro_para_atualizar() {
    let (addr, srv) = servidor_falso().await;
    let c = novo_core("sessao", true, Some(format!("http://{addr}"))).await;
    let ok = espera(Duration::from_secs(5), || erro_de_protocolo(&c)).await;
    let estado = c.snapshot().server.clone();
    let novo = c.snapshot().newer_protocol;
    desligar(c).await;
    srv.abort();
    assert!(ok, "servidor = {estado:?}, newer_protocol = {novo:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn criar_sala_falha_sem_deixar_sala() {
    let (addr, srv) = servidor_falso().await;
    let c = novo_core("sala", true, Some(format!("http://{addr}"))).await;
    let pronto = espera(Duration::from_secs(5), || erro_de_protocolo(&c)).await;
    let r = c
        .create_room(CreateRoom {
            name: "Teste".into(),
            mode: RoomMode::Online,
            password: None,
        })
        .await;
    let sem_sala = c.snapshot().room.is_none();
    desligar(c).await;
    srv.abort();
    assert!(pronto, "a sessão não chegou ao erro de protocolo");
    match r {
        Err(e) => assert!(!e.to_string().is_empty(), "erro sem mensagem"),
        Ok(()) => panic!("criar sala não deveria funcionar com protocolo incompatível"),
    }
    assert!(sem_sala, "a sala ficou guardada mesmo com falha");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn endereco_sem_resposta_da_conexao() {
    // Porta reservada e liberada logo em seguida: nada escuta nela.
    let porta = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("endereço").port()
    };
    let c = novo_core("fechado", false, None).await;
    let r = c.server_test(format!("http://127.0.0.1:{porta}"), String::new()).await;
    desligar(c).await;
    assert!(!r.ok, "não deveria passar: {r:?}");
    assert_eq!(r.code.as_deref(), Some("conexao"), "mensagem: {}", r.message);
}
