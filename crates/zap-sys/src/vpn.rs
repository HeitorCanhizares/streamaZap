//! Detecção de VPNs conhecidas pelo nome do adaptador. Porte de `other_vpns` de `netutil.py`.

use std::collections::BTreeSet;

/// Tabela `(trecho em minúsculas, rótulo)`: Proton, NordLynx/NordVPN, ExpressVPN, Surfshark,
/// Windscribe, Mullvad, CyberGhost, PIA, Hotspot Shield, Hamachi, ZeroTier, Tailscale,
/// WireGuard, OpenVPN, TAP-Windows.
pub const VPNS_CONHECIDAS: &[(&str, &str)] = &[
    ("proton", "Proton VPN"),
    ("nordlynx", "NordVPN"),
    ("nordvpn", "NordVPN"),
    ("expressvpn", "ExpressVPN"),
    ("surfshark", "Surfshark"),
    ("windscribe", "Windscribe"),
    ("mullvad", "Mullvad"),
    ("cyberghost", "CyberGhost"),
    ("pia", "Private Internet Access"),
    ("privateinternetaccess", "Private Internet Access"),
    ("hotspot shield", "Hotspot Shield"),
    ("hamachi", "Hamachi"),
    ("zerotier", "ZeroTier"),
    ("tailscale", "Tailscale"),
    ("wireguard", "WireGuard"),
    ("openvpn", "OpenVPN"),
    ("tap-windows", "OpenVPN (TAP)"),
];

/// Rótulos de VPNs (exceto Radmin) cujo trecho aparece em algum dos `nomes` de adaptador.
/// Minúsculas, `_` vira espaço, nomes com "radmin" são ignorados; "pia" só vale no começo do nome.
/// Resultado ordenado e sem repetição. Função pura.
pub fn detectar_vpns(nomes: &[String]) -> Vec<String> {
    let mut achados: BTreeSet<&'static str> = BTreeSet::new();
    for nome in nomes {
        let norm = nome.to_lowercase().replace('_', " ");
        // Radmin tem tratamento próprio em `net.rs`; aqui não conta como VPN conhecida.
        if norm.contains("radmin") {
            continue;
        }
        for (chave, rotulo) in VPNS_CONHECIDAS {
            // "pia" é curto demais: "Mapia" não pode virar PIA, então exige o começo do nome.
            let vale = if *chave == "pia" {
                norm.starts_with("pia")
            } else {
                norm.contains(chave)
            };
            if vale {
                achados.insert(rotulo);
            }
        }
    }
    achados.into_iter().map(String::from).collect()
}

/// VPNs ativas neste PC (nomes das interfaces de `net::interfaces_ipv4` + adaptadores do sistema
/// via `if_addrs::get_if_addrs`). Fora do Windows também funciona (mesma lógica).
pub fn outras_vpns() -> Vec<String> {
    // Inclui IPv6 de propósito: adaptadores de VPN às vezes só têm endereço v6 configurado.
    let mut nomes: Vec<String> = match if_addrs::get_if_addrs() {
        Ok(v) => v.into_iter().map(|i| i.name).collect(),
        Err(e) => {
            tracing::warn!("falha ao listar adaptadores para detectar VPN: {e}");
            Vec::new()
        }
    };
    nomes.extend(crate::net::interfaces_ipv4().into_iter().map(|i| i.nome));
    detectar_vpns(&nomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nomes(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detecta_nordlynx_como_nordvpn() {
        assert_eq!(detectar_vpns(&nomes(&["NordLynx Tunnel"])), vec!["NordVPN"]);
    }

    #[test]
    fn detecta_varias_ordenadas_sem_repetir() {
        let r = detectar_vpns(&nomes(&[
            "Tailscale Tunnel",
            "NordLynx",
            "nordvpn adapter",
            "WireGuard Tunnel",
        ]));
        assert_eq!(r, vec!["NordVPN", "Tailscale", "WireGuard"]);
    }

    #[test]
    fn ignora_radmin() {
        assert!(detectar_vpns(&nomes(&["Radmin VPN"])).is_empty());
    }

    #[test]
    fn pia_so_no_comeco() {
        assert_eq!(detectar_vpns(&nomes(&["pia_x"])), vec!["Private Internet Access"]);
        assert!(detectar_vpns(&nomes(&["Mapia Adapter"])).is_empty());
    }

    #[test]
    fn sublinhado_vira_espaco() {
        assert_eq!(detectar_vpns(&nomes(&["hotspot_shield"])), vec!["Hotspot Shield"]);
    }

    #[test]
    fn tap_windows_e_openvpn_tap() {
        assert_eq!(
            detectar_vpns(&nomes(&["TAP-Windows Adapter V9"])),
            vec!["OpenVPN (TAP)"]
        );
    }

    #[test]
    fn lista_vazia() {
        assert!(detectar_vpns(&[]).is_empty());
    }

    #[test]
    fn outras_vpns_nao_entra_em_panico() {
        // Só garante que roda sem pânico, qualquer que seja o ambiente de teste.
        let _ = outras_vpns();
    }
}
