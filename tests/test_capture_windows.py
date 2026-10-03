"""Captura real de monitor no Windows (roda no CI do Windows): DXGI x BitBlt."""

import ctypes
import sys
import time

import numpy as np
import pytest

pytestmark = pytest.mark.skipif(sys.platform != "win32", reason="só no Windows")


@pytest.fixture
def monitor():
    from streamazap.capture.screen import list_monitors

    return list_monitors()[0]


@pytest.fixture
def dxgi(monitor):
    from streamazap.capture.screen import _win

    try:
        capturer = _win.DxgiMonitorCapturer(*monitor.rect)
    except OSError as exc:  # ex.: sessão sem GPU/duplicação; o app cai para o BitBlt
        pytest.skip(f"Desktop Duplication indisponível aqui: {exc}")
    yield capturer
    capturer.close()


def test_dxgi_matches_bitblt(monitor, dxgi):
    from streamazap.capture.screen import _win

    frames = []
    for _ in range(40):
        frames.append(dxgi.grab())
        if dxgi._staged:  # a imagem veio mesmo pela GPU (e não pelo BitBlt de reserva)
            break
        time.sleep(0.05)
    assert dxgi._staged, "nenhuma imagem chegou pela Desktop Duplication"
    left, top, width, height = monitor.rect
    ctypes.windll.user32.SetCursorPos(left + width // 3, top + height // 3)  # só o mouse muda
    time.sleep(0.1)
    frames += [dxgi.grab() for _ in range(3)]
    gdi = _win.MonitorCapturer(*monitor.rect)
    try:
        reference = gdi.grab()
    finally:
        gdi.close()
    for frame in frames:
        assert frame.shape == (height, width, 4) and frame.dtype == np.uint8
    # Mesma imagem pelos dois caminhos (tirando o cursor e o que mudou entre as capturas).
    diff = np.abs(frames[-1][:, :, :3].astype(np.int16) - reference[:, :, :3].astype(np.int16))
    assert (diff > 8).mean() < 0.02, f"DXGI ({dxgi.backend}) difere do BitBlt em {(diff > 8).mean():.1%} dos pixels"


def test_dxgi_falls_back_to_bitblt_on_error(monitor, dxgi):
    def broken():
        raise OSError("falha simulada")

    dxgi._grab_dxgi = broken
    frame = dxgi.grab()
    assert frame.shape[:2] == (monitor.rect[3], monitor.rect[2])
    assert dxgi.backend == "BitBlt"
    assert dxgi.grab() is not None  # segue capturando pelo BitBlt


def test_create_capturer_always_works(monitor):
    from streamazap.capture.screen import create_capturer

    capturer = create_capturer(monitor)
    try:
        assert capturer.grab().shape[:2] == (monitor.rect[3], monitor.rect[2])
        assert capturer.backend in ("DXGI", "BitBlt")
    finally:
        capturer.close()
