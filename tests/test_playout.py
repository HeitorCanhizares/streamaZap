import itertools
import threading

from streamazap.host import BitrateController
from streamazap.playout import SlidingMin, VideoPlayout


class FakeClock:
    def __init__(self):
        self.now = 1000.0

    def __call__(self):
        return self.now


def test_sliding_min():
    m = SlidingMin(10)
    assert m.add(0, 5) == 5
    assert m.add(1, 3) == 3
    assert m.add(2, 4) == 3
    assert m.add(12, 6) == 4  # o 3 (t=1) saiu da janela
    assert m.add(13, 7) == 6


def test_playout_absorbs_network_jitter():
    """Quadros capturados a cada 33 ms chegam com atraso variável, mas são agendados em ritmo constante."""
    clock = FakeClock()
    playout = VideoPlayout(lambda p: [], lambda f: None, lambda: None, delay_ms=200, clock=clock)
    jitter = [0.0, 0.12, 0.01, 0.15, 0.02, 0.0, 0.09]
    dues = []
    for i, extra in enumerate(jitter):
        host_ts = 5000 + i * 33  # relógio do host em ms (offset arbitrário)
        clock.now = 1000.0 + i * 0.033 + 0.050 + extra  # 50 ms de trânsito + jitter
        dues.append(playout.due_time(host_ts))
    gaps = [round(b - a, 3) for a, b in itertools.pairwise(dues)]
    assert gaps == [0.033] * (len(jitter) - 1)
    # Cada quadro é exibido 200 ms depois do trânsito mínimo, logo nunca antes de chegar.
    assert abs(dues[0] - (1000.0 + 0.050 + 0.200)) < 1e-6


def test_playout_shows_frames_in_order():
    shown = []
    done = threading.Event()

    def on_frame(frame):
        shown.append(frame)
        if frame == b"c":
            done.set()

    playout = VideoPlayout(lambda p: [p], on_frame, lambda: None, delay_ms=0)
    playout.start()
    for i, packet in enumerate([b"a", b"b", b"c"]):
        playout.push(packet, 1000 + i * 10)
    assert done.wait(2)
    playout.stop()
    # Pacotes que vencem juntos são todos decodificados, mas só o mais novo é exibido.
    assert shown[-1] == b"c"
    assert playout.frames_shown + playout.frames_skipped == 3


def test_bitrate_controller_drops_fast_and_recovers_slowly():
    c = BitrateController(4_000_000)
    assert c.update(0, lag=0.6, dropped=False) == 2_800_000
    assert c.update(1, lag=0.7, dropped=False) is None  # espera 2 s entre quedas
    assert c.update(2, lag=0.0, dropped=True) == 1_960_000
    assert c.update(4, lag=0.0, dropped=False) is None  # subida só depois de 5 s estável
    assert c.update(7, lag=0.0, dropped=False) == 2_254_000
    for t in range(10, 200, 5):
        c.update(t, lag=0.0, dropped=False)
    assert c.current == 4_000_000
    for t in range(200, 260, 3):
        c.update(t, lag=1.0, dropped=True)
    assert c.current == c.minimum
    assert BitrateController(4_000_000, enabled=False).update(0, 5.0, True) is None


def test_bitrate_controller_waits_while_queue_drains():
    """Depois de uma queda, se o atraso já está diminuindo, não cai de novo."""
    c = BitrateController(4_000_000)
    assert c.update(0, lag=2.0, dropped=False) == 2_800_000
    assert c.update(3, lag=1.2, dropped=False) is None  # esvaziando: espera
    assert c.update(6, lag=0.7, dropped=False) is None
    assert c.update(9, lag=0.7, dropped=False) == 1_960_000  # parou de melhorar: cai
