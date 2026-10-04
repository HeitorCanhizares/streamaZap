"""Servidor de stream: captura, codifica e transmite para os espectadores.

O mesmo servidor serve dois papéis:
  * dono da sala (announce=True): anuncia a sala na rede, guarda o chat, a lista
    de participantes e a lista de streams da sala;
  * participante compartilhando (announce=False): só serve o próprio vídeo/áudio.
    Os outros conectam direto nele (P2P pelo Radmin), o que divide a carga de
    upload entre quem está compartilhando.
"""

from __future__ import annotations

import collections
import logging
import secrets
import socket
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from typing import TYPE_CHECKING

from streamazap import __version__, config, netutil, protocol
from streamazap.capture.audio import AUDIO_NONE, AudioApp, AudioCaptureGroup
from streamazap.capture.screen import VideoSource, create_capturer
from streamazap.discovery import Announcer
from streamazap.playout import SlidingMin
from streamazap.updater import RELEASES_URL

if TYPE_CHECKING:
    from streamazap.media.audio import AudioEncoder

log = logging.getLogger(__name__)

# Proteção extra além do limite por tempo (config.MAX_VIEWER_LAG).
MAX_QUEUE_BYTES = 8 * 1024 * 1024


@dataclass
class HostSettings:
    room_name: str
    host_name: str
    password: str = ""
    source: VideoSource | None = None
    max_height: int = 720
    fps: int = 30
    bitrate: int = 2_500_000
    encoder: str = config.ENCODER_AUTO
    audio_mode: str = AUDIO_NONE
    audio_apps: list[AudioApp] = field(default_factory=list)
    adaptive: bool = True
    announce: bool = True
    port: int = config.STREAM_PORT

    def video_key(self):
        return (self.source, self.max_height, self.fps, self.bitrate, self.encoder, self.adaptive)

    def audio_key(self):
        return (self.audio_mode, tuple(self.audio_apps))


