"""Reprodução do áudio recebido, alinhada ao vídeo pelo MediaClock.

Cada bloco de áudio chega com o horário de captura no host. O callback da placa
de som calcula quando a próxima amostra vai sair no alto-falante (agora +
latência da saída) e entrega exatamente o áudio que deve tocar naquele instante:
  * erro pequeno (relógios em ritmos levemente diferentes, suavização automática
    mudando): toca até 2% mais rápido/devagar, reamostrando o bloco inteiro
    (contínuo, sem estalos), proporcional ao erro;
  * erro grande (rede travou): espera ou pula de uma vez.
Toda emenda (silêncio, pulo, falta de dados) ganha um fade de 2 ms para não estalar.
"""

from __future__ import annotations

import collections
import logging
import threading

import numpy as np

from streamazap import config
from streamazap.config import IS_WINDOWS
from streamazap.playout import MediaClock

log = logging.getLogger(__name__)

FRAME_BYTES = config.AUDIO_CHANNELS * 2  # uma amostra estéreo s16
BYTES_PER_SECOND = config.AUDIO_RATE * FRAME_BYTES
SOFT_ERROR = 0.004  # até aqui toca na velocidade normal
HARD_ERROR = 0.120  # acima disso corrige de uma vez
SPEED_GAIN = 1.0  # 1% mais rápido/devagar a cada 10 ms de erro...
MAX_SPEED_CHANGE = 0.02  # ...até 2% (mudança de tom imperceptível na prática)
FADE_SAMPLES = 96  # 2 ms
MAX_QUEUE_SECONDS = 3.0


def _bytes_for(seconds: float) -> int:
    return max(0, int(seconds * config.AUDIO_RATE)) * FRAME_BYTES


def _frames_for(seconds: float) -> int:
    return max(0, int(seconds * config.AUDIO_RATE))


def _stretch(chunk: np.ndarray, frames: int) -> np.ndarray:
    """Reamostra `chunk` (amostras, canais) para `frames` amostras (interpolação linear)."""
    if len(chunk) == frames or len(chunk) < 2:
        return chunk[:frames]
    x = np.linspace(0, len(chunk) - 1, frames)
    xp = np.arange(len(chunk))
    return np.stack([np.interp(x, xp, chunk[:, c]) for c in range(chunk.shape[1])], axis=1).astype(np.int16)


