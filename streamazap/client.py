"""Lado do espectador: conecta num servidor de stream, toca vídeo/áudio e reconecta sozinho."""

from __future__ import annotations

import logging
import socket
import threading
import time
from collections.abc import Callable

import numpy as np

from streamazap import __version__, config, protocol
from streamazap.media.audio import AudioDecoder
from streamazap.media.video import VideoDecoder
from streamazap.player import AudioPlayer
from streamazap.playout import MediaClock, VideoPlayout

log = logging.getLogger(__name__)


class JoinError(Exception):
    def __init__(self, message: str, fatal: bool = False):
        super().__init__(message)
        self.fatal = fatal  # senha errada / versão diferente: não adianta tentar de novo


FIREWALL_HINT = (
    "A sala foi encontrada, mas o PC de quem está transmitindo não aceitou a conexão de vídeo — "
    "quase sempre é o Firewall do Windows ou o firewall do antivírus dele bloqueando.\n"
    "Peça para ele clicar em “Corrigir firewall” no StreamaZap (ou liberar o StreamaZap no antivírus)."
)


def _explain(address: str, exc: Exception) -> str:
    if isinstance(exc, socket.timeout | TimeoutError):
        return f"{address}: tempo esgotado"
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
        callback_request: Callable[[int], None] | None = None,
        on_callback: Callable[[str, int], None] = lambda ip, port: None,
    ):
        """`callback_request(porta)`: pede ao outro lado que conecte em nós (conexão reversa)
        se a conexão direta falhar. `on_callback(ip, porta)`: o host da sala pede que nosso
        compartilhamento conecte em alguém que não conseguiu chegar até nós."""
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
        self.callback_request = callback_request
        self.on_callback = on_callback
        self.clock = MediaClock(delay_ms)
        self.player = AudioPlayer(self.clock)
        self.used_callback = False
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
        """Conecta e faz o handshake (bloqueante). Levanta JoinError em caso de falha.

        Tenta cada endereço direto; se nenhum responder e houver como pedir, tenta a
        conexão reversa (o outro lado conecta em nós).
        """
        errors = []
        timed_out = False
        for address in self.addresses:
            try:
                sock = socket.create_connection((address, self.port), timeout=config.CONNECT_TIMEOUT)
            except OSError as exc:
                timed_out |= isinstance(exc, TimeoutError)
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
            self._set_socket(sock)
            return
        if self.callback_request is not None:
            log.info("conexão direta falhou (%s); tentando conexão reversa", "; ".join(errors))
            try:
                self._connect_reverse()
                return
            except JoinError:
                raise
            except (OSError, ConnectionError, protocol.ProtocolError) as exc:
                errors.append(f"conexão reversa: {exc}")
        message = "Não foi possível conectar:\n" + "\n".join(errors)
        if timed_out:
            message += "\n\n" + FIREWALL_HINT
        raise JoinError(message)

    def _connect_reverse(self) -> None:
        """Abre uma porta e pede para o outro lado conectar nela."""
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(("0.0.0.0", 0))
            listener.listen(4)
            port = listener.getsockname()[1]
            self.callback_request(port)
            deadline = time.monotonic() + config.CALLBACK_TIMEOUT
            while (remaining := deadline - time.monotonic()) > 0:
                listener.settimeout(remaining)
                try:
                    sock, (ip, _) = listener.accept()
                except TimeoutError:
                    break
                if ip not in self.addresses:
                    log.info("conexão reversa de %s ignorada (esperava %s)", ip, self.addresses)
                    sock.close()
                    continue
                try:
                    self._handshake(sock)
                except BaseException:
                    sock.close()
                    raise
                log.info("conectado por conexão reversa a %s", ip)
                self.used_callback = True
                self.addresses = [ip] + [a for a in self.addresses if a != ip]
                self._set_socket(sock)
                return
        raise TimeoutError("o host não conseguiu conectar de volta (o seu firewall também pode estar bloqueando)")

    def _set_socket(self, sock: socket.socket) -> None:
        with self._send_lock:
            self._sock = sock

    def _handshake(self, sock: socket.socket) -> None:
        sock.settimeout(config.HANDSHAKE_TIMEOUT)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
        msg_type, payload = protocol.recv_message(sock)
        if msg_type != protocol.SERVER_HELLO:
            raise protocol.ProtocolError("resposta inesperada do host")
        hello = protocol.decode_json(payload)
        if hello.get("version") != config.PROTOCOL_VERSION:
            raise JoinError(
                f"O host está com outra versão do StreamaZap (você tem a {__version__}). "
                "Atualizem os dois para a mais recente.",
                fatal=True,
            )
        auth = protocol.auth_token(self.password, hello.get("nonce", "")) if hello.get("locked") else ""
        sock.sendall(
            protocol.encode_json(
                protocol.HELLO,
                {
                    "name": self.name,
                    "auth": auth,
                    "media": self._media,
                    "version": config.PROTOCOL_VERSION,
                    "app": __version__,
                },
            )
        )
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
        """Muda a suavização: vídeo e áudio seguem o mesmo relógio, então mudam juntos."""
        self.delay_ms = delay_ms
        self.clock.set_delay(delay_ms)
        if self._playout:
            self._playout.wake()

    def request_stream_callback(self, stream_id: str, port: int) -> None:
        """Pede (via host da sala) que o participante `stream_id` conecte em nós na `port`."""
        self._send(protocol.encode_json(protocol.CALLBACK, {"stream": stream_id, "port": port}))

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
                    "audio_latency_ms": int(self.player.output_latency * 1000),
                    "reverse": self.used_callback,
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
        playout = VideoPlayout(
            video.decode, lambda frame: self.on_frame(video.to_image(frame)), self._request_keyframe, self.clock
        )
        self._playout = playout
        self.player.clear()
        playout.start()
        sock = self._sock
        try:
            while not self._stop.is_set():
                msg_type, payload = protocol.recv_message(sock)
                self._received_bytes += len(payload) + protocol.HEADER.size
                if msg_type == protocol.VIDEO:
                    packet, _, timestamp_ms = protocol.decode_video(payload)
                    self.clock.observe(timestamp_ms)
                    playout.push(packet, timestamp_ms)
                elif msg_type == protocol.AUDIO:
                    packet, timestamp_ms = protocol.decode_audio(payload)
                    self.clock.observe(timestamp_ms)
                    try:
                        self.player.push(audio.decode(packet), timestamp_ms)
                    except Exception:  # noqa: BLE001
                        log.debug("pacote de áudio inválido")
                elif msg_type == protocol.CALLBACK:
                    msg = protocol.decode_json(payload)
                    ip = msg.get("ip") if isinstance(msg.get("ip"), str) else self.addresses[0]
                    if isinstance(msg.get("port"), int):
                        self.on_callback(ip, msg["port"])
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
            log.info("sem dados do host há %.0f s", config.STALL_TIMEOUT)
            return False, "sem resposta do host"
        except (OSError, ConnectionError, protocol.ProtocolError) as exc:
            if not self._stop.is_set():
                log.info("conexão caiu: %s", exc)
            return False, str(exc) or exc.__class__.__name__
        finally:
            playout.stop()