class BitrateController:
    """Ajusta o bitrate pela conexão mais lenta da sala.

    Começa em 70% e sobe a cada 2 s até o alvo (início suave: passar da capacidade logo de
    cara enchia as filas da rede e deixava um pico de atraso). Cai rápido quando algum
    espectador acumula atraso (ou teve vídeo descartado), para perto do que a conexão dele
    realmente escoou, e espera o efeito antes de cair de novo. Sobe aos poucos quando todos
    estão em dia.
    """

    DECREASE_INTERVAL = 2.0
    SETTLE_TIME = 6.0
    INCREASE_INTERVAL = 5.0
    HIGH_LAG = 0.4
    LOW_LAG = 0.15
    # Lembra o bitrate em que a conexão engasgou: sobe rápido só até perto dele e, dali para
    # cima, sonda devagar. Sem isso o bitrate passava do limite a cada ~15 s (pico de atraso).
    CEILING_MEMORY = 60.0
    CEILING_MARGIN = 0.85
    PROBE_INTERVAL = 20.0
    PROBE_STEP = 1.05
    DRAIN_TIME = 3.0
    SLOW_START = 0.7
    SLOW_START_INTERVAL = 2.0

    def __init__(self, base: int, enabled: bool = True):
        self.base = base
        self.current = int(round(base * self.SLOW_START, -3)) if enabled else base
        self._slow_start = enabled
        self.minimum = max(300_000, int(round(base * 0.15, -3)))
        self.enabled = enabled
        self._last_change = float("-inf")
        self._last_drop = float("-inf")
        self._last_lag = 0.0
        self._ceiling = 0
        self._ceiling_time = float("-inf")
        self._measured_drop = False

    @property
    def starting(self) -> bool:
        """Ainda no início suave (qualidade subindo aos poucos, não é conexão lenta)."""
        return self._slow_start and self.current < self.base

    def update(self, now: float, lag: float, dropped: bool, throughput: float | None = None) -> int | None:
        """Retorna o novo bitrate se ele mudou.

        `throughput`: vídeo que a conexão mais lenta realmente escoou (bit/s) enquanto estava
        congestionada; é a capacidade dela, então a queda vai direto para perto disso.
        """
        previous_lag, self._last_lag = self._last_lag, lag
        if not self.enabled:
            return None
        elapsed = now - self._last_change
        since_drop = now - self._last_drop
        # Logo depois de uma queda a medida ainda reflete a fila antiga (o ping leva 1-2 s).
        # Se a queda foi pela vazão medida, confia nela: só cai de novo se a fila estourar.
        # Senão, só cai de novo se piorar; passado esse tempo, se não estiver esvaziando.
        # (Logo depois de uma subida pode cair na hora: foi ela que passou do limite.)
        if since_drop < self.SETTLE_TIME:
            keeps_growing = not self._measured_drop and lag > previous_lag * 1.05
        else:
            keeps_growing = lag >= previous_lag * 0.85
        if (dropped or (lag > self.HIGH_LAG and keeps_growing)) and since_drop >= self.DECREASE_INTERVAL:
            self._measured_drop = bool(throughput)
            self._last_drop = now
            if throughput:
                capacity = min(throughput, self.current)
                self._ceiling = int(capacity)
                # Abaixo da capacidade o bastante para também esvaziar, em ~3 s, a fila que já
                # se formou (lag segundos dela); depois sobe de novo até perto do teto.
                drain = capacity * min(lag, 2.0) / self.DRAIN_TIME
                wanted = min(max(capacity * self.CEILING_MARGIN - drain, self.current * 0.5), self.current * 0.9)
            else:
                self._ceiling, wanted = self.current, self.current * 0.7
            self._ceiling_time = now
            self._slow_start = False
            new = max(self.minimum, int(round(wanted, -3)))
        elif (
            not dropped
            and lag < self.LOW_LAG
            and elapsed >= (self.SLOW_START_INTERVAL if self._slow_start else self.INCREASE_INTERVAL)
            and self.current < self.base
        ):
            new = min(self.base, int(round(self.current * 1.15, -3)))
            if now - self._ceiling_time < self.CEILING_MEMORY:
                near = int(self._ceiling * self.CEILING_MARGIN)
                if self.current < near:
                    new = min(new, near)
                elif elapsed >= self.PROBE_INTERVAL:
                    new = min(self.base, int(round(self.current * self.PROBE_STEP, -3)))
                else:
                    return None
        else:
            return None
        if new == self.current:
            return None
        self.current = new
        self._last_change = now
        return new


