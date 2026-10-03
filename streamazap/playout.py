"""Reprodução suave e sincronizada (jitter buffer).

Pela internet os pacotes chegam em rajadas: alguns atrasam, outros chegam
juntos. Em vez de tocar cada coisa assim que chega, tudo (vídeo E áudio) é
tocado no instante

    horário de captura no host + menor atraso de rede visto + suavização

calculado pelo mesmo `MediaClock`. Assim o ritmo segue o do host, a variação
da rede fica invisível e som e imagem ficam alinhados entre si.

Os pacotes de vídeo ficam na fila ainda comprimidos (pouca memória) e são
decodificados na hora de exibir.
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


class MediaClock:
    """Converte horário do host em horário local de reprodução (compartilhado por vídeo e áudio)."""

    def __init__(self, delay_ms: int, clock: Callable[[], float] = time.monotonic):
        self.now = clock
        self.delay = delay_ms / 1000
        # Offset entre o relógio do host e o nosso + menor tempo de trânsito recente.
        self._base = SlidingMin(10.0)
        self._base_value: float | None = None
        self._lock = threading.Lock()

    def set_delay(self, delay_ms: int) -> None:
        self.delay = delay_ms / 1000

    def observe(self, host_ts_ms: int) -> None:
        """Registra a chegada de um pacote (vídeo ou áudio) com esse timestamp."""
        now = self.now()
        with self._lock:
            self._base_value = self._base.add(now, now - host_ts_ms / 1000)

    def play_time(self, host_ts: float) -> float:
        """Instante local (relógio monotônico) em que algo capturado em `host_ts` (segundos) deve tocar."""
        base = self._base_value
        if base is None:
            return self.now()
        return host_ts + base + self.delay

    def reset(self) -> None:
        with self._lock:
            self._base.reset()
            self._base_value = None


class VideoPlayout:
    def __init__(
        self,
        decode: Callable[[bytes], list[np.ndarray]],
        on_frame: Callable[[np.ndarray], None],
        on_decode_error: Callable[[], None],
        clock: MediaClock,
    ):
        self._decode = decode
        self._on_frame = on_frame
        self._on_decode_error = on_decode_error
        self.clock = clock
        self._queue: collections.deque[tuple[float, bytes]] = collections.deque()  # (host_ts s, pacote)
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

    def wake(self) -> None:
        """Reavalia horários (ex.: suavização mudou)."""
        with self._cond:
            self._cond.notify()

    def buffered_seconds(self) -> float:
        with self._cond:
            if not self._queue:
                return 0.0
            return max(0.0, self.clock.play_time(self._queue[-1][0]) - self.clock.now())

    def push(self, packet: bytes, host_ts_ms: int) -> None:
        """O chamador já deve ter feito clock.observe(host_ts_ms)."""
        with self._cond:
            self._queue.append((host_ts_ms / 1000, packet))
            self._cond.notify()

    def _take_due(self) -> list[bytes] | None:
        """Espera até haver pacote(s) no horário; retorna todos os vencidos."""
        with self._cond:
            while not self._stop:
                if not self._queue:
                    self._cond.wait(0.5)
                    continue
                now = self.clock.now()
                # Fila grande demais (ex.: rede travou e depois despejou tudo): alcança o presente.
                if self.clock.play_time(self._queue[-1][0]) - now > self.clock.delay + MAX_EXTRA_BACKLOG:
                    items = [p for _, p in self._queue]
                    self._queue.clear()
                    return items
                wait = self.clock.play_time(self._queue[0][0]) - now
                if wait > 0:
                    self._cond.wait(min(wait, 0.05))
                    continue
                due = []
                while self._queue and self.clock.play_time(self._queue[0][0]) <= now:
                    due.append(self._queue.popleft()[1])
                return due
            return None

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
