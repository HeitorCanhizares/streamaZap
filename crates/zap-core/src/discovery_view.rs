//! Descoberta ↔ interface: anúncios que publicamos e listas que mostramos.

use std::net::IpAddr;

use zap_net::discovery::{InterfaceInfo, PeerInfo};
use zap_proto::discovery::{Highlight, OwnerSummary, PresenceAnnouncement, RoomAnnouncement, RoomSummary};
use zap_proto::{Access, MediaAddress, Participant, Room, RoomMode};

use crate::snapshot::{NetworkStatus, PeerListing, RoomListing};

/// Anúncio da minha sala (`dono` = o participante dono, `destaque` = `(altura, fps)` da camada 0
/// da trilha de vídeo mais recente, se houver). `versao_app` = `env!("CARGO_PKG_VERSION")`.
pub fn room_announcement(
    room: &Room,
    owner: &Participant,
    address: MediaAddress,
    participants: u32,
    highlight: Option<(u16, u8)>,
) -> RoomAnnouncement {
    RoomAnnouncement {
        room: RoomSummary {
            id: room.id.clone(),
            name: room.name.clone(),
            access: room.access,
        },
        owner: OwnerSummary {
            id: owner.id.clone(),
            name: owner.name.clone(),
            color: owner.color.clone(),
        },
        address,
        participants,
        highlight: highlight.map(|(height, fps)| Highlight { height, fps }),
        // Quem ouve compara com a própria versão para avisar de incompatibilidade.
        app_version: env!("CARGO_PKG_VERSION").into(),
    }
}

pub fn presence_announcement(install_id: &str, name: &str, hears: Vec<String>) -> PresenceAnnouncement {
    PresenceAnnouncement {
        install_id: install_id.into(),
        name: name.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        hears,
    }
}

/// `RoomListing::basic(...)` com `room_id` = id da sala, `host` = nome do dono, `has_password`,
/// `mode` Local, `addr` = `"ip:porta"` (ip de origem do anúncio quando `enderecos` está vazio,
/// senão o primeiro endereço), `live_title`/`quality` quando há `destaque` (`"1080p60"`),
/// `version_warn` se `versao_app` diferir da nossa.
pub fn room_listings(rooms: &[(RoomAnnouncement, IpAddr)]) -> Vec<RoomListing> {
    rooms
        .iter()
        .map(|(an, origem)| {
            let mut listing = RoomListing::basic(
                an.room.id.clone(),
                an.room.name.clone(),
                an.owner.name.clone(),
                an.participants,
                an.room.access == Access::Password,
                RoomMode::Local,
            );

            // Sem endereço anunciado, o próprio pacote diz de onde veio: usamos esse IP.
            let ip = match an.address.addresses.first() {
                Some(primeiro) => primeiro.clone(),
                None => origem.to_string(),
            };
            listing.addr = Some(format!("{ip}:{}", an.address.port));

            if let Some(h) = &an.highlight {
                let texto = format!("{}p{}", h.height, h.fps);
                listing.live_title = Some(texto.clone());
                listing.quality = Some(texto);
            }

            // Versão diferente da nossa: a interface mostra o aviso ao lado da sala.
            if an.app_version != env!("CARGO_PKG_VERSION") {
                listing.version_warn = Some(format!("v{}", an.app_version));
            }

            listing
        })
        .collect()
}

pub fn peer_listings(peers: &[PeerInfo]) -> Vec<PeerListing> {
    peers
        .iter()
        .map(|p| PeerListing {
            name: p.name.clone(),
            version: p.app_version.clone(),
            ip: p.ip.to_string(),
            receives_you: p.hears_me,
        })
        .collect()
}

