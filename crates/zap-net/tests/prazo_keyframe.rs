//! Prazo de keyframe (midia.md "Prazo"): um keyframe grande com quadros comuns atrás não pode
//! causar reset, e um quadro comum sozinho num caminho parado estoura o prazo normal.
//!
//! Emissor = cliente QUIC; o proxy fica no meio e limita só cliente->servidor.

mod support;

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use support::proxy::{Proxy, ProxyConfig};
use tokio::task::JoinHandle;
use zap_net::{
    endpoint,
    media_rx::read_frame,
    media_tx::MediaSender,
    tls::{self, Identity},
    Clock,
};
use zap_proto::media::{VideoFlags, VideoHeader};

type Erro = Box<dyn std::error::Error>;
type Res = Result<(), Erro>;

/// Keyframe da camada 1 (2500 kbps, 30 fps): ~8 × um quadro comum.
const TAM_KEYFRAME: usize = 83_333;
const TAM_COMUM: usize = 10_417;
const QUADROS_ATRAS: usize = 15;
const INTERVALO: Duration = Duration::from_millis(33);

fn cabecalho(seq: u32, keyframe: bool, clock: &Clock) -> VideoHeader {
    VideoHeader {
        flags: VideoFlags(if keyframe { VideoFlags::KEYFRAME } else { 0 }),
        track: 1,
        layer: 1,
        width: 1280,
        height: 720,
        seq,
        capture_us: clock.now_us(),
        send_us: 0,
        size: 0,
    }
}

/// Servidor, proxy e cliente de um cenário; tudo fica vivo enquanto a montagem existir.
struct Montagem {
    sender: Arc<MediaSender>,
    clock: Clock,
    conn: quinn::Connection,
    proxy: Proxy,
    _servidor: quinn::Endpoint,
    _cliente: quinn::Endpoint,
    _conn_srv: quinn::Connection,
    _drena: JoinHandle<()>,
}

/// Conecta o cliente ao servidor através do proxy e põe o servidor para esvaziar todo stream.
async fn montar(rate_kbps: u32, seed: u64) -> Result<Montagem, Erro> {
    let id = Identity::generate("zap")?;
    let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
    let addr_srv: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
    let proxy = Proxy::start(
        addr_srv,
        ProxyConfig {
            delay: Duration::from_millis(10),
            loss: 0.0,
            // O limite entra logo depois do handshake: a 50 kbps o handshake sozinho levaria o RTT
            // a ~200 ms e o prazo deixaria de ser o do caminho vazio (150 ms).
            rate_kbps: None,
            queue_ms: 100,
            seed,
        },
    )
    .await?;

    let cliente = endpoint::client_endpoint()?;
    let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, proxy.listen_addr, "zap")?;
    let conn_srv = servidor.accept().await.ok_or("sem conexão")?.await?;
    let conn = conectando.await?;
    proxy.set_rate_kbps(Some(rate_kbps));

    let clock = Clock::new();
    let (conn_rx, clock_rx) = (conn_srv.clone(), clock.clone());
    let drena = tokio::spawn(async move {
        while let Ok(recv) = conn_rx.accept_uni().await {
            let c = clock_rx.clone();
            tokio::spawn(async move {
                // Resetado ou incompleto não importa aqui: só esvazia.
                let _ = read_frame(recv, &c).await;
            });
        }
    });

    Ok(Montagem {
        sender: Arc::new(MediaSender::new(conn.clone(), clock.clone())),
        clock,
        conn,
        proxy,
        _servidor: servidor,
        _cliente: cliente,
        _conn_srv: conn_srv,
        _drena: drena,
    })
}

/// Resultado de um quadro: (seq, entrada, saída, prazo, entregue), instantes em ms desde o keyframe.
type Quadro = (usize, u128, u128, u128, bool);

/// Keyframe + 15 quadros comuns a cada 33 ms, cada `send_video` numa tarefa própria. O prazo é
/// `sender.deadline()` lido na hora de cada envio, como fazem o núcleo e a ponte.
async fn cenario_keyframe(m: &Montagem) -> Result<Vec<Quadro>, Erro> {
    let t0 = Instant::now();
    let mut envios = tokio::task::JoinSet::new();
    let mut tick = tokio::time::interval(INTERVALO);
    for i in 0..=QUADROS_ATRAS {
        tick.tick().await;
        let keyframe = i == 0;
        let tam = if keyframe { TAM_KEYFRAME } else { TAM_COMUM };
        let h = cabecalho(i as u32, keyframe, &m.clock);
        let dados = Bytes::from(vec![i as u8; tam]);
        let s = m.sender.clone();
        envios.spawn(async move {
            let entrada = t0.elapsed().as_millis();
            let prazo = s.deadline();
            let r = s.send_video(h, dados, prazo).await;
            r.map(|ok| (i, entrada, t0.elapsed().as_millis(), prazo.as_millis(), ok))
        });
    }
    let mut quadros = Vec::with_capacity(QUADROS_ATRAS + 1);
    while let Some(r) = envios.join_next().await {
        quadros.push(r??);
    }
    quadros.sort_by_key(|q| q.0);
    Ok(quadros)
}

#[tokio::test(flavor = "multi_thread")]
async fn keyframe_de_83kb_com_quadros_atras_nao_reseta() -> Res {
    let _timer = support::timer();
    let mut falhas = Vec::new();
    for rodada in 0..3u64 {
        // Conexão e proxy novos a cada rodada.
        let m = montar(4000, 100 + rodada).await?;
        let quadros = cenario_keyframe(&m).await?;

        let caminho = m.conn.stats().path;
        eprintln!(
            "rodada {rodada}: perdas QUIC {} (eventos {}), proxy {:?}",
            caminho.lost_packets,
            caminho.congestion_events,
            m.proxy.stats()
        );
        for &(i, entrada, saida, prazo, ok) in &quadros {
            eprintln!(
                "  quadro {i:2}: entra {entrada:3} ms, termina {saida:3} ms ({:3} ms), prazo {prazo} ms, ok={ok}",
                saida - entrada
            );
        }
        assert_eq!(quadros.len(), QUADROS_ATRAS + 1);
        let resetados: Vec<usize> = quadros.iter().filter(|q| !q.4).map(|q| q.0).collect();
        if !resetados.is_empty() {
            falhas.push((rodada, resetados));
        }
    }
    assert!(falhas.is_empty(), "quadros descartados (rodada, seqs): {falhas:?}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn quadro_comum_sozinho_em_caminho_parado_reseta_entre_150_e_250ms() -> Res {
    let _timer = support::timer();
    let m = montar(50, 7).await?;
    let prazo = m.sender.deadline();
    assert_eq!(prazo, Duration::from_millis(150), "prazo inesperado no loopback");

    // 10 KB a 50 kbps levam ~1,7 s: o prazo vence muito antes.
    let h = cabecalho(0, false, &m.clock);
    let t0 = Instant::now();
    let ok = m.sender.send_video(h, Bytes::from(vec![1u8; TAM_COMUM]), prazo).await?;
    let ms = t0.elapsed().as_millis();
    eprintln!("quadro comum em caminho parado: ok={ok} em {ms} ms");
    if !(150..=250).contains(&ms) {
        eprintln!("aviso: {ms} ms fora de 150..=250 (timer ruidoso); margem aceita 140..=300");
    }

    assert!(!ok, "o quadro deveria ser resetado por prazo");
    assert!((140..=300).contains(&ms), "reset em {ms} ms, fora de 140..=300");
    Ok(())
}
