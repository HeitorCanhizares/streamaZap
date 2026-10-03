"""Lado do espectador: conecta na sala, decodifica vídeo e toca o áudio."""

from __future__ import annotations

import logging
import socket
import threading
import time
from collections.abc import Callable

import numpy as np

from streamazap import protocol
from streamazap.media.audio import AudioDecoder
from streamazap.media.video import VideoDecoder
from streamazap.player import AudioPlayer

log = logging.getLogger(__name__)


class JoinError(Exception):
    pass


class StreamViewer:
    def __init__(
        self,
        addresses: list[str],
        port: int,
        name: str,
        password: str = "",
        on_frame: Callable[[np.ndarray], None] = lambda frame: None,
        on_chat: Callable[[str, str], None] = lambda name, text: None,
        on_info: Callable[[dict], None] = lambda info: None,
        on_closed: Callable[[str], None] = lambda reason: None,
    ):
        self.addresses = addresses
        self.port = port
        self.name = name
        self.password = password
        self.on_frame = on_frame
        self.on_chat = on_chat
        self.on_info = on_info
        self.on_closed = on_closed
        self.player = AudioPlayer()
        self.room_name = ""
        self.host_name = ""
        self._sock: socket.socket | None = None
        self._send_lock = threading.Lock()
        self._stop = threading.Event()
        self._last_keyframe_request = 0.0

    def connect(self) -> None:
        """Conecta e faz o handshake (bloqueante). Levanta JoinError em caso de falha."""
        errors = []
        for address in self.addresses:
            try:
                sock = socket.create_connection((address, self.port), timeout=5)
            except OSError as exc:
                errors.append(f"{address}: {exc}")
                continue
            try:
                self._handshake(sock)
            except JoinError:
                sock.close()
                raise
            except (OSError, ConnectionError, protocol.ProtocolError) as exc:
                sock.close()
                errors.append(f"{address}: {exc}")
                continue
            self._sock = sock
            return
        raise JoinError("Não foi possível conectar:\n" + "\n".join(errors))

    def _handshake(self, sock: socket.socket) -> None:
        sock.settimeout(10)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        msg_type, payload = protocol.recv_message(sock)
        if msg_type != protocol.SERVER_HELLO:
            raise protocol.ProtocolError("resposta inesperada do host")
        hello = protocol.decode_json(payload)
        auth = protocol.auth_token(self.password, hello.get("nonce", "")) if hello.get("locked") else ""
        sock.sendall(protocol.encode_json(protocol.HELLO, {"name": self.name, "auth": auth}))
        msg_type, payload = protocol.recv_message(sock)
        if msg_type == protocol.REJECT:
            raise JoinError(protocol.decode_json(payload).get("reason", "Entrada recusada"))
        if msg_type != protocol.WELCOME:
            raise protocol.ProtocolError("resposta inesperada do host")
        welcome = protocol.decode_json(payload)
        self.room_name = welcome.get("room", "")
        self.host_name = welcome.get("host", "")
        sock.settimeout(None)

    def start(self) -> None:
        self.player.start()
        threading.Thread(target=self._run, name="viewer", daemon=True).start()

    def stop(self) -> None:
        self._stop.set()
        if self._sock is not None:
            try:
                self._sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            self._sock.close()
        self.player.stop()

    def send_chat(self, text: str) -> None:
        self._send(protocol.encode_json(protocol.CHAT, {"text": text}))

    def _send(self, data: bytes) -> None:
        with self._send_lock:
            try:
                self._sock.sendall(data)
            except OSError:
                pass

    def _request_keyframe(self) -> None:
        now = time.monotonic()
        if now - self._last_keyframe_request > 1.0:
            self._last_keyframe_request = now
            self._send(protocol.encode(protocol.KEYFRAME_REQUEST))

    def _run(self) -> None:
        video = VideoDecoder()
        audio = AudioDecoder()
        reason = "Conexão encerrada pelo host"
        try:
            while not self._stop.is_set():
                msg_type, payload = protocol.recv_message(self._sock)
                if msg_type == protocol.VIDEO:
                    packet, _ = protocol.decode_video(payload)
                    try:
                        frames = video.decode(packet)
                    except Exception:  # noqa: BLE001 - quadro corrompido/sem referência
                        self._request_keyframe()
                        continue
                    if frames:
                        self.on_frame(frames[-1])
                elif msg_type == protocol.AUDIO:
                    try:
                        self.player.push(audio.decode(payload))
                    except Exception:  # noqa: BLE001
                        log.debug("pacote de áudio inválido")
                elif msg_type == protocol.CHAT:
                    msg = protocol.decode_json(payload)
                    self.on_chat(str(msg.get("name", "?")), str(msg.get("text", "")))
                elif msg_type == protocol.INFO:
                    self.on_info(protocol.decode_json(payload))
        except (OSError, ConnectionError, protocol.ProtocolError) as exc:
            if not self._stop.is_set():
                log.info("desconectado: %s", exc)
        finally:
            if not self._stop.is_set():
                self.on_closed(reason)
