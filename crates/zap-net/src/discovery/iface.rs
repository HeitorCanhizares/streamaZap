//! Interfaces de rede e broadcast dirigido.

use std::net::Ipv4Addr;

use super::InterfaceInfo;

/// Radmin VPN usa a faixa 26.0.0.0/8.
const RADMIN_PRIMEIRO_OCTETO: u8 = 26;

/// Interfaces IPv4 ativas, sem loopback; Radmin (26.0.0.0/8) primeiro.
pub fn ipv4_interfaces() -> Vec<InterfaceInfo> {
    let brutas = match if_addrs::get_if_addrs() {
        Ok(lista) => lista,
        Err(erro) => {
            tracing::warn!(%erro, "não foi possível listar interfaces de rede");
            return Vec::new();
        }
    };

    let candidatas = brutas
        .into_iter()
        .filter_map(|iface| match iface.addr {
            if_addrs::IfAddr::V4(v4) => Some((iface.name, v4.ip, v4.netmask, v4.broadcast)),
            _ => None,
        })
        .collect();

    select(candidatas)
}

/// Pura: broadcast dirigido de `ip`/`netmask`.
pub fn directed_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) | !u32::from(netmask))
}

/// Pura: filtra loopback e link-local, calcula o broadcast, remove duplicatas
/// por IP e coloca Radmin primeiro (ordenação estável).
fn select(lista: Vec<(String, Ipv4Addr, Ipv4Addr, Option<Ipv4Addr>)>) -> Vec<InterfaceInfo> {
    let mut vistos: Vec<Ipv4Addr> = Vec::new();
    let mut saida: Vec<InterfaceInfo> = Vec::new();

    for (name, ip, netmask, broadcast) in lista {
        // Loopback (127/8) e link-local (169.254/16) não servem para a LAN.
        if ip.is_loopback() || ip.is_link_local() {
            continue;
        }
        if vistos.contains(&ip) {
            continue;
        }
        vistos.push(ip);

        let is_radmin = ip.octets()[0] == RADMIN_PRIMEIRO_OCTETO;
        saida.push(InterfaceInfo {
            name,
            ip,
            broadcast: broadcast.unwrap_or_else(|| directed_broadcast(ip, netmask)),
            is_radmin,
        });
    }

    // `false` < `true`: Radmin (is_radmin = true) vai para o começo.
    saida.sort_by_key(|i| !i.is_radmin);
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr::new(a, b, c, d)
    }

    #[test]
    fn broadcast_24() {
        assert_eq!(
            directed_broadcast(ip(192, 168, 1, 10), ip(255, 255, 255, 0)),
            ip(192, 168, 1, 255)
        );
    }

    #[test]
    fn broadcast_radmin() {
        assert_eq!(
            directed_broadcast(ip(26, 1, 2, 3), ip(255, 0, 0, 0)),
            ip(26, 255, 255, 255)
        );
    }

    #[test]
    fn select_ignora_loopback_e_linklocal() {
        let lista = vec![
            ("lo".to_string(), ip(127, 0, 0, 1), ip(255, 0, 0, 0), None),
            ("eth9".to_string(), ip(169, 254, 10, 20), ip(255, 255, 0, 0), None),
            (
                "eth0".to_string(),
                ip(192, 168, 1, 5),
                ip(255, 255, 255, 0),
                Some(ip(192, 168, 1, 255)),
            ),
        ];
        let out = select(lista);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "eth0");
        assert_eq!(out[0].ip, ip(192, 168, 1, 5));
    }

    #[test]
    fn select_radmin_primeiro() {
        let lista = vec![
            ("eth".to_string(), ip(192, 168, 1, 5), ip(255, 255, 255, 0), None),
            ("radmin".to_string(), ip(26, 4, 5, 6), ip(255, 0, 0, 0), None),
        ];
        let out = select(lista);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "radmin");
        assert!(out[0].is_radmin);
        assert_eq!(out[0].broadcast, ip(26, 255, 255, 255));
        assert_eq!(out[1].name, "eth");
        assert!(!out[1].is_radmin);
        assert_eq!(out[1].broadcast, ip(192, 168, 1, 255));
    }

    #[test]
    fn select_dedup_por_ip() {
        let lista = vec![
            ("eth0".to_string(), ip(192, 168, 1, 5), ip(255, 255, 255, 0), None),
            ("eth0:1".to_string(), ip(192, 168, 1, 5), ip(255, 255, 255, 0), None),
        ];
        let out = select(lista);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "eth0");
    }

    #[test]
    fn ipv4_interfaces_nao_entra_em_panico() {
        let out = ipv4_interfaces();
        assert!(out.iter().all(|i| !i.ip.is_loopback()));
    }
}
