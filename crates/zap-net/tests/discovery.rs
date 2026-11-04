//! Teste de aceite da descoberta na LAN: instâncias reais em loopback, com
//! destinos unicast cruzados (broadcast desligado). Cada teste usa portas
//! altas próprias para não interferir nos outros.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use zap_net::discovery::{Discovery, DiscoveryConfig};
use zap_proto::discovery::{OwnerSummary, PresenceAnnouncement, RoomAnnouncement, RoomSummary};
use zap_proto::types::{Access, MediaAddress};
use zap_proto::DISCOVERY_MAGIC;

fn loopback() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// Config de teste: porta fixa, 100 ms de intervalo, sem broadcast, alvos unicast em 127.0.0.1.
fn cfg(porta: u16, alvos: &[u16], room_timeout: Duration) -> DiscoveryConfig {
    DiscoveryConfig {
        port: porta,
        interval: Duration::from_millis(100),
        room_timeout,
        extra_targets: alvos
            .iter()
            .map(|p| SocketAddr::from((Ipv4Addr::LOCALHOST, *p)))
            .collect(),
        broadcast: false,
    }
}

fn sala_de_exemplo(dono_id: &str) -> RoomAnnouncement {
    RoomAnnouncement {
        room: RoomSummary {
            id: "s_1".into(),
            name: "Corrida em Interlagos".into(),
            access: Access::Open,
        },
        owner: OwnerSummary {
            id: dono_id.into(),
            name: "Rafa".into(),
            color: "#5CD6FF".into(),
        },
        address: MediaAddress {
            addresses: vec!["127.0.0.1".into()],
            port: 47800,
            fingerprint: "ab12".into(),
        },
        participants: 3,
        highlight: None,
        app_version: "3.0.0".into(),
    }
}

/// Espera até `limite` que `f` dê verdadeiro; devolve o resultado final.
async fn espera<F: Fn() -> bool>(limite: Duration, f: F) -> bool {
    let ini = Instant::now();
    while ini.elapsed() < limite {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    f()
}

/// Bytes pseudoaleatórios determinísticos (xorshift), para o teste ser reproduzível.
fn bytes_aleatorios(mut semente: u64, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| {
            semente ^= semente << 13;
            semente ^= semente >> 7;
            semente ^= semente << 17;
            (semente >> 24) as u8
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn duas_instancias_se_enxergam() {
    // A anuncia sala; B anuncia presença ouvindo A.
    let a = Discovery::start(cfg(48011, &[48012], Duration::from_secs(5)), "a".into())
        .await
        .expect("start A");
    let b = Discovery::start(cfg(48012, &[48011], Duration::from_secs(5)), "b".into())
        .await
        .expect("start B");
    a.set_room(Some(sala_de_exemplo("a")));
    b.set_presence(PresenceAnnouncement {
        install_id: "b".into(),
        name: "Heitor".into(),
        app_version: "3.0.0".into(),
        hears: vec!["a".into()],
    });

    let ok = espera(Duration::from_secs(2), || {
        !b.rooms().is_empty() && a.peers().iter().any(|p| p.install_id == "b" && p.hears_me)
    })
    .await;
    assert!(ok, "B deveria ver a sala e A deveria ver B com hears_me=true em 2 s");

    let salas = b.rooms();
    assert_eq!(salas.len(), 1);
    assert_eq!(salas[0].0.room.id, "s_1");
    assert_eq!(salas[0].1, loopback());

    let pares = a.peers();
    assert_eq!(pares.len(), 1);
    assert_eq!(pares[0].install_id, "b");
    assert!(pares[0].hears_me);
}

#[tokio::test(flavor = "multi_thread")]
async fn sala_expira() {
    // room_timeout de 500 ms: depois que A tira a sala, B deve esvaziar a lista logo.
    let a = Discovery::start(cfg(48021, &[48022], Duration::from_millis(500)), "a".into())
        .await
        .expect("start A");
    let b = Discovery::start(cfg(48022, &[], Duration::from_millis(500)), "b".into())
        .await
        .expect("start B");
    a.set_room(Some(sala_de_exemplo("a")));
    assert!(
        espera(Duration::from_secs(1), || !b.rooms().is_empty()).await,
        "B deveria ver a sala antes de ela expirar"
    );

    a.set_room(None);
    let expirou = espera(Duration::from_millis(1500), || b.rooms().is_empty()).await;
    assert!(
        expirou,
        "a sala deveria sumir de B em até 1,5 s após A parar de anunciar"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn versao_diferente_vai_para_other_versions() {
    let b = Discovery::start(cfg(48031, &[], Duration::from_secs(5)), "b".into())
        .await
        .expect("start B");

    // Socket externo fingindo ser outra versão do app (v = 3, não 4).
    let remetente = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind remetente");
    let msg = format!("{{\"app\":\"{DISCOVERY_MAGIC}\",\"v\":3,\"tipo\":\"sala\",\"versao_app\":\"2.1.0\"}}");
    remetente
        .send_to(msg.as_bytes(), (Ipv4Addr::LOCALHOST, 48031))
        .expect("enviar para B");

    let visto = espera(Duration::from_secs(1), || !b.other_versions().is_empty()).await;
    assert!(visto, "B deveria registrar a outra versão em até 1 s");
    assert_eq!(b.other_versions(), vec![(loopback(), 3)]);
    assert!(b.rooms().is_empty(), "outra versão não entra em rooms()");
}

#[tokio::test(flavor = "multi_thread")]
async fn lixo_e_ignorado() {
    let a = Discovery::start(cfg(48041, &[48042], Duration::from_secs(5)), "a".into())
        .await
        .expect("start A");
    let b = Discovery::start(cfg(48042, &[], Duration::from_secs(5)), "b".into())
        .await
        .expect("start B");
    a.set_room(Some(sala_de_exemplo("a")));

    let remetente = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind remetente");
    let destino = (Ipv4Addr::LOCALHOST, 48042);

    // Bytes aleatórios de vários tamanhos.
    for (i, n) in [1usize, 64, 300, 1000].into_iter().enumerate() {
        remetente
            .send_to(&bytes_aleatorios(0x9E37_79B9 + i as u64, n), destino)
            .expect("enviar bytes aleatórios");
    }
    // JSON válido, mas sem o campo `app`: não é do streamaZap.
    let sem_app = br#"{"v":4,"tipo":"sala","sala":{"id":"lixo_sem_app","nome":"Lixo","acesso":"aberta"}}"#;
    remetente.send_to(sem_app, destino).expect("enviar JSON sem app");
    // Texto que não é JSON.
    remetente.send_to(b"nao e json", destino).expect("enviar texto");

    // A sala legítima de A segue chegando: B não pode ter travado com o lixo.
    let chegou = espera(Duration::from_secs(2), || {
        b.rooms().iter().any(|(r, _)| r.room.id == "s_1")
    })
    .await;
    assert!(chegou, "B deveria continuar recebendo a sala legítima após o lixo");

    // Nada do lixo aparece nas tabelas.
    let salas = b.rooms();
    assert!(
        salas.iter().all(|(r, _)| r.room.id != "lixo_sem_app"),
        "JSON sem app não pode virar sala"
    );
    assert!(b.peers().is_empty(), "lixo não pode virar par");
    assert!(b.other_versions().is_empty(), "lixo não pode virar outra versão");
}
