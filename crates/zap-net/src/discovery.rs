//! Descoberta de salas e presença na LAN/Radmin por UDP.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use zap_proto::discovery::{Announcement, ParsedAnnouncement, PresenceAnnouncement, RoomAnnouncement};

mod iface;
mod runtime;

pub use iface::{directed_broadcast, ipv4_interfaces};

/// Tempo máximo guardado nas tabelas (a maior janela de leitura, 10 s).
/// `record_announcement` não recebe a config, então poda por este teto fixo;
/// cada leitura (`rooms`, `peers`, ...) aplica a sua janela.
const RETENCAO: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    /// 47801 por padrão.
    pub port: u16,
    /// 1,5 s por padrão.
    pub interval: Duration,
    /// 5 s por padrão.
    pub room_timeout: Duration,
    /// Destinos unicast extras (testes).
    pub extra_targets: Vec<SocketAddr>,
    /// `false` nos testes.
    pub broadcast: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            port: zap_proto::DISCOVERY_PORT,
            interval: Duration::from_millis(1500),
            room_timeout: Duration::from_secs(5),
            extra_targets: Vec::new(),
            broadcast: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub install_id: String,
    pub name: String,
    pub app_version: String,
    pub ip: IpAddr,
    /// Esse par lista a minha instalação em `ouve`.
    pub hears_me: bool,
}

#[derive(Clone, Debug)]
pub struct InterfaceInfo {
    pub name: String,
    pub ip: Ipv4Addr,
    pub broadcast: Ipv4Addr,
    pub is_radmin: bool,
}

/// Estado compartilhado entre a task de rede e a API pública.
#[derive(Default)]
pub(crate) struct Tables {
    pub(crate) my_room: Option<RoomAnnouncement>,
    pub(crate) my_presence: Option<PresenceAnnouncement>,
    /// (anúncio, ip de origem, último visto)
    pub(crate) rooms: Vec<(RoomAnnouncement, IpAddr, std::time::Instant)>,
    pub(crate) peers: Vec<(PeerInfo, std::time::Instant)>,
    pub(crate) other_versions: Vec<(IpAddr, u32, std::time::Instant)>,
}