/// `radmin_ip` = ip da primeira interface com `is_radmin`; `interfaces` = nomes.
pub fn network_status(ifaces: &[InterfaceInfo]) -> NetworkStatus {
    NetworkStatus {
        radmin_ip: ifaces.iter().find(|i| i.is_radmin).map(|i| i.ip.to_string()),
        interfaces: ifaces.iter().map(|i| i.name.clone()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    use zap_proto::Role;

    fn sala() -> Room {
        Room {
            id: "s_1".into(),
            code: None,
            name: "Corrida em Interlagos".into(),
            owner_id: "u_1".into(),
            access: Access::Open,
            mode: RoomMode::Local,
            created_at: "2026-11-04T10:00:00Z".into(),
        }
    }

    fn dono() -> Participant {
        Participant {
            id: "u_1".into(),
            name: "Rafa".into(),
            color: "#5CD6FF".into(),
            role: Role::Owner,
            joined_at: "2026-11-04T10:00:00Z".into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            protocol: 4,
            latency_ms: None,
            watching: None,
        }
    }

    fn endereco(ips: Vec<String>) -> MediaAddress {
        MediaAddress {
            addresses: ips,
            port: 47800,
            fingerprint: "ab12".into(),
        }
    }

    fn anuncio(ips: Vec<String>, destaque: Option<Highlight>, versao: &str) -> RoomAnnouncement {
        RoomAnnouncement {
            room: RoomSummary {
                id: "s_1".into(),
                name: "Corrida em Interlagos".into(),
                access: Access::Open,
            },
            owner: OwnerSummary {
                id: "u_1".into(),
                name: "Rafa".into(),
                color: "#5CD6FF".into(),
            },
            address: endereco(ips),
            participants: 3,
            highlight: destaque,
            app_version: versao.into(),
        }
    }

    #[test]
    fn anuncio_sala_campos() {
        let a = room_announcement(
            &sala(),
            &dono(),
            endereco(vec!["26.9.77.140".into()]),
            3,
            Some((1080, 60)),
        );
        assert_eq!(a.room.id, "s_1");
        assert_eq!(a.room.name, "Corrida em Interlagos");
        assert_eq!(a.room.access, Access::Open);
        assert_eq!(a.owner.id, "u_1");
        assert_eq!(a.owner.name, "Rafa");
        assert_eq!(a.owner.color, "#5CD6FF");
        assert_eq!(a.address, endereco(vec!["26.9.77.140".into()]));
        assert_eq!(a.participants, 3);
        assert_eq!(a.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(a.highlight, Some(Highlight { height: 1080, fps: 60 }));

        let sem = room_announcement(&sala(), &dono(), endereco(vec![]), 1, None);
        assert_eq!(sem.highlight, None);
    }

    #[test]
    fn anuncio_presenca() {
        let p = presence_announcement("i_1", "Heitor", vec!["i_2".into()]);
        assert_eq!(p.install_id, "i_1");
        assert_eq!(p.name, "Heitor");
        assert_eq!(p.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(p.hears, vec!["i_2".to_owned()]);
    }

    #[test]
    fn listagem_basica_e_destaque() {
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        let an = anuncio(
            vec!["26.9.77.140".into()],
            Some(Highlight { height: 1080, fps: 60 }),
            env!("CARGO_PKG_VERSION"),
        );
        let listas = room_listings(&[(an, ip)]);
        assert_eq!(listas.len(), 1);
        let l = &listas[0];
        assert_eq!(l.room_id, "s_1");
        assert_eq!(l.name, "Corrida em Interlagos");
        assert_eq!(l.host, "Rafa");
        assert_eq!(l.participants, 3);
        assert!(!l.has_password);
        assert_eq!(l.mode, RoomMode::Local);
        assert_eq!(l.addr.as_deref(), Some("26.9.77.140:47800"));
        assert_eq!(l.live_title.as_deref(), Some("1080p60"));
        assert_eq!(l.quality.as_deref(), Some("1080p60"));
        assert_eq!(l.version_warn, None);

        let mut com_senha = anuncio(vec![], None, env!("CARGO_PKG_VERSION"));
        com_senha.room.access = Access::Password;
        let l2 = &room_listings(&[(com_senha, ip)])[0];
        assert!(l2.has_password);
        assert_eq!(l2.live_title, None);
        assert_eq!(l2.quality, None);
    }

    #[test]
    fn listagem_usa_ip_de_origem_quando_sem_enderecos() {
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        let an = anuncio(vec![], None, env!("CARGO_PKG_VERSION"));
        let l = &room_listings(&[(an, ip)])[0];
        assert_eq!(l.addr.as_deref(), Some("10.0.0.9:47800"));
    }

    #[test]
    fn listagem_aviso_de_versao() {
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        let an = anuncio(vec!["10.0.0.9".into()], None, "9.9.9");
        let l = &room_listings(&[(an, ip)])[0];
        assert_eq!(l.version_warn.as_deref(), Some("v9.9.9"));
    }

    #[test]
    fn peers_mapeiam_receives_you() {
        let peers = vec![
            PeerInfo {
                install_id: "i_2".into(),
                name: "Heitor".into(),
                app_version: "3.0.0".into(),
                ip: IpAddr::V4(Ipv4Addr::new(26, 1, 2, 3)),
                hears_me: true,
            },
            PeerInfo {
                install_id: "i_3".into(),
                name: "Ana".into(),
                app_version: "3.1.0".into(),
                ip: IpAddr::V4(Ipv4Addr::new(26, 4, 5, 6)),
                hears_me: false,
            },
        ];
        let out = peer_listings(&peers);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "Heitor");
        assert_eq!(out[0].version, "3.0.0");
        assert_eq!(out[0].ip, "26.1.2.3");
        assert!(out[0].receives_you);
        assert_eq!(out[1].name, "Ana");
        assert!(!out[1].receives_you);
    }

    #[test]
    fn network_status_radmin_e_nomes() {
        let eth = InterfaceInfo {
            name: "Ethernet".into(),
            ip: Ipv4Addr::new(192, 168, 0, 10),
            broadcast: Ipv4Addr::new(192, 168, 0, 255),
            is_radmin: false,
        };
        let radmin = InterfaceInfo {
            name: "Radmin VPN".into(),
            ip: Ipv4Addr::new(26, 14, 7, 88),
            broadcast: Ipv4Addr::new(26, 255, 255, 255),
            is_radmin: true,
        };

        let com = network_status(&[eth.clone(), radmin]);
        assert_eq!(com.radmin_ip.as_deref(), Some("26.14.7.88"));
        assert_eq!(com.interfaces, vec!["Ethernet".to_owned(), "Radmin VPN".to_owned()]);

        let sem = network_status(&[eth]);
        assert_eq!(sem.radmin_ip, None);
        assert_eq!(sem.interfaces, vec!["Ethernet".to_owned()]);
    }
}
