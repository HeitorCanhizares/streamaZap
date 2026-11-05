#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 2 Online: seleção de camada por enlace.
//!
//! Cenário: P publica 1080p60 sintético (escada 8000/2500/400 kbps nas camadas 0/1/2, veja
//! `zap_rate::ladder::screen_ladder`). A assiste pela ponte atrás de um proxy com limite, B
//! assiste direto (sem limite) e C assiste como miniatura.
//!
//! Calibração: pela folga de `docs/api/balanceamento.md` (disponível = banda x 0,9; a camada só
//! sobe com disponível >= 1,15 x kbps), a camada 1 (2500 kbps) pede >= ~3,2 Mbps. Com 3 Mbps
//! ela nunca seria escolhida (2700 < 2875), então o proxy usa 4000 kbps: disponível 3600, que
//! cabe a camada 1 (2875) mas não a camada 0 (9200).

#[path = "common/mod.rs"]
mod common;

use std::time::Duration;

use common::{core_online, core_online_via, desligar, espera, servidor, LinkProxy};
use zap_core::{AudioChoice, Core, CreateRoom, JoinTarget, Preset, ShareRequest};
use zap_proto::{RoomMode, TileSize};

fn camada(c: &Core) -> Option<u8> {
    c.snapshot().tiles.iter().find(|t| t.tile == 1).map(|t| t.layer)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn aceite_online_camadas_por_enlace() {
    tokio::time::timeout(Duration::from_secs(60), corpo())
        .await
        .expect("estourou 60 s");
}

async fn corpo() {
    let srv = servidor().await;
    let (p, _sp) = core_online("P", &srv, true).await;
    p.create_room(CreateRoom {
        name: "Camadas".into(),
        mode: RoomMode::Online,
        password: None,
    })
    .await
    .expect("criar sala");
    let code = p
        .snapshot()
        .room
        .as_ref()
        .and_then(|r| r.code.clone())
        .expect("sala sem código");

    let proxy = LinkProxy::start(srv.quic_addr, 4000, 100).await;
    let (a, sa) = core_online_via("A", &srv, proxy.addr).await;
    let (b, sb) = core_online("B", &srv, true).await;
    let (c, sc) = core_online("C", &srv, true).await;
    for (nome, x) in [("A", &a), ("B", &b), ("C", &c)] {
        x.join_room(JoinTarget::Code { code: code.clone() }, None)
            .await
            .unwrap_or_else(|e| panic!("{nome} não entrou: {e:?}"));
    }

    p.start_share(ShareRequest {
        source: "synthetic:1920x1080".into(),
        audio: AudioChoice::Keyword("none".into()),
        preset: Preset::P1080p60,
        encoder: "software".into(),
    })
    .await
    .expect("start_share");

    let mut track = None;
    let ok = espera(Duration::from_secs(10), || {
        track = a
            .snapshot()
            .room
            .as_ref()
            .and_then(|r| r.tracks.first().map(|t| t.id.clone()));
        track.is_some()
    })
    .await;
    assert!(ok, "A não viu a trilha");
    let track = track.unwrap();
    assert!(
        espera(Duration::from_secs(10), || [&b, &c].iter().all(|x| {
            x.snapshot().room.as_ref().is_some_and(|r| !r.tracks.is_empty())
        }))
        .await,
        "B/C não viram a trilha"
    );

    a.watch(&track, 1, TileSize::Main, None).expect("watch A");
    b.watch(&track, 1, TileSize::Main, None).expect("watch B");
    c.watch(&track, 1, TileSize::Thumbnail, None).expect("watch C");

    let alvo = |a: &Core, b: &Core, c: &Core| camada(a) == Some(1) && camada(b) == Some(0) && camada(c) == Some(2);
    let ok = espera(Duration::from_secs(30), || {
        alvo(&a, &b, &c)
            && sa.video_count(1) > 10
            && sb.video_count(1) > 10
            && sc.video_count(1) > 10
            && p.snapshot()
                .share
                .as_ref()
                .is_some_and(|s| [0, 1, 2].iter().all(|l| s.layers.contains(l)))
    })
    .await;
    // Falhas viram mensagem e o pânico só vem depois de desligar os Cores: soltar um `Core`
    // dentro do runtime (no desenrolar do pânico) aborta o processo e esconde a mensagem.
    let falha = if !ok {
        Some(format!(
            "camadas A/B/C = {:?}/{:?}/{:?}, pacotes {}/{}/{}",
            camada(&a),
            camada(&b),
            camada(&c),
            sa.video_count(1),
            sb.video_count(1),
            sc.video_count(1)
        ))
    } else if espera(Duration::from_secs(3), || !alvo(&a, &b, &c)).await {
        // Estabilidade: as camadas não podem oscilar por 3 s.
        Some(format!(
            "camadas oscilaram: A={:?} B={:?} C={:?}",
            camada(&a),
            camada(&b),
            camada(&c)
        ))
    } else {
        None
    };

    for x in [a, b, c, p] {
        desligar(x).await;
    }
    if let Some(m) = falha {
        panic!("{m}");
    }
}
