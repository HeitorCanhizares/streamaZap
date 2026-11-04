//! Aceite 2 de `docs/planos/m4-online.md`: mídia sintética pela ponte, camada por espectador.
//!
//! Sem proxy de rede: o "enlace limitado" é simulado pelo `banda_kbps` que cada espectador
//! reporta (é o número que o servidor usa, `docs/api/balanceamento.md`). Tudo o que é da mídia
//! (`camadas.pedidas`, `keyframe.pedir`, `camada.atual`, `relatorio`) anda pelo stream de controle
//! QUIC, como no servidor de produção.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use parking_lot::Mutex;
use zap_net::{
    control::{ControlRx, ControlTx},
    handshake::{connect_handshake, Hello},
    media_rx::read_frame,
    media_tx::MediaSender,
    online::{CreateRoomRequest, LoginRequest, ServerClient, WsRx, WsTx},
    Clock,
};
use zap_proto::{
    media::{AudioHeader, VideoFlags, VideoHeader},
    Access, AudioSpec, ControlMsg, DelayState, LayerSpec, Report, TileSize, TrackKind,
};
use zap_test_server::{spawn, Config, Handle};

/// Nome no certificado da ponte (`spawn` usa este).
const NOME_CERT: &str = "zap-test-server";
const ESPERA: Duration = Duration::from_secs(3);

/// Escada 1080p60 de `docs/api/balanceamento.md`.
fn escada() -> Vec<LayerSpec> {
    vec![
        LayerSpec {
            layer: 0,
            width: 1920,
            height: 1080,
            fps: 60,
            kbps: 8000,
        },
        LayerSpec {
            layer: 1,
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 2500,
        },
        LayerSpec {
            layer: 2,
            width: 640,
            height: 360,
            fps: 15,
            kbps: 400,
        },
    ]
}

/// Um app conectado: REST, WebSocket e QUIC com a ponte.
struct App {
    cli: ServerClient,
    ws_tx: WsTx,
    ws_rx: WsRx,
    conn: quinn::Connection,
    ctl_tx: ControlTx,
    ctl_rx: ControlRx,
    _ep: quinn::Endpoint,
}

fn pedido(nome: &str) -> LoginRequest {
    LoginRequest {
        name: nome.into(),
        color: "#3BF08A".into(),
        install_id: format!("inst-{nome}"),
        protocol: 4,
        app_version: "3.0.0".into(),
        server_password: None,
    }
}