class _Viewer:
    def __init__(self, host: StreamHost, sock: socket.socket, address: str):
        self.host = host
        self.sock = sock
        self.address = address
        self.viewer_id = uuid.uuid4().hex[:8]
        self.name = address
        self.authed = False
        self.media = True  # quer receber vídeo/áudio deste servidor
        self.waiting_keyframe = True
        self.rtt_ms: float | None = None
        self.rtt_floor_ms: float | None = None
        self.playout_delay_ms: int | None = None  # suavização em uso no espectador
        # Atraso de fila na rede = latência atual - menor latência do último minuto.
        # Pega o "bufferbloat" (dados presos em buffers do sistema/VPN), que a fila do app não vê.
        self._rtt_floor = SlidingMin(60.0)
        self.network_queue_delay = 0.0
        self.stream: dict | None = None  # {port, addresses} se estiver compartilhando
        # Áudio e controle passam na frente do vídeo: um keyframe grande na fila não pode
        # atrasar o som (nem o PONG, que mede a latência).
        self._urgent: collections.deque[tuple[int, bytes, float]] = collections.deque()
        self._video: collections.deque[tuple[int, bytes, float]] = collections.deque()
        self._queued_bytes = 0
        self._send_buffer = config.SEND_BUFFER_BYTES
        self.sent_bytes = 0  # aceitos pelo sistema; congestionado, isso anda no ritmo da conexão
        self.offered_bytes = 0  # tudo que o host quis mandar para ele
        self._rate_mark = 0
        self._offered_mark = 0
        self.send_rate = 0.0  # bit/s no último segundo
        self.offered_rate = 0.0
        # (oferecido, recebido) dos últimos segundos: um segundo sozinho oscila demais (keyframes).
        # O "recebido" chega no ping ~1 s depois, então é comparado com o oferecido do segundo anterior.
        self._rates: collections.deque[tuple[float, float]] = collections.deque(maxlen=3)
        self._previous_offered: float | None = None
        self.receive_rate: float | None = None  # bit/s que o espectador diz ter recebido (mais exato)
        self._cond = threading.Condition()
        self._closed = False

    def _media_age(self, now: float) -> float:
        oldest = now
        if self._video:
            oldest = self._video[0][2]
        for msg_type, _, queued_at in self._urgent:
            if msg_type == protocol.AUDIO:
                oldest = min(oldest, queued_at)
                break
        return now - oldest

    def lag(self) -> float:
        """Há quanto tempo a mídia mais antiga da fila espera para ser enviada."""
        with self._cond:
            return self._media_age(time.monotonic())

    def record_rtt(self, rtt_ms: float | None) -> None:
        self.rtt_ms = rtt_ms
        if rtt_ms is not None:
            floor = self.rtt_floor_ms = self._rtt_floor.add(time.monotonic(), rtt_ms)
            self.network_queue_delay = max(0.0, (rtt_ms - floor) / 1000)

    def congestion_delay(self) -> float:
        """Quanto este espectador está atrasado por falta de banda (fila do app + fila na rede)."""
        return max(self.lag(), self.network_queue_delay)

    def measure_rate(self, elapsed: float) -> None:
        if elapsed <= 0:
            return
        sent, self._rate_mark = self.sent_bytes - self._rate_mark, self.sent_bytes
        offered, self._offered_mark = self.offered_bytes - self._offered_mark, self.offered_bytes
        self.send_rate = sent * 8 / elapsed
        self.offered_rate = offered * 8 / elapsed
        if self.receive_rate is not None and self._previous_offered is not None:
            self._rates.append((self._previous_offered, self.receive_rate))
        self._previous_offered = self.offered_rate

    @property
    def overused(self) -> bool:
        """Recebendo bem menos do que mandamos (3 s): a fila está crescendo em algum ponto do
        caminho, antes mesmo do atraso medido pelo ping (que chega 1-2 s depois) acusar."""
        if len(self._rates) < self._rates.maxlen:
            return False
        offered = sum(o for o, _ in self._rates)
        return offered > 900_000 and sum(r for _, r in self._rates) < offered * 0.9

    def capacity(self) -> float:
        """O que a conexão dele escoou (bit/s): a média dos últimos segundos ou o último, se menor
        (quando o link piora de repente, a média ainda carrega os segundos bons)."""
        if self._rates:
            received = [r for _, r in self._rates]
            return min(received[-1], sum(received) / len(received))
        return self.send_rate

    def tune_send_buffer(self, bitrate: int) -> None:
        """Buffer do sistema do tamanho da conexão (banda x latência + folga).

        Grande demais (512 KB fixos), a 2 Mbps ele segura ~2 s de vídeo escondidos do app:
        o atraso cresce sem a fila do app perceber nem conseguir descartar.
        """
        if self.rtt_floor_ms is None:
            return
        wanted = int((bitrate + config.AUDIO_BITRATE) / 8 * (self.rtt_floor_ms / 1000 + 0.2))
        wanted = min(max(wanted, 64 * 1024), 1024 * 1024)
        if abs(wanted - self._send_buffer) > self._send_buffer * 0.2:
            try:
                self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, wanted)
                self._send_buffer = wanted
            except OSError:
                pass

    def send(self, msg_type: int, data: bytes) -> None:
        now = time.monotonic()
        with self._cond:
            if self._closed:
                return
            lagging = self._media_age(now) > config.MAX_VIEWER_LAG
            if lagging or self._queued_bytes + len(data) > MAX_QUEUE_BYTES:
                # Conexão não está dando conta: joga fora a mídia atrasada e recomeça
                # do próximo keyframe, em vez de deixar o atraso crescer sem fim.
                self._video.clear()
                self._urgent = collections.deque(item for item in self._urgent if item[0] != protocol.AUDIO)
                self._queued_bytes = sum(len(item[1]) for item in self._urgent)
                self.waiting_keyframe = True
                self.host.report_congestion()
                if msg_type == protocol.VIDEO:
                    return
            (self._video if msg_type == protocol.VIDEO else self._urgent).append((msg_type, data, now))
            self._queued_bytes += len(data)
            self.offered_bytes += len(data)
            self._cond.notify()

    def send_video(self, message: bytes, keyframe: bool) -> None:
        if self.waiting_keyframe:
            if not keyframe:
                return
            self.waiting_keyframe = False
        self.send(protocol.VIDEO, message)

    def close(self) -> None:
        with self._cond:
            self._closed = True
            self._cond.notify()
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()

    def flush(self, timeout: float) -> None:
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            with self._cond:
                if not (self._urgent or self._video) or self._closed:
                    return
            time.sleep(0.02)

    def sender_loop(self) -> None:
        try:
            while True:
                with self._cond:
                    while not (self._urgent or self._video) and not self._closed:
                        self._cond.wait()
                    if self._closed:
                        return
                    _, data, _ = (self._urgent or self._video).popleft()
                    self._queued_bytes -= len(data)
                self.sock.sendall(data)
                self.sent_bytes += len(data)
        except OSError as exc:
            if not self._closed:
                log.info("envio para %s (%s) falhou: %s", self.name, self.address, exc)
        finally:
            self.host._drop_viewer(self)

    @property
    def closed(self) -> bool:
        return self._closed

    def describe(self) -> dict:
        return {
            "id": self.viewer_id,
            "name": self.name,
            "rtt": self.rtt_ms,
            "delay": self.playout_delay_ms,
            "sharing": self.stream is not None,
        }


