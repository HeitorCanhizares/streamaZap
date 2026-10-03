"""Descoberta automática de salas por broadcast UDP.

O host anuncia a sala periodicamente em todas as interfaces (incluindo a do
Radmin VPN). Quem está na tela inicial escuta esses anúncios e monta a lista.

O mesmo socket que anuncia também recebe pedidos de "conexão reversa": se o
espectador não consegue abrir a conexão TCP até o host (firewall/antivírus do
host bloqueando entrada), ele responde ao anúncio pedindo que o host conecte
nele. O Windows aceita respostas a um broadcast por alguns segundos mesmo quando
a entrada está bloqueada, e o host anuncia a cada 1,5 s.
"""

from __future__ import annotations

import json
import logging
import socket
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass, field

from streamazap import __version__, config, netutil

log = logging.getLogger(__name__)


@dataclass
class Room:
    room_id: str
    name: str
    host_name: str
    port: int
    viewers: int
    locked: bool
    addresses: list[str] = field(default_factory=list)
    # Porta UDP de onde vêm os anúncios, por endereço (para pedir conexão reversa).
    callback_ports: dict[str, int] = field(default_factory=dict)
    version: int = config.PROTOCOL_VERSION
    app_version: str = ""
    last_seen: float = 0.0

    @property
    def compatible(self) -> bool:
        return self.version == config.PROTOCOL_VERSION


def _message(kind: str, **fields) -> bytes:
    return json.dumps({"app": config.DISCOVERY_MAGIC, "v": config.PROTOCOL_VERSION, "type": kind, **fields}).encode("utf-8")


def build_announcement(room_id: str, name: str, host_name: str, port: int, viewers: int, locked: bool) -> bytes:
    return _message(
        "announce", id=room_id, name=name, host=host_name, port=port, viewers=viewers, locked=locked, appv=__version__
    )


def build_callback_request(room_id: str, port: int, token: str) -> bytes:
    return _message("callback", room=room_id, port=port, token=token)


def parse_message(data: bytes) -> dict | None:
    """Mensagem de descoberta válida (de qualquer versão) ou None."""
    try:
        msg = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    if not isinstance(msg, dict) or msg.get("app") != config.DISCOVERY_MAGIC or not isinstance(msg.get("v"), int):
        return None
    msg.setdefault("type", "announce")  # versões antigas não mandavam "type"
    if msg["type"] == "announce":
        if not isinstance(msg.get("id"), str):
            return None
        if not isinstance(msg.get("port"), int) or not 0 < msg["port"] < 65536:
            return None
    elif msg["type"] == "callback":
        if not isinstance(msg.get("port"), int) or not 0 < msg["port"] < 65536:
            return None
    return msg


def parse_announcement(data: bytes) -> dict | None:
    msg = parse_message(data)
    if msg is None or msg["type"] != "announce" or msg["v"] != config.PROTOCOL_VERSION:
        return None
    return msg


class Announcer:
    """Anuncia uma sala até ser parado e atende pedidos de conexão reversa."""

    def __init__(
        self,
        info: Callable[[], dict],
        port: int = config.DISCOVERY_PORT,
        on_callback: Callable[[str, int], None] = lambda ip, port: None,
    ):
        self.room_id = uuid.uuid4().hex
        self._info = info
        self._port = port
        self._on_callback = on_callback
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="announcer", daemon=True)
        self._recent_callbacks: dict[str, float] = {}

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def _handle(self, data: bytes, ip: str) -> None:
        msg = parse_message(data)
        if msg is None or msg["type"] != "callback" or msg.get("room") != self.room_id:
            return
        # O pedido chega repetido (várias tentativas e, com várias placas de rede, por
        # vários IPs); o token identifica o pedido para atender uma vez só.
        key = str(msg.get("token") or f"{ip}:{msg['port']}")
        now = time.monotonic()
        if key in self._recent_callbacks:
            return
        self._recent_callbacks = {k: t for k, t in self._recent_callbacks.items() if now - t < 30}
        self._recent_callbacks[key] = now
        self._on_callback(ip, msg["port"])

    def _run(self) -> None:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        sock.bind(("0.0.0.0", 0))
        try:
            while not self._stop.is_set():
                payload = build_announcement(self.room_id, **self._info())
                for target in netutil.broadcast_targets():
                    try:
                        sock.sendto(payload, (target, self._port))
                    except OSError as exc:
                        log.debug("falha ao anunciar em %s: %s", target, exc)
                deadline = time.monotonic() + config.ANNOUNCE_INTERVAL
                while not self._stop.is_set() and (remaining := deadline - time.monotonic()) > 0:
                    sock.settimeout(remaining)
                    try:
                        data, (ip, _) = sock.recvfrom(4096)
                    except TimeoutError:
                        break
                    except OSError:
                        continue  # ex.: WSAECONNRESET no Windows
                    self._handle(data, ip)
        finally:
            sock.close()


def request_callback(room: Room, port: int, attempts: int = 3) -> None:
    """Pede ao host da sala que conecte em nós na porta TCP `port`."""
    payload = build_callback_request(room.room_id, port, uuid.uuid4().hex)
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        for attempt in range(attempts):
            for ip, udp_port in room.callback_ports.items():
                try:
                    sock.sendto(payload, (ip, udp_port))
                except OSError as exc:
                    log.debug("pedido de conexão reversa para %s falhou: %s", ip, exc)
            if attempt < attempts - 1:
                time.sleep(0.4)


class RoomBrowser:
    """Escuta anúncios e mantém a lista de salas ativas."""

    def __init__(self, port: int = config.DISCOVERY_PORT):
        self._port = port
        self._rooms: dict[str, Room] = {}
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="room-browser", daemon=True)
        self.error: str | None = None

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def rooms(self) -> list[Room]:
        now = time.monotonic()
        with self._lock:
            for room_id in [r for r, room in self._rooms.items() if now - room.last_seen > config.ROOM_TIMEOUT]:
                del self._rooms[room_id]
            return sorted(self._rooms.values(), key=lambda r: (not r.compatible, r.name.lower()))

    def handle_datagram(self, data: bytes, sender_ip: str, sender_port: int = 0) -> None:
        msg = parse_message(data)
        if msg is None or msg["type"] != "announce":
            return
        with self._lock:
            room = self._rooms.get(msg["id"])
            if room is None:
                room = Room(msg["id"], "", "", 0, 0, False)
                self._rooms[msg["id"]] = room
            room.name = str(msg.get("name", "Sala"))[:80]
            room.host_name = str(msg.get("host", "?"))[:80]
            room.port = msg["port"]
            room.viewers = int(msg.get("viewers", 0))
            room.locked = bool(msg.get("locked", False))
            room.version = msg["v"]
            room.app_version = str(msg.get("appv") or "")[:20]
            room.last_seen = time.monotonic()
            # A mesma sala pode chegar por várias interfaces (LAN e Radmin).
            if sender_ip not in room.addresses:
                room.addresses.append(sender_ip)
            if sender_port:
                room.callback_ports[sender_ip] = sender_port

    def _run(self) -> None:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            sock.bind(("", self._port))
        except OSError as exc:
            self.error = f"Não foi possível escutar a porta UDP {self._port}: {exc}"
            log.error(self.error)
            sock.close()
            return
        sock.settimeout(0.5)
        try:
            while not self._stop.is_set():
                try:
                    data, (ip, port) = sock.recvfrom(4096)
                except TimeoutError:
                    continue
                except OSError:
                    continue  # ex.: WSAECONNRESET no Windows
                self.handle_datagram(data, ip, port)
        finally:
            sock.close()
