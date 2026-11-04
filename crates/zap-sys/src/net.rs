//! Interfaces IPv4 (LAN, Radmin VPN, ...). Porte de `streamazap/netutil.py`.

use std::net::Ipv4Addr;

/// Interface IPv4 ativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Nome amigável (no Windows, o nome do adaptador).
    pub nome: String,
    pub ip: Ipv4Addr,
    /// Broadcast dirigido da sub-rede (`None` para /31 e /32).
    pub broadcast: Option<Ipv4Addr>,
    /// Nome contém "radmin" ou IP em `26.0.0.0/8`.
    pub radmin: bool,
}

/// Broadcast dirigido de `ip`/`mascara`; `None` se prefixo >= 31.
pub fn broadcast_address(ip: Ipv4Addr, mascara: Ipv4Addr) -> Option<Ipv4Addr> {
    let bits = mascara.to_bits();
    // Máscara contígua: uns à esquerda seguidos de zeros. Caso contrário, não é máscara de sub-rede.
    if bits.leading_ones() + bits.trailing_zeros() != 32 {
        return None;
    }
    // /31 e /32 não têm endereço de broadcast dirigido.
    if bits.count_ones() >= 31 {
        return None;
    }
    Some(Ipv4Addr::from(ip.to_bits() | !bits))
}

/// `true` se o nome contém "radmin" (sem diferenciar maiúsculas) ou o IP está em 26.0.0.0/8.
pub fn eh_radmin(nome: &str, ip: Ipv4Addr) -> bool {
    nome.to_lowercase().contains("radmin") || ip.octets()[0] == 26
}

/// Ordena com Radmin primeiro (ordem estável para o resto).
pub fn ordenar_radmin_primeiro(v: &mut [Interface]) {
    // `sort_by_key` é estável: `false < true`, então `!radmin` põe Radmin primeiro.
    v.sort_by_key(|i| !i.radmin);
}

/// Broadcast global, sempre o último destino.
const BROADCAST_GLOBAL: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 255);

/// Destinos de broadcast: um por interface (sem repetição) + `255.255.255.255` por último.
pub fn broadcast_targets(interfaces: &[Interface]) -> Vec<Ipv4Addr> {
    let mut destinos: Vec<Ipv4Addr> = Vec::new();
    for bc in interfaces.iter().filter_map(|i| i.broadcast) {
        // O global é acrescentado no fim, então não entra aqui (evita duplicar).
        if bc != BROADCAST_GLOBAL && !destinos.contains(&bc) {
            destinos.push(bc);
        }
    }
    destinos.push(BROADCAST_GLOBAL);
    destinos
}

/// Filtro de endereço: descarta loopback (`127/8`) e link-local (`169.254/16`).
/// Esses não servem para falar com outros computadores da rede.
fn aceita(ip: Ipv4Addr) -> bool {
    !ip.is_loopback() && !ip.is_link_local()
}

