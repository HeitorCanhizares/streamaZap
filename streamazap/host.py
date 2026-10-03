"""Lado de quem cria a sala: captura, codifica e transmite para os espectadores."""

from __future__ import annotations

import collections
import logging
import secrets
import socket
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass, field

from streamazap import config, protocol
from streamazap.capture.audio import AUDIO_NONE, AudioApp, AudioCaptureGroup
from streamazap.capture.screen import VideoSource, create_capturer
from streamazap.discovery import Announcer
from streamazap.media.audio import AudioEncoder
from streamazap.media.video import ENCODER_AUTO, VideoEncoder

log = logging.getLogger(__name__)

# Se a fila de um espectador passar disso, a conexão dele não está dando conta:
# descartamos o vídeo pendente e esperamos o próximo keyframe.
MAX_QUEUE_BYTES = 3 * 1024 * 1024


@dataclass
class HostSettings:
    room_name: str
    host_name: str
    password: str = ""
    source: VideoSource | None = None
    max_height: int = 720
    fps: int = 30
    bitrate: int = 2_500_000
    encoder: str = ENCODER_AUTO
    audio_mode: str = AUDIO_NONE
    audio_apps: list[AudioApp] = field(default_factory=list)
    port: int = config.STREAM_PORT


class _Viewer:
    def __init__(self, host: StreamHost, sock: socket.socket, address: str):
        self.host = host
        self.sock = sock
        self.address = address
        self.name = address
        self.authed = False
        self.waiting_keyframe = True
        self._queue: collections.deque[tuple[int, bytes]] = collections.deque()
        self._queued_bytes = 0
        self._cond = threading.Condition()
        self._closed = False

    def send(self, msg_type: int, data: bytes) -> None:
        with self._cond:
            if self._closed:
                return
            if self._queued_bytes + len(data) > MAX_QUEUE_BYTES:
                # Conexão lenta: joga fora o vídeo atrasado e pede keyframe.
                self._queue = collections.deque(item for item in self._queue if item[0] != protocol.VIDEO)
                self._queued_bytes = sum(len(item[1]) for item in self._queue)
                self.waiting_keyframe = True
                self.host.request_keyframe()
                if msg_type == protocol.VIDEO:
                    return
            self._queue.append((msg_type, data))
            self._queued_bytes += len(data)
            self._cond.notify()

    def send_video(self, packet: bytes, keyframe: bool) -> None:
        if self.waiting_keyframe:
            if not keyframe:
                return
            self.waiting_keyframe = False
        self.send(protocol.VIDEO, protocol.encode_video(packet, keyframe))

    def close(self) -> None:
        with self._cond:
            self._closed = True
            self._cond.notify()
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()

    def sender_loop(self) -> None:
        try:
            while True:
                with self._cond:
                    while not self._queue and not self._closed:
                        self._cond.wait()
                    if self._closed:
                        return
                    _, data = self._queue.popleft()
                    self._queued_bytes -= len(data)
                self.sock.sendall(data)
        except OSError:
            pass
        finally:
            self.host._drop_viewer(self)


