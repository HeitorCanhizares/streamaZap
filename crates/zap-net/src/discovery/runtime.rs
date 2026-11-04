//! Socket, tasks de envio/recepção.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use zap_proto::discovery::{encode_announcement, parse_announcement, Announcement};

use super::{record_announcement, Discovery, DiscoveryConfig, Tables};
use crate::NetError;

impl Discovery {
    /// Abre o socket (`SO_REUSEADDR`, `SO_BROADCAST`) e sobe as tasks de envio e recepção.
    pub async fn start(cfg: DiscoveryConfig, install_id: String) -> Result<Self, NetError> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        // Reuso de endereço deixa várias instâncias/abas na mesma máquina ouvirem a porta.
        socket.set_reuse_address(true)?;
        #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
        socket.set_reuse_port(true)?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        let bind: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, cfg.port).into();
        socket.bind(&bind.into())?;
        let socket = Arc::new(UdpSocket::from_std(socket.into())?);

        let tables = Arc::new(Mutex::new(Tables::default()));

        let h_envio = tokio::spawn(tarefa_envio(socket.clone(), tables.clone(), cfg.clone()));
        let h_recepcao = tokio::spawn(tarefa_recepcao(socket, tables.clone(), install_id.clone()));

        Ok(Discovery {
            tables,
            cfg,
            install_id,
            tasks: vec![h_envio, h_recepcao],
        })
    }
}

/// Destinos de cada rodada: broadcast (se ligado) e sempre os alvos extras.
fn destinos(cfg: &DiscoveryConfig) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    if cfg.broadcast {
        for i in super::iface::ipv4_interfaces() {
            out.push(SocketAddrV4::new(i.broadcast, cfg.port).into());
        }
        out.push(SocketAddrV4::new(Ipv4Addr::BROADCAST, cfg.port).into());
    }
    out.extend(cfg.extra_targets.iter().copied());
    out
}

async fn tarefa_envio(socket: Arc<UdpSocket>, tables: Arc<Mutex<Tables>>, cfg: DiscoveryConfig) {
    let mut tick = tokio::time::interval(cfg.interval);
    loop {
        tick.tick().await;
        // Clona sob o lock e solta antes de enviar.
        let (sala, presenca) = {
            let t = tables.lock();
            (t.my_room.clone(), t.my_presence.clone())
        };
        let mut datagramas: Vec<Vec<u8>> = Vec::new();
        if let Some(r) = sala {
            match encode_announcement(&Announcement::Room(r)) {
                Ok(b) => datagramas.push(b),
                Err(erro) => tracing::debug!(%erro, "falha ao codificar anúncio de sala"),
            }
        }
        if let Some(p) = presenca {
            match encode_announcement(&Announcement::Presence(p)) {
                Ok(b) => datagramas.push(b),
                Err(erro) => tracing::debug!(%erro, "falha ao codificar presença"),
            }
        }
        if datagramas.is_empty() {
            continue;
        }
        for destino in destinos(&cfg) {
            for d in &datagramas {
                if let Err(erro) = socket.send_to(d, destino).await {
                    tracing::debug!(%erro, %destino, "falha ao enviar anúncio");
                }
            }
        }
    }
}

