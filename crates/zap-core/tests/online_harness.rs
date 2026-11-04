#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Testes do próprio harness dos testes Online.

mod common;

use std::time::Duration;

use common::*;
use tokio::net::UdpSocket;
use zap_core::ServerState;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn servidor_e_core_ficam_online() {
    let srv = servidor().await;
    let (core, _sink) = core_online("a", &srv, true).await;
    let ok = espera(Duration::from_secs(5), || {
        core.snapshot()
            .server
            .as_ref()
            .is_some_and(|s| s.state == ServerState::Online && s.ping_ms.is_some())
    })
    .await;
    let snap = core.snapshot();
    assert!(ok, "status: {:?}", snap.server);
    let st = snap.server.as_ref().unwrap();
    assert!(st.ping_ms.is_some());
    assert!(st.label().starts_with("Servidor online · "), "{}", st.label());
    desligar(core).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn proxy_limita_taxa() {
    let servidor = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let proxy = LinkProxy::start(servidor.local_addr().unwrap(), 3000, 100).await;
    let cliente = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    cliente.send_to(b"oi", proxy.addr).await.unwrap();
    let mut buf = [0u8; 2048];
    let (_, origem) = tokio::time::timeout(Duration::from_secs(2), servidor.recv_from(&mut buf))
        .await
        .unwrap()
        .unwrap();

    // O servidor envia 2 MB bem acima da taxa do enlace (~7,7 Mbps, em rajadas de 8 pacotes a cada
    // 10 ms): uma rajada única cairia quase toda no tail drop e não mediria a taxa sustentada.
    let envio = tokio::spawn(async move {
        let pacote = [7u8; 1200];
        let mut enviados = 0;
        while enviados < 1667 {
            for _ in 0..8 {
                let _ = servidor.send_to(&pacote, origem).await;
                enviados += 1;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        servidor
    });

    let inicio = tokio::time::Instant::now();
    let mut total = 0usize;
    while let Some(restante) = Duration::from_secs(2).checked_sub(inicio.elapsed()) {
        match tokio::time::timeout(restante, cliente.recv_from(&mut buf)).await {
            Ok(Ok((n, _))) => total += n,
            _ => break,
        }
    }
    let _servidor = envio.await.unwrap();
    let esperado = 3000.0 * 2.0 * 125.0;
    let t = total as f64;
    assert!(
        t >= esperado * 0.4 && t <= esperado * 1.3,
        "recebido {total} B, esperado ~{esperado} B"
    );
    let (_, descartados) = proxy.stats();
    assert!(descartados > 0, "nada descartado");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn proxy_sentido_cliente_servidor_sem_limite() {
    let servidor = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let proxy = LinkProxy::start(servidor.local_addr().unwrap(), 3000, 100).await;
    let cliente = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let pacote = [1u8; 1200];
    let mut buf = [0u8; 2048];
    let mut recebidos = 0usize;
    let inicio = tokio::time::Instant::now();
    for i in 0..500 {
        cliente.send_to(&pacote, proxy.addr).await.unwrap();
        if i % 10 == 9 {
            // Drena para não estourar o buffer do socket de teste.
            while let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(1), servidor.recv_from(&mut buf)).await {
                recebidos += 1;
            }
        }
    }
    let envio_fim = inicio.elapsed();
    while let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(200), servidor.recv_from(&mut buf)).await {
        recebidos += 1;
    }
    // 600 kB a 3 Mbps levariam 1,6 s; sem limite o envio termina bem antes.
    assert!(envio_fim < Duration::from_millis(1500), "lento: {envio_fim:?}");
    assert!(recebidos >= 450, "recebidos {recebidos}/500");
    assert_eq!(proxy.stats().1, 0);
}
