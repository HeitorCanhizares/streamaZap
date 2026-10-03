"""Seleção de fontes de áudio e mixagem.

O host escolhe entre: nenhum som, todo o som do sistema, ou apenas alguns
aplicativos. Cada aplicativo é capturado separadamente e mixado aqui em
quadros de 20 ms prontos para o codificador Opus.
"""

from __future__ import annotations

import logging
import os
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass

import numpy as np

from streamazap import config
from streamazap.config import IS_WINDOWS

log = logging.getLogger(__name__)

FRAME_BYTES = config.AUDIO_FRAME_SAMPLES * config.AUDIO_CHANNELS * 2  # s16
BYTES_PER_SECOND = config.AUDIO_RATE * config.AUDIO_CHANNELS * 2
PREBUFFER_FRAMES = 2
MAX_BUFFER_FRAMES = 10

AUDIO_NONE = "none"
AUDIO_SYSTEM = "system"
AUDIO_APPS = "apps"


@dataclass(frozen=True)
class AudioApp:
    name: str  # nome do executável, ex.: chrome.exe
    pids: tuple[int, ...]  # processos raiz (a captura inclui os filhos)
    title: str = ""

    @property
    def label(self) -> str:
        return f"{self.name} — {self.title}" if self.title else self.name


def audio_supported() -> bool:
    if not IS_WINDOWS:
        return False
    import sys

    return sys.getwindowsversion().build >= 19041


def _root_pid(proc) -> int:
    """Sobe na árvore enquanto o pai for o mesmo executável (ex.: abas do Chrome)."""
    import psutil

    try:
        name = proc.name()
        while True:
            parent = proc.parent()
            if parent is None or parent.name() != name:
                return proc.pid
            proc = parent
    except psutil.Error:
        return proc.pid


def list_audio_apps(windows: list | None = None) -> list[AudioApp]:
    """Aplicativos com janela visível, agrupados por executável.

    `windows`: resultado de list_windows() se o chamador já tiver (evita enumerar de novo).
    """
    import psutil

    from streamazap.capture.screen import list_windows

    groups: dict[str, tuple[set[int], str]] = {}
    for window in list_windows() if windows is None else windows:
        if window.pid is None or not window.process_name:
            continue
        try:
            root = _root_pid(psutil.Process(window.pid))
        except psutil.Error:
            continue
        pids, title = groups.setdefault(window.process_name.lower(), (set(), window.title))
        pids.add(root)
    apps = [AudioApp(name, tuple(sorted(pids)), title) for name, (pids, title) in groups.items()]
    return sorted(apps, key=lambda a: a.name)


class _SourceBuffer:
    def __init__(self):
        self.data = bytearray()
        self.primed = False
        self.last_push = 0.0  # instante em que chegou a última amostra do buffer


class AudioMixer:
    """Recebe PCM de várias fontes e entrega quadros mixados de 20 ms.

    Cada quadro sai com o horário (time.monotonic, ms) em que a primeira amostra
    foi capturada, no mesmo relógio do timestamp do vídeo: é isso que permite ao
    espectador tocar som e imagem alinhados.
    """

    def __init__(self, on_frame: Callable[[bytes, int], None], clock: Callable[[], float] = time.monotonic):
        self._on_frame = on_frame
        self._clock = clock
        self._sources: dict[object, _SourceBuffer] = {}
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="audio-mixer", daemon=True)

    def add_source(self, key) -> Callable[[bytes], None]:
        buf = _SourceBuffer()
        with self._lock:
            self._sources[key] = buf

        def push(chunk: bytes) -> None:
            with self._lock:
                buf.data += chunk
                buf.last_push = self._clock()
                excess = len(buf.data) - MAX_BUFFER_FRAMES * FRAME_BYTES
                if excess > 0:
                    del buf.data[: excess - excess % 4]

        return push

    def mix_once(self) -> tuple[bytes, int] | None:
        """Mixa um quadro; retorna (pcm, horário de captura em ms) ou None se nenhuma fonte tiver áudio."""
        chunks = []
        captured = []
        with self._lock:
            for buf in self._sources.values():
                if not buf.primed:
                    if len(buf.data) < PREBUFFER_FRAMES * FRAME_BYTES:
                        continue
                    buf.primed = True
                if len(buf.data) < FRAME_BYTES:
                    buf.primed = False  # underrun: volta a acumular antes de tocar
                    continue
                # A 1ª amostra do buffer chegou "duração do buffer" antes da última.
                captured.append(buf.last_push - len(buf.data) / BYTES_PER_SECOND)
                chunks.append(bytes(buf.data[:FRAME_BYTES]))
                del buf.data[:FRAME_BYTES]
        if not chunks:
            return None
        timestamp_ms = int(min(captured) * 1000)
        if len(chunks) == 1:
            return chunks[0], timestamp_ms
        mixed = np.sum([np.frombuffer(c, dtype=np.int16).astype(np.int32) for c in chunks], axis=0)
        return np.clip(mixed, -32768, 32767).astype(np.int16).tobytes(), timestamp_ms

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def _run(self) -> None:
        interval = config.AUDIO_FRAME_SAMPLES / config.AUDIO_RATE
        next_tick = time.perf_counter()
        while not self._stop.is_set():
            next_tick += interval
            mixed = self.mix_once()
            if mixed is not None:
                self._on_frame(*mixed)
            delay = next_tick - time.perf_counter()
            if delay > 0:
                time.sleep(delay)
            elif delay < -0.2:
                next_tick = time.perf_counter()  # atrasou demais (ex.: PC suspenso)


class AudioCaptureGroup:
    """Inicia as capturas escolhidas e mixa tudo."""

    def __init__(self, mode: str, apps: list[AudioApp], on_frame: Callable[[bytes, int], None]):
        self.mode = mode
        self.apps = apps
        self.mixer = AudioMixer(on_frame)
        self._captures = []

    def start(self) -> None:
        if self.mode == AUDIO_NONE:
            return
        if not audio_supported():
            log.warning("captura de áudio requer Windows 10 2004 ou superior")
            return
        from streamazap.capture.audio_win import ProcessLoopbackCapture

        if self.mode == AUDIO_SYSTEM:
            # Tudo, menos o próprio StreamaZap (evita eco).
            targets = [(os.getpid(), False)]
        else:
            targets = [(pid, True) for app in self.apps for pid in app.pids]
        for pid, include in targets:
            capture = ProcessLoopbackCapture(pid, self.mixer.add_source(pid), include_tree=include)
            capture.start()
            self._captures.append(capture)
        self.mixer.start()

    def errors(self) -> list[str]:
        return [c.error for c in self._captures if c.error]

    def stop(self) -> None:
        self.mixer.stop()
        for capture in self._captures:
            capture.stop()
        self._captures.clear()
