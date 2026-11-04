#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Aceite 2 e 3: senha errada da sala e queda de membro (sem `sala.sair`).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zap_core::testutil::TestSink;
use zap_core::{Core, CoreOptions, CreateRoom};
use zap_proto::RoomMode;

async fn novo_core(nome: &str) -> (Core, Arc<TestSink>) {
    let sink = TestSink::new();
    let dir = std::env::temp_dir().join(format!("zapcore-{}-{}", nome, uuid::Uuid::new_v4()));
    let s = sink.clone();
    let core = tokio::task::spawn_blocking(move || {
        Core::start_with(
            dir,
            s,
            CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                ..Default::default()
            },
        )
    })
    .await
    .unwrap()
    .expect("core");
    (core, sink)
}

async fn desligar(mut c: Core) {
    tokio::task::spawn_blocking(move || c.shutdown()).await.unwrap();
}

/// Endereço e impressão digital da sala do dono, para os membros entrarem sem descoberta.
fn endereco_da_sala(dono: &Core) -> (SocketAddr, String) {
    let porta = dono.control_port().expect("porta");
    let fp = dono.control_fingerprint().expect("fingerprint");
    (SocketAddr::from(([127, 0, 0, 1], porta)), fp)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn senha_errada_da_senha_sala() {
    let (d, _) = novo_core("d").await;
    let (a, _) = novo_core("a").await;

    d.create_room(CreateRoom {
        name: "Protegida".into(),
        mode: RoomMode::Local,
        password: Some("segredo".into()),
    })
    .await
    .expect("criar sala");
    let (addr, fp) = endereco_da_sala(&d);

    // Sem senha quando a sala exige: recusa com código estável.
    let e = a
        .join_room_pinned(addr, fp.clone(), None)
        .await
        .expect_err("sem senha deveria falhar");
    assert_eq!(e.code(), "senha_sala", "sem senha: {e:?}");

    // Senha errada: mesma recusa.
    let e = a
        .join_room_pinned(addr, fp.clone(), Some("errada".into()))
        .await
        .expect_err("senha errada deveria falhar");
    assert_eq!(e.code(), "senha_sala", "senha errada: {e:?}");

    // Senha certa entra (a tentativa anterior não deixou sala pendurada).
    a.join_room_pinned(addr, fp, Some("segredo".into()))
        .await
        .expect("senha certa entra");

    a.leave_room().await;
    d.leave_room().await;
    for c in [a, d] {
        desligar(c).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn membro_que_cai_some_da_sala_em_ate_6s() {
    let (d, sink_d) = novo_core("d").await;
    let (a, _) = novo_core("a").await;
    let (b, sink_b) = novo_core("b").await;

    d.create_room(CreateRoom {
        name: "Queda".into(),
        mode: RoomMode::Local,
        password: None,
    })
    .await
    .expect("criar sala");
    let (addr, fp) = endereco_da_sala(&d);
    a.join_room_pinned(addr, fp.clone(), None).await.expect("A entra");
    b.join_room_pinned(addr, fp, None).await.expect("B entra");

    // Antes da queda: dono + A + B.
    let ok = sink_b
        .wait_for(Duration::from_secs(5), |_| {
            b.snapshot().room.as_ref().map(|r| r.participants.len()) == Some(3)
        })
        .await;
    assert!(ok, "B deveria ver 3 participantes antes da queda");

    // A cai sem avisar (sem `leave_room`): a conexão fecha por baixo.
    let inicio = Instant::now();
    a.simulate_drop();

    let ok = sink_b
        .wait_for(Duration::from_secs(6), |_| {
            b.snapshot().room.as_ref().map(|r| r.participants.len()) == Some(2)
        })
        .await;
    let decorrido = inicio.elapsed();
    assert!(ok, "B não viu A sair em 6 s");
    assert!(decorrido <= Duration::from_secs(6), "demorou {decorrido:?}");

    // O dono também tem de ver só dono + B.
    let ok = sink_d
        .wait_for(Duration::from_secs(6), |_| {
            d.snapshot().room.as_ref().map(|r| r.participants.len()) == Some(2)
        })
        .await;
    assert!(ok, "dono deveria ver 2 participantes depois da queda");

    b.leave_room().await;
    d.leave_room().await;
    a.leave_room().await;
    for c in [a, b, d] {
        desligar(c).await;
    }
}
