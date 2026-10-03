"""Utilitários para enumerar interfaces IPv4 (LAN, Radmin VPN, Hamachi, ...)."""

from __future__ import annotations

import ipaddress
import socket
from dataclasses import dataclass

import psutil

RADMIN_NETWORK = ipaddress.ip_network("26.0.0.0/8")


@dataclass(frozen=True)
class Interface:
    name: str
    ip: str
    broadcast: str | None

    @property
    def is_radmin(self) -> bool:
        return "radmin" in self.name.lower() or ipaddress.ip_address(self.ip) in RADMIN_NETWORK


def broadcast_address(ip: str, netmask: str) -> str | None:
    """Endereço de broadcast dirigido da sub-rede, ou None se não houver (máscara /31, /32)."""
    try:
        network = ipaddress.ip_network(f"{ip}/{netmask}", strict=False)
    except ValueError:
        return None
    if network.prefixlen >= 31:
        return None
    return str(network.broadcast_address)


def ipv4_interfaces() -> list[Interface]:
    result = []
    stats = psutil.net_if_stats()
    for name, addrs in psutil.net_if_addrs().items():
        if name in stats and not stats[name].isup:
            continue
        for addr in addrs:
            if addr.family != socket.AF_INET:
                continue
            ip = ipaddress.ip_address(addr.address)
            if ip.is_loopback or ip.is_link_local:
                continue
            bcast = broadcast_address(addr.address, addr.netmask) if addr.netmask else None
            result.append(Interface(name, addr.address, bcast))
    # Radmin primeiro: é por onde os amigos costumam se conectar.
    result.sort(key=lambda i: not i.is_radmin)
    return result


def broadcast_targets(interfaces: list[Interface] | None = None) -> list[str]:
    """Destinos de broadcast: um por interface + o broadcast global.

    No Windows o 255.255.255.255 só sai pela interface padrão, por isso também
    enviamos para o broadcast dirigido de cada interface (ex.: 26.255.255.255 no Radmin).
    """
    interfaces = ipv4_interfaces() if interfaces is None else interfaces
    targets = [i.broadcast for i in interfaces if i.broadcast]
    targets.append("255.255.255.255")
    return list(dict.fromkeys(targets))


# VPNs com "kill switch" bloqueiam tudo que não passa pelo túnel delas — inclusive o
# Radmin (a faixa 26.x não conta como "rede local"). Isso fica abaixo do Firewall do
# Windows, então a verificação de regras não enxerga.
_OTHER_VPNS = {
    "proton": "Proton VPN",
    "nordlynx": "NordVPN",
    "nordvpn": "NordVPN",
    "expressvpn": "ExpressVPN",
    "surfshark": "Surfshark",
    "windscribe": "Windscribe",
    "mullvad": "Mullvad",
    "cyberghost": "CyberGhost",
    "pia": "Private Internet Access",
    "privateinternetaccess": "Private Internet Access",
    "hotspot shield": "Hotspot Shield",
    "hamachi": "Hamachi",
    "zerotier": "ZeroTier",
    "wireguard": "WireGuard",
    "openvpn": "OpenVPN",
    "tap-windows": "OpenVPN (TAP)",
}


def other_vpns() -> list[str]:
    """VPNs (além do Radmin) com adaptador de rede ou programa em execução."""
    found = set()
    names = list(psutil.net_if_addrs())
    try:
        names += [proc.info["name"] or "" for proc in psutil.process_iter(["name"])]
    except psutil.Error:
        pass
    for name in names:
        lowered = name.lower().replace("_", " ")
        if "radmin" in lowered:
            continue
        for key, label in _OTHER_VPNS.items():
            if key == "pia" and not lowered.startswith("pia"):
                continue
            if key in lowered:
                found.add(label)
    return sorted(found)
