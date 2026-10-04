"""Reprodução suave e sincronizada (jitter buffer).

Pela internet os pacotes chegam em rajadas: alguns atrasam, outros chegam
juntos. Em vez de tocar cada coisa assim que chega, tudo (vídeo E áudio) é
tocado no instante

    horário de captura no host + menor atraso de rede visto + suavização

calculado pelo mesmo `MediaClock`. Assim o ritmo segue o do host, a variação
da rede fica invisível e som e imagem ficam alinhados entre si.

Os pacotes de vídeo ficam na fila ainda comprimidos (pouca memória) e são
decodificados na hora de exibir. Quando vários vencem juntos, todos são
decodificados (referências do H.264), mas só o último vai para `on_frame`.
"""

from __future__ import annotations

import collections
import logging
import threading
import time
from collections.abc import Callable
from typing import Any

from streamazap import config

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


class SlidingMax(SlidingMin):
    """Máximo de uma série nos últimos `window` segundos."""

    def add(self, now: float, value: float) -> float:
        return -super().add(now, -value)


class MediaClock:
    """Converte horário do host em horário local de reprodução (compartilhado por vídeo e áudio).

    Manual: toca em `captura + menor trânsito recente + delay` (a suavização escolhida).

    Automático: a suavização se ajusta sozinha pela rede. Cada pacote precisa chegar
    `lead` antes de tocar (decodificar o vídeo; a placa de som pede o áudio adiantado).
      * pacote atrasado → sobe na hora se o áudio atrasou (ele já está em silêncio, então
        não cria corte novo), se não há áudio ou se o atraso é enorme; se só o vídeo
        atrasou, sobe até 100 ms na hora (o áudio absorve tocando um pouco mais devagar) e o
        resto a 2%/s, ritmo que o áudio acompanha sem cortar;
      * rede estável por um tempo → desce até cobrir o pior atraso recente: 1,5%/s com som
        tocando (o áudio acompanha sem cortes); rápido se não há áudio ou ele está em silêncio
        (pular silêncio não se ouve).
    O vídeo e o áudio seguem o mesmo relógio, então continuam sincronizados.
    """

    AUTO_MIN_DELAY = 0.04
    AUTO_MARGIN = 0.03
    AUTO_WINDOW = 20.0  # o pior atraso desses últimos segundos define até onde dá para descer
    AUTO_HOLD = 5.0  # sem atraso novo por esse tempo antes de começar a descer
    AUTO_UP_RATE = 0.02  # s por s
    AUTO_STEP = 0.1  # subida imediata quando só o vídeo atrasou (o áudio estica sem cortar)
    AUTO_BIG_LATE = 0.3  # atraso tão grande que vale subir tudo de uma vez
    AUTO_DOWN_RATE = 0.015
    AUTO_FAST_DOWN_RATE = 0.10  # sem áudio ou com áudio em silêncio
    AUDIO_ACTIVE = 1.0  # áudio chegando nos últimos X s
    QUIET_LEVEL = 0.005  # nível médio do áudio (0-1) abaixo do qual é silêncio

    def __init__(self, delay_ms: int, clock: Callable[[], float] = time.monotonic, auto: bool = False):
        self.now = clock
        self.delay = delay_ms / 1000
        self.max_delay = config.MAX_PLAYOUT_DELAY_MS / 1000
        self.auto = auto
        # Antecedência com que cada mídia precisa chegar (ajustadas pelo player e pelo playout).
        self.audio_lead = 0.05
        self.video_lead = 0.015
        self.late_events = 0  # pacotes que chegaram depois da hora (contador)
        self.audio_level = 1.0  # nível recente do áudio tocado (o player atualiza)
        # Offset entre o relógio do host e o nosso + menor tempo de trânsito recente.
        self._base = SlidingMin(10.0)
        self._base_value: float | None = None
        self._peak = SlidingMax(self.AUTO_WINDOW)
        self._offset: float | None = None  # automático: tocar em host_ts + offset
        self._ramp_to = 0.0
        self._last_update = 0.0
        self._last_late = float("-inf")
        self._last_audio = float("-inf")
        self._lock = threading.Lock()

    def set_delay(self, delay_ms: int) -> None:
        """Suavização manual (e ponto de partida do automático)."""
        self.delay = delay_ms / 1000

    def set_auto(self, enabled: bool) -> None:
        with self._lock:
            if enabled and not self.auto and self._base_value is not None:
                self._offset = self._base_value + self.delay  # continua de onde está
                self._last_update = self._last_late = self.now()
            self.auto = enabled

    def current_delay(self) -> float:
        """Suavização em uso agora (segundos além do menor trânsito)."""
        base, offset = self._base_value, self._offset
        if self.auto and base is not None and offset is not None:
            return offset - base
        return self.delay

    def observe(self, host_ts_ms: int, kind: str = "video") -> None:
        """Registra a chegada de um pacote ("video" ou "audio") com esse timestamp."""
        now = self.now()
        transit = now - host_ts_ms / 1000
        with self._lock:
            base = self._base_value = self._base.add(now, transit)
            if not self.auto:
                return
            need = transit + (self.audio_lead if kind == "audio" else self.video_lead)
            peak = self._peak.add(now, need)
            if kind == "audio":
                self._last_audio = now
            if self._offset is None:
                self._offset = base + self.delay
                self._last_update = now
                self._last_late = now  # observa a rede um pouco antes de começar a descer
            offset = self._advance(now, base, peak)
            if need > offset:
                self.late_events += 1
                self._last_late = now
                wanted = need + self.AUTO_MARGIN
                if kind == "audio" or now - self._last_audio > self.AUDIO_ACTIVE or need - offset > self.AUTO_BIG_LATE:
                    offset = wanted
                else:
                    # Só o vídeo atrasou: até 100 ms sobe na hora (o áudio absorve tocando um pouco
                    # mais devagar, sem corte); o resto sobe aos poucos.
                    self._ramp_to = max(self._ramp_to, wanted)
                    offset = min(wanted, offset + self.AUTO_STEP)
            self._offset = min(max(offset, base + self.AUTO_MIN_DELAY), base + self.max_delay)

    def _advance(self, now: float, base: float, peak: float) -> float:
        """Aplica a subida gradual ou a descida lenta desde a última atualização."""
        elapsed, self._last_update = now - self._last_update, now
        offset = self._offset
        if self._ramp_to > offset:
            self._last_late = now  # subindo: não começa a descer
            return min(self._ramp_to, offset + self.AUTO_UP_RATE * elapsed)
        self._ramp_to = 0.0
        target = peak + self.AUTO_MARGIN
        if now - self._last_late > self.AUTO_HOLD and offset > target:
            quiet = now - self._last_audio > self.AUDIO_ACTIVE or self.audio_level < self.QUIET_LEVEL
            rate = self.AUTO_FAST_DOWN_RATE if quiet else self.AUTO_DOWN_RATE
            return max(target, offset - rate * elapsed)
        return offset

    def play_time(self, host_ts: float) -> float:
        """Instante local (relógio monotônico) em que algo capturado em `host_ts` (segundos) deve tocar."""
        base, offset = self._base_value, self._offset
        if base is None:
            return self.now()
        if self.auto and offset is not None:
            return host_ts + offset
        return host_ts + base + self.delay

    def reset(self) -> None:
        with self._lock:
            self._base.reset()
            self._peak.reset()
            self._base_value = None
            self._offset = None
            self._ramp_to = 0.0