async fn tarefa_recepcao(socket: Arc<UdpSocket>, tables: Arc<Mutex<Tables>>, install_id: String) {
    let mut buf = [0u8; 1500];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, addr)) => {
                if let Some(p) = parse_announcement(&buf[..n]) {
                    record_announcement(&mut tables.lock(), p, addr.ip(), &install_id, Instant::now());
                }
            }
            Err(erro) => {
                // Em Windows um ICMP de porta fechada vira erro aqui; segue ouvindo.
                tracing::debug!(%erro, "erro ao receber anúncio");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use zap_proto::discovery::{OwnerSummary, PresenceAnnouncement, RoomAnnouncement, RoomSummary};
    use zap_proto::types::{Access, MediaAddress};

    use super::*;

    fn anuncio_de_exemplo(dono: &str) -> RoomAnnouncement {
        RoomAnnouncement {
            room: RoomSummary {
                id: "s_1".into(),
                name: "Corrida".into(),
                access: Access::Open,
            },
            owner: OwnerSummary {
                id: dono.into(),
                name: "Rafa".into(),
                color: "#5CD6FF".into(),
            },
            address: MediaAddress {
                addresses: vec!["127.0.0.1".into()],
                port: 47800,
                fingerprint: "ab12".into(),
            },
            participants: 1,
            highlight: None,
            app_version: "3.0.0".into(),
        }
    }

    fn presenca(id: &str) -> PresenceAnnouncement {
        PresenceAnnouncement {
            install_id: id.into(),
            name: "Heitor".into(),
            app_version: "3.0.0".into(),
            hears: vec![],
        }
    }

    fn cfg(port: u16, alvos: &[u16], timeout_ms: u64) -> DiscoveryConfig {
        DiscoveryConfig {
            port,
            interval: Duration::from_millis(100),
            room_timeout: Duration::from_millis(timeout_ms),
            extra_targets: alvos.iter().map(|p| SocketAddr::from(([127, 0, 0, 1], *p))).collect(),
            broadcast: false,
        }
    }

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

    #[tokio::test(flavor = "multi_thread")]
    async fn duas_instancias_se_enxergam() {
        let a = Discovery::start(cfg(47911, &[47912], 5000), "a".into()).await.unwrap();
        let b = Discovery::start(cfg(47912, &[47911], 5000), "b".into()).await.unwrap();
        a.set_room(Some(anuncio_de_exemplo("a")));
        b.set_presence(presenca("b"));
        let ok = espera(Duration::from_secs(1), || {
            !b.rooms().is_empty() && !a.peers().is_empty()
        })
        .await;
        assert!(ok);
        let rooms = b.rooms();
        assert_eq!(rooms[0].1, "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
        assert_eq!(a.peers()[0].install_id, "b");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ignora_proprio() {
        let a = Discovery::start(cfg(47913, &[47913], 5000), "a".into()).await.unwrap();
        a.set_room(Some(anuncio_de_exemplo("a")));
        a.set_presence(presenca("a"));
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(a.rooms().is_empty());
        assert!(a.peers().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sala_expira() {
        let a = Discovery::start(cfg(47914, &[47915], 500), "a".into()).await.unwrap();
        let b = Discovery::start(cfg(47915, &[], 500), "b".into()).await.unwrap();
        a.set_room(Some(anuncio_de_exemplo("a")));
        assert!(espera(Duration::from_secs(1), || !b.rooms().is_empty()).await);
        a.set_room(None);
        assert!(espera(Duration::from_secs(2), || b.rooms().is_empty()).await);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn outra_versao() {
        let b = Discovery::start(cfg(47916, &[], 5000), "b".into()).await.unwrap();
        let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let msg = format!(
            "{{\"app\":\"{}\",\"v\":3,\"tipo\":\"sala\"}}",
            zap_proto::DISCOVERY_MAGIC
        );
        s.send_to(msg.as_bytes(), "127.0.0.1:47916").unwrap();
        let ok = espera(Duration::from_secs(1), || !b.other_versions().is_empty()).await;
        assert!(ok);
        assert_eq!(
            b.other_versions()[0],
            ("127.0.0.1".parse::<std::net::IpAddr>().unwrap(), 3)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn drop_encerra() {
        let a = Discovery::start(cfg(47917, &[], 500), "a".into()).await.unwrap();
        drop(a);
        // O abort das tasks libera o socket de forma assíncrona.
        let mut ok = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let r = std::net::UdpSocket::bind("0.0.0.0:47917");
            if r.is_ok() {
                ok = true;
                break;
            }
        }
        assert!(ok);
        // Com reuse_port, o bind acima não prova a liberação; reabrir também deve funcionar.
        let _b = Discovery::start(cfg(47917, &[], 500), "b".into()).await.unwrap();
    }
}