/// Interfaces IPv4 ativas (sem loopback nem link-local `169.254/16`), Radmin primeiro.
/// Usa o crate `if-addrs`; erro de sistema -> lista vazia.
pub fn interfaces_ipv4() -> Vec<Interface> {
    let todas = match if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("falha ao listar interfaces de rede: {e}");
            return Vec::new();
        }
    };

    let mut v: Vec<Interface> = Vec::new();
    for i in todas {
        // Interface desligada não serve, mesmo com endereço configurado.
        if !i.is_oper_up() {
            continue;
        }
        let v4 = match &i.addr {
            if_addrs::IfAddr::V4(v4) => v4,
            _ => continue,
        };
        let ip = v4.ip;
        if !aceita(ip) {
            continue;
        }
        // Broadcast calculado aqui: o do crate pode vir ausente ou diferente em alguns sistemas.
        let nova = Interface {
            radmin: eh_radmin(&i.name, ip),
            broadcast: broadcast_address(ip, v4.netmask),
            nome: i.name,
            ip,
        };
        // Mesmo nome e mesmo IP aparecem repetidos em alguns adaptadores; mantém só um.
        if !v.iter().any(|x| x.nome == nova.nome && x.ip == nova.ip) {
            v.push(nova);
        }
    }

    ordenar_radmin_primeiro(&mut v);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    fn iface(nome: &str, addr: &str, bc: Option<&str>, radmin: bool) -> Interface {
        Interface {
            nome: nome.to_string(),
            ip: ip(addr),
            broadcast: bc.map(ip),
            radmin,
        }
    }

    #[test]
    fn broadcast_de_24() {
        assert_eq!(
            broadcast_address(ip("192.168.1.10"), ip("255.255.255.0")),
            Some(ip("192.168.1.255"))
        );
    }

    #[test]
    fn broadcast_radmin_8() {
        assert_eq!(
            broadcast_address(ip("26.1.2.3"), ip("255.0.0.0")),
            Some(ip("26.255.255.255"))
        );
    }

    #[test]
    fn broadcast_31_e_32_none() {
        assert_eq!(broadcast_address(ip("10.0.0.1"), ip("255.255.255.254")), None);
        assert_eq!(broadcast_address(ip("10.0.0.1"), ip("255.255.255.255")), None);
    }

    #[test]
    fn broadcast_mascara_nao_contigua_none() {
        assert_eq!(broadcast_address(ip("10.0.0.1"), ip("255.0.255.0")), None);
        assert_eq!(broadcast_address(ip("10.0.0.1"), ip("255.255.0.128")), None);
    }

    #[test]
    fn radmin_por_nome_e_por_ip() {
        assert!(eh_radmin("Radmin VPN", ip("192.168.0.2")));
        assert!(eh_radmin("ADAPTADOR RADMIN", ip("10.0.0.2")));
        assert!(eh_radmin("Ethernet", ip("26.200.1.1")));
        assert!(!eh_radmin("Ethernet", ip("192.168.0.2")));
        assert!(!eh_radmin("Ethernet", ip("126.0.0.1")));
    }

    #[test]
    fn ordena_radmin_primeiro_estavel() {
        let mut v = vec![
            iface("Wi-Fi", "192.168.0.2", None, false),
            iface("Ethernet", "10.0.0.2", None, false),
            iface("Radmin A", "26.1.1.1", None, true),
            iface("Radmin B", "26.2.2.2", None, true),
            iface("Virtual", "172.16.0.2", None, false),
        ];
        ordenar_radmin_primeiro(&mut v);
        let nomes: Vec<&str> = v.iter().map(|i| i.nome.as_str()).collect();
        assert_eq!(nomes, ["Radmin A", "Radmin B", "Wi-Fi", "Ethernet", "Virtual"]);
    }

    #[test]
    fn targets_sem_repeticao_e_global_no_fim() {
        let v = vec![
            iface("A", "192.168.1.10", Some("192.168.1.255"), false),
            iface("B", "192.168.1.11", Some("192.168.1.255"), false),
            iface("C", "10.0.0.2", Some("10.0.0.255"), false),
            iface("D", "10.0.0.3", Some("255.255.255.255"), false),
            iface("E", "172.16.0.2", None, false),
        ];
        assert_eq!(
            broadcast_targets(&v),
            vec![ip("192.168.1.255"), ip("10.0.0.255"), ip("255.255.255.255")]
        );
    }

    #[test]
    fn targets_vazio_so_global() {
        assert_eq!(broadcast_targets(&[]), vec![ip("255.255.255.255")]);
    }

    #[test]
    fn aceita_rejeita_loopback_e_link_local() {
        assert!(!aceita(ip("127.0.0.1")));
        assert!(!aceita(ip("127.255.255.254")));
        assert!(!aceita(ip("169.254.10.20")));
        assert!(!aceita(ip("169.254.0.1")));
    }

    #[test]
    fn aceita_aceita_privado_e_radmin() {
        assert!(aceita(ip("192.168.0.2")));
        assert!(aceita(ip("10.0.0.2")));
        assert!(aceita(ip("172.16.0.2")));
        assert!(aceita(ip("26.1.2.3")));
        // Logo acima do link-local e logo abaixo do loopback não são bloqueados.
        assert!(aceita(ip("169.253.255.255")));
        assert!(aceita(ip("126.255.255.255")));
    }

    #[test]
    fn interfaces_ipv4_nao_entra_em_panico_e_sem_loopback() {
        // Roda a função real: o resultado depende da máquina, então só checamos as invariantes.
        let v = interfaces_ipv4();
        for i in &v {
            assert!(!i.ip.is_loopback(), "loopback não deve aparecer: {}", i.ip);
            assert!(!i.ip.is_link_local(), "link-local não deve aparecer: {}", i.ip);
        }
        // Radmin vem sempre antes dos demais.
        assert!(v.windows(2).all(|w| w[0].radmin || !w[1].radmin));
    }
}