class AudioPlayer:
    def __init__(self, clock: MediaClock):
        self.clock = clock
        self.volume = 1.0
        self.muted = False
        self._queue: collections.deque[list] = collections.deque()  # [host_ts (s) da 1ª amostra, bytearray]
        self._queued_bytes = 0
        self._lock = threading.Lock()
        self._fraction = 0.0  # resto de amostra da reamostragem
        self._continuous = False  # o próximo áudio continua o anterior sem emenda
        self._last_sample: np.ndarray | None = None  # última amostra tocada (se o bloco terminou com som)
        self.skipped_frames = 0
        self._stream = None
        self.error: str | None = None
        self.output_latency = 0.0

    # -- saída de áudio ------------------------------------------------------------
    def start(self) -> None:
        if self._stream is not None or self.error:
            return
        try:
            import sounddevice as sd
        except Exception as exc:  # noqa: BLE001 - sem saída de áudio o vídeo continua
            self.error = f"Sem saída de áudio: {exc}"
            log.warning(self.error)
            return
        attempts = []
        if IS_WINDOWS:
            wasapi = _wasapi_output(sd)
            if wasapi:
                attempts.append(wasapi)
        attempts.append({})
        last_error = None
        for extra in attempts:
            try:
                stream = sd.RawOutputStream(
                    samplerate=config.AUDIO_RATE,
                    channels=config.AUDIO_CHANNELS,
                    dtype="int16",
                    latency="low",
                    callback=self._callback,
                    **extra,
                )
                stream.start()
            except Exception as exc:  # noqa: BLE001
                last_error = exc
                log.info("saída de áudio %s indisponível: %s", extra or "padrão", exc)
                continue
            self._stream = stream
            self.output_latency = float(stream.latency)
            log.info("saída de áudio aberta (latência %.0f ms)", self.output_latency * 1000)
            return
        self.error = f"Sem saída de áudio: {last_error}"
        log.warning(self.error)

    def stop(self) -> None:
        if self._stream is not None:
            self._stream.stop()
            self._stream.close()
            self._stream = None

    # -- fila ----------------------------------------------------------------------
    def push(self, pcm: bytes, host_ts_ms: int) -> None:
        """O chamador já deve ter feito clock.observe(host_ts_ms)."""
        with self._lock:
            self._queue.append([host_ts_ms / 1000, bytearray(pcm)])
            self._queued_bytes += len(pcm)
            while self._queued_bytes > _bytes_for(MAX_QUEUE_SECONDS) and len(self._queue) > 1:
                self._queued_bytes -= len(self._queue.popleft()[1])

    def clear(self) -> None:
        with self._lock:
            self._queue.clear()
            self._queued_bytes = 0

    def _take(self, frames: int) -> np.ndarray:
        """Tira até `frames` amostras do início da fila (lock já adquirido)."""
        pieces = []
        size = frames * FRAME_BYTES
        while size > 0 and self._queue:
            head = self._queue[0]
            take = min(size, len(head[1]))
            pieces.append(bytes(head[1][:take]))
            del head[1][:take]
            head[0] += take / BYTES_PER_SECOND
            self._queued_bytes -= take
            size -= take
            if not head[1]:
                self._queue.popleft()
        data = b"".join(pieces)
        return np.frombuffer(data, np.int16).reshape(-1, config.AUDIO_CHANNELS)

    def render(self, size: int, play_start: float) -> bytes:
        """Monta `size` bytes que começam a soar no instante local `play_start`."""
        frames = size // FRAME_BYTES
        out = np.zeros((frames, config.AUDIO_CHANNELS), np.int16)
        segments: list[tuple[int, int, bool]] = []  # (início, fim, emendado ao anterior sem corte)
        pos = 0
        with self._lock:
            while pos < frames and self._queue:
                error = self.clock.play_time(self._queue[0][0]) - (play_start + pos / config.AUDIO_RATE)
                # Começando/retomando (depois de silêncio) dá para alinhar exato sem ninguém notar;
                # tocando sem parar, só um erro grande justifica esperar/pular de uma vez.
                limit = HARD_ERROR if self._continuous else SOFT_ERROR
                if error > limit:  # ainda não é hora: silêncio até lá
                    pos += min(frames - pos, max(1, _frames_for(error)))
                    self._continuous = False
                    continue
                if error < -limit:  # atrasado (rede travou): pula para o presente
                    self.skipped_frames += len(self._take(_frames_for(-error)))
                    self._continuous = False
                    continue
                remaining = frames - pos
                # >0 adiantado: toca mais devagar (consome menos); <0 atrasado: mais rápido.
                speed = 1.0 if abs(error) <= SOFT_ERROR else 1.0 - max(-MAX_SPEED_CHANGE, min(MAX_SPEED_CHANGE, error * SPEED_GAIN))
                wanted = self._fraction + remaining * speed
                count = int(wanted)
                self._fraction = wanted - count
                chunk = self._take(count)
                if not len(chunk):
                    break
                produced = remaining if len(chunk) == count else max(1, round(len(chunk) / speed))
                piece = _stretch(chunk, min(produced, remaining))
                segments.append((pos, pos + len(piece), self._continuous and (not segments or segments[-1][1] == pos)))
                out[pos : pos + len(piece)] = piece
                pos += len(piece)
                self._continuous = True
            if pos < frames:
                self._continuous = False  # acabou o áudio (ou chegou cedo): a próxima entrada é emenda
        self._smooth_edges(out, segments)
        # Nível do que está tocando: em silêncio o relógio pode reduzir o atraso mais rápido.
        level = float(np.abs(out).mean()) / 32768 if frames else 0.0
        self.clock.audio_level += (level - self.clock.audio_level) * 0.2
        return out.tobytes()

    def _smooth_edges(self, out: np.ndarray, segments: list[tuple[int, int, bool]]) -> None:
        """Fade de 2 ms em toda emenda (silêncio <-> áudio, pulos) para não estalar."""
        fade_in = np.linspace(0.0, 1.0, FADE_SAMPLES, dtype=np.float32)[:, None]
        fade_out = fade_in[::-1]
        continues = bool(segments) and segments[0][0] == 0 and segments[0][2]
        if self._last_sample is not None and not continues:
            # O bloco anterior terminou com som e este não o continua: decai do último valor até zero.
            n = min(FADE_SAMPLES, len(out))
            tail = self._last_sample[None, :].astype(np.float32) * fade_out[:n]
            out[:n] = np.clip(out[:n] + tail, -32768, 32767).astype(np.int16)
        for i, (start, end, joined) in enumerate(segments):
            n = min(FADE_SAMPLES, end - start)
            if not joined:
                out[start : start + n] = (out[start : start + n] * fade_in[:n]).astype(np.int16)
            followed = i + 1 < len(segments) and segments[i + 1][2]
            if end < len(out) and not followed:
                out[end - n : end] = (out[end - n : end] * fade_out[-n:]).astype(np.int16)
        ends_with_audio = bool(segments) and segments[-1][1] == len(out)
        self._last_sample = out[-1].copy() if ends_with_audio else None

    def buffered_seconds(self) -> float:
        with self._lock:
            return self._queued_bytes / BYTES_PER_SECOND

    def _callback(self, outdata, frames, time_info, status) -> None:
        latency = time_info.outputBufferDacTime - time_info.currentTime
        if not 0 < latency < 1:
            latency = self.output_latency
        chunk = self.render(len(outdata), self.clock.now() + latency)
        if self.muted or self.volume == 0:
            self.clock.audio_level = 0.0  # ninguém está ouvindo: a suavização pode descer rápido
            chunk = bytes(len(chunk))
        elif self.volume != 1.0:
            samples = np.frombuffer(chunk, dtype=np.int16).astype(np.float32) * self.volume
            chunk = np.clip(samples, -32768, 32767).astype(np.int16).tobytes()
        outdata[:] = chunk


def _wasapi_output(sd) -> dict | None:
    """WASAPI tem latência bem menor (e mais previsível) que o MME padrão do PortAudio."""
    try:
        for api in sd.query_hostapis():
            if "WASAPI" in api["name"] and api["default_output_device"] >= 0:
                try:
                    settings = sd.WasapiSettings(auto_convert=True)
                except TypeError:
                    settings = sd.WasapiSettings()
                return {"device": api["default_output_device"], "extra_settings": settings}
    except Exception:  # noqa: BLE001
        return None
    return None
