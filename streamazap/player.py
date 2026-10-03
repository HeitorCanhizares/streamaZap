"""Reprodução do áudio recebido, alinhada ao vídeo pelo MediaClock.

Cada bloco de áudio chega com o horário de captura no host. O callback da placa
de som calcula quando a próxima amostra vai sair no alto-falante (agora +
latência da saída) e entrega exatamente o áudio que deve tocar naquele instante:
  * erro pequeno (relógios andando em ritmos levemente diferentes): corrige aos
    poucos, descartando/repetindo algumas amostras (inaudível);
  * erro grande (rede travou, suavização mudou): pula ou espera de uma vez.
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
SOFT_ERROR = 0.012  # acima disso começa a corrigir aos poucos
HARD_ERROR = 0.080  # acima disso corrige de uma vez
MAX_QUEUE_SECONDS = 3.0


def _bytes_for(seconds: float) -> int:
    return max(0, int(seconds * config.AUDIO_RATE)) * FRAME_BYTES


class AudioPlayer:
    def __init__(self, clock: MediaClock):
        self.clock = clock
        self.volume = 1.0
        self.muted = False
        self._queue: collections.deque[list] = collections.deque()  # [host_ts (s) da 1ª amostra, bytearray]
        self._queued_bytes = 0
        self._lock = threading.Lock()
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

    def _consume(self, size: int) -> bytes:
        """Tira `size` bytes do início da fila (lock já adquirido)."""
        out = bytearray()
        while size > 0 and self._queue:
            head = self._queue[0]
            take = min(size, len(head[1]))
            out += head[1][:take]
            del head[1][:take]
            head[0] += take / BYTES_PER_SECOND
            self._queued_bytes -= take
            size -= take
            if not head[1]:
                self._queue.popleft()
        return bytes(out)

    def render(self, size: int, play_start: float) -> bytes:
        """Monta `size` bytes que começam a soar no instante local `play_start`."""
        out = bytearray()
        nudged = False
        with self._lock:
            while len(out) < size:
                if not self._queue:
                    break
                sample_time = play_start + len(out) / BYTES_PER_SECOND
                error = self.clock.play_time(self._queue[0][0]) - sample_time  # >0 adiantado, <0 atrasado
                remaining = size - len(out)
                if error > HARD_ERROR:
                    # Ainda não é hora: silêncio até lá (no máximo o que falta deste bloco).
                    out += bytes(min(remaining, _bytes_for(error)))
                    continue
                if error < -HARD_ERROR:
                    # Atrasado (rede travou): pula direto para o presente.
                    dropped = self._consume(_bytes_for(-error))
                    if not dropped:
                        break
                    continue
                chunk = remaining
                if abs(error) > SOFT_ERROR and not nudged:
                    # Correção suave (uma vez por callback): ~0,5% mais rápido/devagar até alinhar.
                    nudged = True
                    nudge = max(FRAME_BYTES, (remaining // 200) // FRAME_BYTES * FRAME_BYTES)
                    if error < 0:
                        self._consume(nudge)  # atrasado: descarta algumas amostras
                    else:
                        piece = self._consume(nudge)  # adiantado: repete algumas amostras
                        out += piece + piece
                        continue
                out += self._consume(chunk)
        if len(out) < size:
            out += bytes(size - len(out))
        return bytes(out[:size])

    def buffered_seconds(self) -> float:
        with self._lock:
            return self._queued_bytes / BYTES_PER_SECOND

    def _callback(self, outdata, frames, time_info, status) -> None:
        latency = time_info.outputBufferDacTime - time_info.currentTime
        if not 0 < latency < 1:
            latency = self.output_latency
        chunk = self.render(len(outdata), self.clock.now() + latency)
        if self.muted:
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
