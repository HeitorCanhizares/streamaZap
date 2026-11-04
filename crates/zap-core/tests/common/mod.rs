#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Harness compartilhado dos testes Online: servidor de teste, Cores e proxy UDP com banda limitada.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use zap_core::testutil::TestSink;
use zap_core::{Core, CoreOptions, ServerState};
use zap_test_server::{Config, Handle};

/// Servidor de teste com a configuração padrão.
pub async fn servidor() -> Handle {
    servidor_com(Config::default()).await
}

pub async fn servidor_com(config: Config) -> Handle {
    zap_test_server::spawn(config).await.expect("servidor de teste")
}

/// Poll a cada 20 ms até `f` devolver true ou estourar o prazo.
pub async fn espera<F: FnMut() -> bool>(timeout: Duration, mut f: F) -> bool {
    let fim = Instant::now() + timeout;
    loop {
        if f() {
            return true;
        }
        if Instant::now() >= fim {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub async fn desligar(mut c: Core) {
    tokio::task::spawn_blocking(move || c.shutdown()).await.unwrap();
}

async fn criar(nome: &str, srv: &Handle, use_relay: bool, bridge: Option<SocketAddr>) -> (Core, Arc<TestSink>) {
    let sink = TestSink::new();
    let dir = std::env::temp_dir().join(format!("zapcore-{}-{}", nome, uuid::Uuid::new_v4()));
    let s = sink.clone();
    let fp = srv.cert_fingerprint.clone();
    let core = tokio::task::spawn_blocking(move || {
        Core::start_with(
            dir,
            s,
            CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: true,
                dev_cert_fp: Some(fp),
                dev_bridge_addr: bridge.map(|a| a.to_string()),
                ..Default::default()
            },
        )
    })
    .await
    .unwrap()
    .expect("core");
    let nome = nome.to_string();
    let url = srv.base_url.clone();
    core.update_settings(|s| {
        s.name = nome;
        s.server.address = url;
        s.server.use_relay = use_relay;
    });
    let ok = espera(Duration::from_secs(5), || {
        core.snapshot()
            .server
            .as_ref()
            .is_some_and(|s| s.state == ServerState::Online)
    })
    .await;
    assert!(ok, "servidor não ficou Online: {:?}", core.snapshot().server);
    (core, sink)
}

/// Core ligado ao servidor de teste e já `Online`.
pub async fn core_online(nome: &str, srv: &Handle, use_relay: bool) -> (Core, Arc<TestSink>) {
    criar(nome, srv, use_relay, None).await
}

/// Igual a `core_online`, mas a ponte QUIC é alcançada por `bridge_addr` (ex.: um `LinkProxy`).
pub async fn core_online_via(nome: &str, srv: &Handle, bridge_addr: SocketAddr) -> (Core, Arc<TestSink>) {
    criar(nome, srv, true, Some(bridge_addr)).await
}

/// Proxy UDP: cliente->alvo sem limite; alvo->cliente por balde de fichas com tail drop.
pub struct LinkProxy {
    pub addr: SocketAddr,
    enviados: Arc<AtomicU64>,
    descartados: Arc<AtomicU64>,
    tarefas: Vec<JoinHandle<()>>,
}

impl LinkProxy {
    pub async fn start(target: SocketAddr, down_kbps: u32, queue_ms: u32) -> LinkProxy {
        let sock = Arc::new(UdpSocket::bind("127.0.0.1:0").await.expect("bind proxy"));
        let addr = sock.local_addr().expect("addr proxy");
        let enviados = Arc::new(AtomicU64::new(0));
        let descartados = Arc::new(AtomicU64::new(0));
        // Fila de saída em ordem: (instante de saída, bytes, destino).
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(Instant, Vec<u8>, SocketAddr)>();

        let s_saida = sock.clone();
        let env = enviados.clone();
        let saida = tokio::spawn(async move {
            while let Some((quando, dados, cliente)) = rx.recv().await {
                tokio::time::sleep_until(quando).await;
                if s_saida.send_to(&dados, cliente).await.is_ok() {
                    env.fetch_add(1, Ordering::Relaxed);
                }
            }
        });

        let s_ent = sock.clone();
        let desc = descartados.clone();
        let entrada = tokio::spawn(async move {
            let taxa = (down_kbps as f64 * 125.0).max(1.0); // bytes/s
            let fila_max = Duration::from_millis(queue_ms as u64);
            let mut cliente: Option<SocketAddr> = None;
            let mut ultimo_fim = Instant::now();
            let mut buf = vec![0u8; 65536];
            loop {
                let Ok((n, de)) = s_ent.recv_from(&mut buf).await else {
                    continue;
                };
                if de == target {
                    let Some(c) = cliente else { continue };
                    let agora = Instant::now();
                    let inicio = ultimo_fim.max(agora);
                    // Atraso acumulado da fila antes deste pacote começar a sair.
                    if inicio - agora > fila_max {
                        desc.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let fim = inicio + Duration::from_secs_f64(n as f64 / taxa);
                    ultimo_fim = fim;
                    let _ = tx.send((fim, buf[..n].to_vec(), c));
                } else {
                    // O primeiro remetente vira o cliente.
                    let c = *cliente.get_or_insert(de);
                    if de == c {
                        let _ = s_ent.send_to(&buf[..n], target).await;
                    }
                }
            }
        });

        LinkProxy {
            addr,
            enviados,
            descartados,
            tarefas: vec![saida, entrada],
        }
    }

    /// (pacotes enviados ao cliente, pacotes descartados).
    pub fn stats(&self) -> (u64, u64) {
        (
            self.enviados.load(Ordering::Relaxed),
            self.descartados.load(Ordering::Relaxed),
        )
    }
}

impl Drop for LinkProxy {
    fn drop(&mut self) {
        for t in &self.tarefas {
            t.abort();
        }
    }
}
