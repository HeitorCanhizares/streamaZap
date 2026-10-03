"""Reprodução do áudio recebido com um pequeno buffer anti-oscilação."""

from __future__ import annotations

import logging
import threading

import numpy as np

from streamazap import config

log = logging.getLogger(__name__)

BYTES_PER_SECOND = config.AUDIO_RATE * config.AUDIO_CHANNELS * 2
TARGET_BUFFER = int(BYTES_PER_SECOND * 0.06)  # 60 ms antes de começar a tocar
MAX_BUFFER = int(BYTES_PER_SECOND * 0.30)  # acima disso, descarta para não acumular atraso


class AudioPlayer:
    def __init__(self):
        self.volume = 1.0
        self.muted = False
        self._buffer = bytearray()
        self._playing = False
        self._lock = threading.Lock()
        self._stream = None
        self.error: str | None = None

    def start(self) -> None:
        if self._stream is not None or self.error:
            return
        try:
            import sounddevice as sd

            self._stream = sd.RawOutputStream(
                samplerate=config.AUDIO_RATE,
                channels=config.AUDIO_CHANNELS,
                dtype="int16",
                latency="low",
                callback=self._callback,
            )
            self._stream.start()
        except Exception as exc:  # noqa: BLE001 - sem saída de áudio o vídeo continua
            self.error = f"Sem saída de áudio: {exc}"
            log.warning(self.error)

    def stop(self) -> None:
        if self._stream is not None:
            self._stream.stop()
            self._stream.close()
            self._stream = None

    def push(self, pcm: bytes) -> None:
        with self._lock:
            self._buffer += pcm
            if len(self._buffer) > MAX_BUFFER:
                cut = len(self._buffer) - TARGET_BUFFER
                del self._buffer[: cut - cut % 4]

    def take(self, size: int) -> bytes:
        """Retira `size` bytes do buffer (silêncio enquanto está enchendo)."""
        with self._lock:
            if not self._playing:
                if len(self._buffer) < TARGET_BUFFER:
                    return bytes(size)
                self._playing = True
            chunk = bytes(self._buffer[:size])
            del self._buffer[:size]
            if len(chunk) < size:
                self._playing = False  # acabou o buffer: volta a acumular
                chunk += bytes(size - len(chunk))
        if self.muted:
            return bytes(size)
        if self.volume != 1.0:
            samples = np.frombuffer(chunk, dtype=np.int16).astype(np.float32) * self.volume
            chunk = np.clip(samples, -32768, 32767).astype(np.int16).tobytes()
        return chunk

    def _callback(self, outdata, frames, time_info, status) -> None:
        outdata[:] = self.take(len(outdata))
