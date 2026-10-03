"""Descoberta automática de salas por broadcast UDP.

O host anuncia a sala periodicamente em todas as interfaces (incluindo a do
Radmin VPN). Quem está na tela inicial escuta esses anúncios e monta a lista.
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

from streamazap import config, netutil

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
    last_seen: float = 0.0


def build_announcement(room_id: str, name: str, host_name: str, port: int, viewers: int, locked: bool) -> bytes:
    return json.dumps(
        {
            "app": config.DISCOVERY_MAGIC,
            "v": config.PROTOCOL_VERSION,
            "id": room_id,
            "name": name,
            "host": host_name,
            "port": port,
            "viewers": viewers,
            "locked": locked,
        }
    ).encode("utf-8")


def parse_announcement(data: bytes) -> dict | None:
    try:
        msg = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    if not isinstance(msg, dict) or msg.get("app") != config.DISCOVERY_MAGIC:
        return None
    if msg.get("v") != config.PROTOCOL_VERSION or not isinstance(msg.get("id"), str):
        return None
    if not isinstance(msg.get("port"), int) or not 0 < msg["port"] < 65536:
        return None
    return msg


class Announcer:
    """Anuncia uma sala até ser parado."""

    def __init__(self, info: Callable[[], dict], port: int = config.DISCOVERY_PORT):
        self.room_id = uuid.uuid4().hex
        self._info = info
        self._port = port
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="announcer", daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def _run(self) -> None:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        try:
            while not self._stop.is_set():
                info = self._info()
                payload = build_announcement(self.room_id, **info)
                for target in netutil.broadcast_targets():
                    try:
                        sock.sendto(payload, (target, self._port))
                    except OSError as exc:
                        log.debug("falha ao anunciar em %s: %s", target, exc)
                self._stop.wait(config.ANNOUNCE_INTERVAL)
        finally:
            sock.close()


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
            return sorted(self._rooms.values(), key=lambda r: r.name.lower())

    def handle_datagram(self, data: bytes, sender_ip: str) -> None:
        msg = parse_announcement(data)
        if msg is None:
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
            room.last_seen = time.monotonic()
            # A mesma sala pode chegar por várias interfaces (LAN e Radmin).
            if sender_ip not in room.addresses:
                room.addresses.append(sender_ip)

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
                    data, (ip, _) = sock.recvfrom(4096)
                except TimeoutError:
                    continue
                except OSError:
                    continue  # ex.: WSAECONNRESET no Windows
                self.handle_datagram(data, ip)
        finally:
            sock.close()
