"""Reprodução suave do vídeo (jitter buffer).

Pela internet os pacotes chegam em rajadas: alguns atrasam, outros chegam
juntos. Em vez de mostrar cada quadro assim que chega, cada quadro é exibido
no instante `timestamp do host + atraso de rede mínimo + atraso de suavização`.
Assim o ritmo de exibição segue o ritmo em que o host capturou, e a variação
da rede (até o tamanho do atraso escolhido) fica invisível.

Os pacotes ficam na fila ainda comprimidos (pouca memória) e são decodificados
na hora de exibir.
"""

from __future__ import annotations

import collections
import logging
import threading
import time
from collections.abc import Callable

import numpy as np

log = logging.getLogger(__name__)

# Se a fila passar desse tanto além do atraso desejado, pula para o presente.
MAX_EXTRA_BACKLOG = 1.0


class SlidingMin:
    """Mínimo de uma série nos últimos `window` segundos (fila monotônica, O(1) amortizado)."""

    def __init__(self, window: float):
        self.window = window
        self._items: collections.deque[tuple[float, float]] = collections.deque()

    def add(self, now: float, value: float) -> float:
        while self._items and self._items[-1][1] >= value:
            self._items.pop()
        self._items.append((now, value))
        while self._items[0][0] < now - self.window:
            self._items.popleft()
        return self._items[0][1]

    def reset(self) -> None:
        self._items.clear()


class VideoPlayout:
    def __init__(
        self,
        decode: Callable[[bytes], list[np.ndarray]],
        on_frame: Callable[[np.ndarray], None],
        on_decode_error: Callable[[], None],
        delay_ms: int,
        clock: Callable[[], float] = time.monotonic,
    ):
        self._decode = decode
        self._on_frame = on_frame
        self._on_decode_error = on_decode_error
        self._clock = clock
        self.delay = delay_ms / 1000
        # Offset entre o relógio do host e o nosso + menor tempo de trânsito visto.
        self._base = SlidingMin(10.0)
        self._queue: collections.deque[tuple[float, bytes]] = collections.deque()
        self._cond = threading.Condition()
        self._stop = False
        self.frames_shown = 0
        self.frames_skipped = 0
        self._thread = threading.Thread(target=self._run, name="video-playout", daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        with self._cond:
            self._stop = True
            self._cond.notify()

    def set_delay(self, delay_ms: int) -> None:
        with self._cond:
            self.delay = delay_ms / 1000
            self._cond.notify()

    def buffered_seconds(self) -> float:
        with self._cond:
            if not self._queue:
                return 0.0
            return max(0.0, self._queue[-1][0] - self._clock())

    def due_time(self, host_ts_ms: int) -> float:
        now = self._clock()
        host_ts = host_ts_ms / 1000
        base = self._base.add(now, now - host_ts)
        return host_ts + base + self.delay

    def push(self, packet: bytes, host_ts_ms: int) -> None:
        due = self.due_time(host_ts_ms)
        with self._cond:
            self._queue.append((due, packet))
            self._cond.notify()

    def _take_due(self) -> list[bytes] | None:
        """Espera até haver pacote(s) no horário; retorna todos os vencidos."""
        with self._cond:
            while not self._stop:
                if not self._queue:
                    self._cond.wait(0.5)
                    continue
                now = self._clock()
                # Fila grande demais (ex.: rede travou e depois despejou tudo): alcança o presente.
                if self._queue[-1][0] - now > self.delay + MAX_EXTRA_BACKLOG:
                    return [p for _, p in self._drain()]
                wait = self._queue[0][0] - now
                if wait > 0:
                    self._cond.wait(min(wait, 0.05))
                    continue
                due = []
                while self._queue and self._queue[0][0] <= now:
                    due.append(self._queue.popleft()[1])
                return due
            return None

    def _drain(self):
        items = list(self._queue)
        self._queue.clear()
        return items

    def _run(self) -> None:
        while True:
            packets = self._take_due()
            if packets is None:
                return
            last = None
            for packet in packets:  # todos precisam ser decodificados (referências do H.264)
                try:
                    frames = self._decode(packet)
                except Exception:  # noqa: BLE001 - quadro corrompido ou sem referência
                    self._on_decode_error()
                    continue
                if frames:
                    if last is not None:
                        self.frames_skipped += 1
                    last = frames[-1]
            if last is not None:
                self.frames_shown += 1
                self._on_frame(last)