class StreamHost:
    """Servidor da sala. Callbacks são chamados de threads de fundo."""

    def __init__(
        self,
        settings: HostSettings,
        on_viewers: Callable[[list[str]], None] = lambda names: None,
        on_chat: Callable[[str, str], None] = lambda name, text: None,
        on_error: Callable[[str], None] = lambda message: None,
        on_stats: Callable[[dict], None] = lambda stats: None,
        capturer_factory=create_capturer,
    ):
        self.settings = settings
        self.on_viewers = on_viewers
        self.on_chat = on_chat
        self.on_error = on_error
        self.on_stats = on_stats
        self._capturer_factory = capturer_factory
        self._viewers: list[_Viewer] = []
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._force_keyframe = threading.Event()
        self._server: socket.socket | None = None
        self._announcer: Announcer | None = None
        self._audio: AudioCaptureGroup | None = None
        self._audio_encoder: AudioEncoder | None = None
        self.port = settings.port
        self.encoder_name: str | None = None

    # -- ciclo de vida -------------------------------------------------------------
    def start(self) -> None:
        self._server = self._listen()
        self._announcer = Announcer(self._announce_info)
        self._announcer.start()
        threading.Thread(target=self._accept_loop, name="host-accept", daemon=True).start()
        if self.settings.source is not None:
            threading.Thread(target=self._video_loop, name="host-video", daemon=True).start()
        if self.settings.audio_mode != AUDIO_NONE:
            self._audio_encoder = AudioEncoder()
            self._audio = AudioCaptureGroup(self.settings.audio_mode, self.settings.audio_apps, self._on_audio_frame)
            self._audio.start()

    def stop(self) -> None:
        self._stop.set()
        if self._announcer:
            self._announcer.stop()
        if self._audio:
            self._audio.stop()
        if self._server:
            self._server.close()
        with self._lock:
            viewers = list(self._viewers)
        for viewer in viewers:
            viewer.close()

    def _listen(self) -> socket.socket:
        last_error = None
        for port in range(self.settings.port, self.settings.port + 10):
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            if hasattr(socket, "SO_EXCLUSIVEADDRUSE"):
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)
            else:
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                sock.bind(("0.0.0.0", port))
            except OSError as exc:
                last_error = exc
                sock.close()
                continue
            sock.listen(16)
            self.port = port
            return sock
        raise OSError(f"Nenhuma porta TCP livre a partir de {self.settings.port}: {last_error}")

    def _announce_info(self) -> dict:
        return {
            "name": self.settings.room_name,
            "host_name": self.settings.host_name,
            "port": self.port,
            "viewers": len(self.viewer_names()),
            "locked": bool(self.settings.password),
        }

    # -- espectadores --------------------------------------------------------------
    def viewer_names(self) -> list[str]:
        with self._lock:
            return [v.name for v in self._viewers if v.authed]

    def request_keyframe(self) -> None:
        self._force_keyframe.set()

    def _broadcast(self, msg_type: int, data: bytes) -> None:
        with self._lock:
            viewers = [v for v in self._viewers if v.authed]
        for viewer in viewers:
            viewer.send(msg_type, data)

    def _notify_viewers_changed(self) -> None:
        names = self.viewer_names()
        self._broadcast(protocol.INFO, protocol.encode_json(protocol.INFO, {"viewers": names, "host": self.settings.host_name}))
        self.on_viewers(names)

    def _drop_viewer(self, viewer: _Viewer) -> None:
        with self._lock:
            if viewer not in self._viewers:
                return
            self._viewers.remove(viewer)
        viewer.close()
        if viewer.authed:
            self._notify_viewers_changed()

    def _accept_loop(self) -> None:
        while not self._stop.is_set():
            try:
                sock, (address, _) = self._server.accept()
            except OSError:
                return
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            viewer = _Viewer(self, sock, address)
            with self._lock:
                self._viewers.append(viewer)
            threading.Thread(target=self._viewer_reader, args=(viewer,), name=f"viewer-{address}", daemon=True).start()

    def _viewer_reader(self, viewer: _Viewer) -> None:
        sock = viewer.sock
        try:
            nonce = secrets.token_hex(16)
            sock.sendall(
                protocol.encode_json(
                    protocol.SERVER_HELLO,
                    {
                        "room": self.settings.room_name,
                        "host": self.settings.host_name,
                        "locked": bool(self.settings.password),
                        "nonce": nonce,
                        "version": config.PROTOCOL_VERSION,
                    },
                )
            )
            sock.settimeout(15)
            msg_type, payload = protocol.recv_message(sock)
            if msg_type != protocol.HELLO:
                raise protocol.ProtocolError("esperava HELLO")
            hello = protocol.decode_json(payload)
            if self.settings.password and not protocol.check_auth(self.settings.password, nonce, hello.get("auth", "")):
                sock.sendall(protocol.encode_json(protocol.REJECT, {"reason": "Senha incorreta"}))
                raise protocol.ProtocolError("senha incorreta")
            viewer.name = str(hello.get("name") or viewer.address)[:40]
            sock.settimeout(None)
            sock.sendall(protocol.encode_json(protocol.WELCOME, {"room": self.settings.room_name, "host": self.settings.host_name}))
            viewer.authed = True
            threading.Thread(target=viewer.sender_loop, name=f"viewer-send-{viewer.address}", daemon=True).start()
            self.request_keyframe()
            self._notify_viewers_changed()
            self.on_chat("", f"{viewer.name} entrou na sala")

            while not self._stop.is_set():
                msg_type, payload = protocol.recv_message(sock)
                if msg_type == protocol.KEYFRAME_REQUEST:
                    viewer.waiting_keyframe = True
                    self.request_keyframe()
                elif msg_type == protocol.CHAT:
                    text = str(protocol.decode_json(payload).get("text", ""))[:500]
                    if text:
                        self._relay_chat(viewer.name, text)
        except (OSError, ConnectionError, protocol.ProtocolError) as exc:
            log.info("espectador %s saiu: %s", viewer.address, exc)
        finally:
            was_authed = viewer.authed
            self._drop_viewer(viewer)
            if was_authed and not self._stop.is_set():
                self.on_chat("", f"{viewer.name} saiu da sala")

    def _relay_chat(self, name: str, text: str) -> None:
        self._broadcast(protocol.CHAT, protocol.encode_json(protocol.CHAT, {"name": name, "text": text}))
        self.on_chat(name, text)

    def send_chat(self, text: str) -> None:
        self._relay_chat(self.settings.host_name, text)

    # -- mídia -------------------------------------------------------------------
    def _on_audio_frame(self, pcm: bytes) -> None:
        for packet in self._audio_encoder.encode(pcm):
            self._broadcast(protocol.AUDIO, protocol.encode(protocol.AUDIO, packet))

    def _video_loop(self) -> None:
        s = self.settings
        encoder = VideoEncoder(s.max_height, s.fps, s.bitrate, s.encoder)
        capturer = None
        interval = 1.0 / s.fps
        last_frame = None
        frames = sent_bytes = 0
        stats_time = time.monotonic()
        next_tick = time.perf_counter()
        try:
            capturer = self._capturer_factory(s.source)
            while not self._stop.is_set():
                next_tick += interval
                frame = capturer.grab()
                if frame is None:
                    # Janela minimizada: só reenvia o último quadro se alguém pediu keyframe.
                    frame = last_frame if self._force_keyframe.is_set() else None
                if frame is not None:
                    last_frame = frame
                    force = self._force_keyframe.is_set()
                    self._force_keyframe.clear()
                    packets = encoder.encode(frame, force_keyframe=force)
                    self.encoder_name = encoder.codec_name
                    with self._lock:
                        viewers = [v for v in self._viewers if v.authed]
                    for packet, keyframe in packets:
                        sent_bytes += len(packet)
                        for viewer in viewers:
                            viewer.send_video(packet, keyframe)
                    frames += 1

                now = time.monotonic()
                if now - stats_time >= 1.0:
                    elapsed = now - stats_time
                    self.on_stats(
                        {
                            "fps": frames / elapsed,
                            "kbps": sent_bytes * 8 / 1000 / elapsed,
                            "encoder": encoder.codec_name,
                            "size": encoder.size,
                            "audio_errors": self._audio.errors() if self._audio else [],
                        }
                    )
                    frames = sent_bytes = 0
                    stats_time = now

                delay = next_tick - time.perf_counter()
                if delay > 0:
                    time.sleep(delay)
                else:
                    next_tick = time.perf_counter()  # captura lenta: não acumula atraso
        except Exception as exc:  # noqa: BLE001 - reportado na interface
            log.exception("loop de vídeo falhou")
            self.on_error(str(exc))
        finally:
            if capturer is not None:
                capturer.close()