class StreamHost:
    """Callbacks são chamados de threads de fundo."""

    def __init__(
        self,
        settings: HostSettings,
        on_viewers: Callable[[list[dict]], None] = lambda viewers: None,
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
        self._congestion = threading.Event()
        self._server: socket.socket | None = None
        self._announcer: Announcer | None = None
        self._audio: AudioCaptureGroup | None = None
        self._audio_encoder: AudioEncoder | None = None
        self._video_stop = threading.Event()
        self._video_thread: threading.Thread | None = None
        self.port = settings.port
        self.encoder_name: str | None = None

    # -- ciclo de vida -------------------------------------------------------------
    def start(self) -> None:
        self._server = self._listen()
        self.settings = replace(self.settings, port=self.port)
        if self.settings.announce:
            self._announcer = Announcer(self._announce_info, on_callback=self.connect_back)
            self._announcer.start()
        threading.Thread(target=self._accept_loop, name="host-accept", daemon=True).start()
        threading.Thread(target=self._info_loop, name="host-info", daemon=True).start()
        self._start_video()
        self._start_audio()

    def stop(self, reason: str = "O host encerrou a transmissão") -> None:
        if self._stop.is_set():
            return
        self._stop.set()
        self._video_stop.set()
        if self._announcer:
            self._announcer.stop()
        if self._audio:
            self._audio.stop()
        if self._server:
            self._server.close()
        with self._lock:
            viewers = list(self._viewers)
        bye = protocol.encode_json(protocol.BYE, {"reason": reason})
        for viewer in viewers:
            viewer.send(protocol.BYE, bye)
        # Os envios correm em paralelo: o prazo é um só, não 0,5 s por espectador.
        deadline = time.monotonic() + 0.5
        for viewer in viewers:
            viewer.flush(max(0.0, deadline - time.monotonic()))
            viewer.close()

    def apply_settings(self, new: HostSettings) -> None:
        """Edita a transmissão ao vivo: os espectadores continuam conectados."""
        old = self.settings
        new = replace(new, port=self.port, announce=old.announce)
        self.settings = new
        if new.video_key() != old.video_key():
            self._restart_video()
        if new.audio_key() != old.audio_key():
            self._restart_audio()
        self._notify_viewers_changed()

    def _listen(self) -> socket.socket:
        last_error = None
        for port in range(self.settings.port, self.settings.port + 20):
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
            "viewers": len(self.viewers()),
            "locked": bool(self.settings.password),
        }

    # -- espectadores --------------------------------------------------------------
    def _authed(self) -> list[_Viewer]:
        with self._lock:
            return [v for v in self._viewers if v.authed]

    def viewers(self) -> list[dict]:
        return [v.describe() for v in self._authed()]

    def viewer_names(self) -> list[str]:
        return [v.name for v in self._authed()]

    def streams(self) -> list[dict]:
        """Streams da sala: o do host (pela própria conexão) e o de quem está compartilhando."""
        result = []
        if self.settings.source is not None:
            result.append({"id": "host", "name": self.settings.host_name, "port": self.port, "addresses": []})
        for viewer in self._authed():
            if viewer.stream:
                addresses = [viewer.address] + [a for a in viewer.stream.get("addresses", []) if a != viewer.address]
                result.append({"id": viewer.viewer_id, "name": viewer.name, "port": viewer.stream["port"], "addresses": addresses})
        return result

    def request_keyframe(self) -> None:
        self._force_keyframe.set()

    def report_congestion(self) -> None:
        # O keyframe para quem teve vídeo descartado sai pelo loop de vídeo (_needs_keyframe).
        self._congestion.set()

    def _broadcast(self, msg_type: int, data: bytes, media_only: bool = False) -> None:
        for viewer in self._authed():
            if not media_only or viewer.media:
                viewer.send(msg_type, data)

    def _info_payload(self, viewer: _Viewer | None = None) -> dict:
        return {
            "room": self.settings.room_name,
            "host": self.settings.host_name,
            "viewers": self.viewers(),
            "streams": self.streams(),
            "you": viewer.viewer_id if viewer else None,
        }

    def _notify_viewers_changed(self) -> None:
        for viewer in self._authed():
            viewer.send(protocol.INFO, protocol.encode_json(protocol.INFO, self._info_payload(viewer)))
        self.on_viewers(self.viewers())

    def _info_loop(self) -> None:
        # Atualiza latências na interface de todos de tempos em tempos.
        while not self._stop.wait(3.0):
            if self._authed():
                self._notify_viewers_changed()

    def _drop_viewer(self, viewer: _Viewer) -> None:
        with self._lock:
            if viewer not in self._viewers:
                return
            self._viewers.remove(viewer)
        viewer.close()
        if viewer.authed and not self._stop.is_set():
            self._notify_viewers_changed()

    def _accept_loop(self) -> None:
        while not self._stop.is_set():
            try:
                sock, (address, _) = self._server.accept()
            except OSError:
                return
            self._add_connection(sock, address)

    def _add_connection(self, sock: socket.socket, address: str) -> None:
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, config.SEND_BUFFER_BYTES)
        viewer = _Viewer(self, sock, address)
        with self._lock:
            self._viewers.append(viewer)
        threading.Thread(target=self._viewer_reader, args=(viewer,), name=f"viewer-{address}", daemon=True).start()

    def connect_back(self, address: str, port: int) -> None:
        """Conexão reversa: o espectador não conseguiu chegar até nós, então nós conectamos nele."""
        if self._stop.is_set() or not 0 < port < 65536:
            return

        def work():
            log.info("conexão reversa para %s:%d", address, port)
            try:
                sock = socket.create_connection((address, port), timeout=config.CONNECT_TIMEOUT)
            except OSError as exc:
                log.info("conexão reversa para %s:%d falhou: %s", address, port, exc)
                return
            sock.settimeout(None)
            self._add_connection(sock, address)

        threading.Thread(target=work, name=f"callback-{address}", daemon=True).start()

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
            sock.settimeout(config.HANDSHAKE_TIMEOUT)
            msg_type, payload = protocol.recv_message(sock)
            if msg_type != protocol.HELLO:
                raise protocol.ProtocolError("esperava HELLO")
            hello = protocol.decode_json(payload)
            if hello.get("version") != config.PROTOCOL_VERSION:
                theirs = hello.get("app") or "antiga"
                sock.sendall(
                    protocol.encode_json(
                        protocol.REJECT,
                        {
                            "reason": f"Versões diferentes do StreamaZap: você tem a {theirs} e o host a {__version__}.\n"
                            f"Atualizem para a mais recente: {RELEASES_URL}"
                        },
                    )
                )
                raise protocol.ProtocolError(f"versão incompatível ({theirs})")
            if self.settings.password and not protocol.check_auth(self.settings.password, nonce, hello.get("auth", "")):
                sock.sendall(protocol.encode_json(protocol.REJECT, {"reason": "Senha incorreta"}))
                raise protocol.ProtocolError("senha incorreta")
            viewer.name = str(hello.get("name") or viewer.address)[:40]
            viewer.media = bool(hello.get("media", True))
            sock.settimeout(config.VIEWER_IDLE_TIMEOUT)
            sock.sendall(
                protocol.encode_json(
                    protocol.WELCOME,
                    {"room": self.settings.room_name, "host": self.settings.host_name, "you": viewer.viewer_id},
                )
            )
            viewer.authed = True
            threading.Thread(target=viewer.sender_loop, name=f"viewer-send-{viewer.address}", daemon=True).start()
            self.request_keyframe()
            self._notify_viewers_changed()
            if self.settings.announce:
                self.on_chat("", f"{viewer.name} entrou na sala")

            while not self._stop.is_set():
                msg_type, payload = protocol.recv_message(sock)
                if msg_type == protocol.PING:
                    ping = protocol.decode_json(payload)
                    rtt = ping.get("rtt")
                    viewer.record_rtt(float(rtt) if isinstance(rtt, (int, float)) else None)
                    delay = ping.get("delay")
                    viewer.playout_delay_ms = int(delay) if isinstance(delay, (int, float)) else None
                    received = ping.get("rx")
                    viewer.receive_rate = float(received) if isinstance(received, (int, float)) else None
                    viewer.send(protocol.PONG, protocol.encode_json(protocol.PONG, {"t": ping.get("t")}))
                elif msg_type == protocol.KEYFRAME_REQUEST:
                    viewer.waiting_keyframe = True
                    self.request_keyframe()
                elif msg_type == protocol.SUBSCRIBE:
                    viewer.media = bool(protocol.decode_json(payload).get("media", True))
                    if viewer.media:
                        viewer.waiting_keyframe = True
                        self.request_keyframe()
                elif msg_type == protocol.STREAM:
                    self._update_stream(viewer, protocol.decode_json(payload))
                elif msg_type == protocol.CALLBACK:
                    self._relay_callback(viewer, protocol.decode_json(payload))
                elif msg_type == protocol.CHAT:
                    text = str(protocol.decode_json(payload).get("text", ""))[:500]
                    if text:
                        self._relay_chat(viewer.name, text)
        except (OSError, ConnectionError, protocol.ProtocolError) as exc:
            if not viewer.closed:  # se já fechamos (ex.: envio falhou), o motivo já foi registrado
                log.info("espectador %s (%s) saiu: %s", viewer.name, viewer.address, exc)
        finally:
            was_authed = viewer.authed
            self._drop_viewer(viewer)
            if was_authed and not self._stop.is_set() and self.settings.announce:
                self.on_chat("", f"{viewer.name} saiu da sala")

    def _update_stream(self, viewer: _Viewer, data: dict) -> None:
        port = data.get("port")
        was_sharing = viewer.stream is not None
        if isinstance(port, int) and 0 < port < 65536:
            addresses = [str(a) for a in data.get("addresses", []) if isinstance(a, str)][:8]
            viewer.stream = {"port": port, "addresses": addresses}
            if not was_sharing:
                self.on_chat("", f"{viewer.name} começou a compartilhar a tela")
        else:
            viewer.stream = None
            if was_sharing:
                self.on_chat("", f"{viewer.name} parou de compartilhar")
        self._notify_viewers_changed()

    def _relay_callback(self, requester: _Viewer, data: dict) -> None:
        """Repassa ao participante dono do stream o pedido de conexão reversa."""
        port = data.get("port")
        target = next((v for v in self._authed() if v.viewer_id == data.get("stream") and v.stream), None)
        if target is not None and isinstance(port, int):
            target.send(protocol.CALLBACK, protocol.encode_json(protocol.CALLBACK, {"ip": requester.address, "port": port}))

    def request_participant_callback(self, stream_id: str, port: int) -> None:
        """O próprio host quer assistir um participante mas não alcança o compartilhamento dele."""
        target = next((v for v in self._authed() if v.viewer_id == stream_id and v.stream), None)
        if target is not None:
            # Sem "ip": o participante conecta no endereço pelo qual ele fala com o host.
            target.send(protocol.CALLBACK, protocol.encode_json(protocol.CALLBACK, {"port": port}))

    def _relay_chat(self, name: str, text: str) -> None:
        self._broadcast(protocol.CHAT, protocol.encode_json(protocol.CHAT, {"name": name, "text": text}))
        self.on_chat(name, text)

    def send_chat(self, text: str) -> None:
        self._relay_chat(self.settings.host_name, text)

    # -- áudio ---------------------------------------------------------------------
    def _start_audio(self) -> None:
        if self.settings.audio_mode == AUDIO_NONE:
            self._audio = None
            return
        if self._audio_encoder is None:
            from streamazap.media.audio import AudioEncoder  # PyAV só é carregado ao transmitir

            self._audio_encoder = AudioEncoder()
        self._audio = AudioCaptureGroup(self.settings.audio_mode, self.settings.audio_apps, self._on_audio_frame)
        self._audio.start()

    def _restart_audio(self) -> None:
        if self._audio:
            self._audio.stop()
        self._start_audio()

    def _on_audio_frame(self, pcm: bytes, timestamp_ms: int) -> None:
        for packet in self._audio_encoder.encode(pcm):
            self._broadcast(protocol.AUDIO, protocol.encode_audio(packet, timestamp_ms), media_only=True)

    # -- vídeo ---------------------------------------------------------------------
    def _start_video(self) -> None:
        if self.settings.source is None:
            return
        self._video_stop = threading.Event()
        self._video_thread = threading.Thread(
            target=self._video_loop, args=(self.settings, self._video_stop), name="host-video", daemon=True
        )
        self._video_thread.start()

    def _restart_video(self) -> None:
        self._video_stop.set()
        if self._video_thread is not None:
            self._video_thread.join(timeout=3)
        for viewer in self._authed():
            viewer.waiting_keyframe = True
        self._start_video()

    def _video_loop(self, s: HostSettings, stop: threading.Event) -> None:
        from streamazap.media.video import VideoEncoder  # PyAV só é carregado ao transmitir

        encoder = VideoEncoder(s.max_height, s.fps, s.bitrate, s.encoder)
        controller = BitrateController(s.bitrate, s.adaptive)
        encoder.set_bitrate(controller.current)
        capturer = None
        interval = 1.0 / s.fps
        last_frame = None
        last_keyframe = float("-inf")
        frames = sent_bytes = grabs = 0
        grab_seconds = 0.0
        stats_time = time.monotonic()
        next_tick = time.perf_counter()
        try:
            capturer = self._capturer_factory(s.source)
            while not stop.is_set():
                next_tick += interval
                viewers = [v for v in self._authed() if v.media]
                if not viewers:
                    # Ninguém assistindo: não captura nem codifica (CPU/GPU livres). Quem entrar
                    # pede um keyframe e tudo volta no próximo quadro.
                    now = time.monotonic()
                    if now - stats_time >= 1.0:
                        self.on_stats(
                            {
                                "idle": True,
                                "fps": 0.0,
                                "kbps": 0.0,
                                "target_kbps": controller.current // 1000,
                                "base_kbps": controller.base // 1000,
                                "audio_errors": self._audio.errors() if self._audio else [],
                            }
                        )
                        stats_time = now
                    next_tick = time.perf_counter()
                    stop.wait(interval)
                    continue
                grab_start = time.perf_counter()
                frame = capturer.grab()
                grab_seconds += time.perf_counter() - grab_start
                grabs += 1
                # Quem teve o vídeo descartado (conexão lenta) espera um keyframe: sai em até 2 s,
                # sem deixar um espectador lento forçar keyframes (caros) para todos o tempo todo.
                recovering = any(v.waiting_keyframe for v in viewers) and time.monotonic() - last_keyframe >= 2.0
                force = self._force_keyframe.is_set() or recovering
                if frame is None:
                    # Janela minimizada: só reenvia o último quadro se alguém precisa de keyframe.
                    frame = last_frame if force else None
                if frame is not None:
                    last_frame = frame
                    self._force_keyframe.clear()
                    timestamp_ms = int(time.monotonic() * 1000)
                    packets = encoder.encode(frame, force_keyframe=force)
                    self.encoder_name = encoder.codec_name
                    for packet, keyframe in packets:
                        if keyframe:
                            last_keyframe = time.monotonic()
                        message = protocol.encode_video(packet, keyframe, timestamp_ms)
                        sent_bytes += len(packet)
                        for viewer in viewers:
                            viewer.send_video(message, keyframe)
                    frames += 1

                now = time.monotonic()
                if now - stats_time >= 1.0:
                    elapsed = now - stats_time
                    for viewer in viewers:
                        viewer.measure_rate(elapsed)
                        viewer.tune_send_buffer(controller.current)
                    lag = max((v.congestion_delay() for v in viewers), default=0.0)
                    # Vídeo que os espectadores congestionados conseguiram escoar (tirando o áudio).
                    congested = [
                        v.capacity() for v in viewers if v.overused or v.congestion_delay() > BitrateController.LOW_LAG
                    ]
                    throughput = max(0.0, min(congested) - config.AUDIO_BITRATE * 1.2) if congested else None
                    dropped = self._congestion.is_set() or any(v.overused for v in viewers)
                    self._congestion.clear()
                    size = encoder.size  # antes de uma troca de bitrate fechar o codificador
                    new_bitrate = controller.update(now, lag, dropped, throughput or None)
                    if new_bitrate is not None:
                        log.info("bitrate ajustado para %d kbps (atraso %.2fs)", new_bitrate // 1000, lag)
                        encoder.set_bitrate(new_bitrate)
                    self.on_stats(
                        {
                            "fps": frames / elapsed,
                            "kbps": sent_bytes * 8 / 1000 / elapsed,
                            "target_kbps": controller.current // 1000,
                            "base_kbps": controller.base // 1000,
                            "starting": controller.starting,
                            "lag": lag,
                            "encoder": encoder.codec_name,
                            "size": size,
                            "capture": getattr(capturer, "backend", None),
                            "capture_ms": grab_seconds * 1000 / grabs if grabs else 0.0,
                            "audio_errors": self._audio.errors() if self._audio else [],
                        }
                    )
                    frames = sent_bytes = grabs = 0
                    grab_seconds = 0.0
                    stats_time = now

                delay = next_tick - time.perf_counter()
                if delay > 0:
                    stop.wait(delay)
                elif delay < -interval:
                    # Mais de um quadro atrasado (captura lenta): recomeça sem acumular atraso.
                    # Um atraso menor é compensado no próximo quadro, mantendo a média de fps.
                    next_tick = time.perf_counter()
        except Exception as exc:  # noqa: BLE001 - reportado na interface
            log.exception("loop de vídeo falhou")
            if not stop.is_set():
                self.on_error(str(exc))
        finally:
            if capturer is not None:
                capturer.close()


def local_addresses() -> list[str]:
    """IPs deste PC, Radmin primeiro (enviados ao host quando começamos a compartilhar)."""
    return [i.ip for i in netutil.ipv4_interfaces()]
