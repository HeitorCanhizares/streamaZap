"""Lado do espectador: conecta num servidor de stream, toca vídeo/áudio e reconecta sozinho."""

from __future__ import annotations

import logging
import socket
import threading
import time
from collections.abc import Callable

import numpy as np

from streamazap import config, protocol
from streamazap.media.audio import AudioDecoder
from streamazap.media.video import VideoDecoder
from streamazap.player import AudioPlayer
from streamazap.playout import VideoPlayout

log = logging.getLogger(__name__)


class JoinError(Exception):
    def __init__(self, message: str, fatal: bool = False):
        super().__init__(message)
        self.fatal = fatal  # senha errada / versão diferente: não adianta tentar de novo


def _explain(address: str, exc: Exception) -> str:
    if isinstance(exc, socket.timeout | TimeoutError):
        return (
            f"{address}: tempo esgotado. Verifique se vocês estão na mesma rede do Radmin, "
            "se o host liberou o StreamaZap no Firewall do Windows e se o Radmin mostra conexão direta."
        )
    if isinstance(exc, ConnectionRefusedError):
        return f"{address}: conexão recusada (a sala já foi encerrada?)"
    return f"{address}: {exc}"


class StreamViewer:
    def __init__(
        self,
        addresses: list[str],
        port: int,
        name: str,
        password: str = "",
        delay_ms: int = config.DEFAULT_PLAYOUT_DELAY_MS,
        media: bool = True,
        on_frame: Callable[[np.ndarray], None] = lambda frame: None,
        on_chat: Callable[[str, str], None] = lambda name, text: None,
        on_info: Callable[[dict], None] = lambda info: None,
        on_closed: Callable[[str], None] = lambda reason: None,
        on_status: Callable[[str], None] = lambda text: None,
        on_stats: Callable[[dict], None] = lambda stats: None,
    ):
        self.addresses = addresses
        self.port = port
        self.name = name
        self.password = password
        self.delay_ms = delay_ms
        self.on_frame = on_frame
        self.on_chat = on_chat
        self.on_info = on_info
        self.on_closed = on_closed
        self.on_status = on_status
        self.on_stats = on_stats
        self.player = AudioPlayer(delay_ms)
        self.room_name = ""
        self.host_name = ""
        self.viewer_id: str | None = None
        self.rtt_ms: float | None = None
        self._media = media
        self._stream_announcement: dict | None = None
        self._sock: socket.socket | None = None
        self._send_lock = threading.Lock()
        self._stop = threading.Event()
        self._playout: VideoPlayout | None = None
        self._last_keyframe_request = 0.0
        self._received_bytes = 0

    # -- conexão -------------------------------------------------------------------
    def connect(self) -> None:
        """Conecta e faz o handshake (bloqueante). Levanta JoinError em caso de falha."""
        errors = []
        for address in self.addresses:
            try:
                sock = socket.create_connection((address, self.port), timeout=config.CONNECT_TIMEOUT)
            except OSError as exc:
                errors.append(_explain(address, exc))
                continue
            try:
                self._handshake(sock)
            except JoinError:
                sock.close()
                raise
            except (OSError, ConnectionError, protocol.ProtocolError) as exc:
                sock.close()
                errors.append(_explain(address, exc))
                continue
            # Endereço que funcionou vai para o começo (reconexões mais rápidas).
            self.addresses = [address] + [a for a in self.addresses if a != address]
            with self._send_lock:
                self._sock = sock
            return
        raise JoinError("Não foi possível conectar:\n" + "\n".join(errors))

    def _handshake(self, sock: socket.socket) -> None:
        sock.settimeout(config.HANDSHAKE_TIMEOUT)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
        msg_type, payload = protocol.recv_message(sock)
        if msg_type != protocol.SERVER_HELLO:
            raise protocol.ProtocolError("resposta inesperada do host")
        hello = protocol.decode_json(payload)
        if hello.get("version") != config.PROTOCOL_VERSION:
            raise JoinError("O host está com outra versão do StreamaZap. Atualizem os dois para a mais recente.", fatal=True)
        auth = protocol.auth_token(self.password, hello.get("nonce", "")) if hello.get("locked") else ""
        sock.sendall(protocol.encode_json(protocol.HELLO, {"name": self.name, "auth": auth, "media": self._media}))
        msg_type, payload = protocol.recv_message(sock)
        if msg_type == protocol.REJECT:
            raise JoinError(protocol.decode_json(payload).get("reason", "Entrada recusada"), fatal=True)
        if msg_type != protocol.WELCOME:
            raise protocol.ProtocolError("resposta inesperada do host")
        welcome = protocol.decode_json(payload)
        self.room_name = welcome.get("room", "")
        self.host_name = welcome.get("host", "")
        self.viewer_id = welcome.get("you")
        sock.settimeout(config.STALL_TIMEOUT)

    def start(self) -> None:
        self.player.start()
        threading.Thread(target=self._run, name="viewer", daemon=True).start()
        threading.Thread(target=self._ping_loop, name="viewer-ping", daemon=True).start()

    def stop(self) -> None:
        self._stop.set()
        self._close_socket()
        if self._playout:
            self._playout.stop()
        self.player.stop()

    def _close_socket(self) -> None:
        with self._send_lock:
            sock = self._sock
        if sock is not None:
            try:
                sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            sock.close()

    # -- comandos ------------------------------------------------------------------
    def send_chat(self, text: str) -> None:
        self._send(protocol.encode_json(protocol.CHAT, {"text": text}))

    def set_media(self, enabled: bool) -> None:
        """Liga/desliga o recebimento de vídeo/áudio (ao assistir outro stream da sala)."""
        self._media = enabled
        self._send(protocol.encode_json(protocol.SUBSCRIBE, {"media": enabled}))

    def announce_stream(self, port: int, addresses: list[str]) -> None:
        self._stream_announcement = {"port": port, "addresses": addresses}
        self._send(protocol.encode_json(protocol.STREAM, self._stream_announcement))

    def stop_stream(self) -> None:
        self._stream_announcement = None
        self._send(protocol.encode_json(protocol.STREAM, {"port": 0}))

    def set_delay(self, delay_ms: int) -> None:
        self.delay_ms = delay_ms
        self.player.set_delay(delay_ms)
        if self._playout:
            self._playout.set_delay(delay_ms)

    def _send(self, data: bytes) -> None:
        with self._send_lock:
            if self._sock is None:
                return
            try:
                self._sock.sendall(data)
            except OSError:
                pass

    def _request_keyframe(self) -> None:
        now = time.monotonic()
        if now - self._last_keyframe_request > 1.0:
            self._last_keyframe_request = now
            self._send(protocol.encode(protocol.KEYFRAME_REQUEST))

    def _ping_loop(self) -> None:
        last_bytes, last_time = 0, time.monotonic()
        while not self._stop.wait(config.PING_INTERVAL):
            self._send(protocol.encode_json(protocol.PING, {"t": time.monotonic(), "rtt": self.rtt_ms}))
            now = time.monotonic()
            playout = self._playout
            self.on_stats(
                {
                    "rtt": self.rtt_ms,
                    "kbps": (self._received_bytes - last_bytes) * 8 / 1000 / (now - last_time),
                    "buffer_ms": int(playout.buffered_seconds() * 1000) if playout else 0,
                    "delay_ms": self.delay_ms,
                }
            )
            last_bytes, last_time = self._received_bytes, now

    # -- recepção ------------------------------------------------------------------
    def _run(self) -> None:
        while not self._stop.is_set():
            ended_by_host, reason = self._session()
            if self._stop.is_set():
                return
            if ended_by_host:
                self.on_closed(reason)
                return
            if not self._reconnect(reason):
                return
            # Restaura o estado da sessão anterior.
            if not self._media:
                self.set_media(False)
            if self._stream_announcement:
                self._send(protocol.encode_json(protocol.STREAM, self._stream_announcement))
            self.on_status("")

    def _reconnect(self, reason: str) -> bool:
        log.info("conexão perdida (%s); tentando reconectar", reason)
        for attempt in range(1, config.RECONNECT_ATTEMPTS + 1):
            self.on_status(f"Conexão instável — reconectando ({attempt}/{config.RECONNECT_ATTEMPTS})…")
            if self._stop.wait(min(2**attempt, 15)):
                return False
            try:
                self.connect()
                return True
            except JoinError as exc:
                if exc.fatal:
                    self.on_closed(str(exc))
                    return False
                log.info("reconexão %d falhou: %s", attempt, exc)
        self.on_closed(f"Conexão perdida com o host ({reason}).")
        return False

    def _session(self) -> tuple[bool, str]:
        """Roda até a conexão acabar. Retorna (encerrado pelo host?, motivo)."""
        video = VideoDecoder()
        audio = AudioDecoder()
        playout = VideoPlayout(video.decode, self.on_frame, self._request_keyframe, self.delay_ms)
        self._playout = playout
        playout.start()
        sock = self._sock
        try:
            while not self._stop.is_set():
                msg_type, payload = protocol.recv_message(sock)
                self._received_bytes += len(payload) + protocol.HEADER.size
                if msg_type == protocol.VIDEO:
                    packet, _, timestamp_ms = protocol.decode_video(payload)
                    playout.push(packet, timestamp_ms)
                elif msg_type == protocol.AUDIO:
                    try:
                        self.player.push(audio.decode(payload))
                    except Exception:  # noqa: BLE001
                        log.debug("pacote de áudio inválido")
                elif msg_type == protocol.PONG:
                    sent = protocol.decode_json(payload).get("t")
                    if isinstance(sent, (int, float)):
                        self.rtt_ms = (time.monotonic() - sent) * 1000
                elif msg_type == protocol.CHAT:
                    msg = protocol.decode_json(payload)
                    self.on_chat(str(msg.get("name", "?")), str(msg.get("text", "")))
                elif msg_type == protocol.INFO:
                    self.on_info(protocol.decode_json(payload))
                elif msg_type == protocol.BYE:
                    return True, str(protocol.decode_json(payload).get("reason") or "O host encerrou a transmissão")
            return True, ""
        except TimeoutError:
            return False, "sem resposta do host"
        except (OSError, ConnectionError, protocol.ProtocolError) as exc:
            return False, str(exc) or exc.__class__.__name__
        finally:
            playout.stop()