pub struct Discovery {
    pub(crate) tables: Arc<Mutex<Tables>>,
    pub(crate) cfg: DiscoveryConfig,
    #[allow(dead_code)]
    pub(crate) install_id: String,
    pub(crate) tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Discovery {
    pub fn set_room(&self, a: Option<RoomAnnouncement>) {
        self.tables.lock().my_room = a;
    }

    pub fn set_presence(&self, p: PresenceAnnouncement) {
        self.tables.lock().my_presence = Some(p);
    }

    /// Salas vistas, sem as expiradas.
    pub fn rooms(&self) -> Vec<(RoomAnnouncement, IpAddr)> {
        let limite = self.cfg.room_timeout;
        let t = self.tables.lock();
        t.rooms
            .iter()
            .filter(|(_, _, visto)| visto.elapsed() <= limite)
            .map(|(r, ip, _)| (r.clone(), *ip))
            .collect()
    }

    /// Pares vistos na janela de 2x o `room_timeout` (10 s por padrão).
    pub fn peers(&self) -> Vec<PeerInfo> {
        let limite = self.cfg.room_timeout * 2;
        let t = self.tables.lock();
        t.peers
            .iter()
            .filter(|(_, visto)| visto.elapsed() <= limite)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Versões de protocolo diferentes vistas recentemente, sem repetir (ip, versão).
    pub fn other_versions(&self) -> Vec<(IpAddr, u32)> {
        let limite = self.cfg.room_timeout;
        let t = self.tables.lock();
        let mut out: Vec<(IpAddr, u32)> = Vec::new();
        for (ip, v, visto) in &t.other_versions {
            if visto.elapsed() <= limite && !out.contains(&(*ip, *v)) {
                out.push((*ip, *v));
            }
        }
        out
    }
}

/// Aplica um anúncio recebido às tabelas. Ignora o que é meu e poda o que expirou.
pub(crate) fn record_announcement(
    t: &mut Tables,
    parsed: ParsedAnnouncement,
    from: IpAddr,
    my_install_id: &str,
    now: Instant,
) {
    let vivo = |visto: &Instant| now.saturating_duration_since(*visto) <= RETENCAO;
    t.rooms.retain(|(_, _, visto)| vivo(visto));
    t.peers.retain(|(_, visto)| vivo(visto));
    t.other_versions.retain(|(_, _, visto)| vivo(visto));

    match parsed {
        ParsedAnnouncement::Ok(Announcement::Room(r)) => {
            if r.owner.id == my_install_id {
                return;
            }
            // Mesma sala vinda do mesmo ip vira uma só entrada.
            match t
                .rooms
                .iter_mut()
                .find(|(a, ip, _)| a.room.id == r.room.id && *ip == from)
            {
                Some(e) => {
                    e.0 = r;
                    e.2 = now;
                }
                None => t.rooms.push((r, from, now)),
            }
        }
        ParsedAnnouncement::Ok(Announcement::Presence(p)) => {
            if p.install_id == my_install_id {
                return;
            }
            let info = PeerInfo {
                install_id: p.install_id,
                name: p.name,
                app_version: p.app_version,
                ip: from,
                hears_me: p.hears.iter().any(|h| h == my_install_id),
            };
            match t.peers.iter_mut().find(|(i, _)| i.install_id == info.install_id) {
                Some(e) => {
                    e.0 = info;
                    e.1 = now;
                }
                None => t.peers.push((info, now)),
            }
        }
        ParsedAnnouncement::OtherVersion { v, .. } => {
            match t
                .other_versions
                .iter_mut()
                .find(|(ip, ver, _)| *ip == from && *ver == v)
            {
                Some(e) => e.2 = now,
                None => t.other_versions.push((from, v, now)),
            }
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use zap_proto::discovery::{OwnerSummary, RoomSummary};
    use zap_proto::types::{Access, MediaAddress};

    use super::*;

    const EU: &str = "eu";

    fn ip(ultimo: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(26, 9, 77, ultimo))
    }

    fn discovery(room_timeout: Duration) -> Discovery {
        Discovery {
            tables: Arc::new(Mutex::new(Tables::default())),
            cfg: DiscoveryConfig {
                room_timeout,
                ..DiscoveryConfig::default()
            },
            install_id: EU.into(),
            tasks: vec![],
        }
    }

    fn sala(sala_id: &str, dono_id: &str) -> RoomAnnouncement {
        RoomAnnouncement {
            room: RoomSummary {
                id: sala_id.into(),
                name: "Corrida".into(),
                access: Access::Open,
            },
            owner: OwnerSummary {
                id: dono_id.into(),
                name: "Rafa".into(),
                color: "#5CD6FF".into(),
            },
            address: MediaAddress {
                addresses: vec!["26.9.77.140".into()],
                port: 47800,
                fingerprint: "ab12".into(),
            },
            participants: 3,
            highlight: None,
            app_version: "3.0.0".into(),
        }
    }

    fn presenca(install_id: &str, ouve: &[&str]) -> PresenceAnnouncement {
        PresenceAnnouncement {
            install_id: install_id.into(),
            name: "Heitor".into(),
            app_version: "3.0.0".into(),
            hears: ouve.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn ok_sala(r: RoomAnnouncement) -> ParsedAnnouncement {
        ParsedAnnouncement::Ok(Announcement::Room(r))
    }

    fn ok_presenca(p: PresenceAnnouncement) -> ParsedAnnouncement {
        ParsedAnnouncement::Ok(Announcement::Presence(p))
    }

    #[test]
    fn set_room_e_presence_guardam() {
        let d = discovery(Duration::from_secs(5));
        d.set_room(Some(sala("s_1", "outro")));
        d.set_presence(presenca("i_2", &[]));
        let t = d.tables.lock();
        assert_eq!(t.my_room.as_ref().map(|r| r.room.id.as_str()), Some("s_1"));
        assert_eq!(t.my_presence.as_ref().map(|p| p.install_id.as_str()), Some("i_2"));
    }

    #[test]
    fn record_room_aparece_em_rooms() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        record_announcement(&mut t, ok_sala(sala("s_1", "outro")), ip(1), EU, Instant::now());
        drop(t);
        let rooms = d.rooms();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].0.room.id, "s_1");
        assert_eq!(rooms[0].1, ip(1));
    }

    #[test]
    fn room_do_proprio_dono_ignorada() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        record_announcement(&mut t, ok_sala(sala("s_1", EU)), ip(1), EU, Instant::now());
        drop(t);
        assert!(d.rooms().is_empty());
    }

    #[test]
    fn room_expira() {
        let d = discovery(Duration::from_millis(50));
        let mut t = d.tables.lock();
        record_announcement(&mut t, ok_sala(sala("s_1", "outro")), ip(1), EU, Instant::now());
        drop(t);
        assert_eq!(d.rooms().len(), 1);
        std::thread::sleep(Duration::from_millis(80));
        assert!(d.rooms().is_empty());
    }

    #[test]
    fn upsert_nao_duplica() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        record_announcement(&mut t, ok_sala(sala("s_1", "outro")), ip(1), EU, Instant::now());
        record_announcement(&mut t, ok_sala(sala("s_1", "outro")), ip(1), EU, Instant::now());
        drop(t);
        assert_eq!(d.rooms().len(), 1);
    }

    #[test]
    fn presenca_hears_me() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        record_announcement(
            &mut t,
            ok_presenca(presenca("i_2", &["eu", "i_3"])),
            ip(2),
            EU,
            Instant::now(),
        );
        record_announcement(
            &mut t,
            ok_presenca(presenca("i_3", &["i_2"])),
            ip(3),
            EU,
            Instant::now(),
        );
        drop(t);
        let peers = d.peers();
        assert_eq!(peers.len(), 2);
        let p2 = peers.iter().find(|p| p.install_id == "i_2").expect("i_2");
        let p3 = peers.iter().find(|p| p.install_id == "i_3").expect("i_3");
        assert!(p2.hears_me);
        assert!(!p3.hears_me);
        assert_eq!(p2.ip, ip(2));
    }

    #[test]
    fn presenca_propria_ignorada() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        record_announcement(&mut t, ok_presenca(presenca(EU, &[])), ip(1), EU, Instant::now());
        drop(t);
        assert!(d.peers().is_empty());
    }

    #[test]
    fn outra_versao_listada_uma_vez() {
        let d = discovery(Duration::from_secs(5));
        let mut t = d.tables.lock();
        for _ in 0..3 {
            record_announcement(
                &mut t,
                ParsedAnnouncement::OtherVersion {
                    v: 5,
                    app_version: None,
                },
                ip(4),
                EU,
                Instant::now(),
            );
        }
        drop(t);
        assert_eq!(d.other_versions(), vec![(ip(4), 5)]);
    }
}