async fn esperar_ws(rx: &mut WsRx, descricao: &str, pred: impl Fn(&ControlMsg) -> bool) -> ControlMsg {
    tokio::time::timeout(ESPERA, async {
        loop {
            match rx.recv().await {
                Some(Ok(m)) if pred(&m) => return m,
                Some(Ok(_)) => {}
                outro => panic!("WebSocket terminou esperando {descricao}: {outro:?}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("tempo esgotado esperando {descricao}"))
}

async fn esperar_quic(rx: &mut ControlRx, descricao: &str, pred: impl Fn(&ControlMsg) -> bool) -> ControlMsg {
    tokio::time::timeout(ESPERA, async {
        loop {
            match rx.recv().await {
                Ok(Some(m)) if pred(&m) => return m,
                Ok(Some(_)) => {}
                outro => panic!("controle QUIC terminou esperando {descricao}: {outro:?}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("tempo esgotado esperando {descricao}"))
}

/// Conecta o QUIC com `ingresso` e faz o aperto de mão.
async fn quic(h: &Handle, ingresso: &str, previa: bool) -> (quinn::Endpoint, quinn::Connection, ControlTx, ControlRx) {
    let ep = zap_net::endpoint::client_endpoint().unwrap();
    let cfg = zap_net::tls::client_config(Some(&h.cert_fingerprint)).unwrap();
    let conn = ep.connect_with(cfg, h.quic_addr, NOME_CERT).unwrap().await.unwrap();
    let hello = Hello {
        app_version: "3.0.0".into(),
        install_id: "inst".into(),
        name: "Teste".into(),
        color: "#3BF08A".into(),
        ticket: Some(ingresso.into()),
        room_id: None,
        preview: previa.then_some(true),
    };
    let (tx, rx, _welcome) = connect_handshake(&conn, hello, None).await.unwrap();
    (ep, conn, tx, rx)
}

/// Loga, entra na sala (`entrar` também para o dono, que precisa do ingresso), abre o WebSocket
/// com `sala.entrar` e conecta o QUIC.
async fn entrar(h: &Handle, cli: ServerClient, sala: &zap_net::online::RoomListing) -> App {
    let ingresso = cli.join(&sala.code, None).await.unwrap().ticket;
    let (ws_tx, mut ws_rx) = cli.connect_ws().await.unwrap();
    ws_tx
        .send(ControlMsg::RoomJoin {
            room_id: sala.id.clone(),
        })
        .await
        .unwrap();
    esperar_ws(&mut ws_rx, "sala.estado", |m| matches!(m, ControlMsg::RoomState { .. })).await;
    let (ep, conn, ctl_tx, ctl_rx) = quic(h, &ingresso, false).await;
    App {
        cli,
        ws_tx,
        ws_rx,
        conn,
        ctl_tx,
        ctl_rx,
        _ep: ep,
    }
}

/// Publica pelo WebSocket e devolve `(trilha_id, numero)`.
async fn publicar(
    app: &mut App,
    local_id: &str,
    kind: TrackKind,
    layers: Vec<LayerSpec>,
    audio: Option<AudioSpec>,
) -> (String, u16) {
    app.ws_tx
        .send(ControlMsg::TrackPublish {
            local_id: local_id.into(),
            kind,
            title: "Tela".into(),
            layers,
            audio,
            address: None,
        })
        .await
        .unwrap();
    let msg = esperar_ws(
        &mut app.ws_rx,
        "trilha.publicada",
        |m| matches!(m, ControlMsg::TrackPublished { local_id: Some(l), .. } if l == local_id),
    )
    .await;
    match msg {
        ControlMsg::TrackPublished { track, .. } => (track.id, track.number),
        _ => unreachable!(),
    }
}

/// A cada 100 ms, o `relatorio` do espectador com a banda que ele "tem".
fn reportar(tx: ControlTx, banda_kbps: u32) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let r = Report {
                arrivals: vec![],
                probes: vec![],
                lost: vec![],
                loss: 0.0,
                bandwidth_kbps: Some(banda_kbps),
                state: Some(DelayState::Normal),
                buffer_ms: Some(100),
            };
            if tx.send(&ControlMsg::Report(r)).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
}

/// Quadros recebidos: `(quando, camada, send_us)`.
type Recebidos = Arc<Mutex<Vec<(Instant, u8, u32)>>>;

/// Aceita os streams de vídeo da conexão e anota cada quadro.
fn receber(conn: quinn::Connection) -> (Recebidos, tokio::task::JoinHandle<()>) {
    let recebidos: Recebidos = Arc::default();
    let r = recebidos.clone();
    let tarefa = tokio::spawn(async move {
        let clock = Clock::new();
        while let Ok(recv) = conn.accept_uni().await {
            let (r, clock) = (r.clone(), clock.clone());
            tokio::spawn(async move {
                if let Ok((h, _, _)) = read_frame(recv, &clock).await {
                    r.lock().push((Instant::now(), h.layer, h.send_us));
                }
            });
        }
    });
    (recebidos, tarefa)
}

/// Camadas recebidas desde `desde`.
fn camadas_desde(r: &Recebidos, desde: Instant) -> HashSet<u8> {
    r.lock()
        .iter()
        .filter(|(t, _, _)| *t >= desde)
        .map(|(_, c, _)| *c)
        .collect()
}

/// Estado do publicador alimentado pelo controle QUIC dele.
#[derive(Default)]
struct Pedidos {
    /// Último `camadas.pedidas`: camada → kbps.
    atuais: BTreeMap<u8, u32>,
    /// Todos os pedidos recebidos.
    historico: Vec<Vec<(u8, u32)>>,
    /// Camadas com `keyframe.pedir` ainda não atendido.
    keyframes: HashSet<u8>,
}

/// Publica quadros sintéticos de 33 em 33 ms: as camadas pedidas (camada 2 até o primeiro pedido,
/// ou todas com `todas`), keyframe a cada 1 s e sempre que pedido.
fn publicar_quadros(
    conn: quinn::Connection,
    numero: u16,
    pedidos: Arc<Mutex<Pedidos>>,
    todas: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let clock = Clock::new();
        let sender = Arc::new(MediaSender::new(conn, clock.clone()));
        let mut seq = [0u32; 3];
        let mut quadro: u64 = 0;
        let mut periodo = tokio::time::interval(Duration::from_millis(33));
        loop {
            periodo.tick().await;
            let (camadas, pedidos_kf): (Vec<(u8, u32)>, HashSet<u8>) = {
                let mut p = pedidos.lock();
                let camadas = if todas {
                    escada().iter().map(|l| (l.layer, l.kbps)).collect()
                } else if p.atuais.is_empty() {
                    vec![(2, 400)]
                } else {
                    p.atuais.iter().map(|(c, k)| (*c, *k)).collect()
                };
                (camadas, std::mem::take(&mut p.keyframes))
            };
            for (camada, kbps) in camadas {
                let Some(spec) = escada().into_iter().find(|l| l.layer == camada) else {
                    continue;
                };
                let keyframe = quadro % 30 == 0 || pedidos_kf.contains(&camada);
                // Bytes de um quadro a 30 quadros/s para dar `kbps`.
                let tamanho = usize::try_from(u64::from(kbps) * 1000 / 8 / 30).unwrap().max(200);
                let header = VideoHeader {
                    flags: VideoFlags(if keyframe {
                        VideoFlags::KEYFRAME | VideoFlags::PARAMS
                    } else {
                        0
                    }),
                    track: numero,
                    layer: camada,
                    width: spec.width,
                    height: spec.height,
                    seq: seq[usize::from(camada)],
                    capture_us: clock.now_us(),
                    send_us: 0,
                    size: 0,
                };
                seq[usize::from(camada)] += 1;
                let s = sender.clone();
                tokio::spawn(async move {
                    let prazo = s.deadline();
                    let _ = s.send_video(header, Bytes::from(vec![0u8; tamanho]), prazo).await;
                });
            }
            quadro += 1;
        }
    })
}

/// Lê o controle QUIC do publicador: `camadas.pedidas` e `keyframe.pedir`.
fn ouvir_publicador(mut rx: ControlRx, pedidos: Arc<Mutex<Pedidos>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(Some(m)) = rx.recv().await {
            match m {
                ControlMsg::LayersRequested { layers, .. } => {
                    let mut p = pedidos.lock();
                    p.atuais = layers.iter().map(|l| (l.layer, l.kbps)).collect();
                    p.historico.push(layers.iter().map(|l| (l.layer, l.kbps)).collect());
                }
                ControlMsg::KeyframeRequest { layer: Some(c), .. } => {
                    pedidos.lock().keyframes.insert(c);
                }
                _ => {}
            }
        }
    })
}

/// Lê o controle QUIC do espectador e anota cada `camada.atual`.
fn ouvir_espectador(mut rx: ControlRx) -> (Arc<Mutex<Vec<u8>>>, tokio::task::JoinHandle<()>) {
    let camadas: Arc<Mutex<Vec<u8>>> = Arc::default();
    let c = camadas.clone();
    let tarefa = tokio::spawn(async move {
        while let Ok(Some(m)) = rx.recv().await {
            if let ControlMsg::LayerCurrent { layer, .. } = m {
                c.lock().push(layer);
            }
        }
    });
    (camadas, tarefa)
}

async fn assinar_ws(app: &mut App, trilha: &str) {
    app.ws_tx
        .send(ControlMsg::Subscribe {
            track_id: trilha.into(),
            size: TileSize::Main,
            max_layer: None,
            audio: Some(false),
        })
        .await
        .unwrap();
    esperar_ws(
        &mut app.ws_rx,
        "midia.rota",
        |m| matches!(m, ControlMsg::MediaRoute { track_id, .. } if track_id == trilha),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn aceite_camadas_por_espectador() {
    let h = spawn(Config::default()).await.unwrap();
    let dono = ServerClient::login(&h.base_url, pedido("P")).await.unwrap();
    let sala = dono
        .create_room(CreateRoomRequest {
            name: "Sexta".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    let mut p = entrar(&h, dono, &sala).await;
    let mut v1 = entrar(&h, ServerClient::login(&h.base_url, pedido("V1")).await.unwrap(), &sala).await;
    let mut v2 = entrar(&h, ServerClient::login(&h.base_url, pedido("V2")).await.unwrap(), &sala).await;

    let (trilha, numero) = publicar(&mut p, "l1", TrackKind::Screen, escada(), None).await;
    assinar_ws(&mut v1, &trilha).await;
    assinar_ws(&mut v2, &trilha).await;

    let pedidos: Arc<Mutex<Pedidos>> = Arc::default();
    let App {
        conn: p_conn,
        ctl_rx: p_rx,
        ..
    } = p;
    let ouvinte = ouvir_publicador(p_rx, pedidos.clone());
    let envio = publicar_quadros(p_conn.clone(), numero, pedidos.clone(), false);

    let App {
        conn: c1,
        ctl_tx: t1,
        ctl_rx: r1,
        ..
    } = v1;
    let App {
        conn: c2,
        ctl_tx: t2,
        ctl_rx: r2,
        ..
    } = v2;
    // `balanceamento.md`: disponível = banda × 0,9, e a camada sobe só com disponível ≥ 1,15 × kbps.
    // Camada 1 (2500) pede ≥ ~3195 kbps; camada 0 (8000), ≥ ~10223. 4 Mbps cai na camada 1 (com
    // 3 Mbps o espectador ficaria na 2).
    let rep1 = reportar(t1, 4_000);
    let rep2 = reportar(t2, 30_000);
    let (quadros1, rx1) = receber(c1.clone());
    let (quadros2, rx2) = receber(c2.clone());
    let (anuncios1, a1) = ouvir_espectador(r1);
    let (anuncios2, a2) = ouvir_espectador(r2);

    // Histerese: sobe depois de 3 s de condição e 5 s desde a última subida; V2 sobe duas vezes.
    // Espera até a situação final valer por 2 s seguidos (no máximo 25 s).
    let inicio = Instant::now();
    let janela = Duration::from_secs(2);
    let mut estavel_desde: Option<Instant> = None;
    while inicio.elapsed() < Duration::from_secs(25) {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let desde = Instant::now() - janela;
        let c1 = camadas_desde(&quadros1, desde);
        let c2 = camadas_desde(&quadros2, desde);
        let pronto = inicio.elapsed() > janela
            && c1.contains(&1)
            && !c1.contains(&0)
            && c2.contains(&0)
            && anuncios1.lock().last() == Some(&1)
            && anuncios2.lock().last() == Some(&0);
        match (pronto, estavel_desde) {
            (true, None) => estavel_desde = Some(Instant::now()),
            (true, Some(t)) if t.elapsed() >= janela => break,
            (false, _) => estavel_desde = None,
            _ => {}
        }
    }
    let gasto = inicio.elapsed();
    // Diagnóstico (o `cargo test` só mostra quando o teste falha).
    let por_camada = |r: &Recebidos| {
        let mut n = [0usize; 3];
        for (_, c, _) in r.lock().iter() {
            n[usize::from(*c).min(2)] += 1;
        }
        n
    };
    eprintln!(
        "pedidos: {:?}\nanúncios V1: {:?}\nanúncios V2: {:?}\nquadros V1 por camada: {:?}\nquadros V2 por camada: {:?}",
        pedidos.lock().historico,
        anuncios1.lock(),
        anuncios2.lock(),
        por_camada(&quadros1),
        por_camada(&quadros2),
    );

    let desde = Instant::now() - janela;
    let c1 = camadas_desde(&quadros1, desde);
    let c2 = camadas_desde(&quadros2, desde);
    assert!(
        c1.contains(&1),
        "V1 (4 Mbps) devia receber a camada 1: {c1:?} em {gasto:?}"
    );
    assert!(!c1.contains(&0), "V1 (4 Mbps) não devia receber a camada 0: {c1:?}");
    assert!(
        c2.contains(&0),
        "V2 (30 Mbps) devia receber a camada 0: {c2:?} em {gasto:?}"
    );
    assert_eq!(anuncios1.lock().last(), Some(&1), "camada.atual de V1");
    assert_eq!(anuncios2.lock().last(), Some(&0), "camada.atual de V2");
    assert_eq!(
        anuncios1.lock().first(),
        Some(&2),
        "todo espectador começa na camada mais barata"
    );

    {
        let pedidos = pedidos.lock();
        let pediu = |c: u8| pedidos.historico.iter().any(|p| p.iter().any(|(l, _)| *l == c));
        assert!(
            pediu(0) && pediu(1) && pediu(2),
            "camadas.pedidas: {:?}",
            pedidos.historico
        );
        let nominal = |c: u8| escada().into_iter().find(|l| l.layer == c).map(|l| l.kbps).unwrap();
        assert!(
            pedidos.historico.iter().flatten().all(|(c, k)| *k <= nominal(*c)),
            "kbps pedido acima do nominal: {:?}",
            pedidos.historico
        );
    }
    // A ponte reescreve `envio_us` (`send_us`) a cada salto.
    assert!(quadros2.lock().iter().any(|(_, _, s)| *s != 0));

    for t in [envio, ouvinte, rep1, rep2, rx1, rx2, a1, a2] {
        t.abort();
    }
    drop(p_conn);
    drop(p.cli);
}

#[tokio::test(flavor = "multi_thread")]
async fn ingresso_de_previa_so_camada_2() {
    let h = spawn(Config::default()).await.unwrap();
    let dono = ServerClient::login(&h.base_url, pedido("P")).await.unwrap();
    let sala = dono
        .create_room(CreateRoomRequest {
            name: "Aberta".into(),
            access: Access::Open,
            password: None,
        })
        .await
        .unwrap();
    let mut p = entrar(&h, dono, &sala).await;
    let (trilha, numero) = publicar(&mut p, "l1", TrackKind::Screen, escada(), None).await;
    let audio = AudioSpec {
        rate: 48_000,
        channels: 2,
        kbps: 64,
    };
    let (_trilha_audio, numero_audio) = publicar(&mut p, "l2", TrackKind::Audio, vec![], Some(audio)).await;

    // Quem só espia: prévia pelo REST, QUIC com o ingresso de prévia, assinatura pelo QUIC.
    let v = ServerClient::login(&h.base_url, pedido("V")).await.unwrap();
    let previa = v.preview(&sala.code).await.unwrap();
    let (_ep, v_conn, v_tx, mut v_rx) = quic(&h, &previa.ticket, true).await;
    esperar_quic(&mut v_rx, "sala.estado da prévia", |m| {
        matches!(m, ControlMsg::RoomState { .. })
    })
    .await;
    v_tx.send(&ControlMsg::Subscribe {
        track_id: trilha.clone(),
        size: TileSize::Main,
        max_layer: Some(0),
        audio: Some(true),
    })
    .await
    .unwrap();
    esperar_quic(
        &mut v_rx,
        "midia.rota pelo QUIC",
        |m| matches!(m, ControlMsg::MediaRoute { track_id, .. } if *track_id == trilha),
    )
    .await;
    let rep = reportar(v_tx, 30_000);
    let (quadros, rx) = receber(v_conn.clone());

    // O publicador manda as três camadas e áudio o tempo todo.
    let pedidos: Arc<Mutex<Pedidos>> = Arc::default();
    let envio = publicar_quadros(p.conn.clone(), numero, pedidos, true);
    let sender = MediaSender::new(p.conn.clone(), Clock::new());
    let clock = Clock::new();
    let datagramas = {
        let conn = v_conn.clone();
        tokio::spawn(async move {
            let mut n = 0u32;
            while tokio::time::timeout(Duration::from_secs(5), conn.read_datagram())
                .await
                .is_ok_and(|r| r.is_ok())
            {
                n += 1;
            }
            n
        })
    };
    for seq in 0..200u32 {
        let h = AudioHeader {
            track: numero_audio,
            seq,
            capture_us: clock.now_us(),
            send_us: 0,
        };
        let _ = sender.send_audio(h, &[0u8; 60]);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let camadas: HashSet<u8> = quadros.lock().iter().map(|(_, c, _)| *c).collect();
    assert!(!quadros.lock().is_empty(), "a prévia devia receber vídeo");
    assert_eq!(camadas, HashSet::from([2]), "prévia só recebe a camada 2");
    envio.abort();
    rep.abort();
    rx.abort();
    v_conn.close(0u32.into(), b"fim");
    assert_eq!(datagramas.await.unwrap(), 0, "prévia não recebe áudio");
}