class VideoPlayout:
    """Decodifica cada quadro um pouco antes da hora (o tempo medido de decodificação) e o
    entrega exatamente no horário do relógio, alinhado com o áudio."""

    def __init__(
        self,
        decode: Callable[[bytes], list],
        on_frame: Callable[[Any], None],
        on_decode_error: Callable[[], None],
        clock: MediaClock,
        prepare: Callable[[Any], Any] = lambda frame: frame,
    ):
        """`prepare`: conversão feita antes da hora (ex.: YUV -> imagem), fora do tempo de exibição."""
        self._decode = decode
        self._prepare = prepare
        self._on_frame = on_frame
        self._on_decode_error = on_decode_error
        self.clock = clock
        self._queue: collections.deque[tuple[float, bytes]] = collections.deque()  # (host_ts s, pacote)
        self._cond = threading.Condition()
        self._stop = False
        self._work_time = 0.005  # média de decodificar + preparar um quadro (s)
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

    def _lead(self) -> float:
        return min(0.05, self._work_time * 1.5 + 0.003)

    def _take_due(self) -> tuple[list[bytes], float | None] | None:
        """Espera até haver pacote(s) quase no horário; retorna (pacotes, host_ts do último)."""
        with self._cond:
            while not self._stop:
                if not self._queue:
                    self._cond.wait(0.5)
                    continue
                now = self.clock.now()
                # Fila grande demais (ex.: rede travou e depois despejou tudo): alcança o presente.
                if self.clock.play_time(self._queue[-1][0]) - now > self.clock.current_delay() + MAX_EXTRA_BACKLOG:
                    items = [p for _, p in self._queue]
                    self._queue.clear()
                    return items, None
                lead = self._lead()
                wait = self.clock.play_time(self._queue[0][0]) - lead - now
                if wait > 0:
                    self._cond.wait(min(wait, 0.05))
                    continue
                due, last_ts = [], None
                while self._queue and self.clock.play_time(self._queue[0][0]) - lead <= now:
                    last_ts, packet = self._queue.popleft()
                    due.append(packet)
                return due, last_ts
            return None

    def _wait_until_due(self, host_ts: float) -> bool:
        """Espera a hora exata de exibir; False se pararam o playout."""
        with self._cond:
            while not self._stop:
                remaining = self.clock.play_time(host_ts) - self.clock.now()
                if remaining <= 0:
                    return True
                self._cond.wait(min(remaining, 0.05))
            return False

    def _run(self) -> None:
        while True:
            taken = self._take_due()
            if taken is None:
                return
            packets, last_ts = taken
            started = time.perf_counter()
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
            if last is None:
                continue
            image = self._prepare(last)
            spent = (time.perf_counter() - started) / len(packets)
            self._work_time += (spent - self._work_time) * 0.1
            self.clock.video_lead = self._lead() + 0.005  # o pacote precisa chegar antes disso
            if last_ts is not None and not self._wait_until_due(last_ts):
                return
            self.frames_shown += 1
            self._on_frame(image)
