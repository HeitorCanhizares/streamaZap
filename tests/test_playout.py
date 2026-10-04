import itertools
import threading

from streamazap.host import BitrateController
from streamazap.playout import MediaClock, SlidingMin, VideoPlayout


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


def test_clock_absorbs_network_jitter():
    """Quadros capturados a cada 33 ms chegam com atraso variável, mas são agendados em ritmo constante."""
    clock = FakeClock()
    media = MediaClock(200, clock=clock)
    jitter = [0.0, 0.12, 0.01, 0.15, 0.02, 0.0, 0.09]
    for i, extra in enumerate(jitter):
        clock.now = 1000.0 + i * 0.033 + 0.050 + extra  # 50 ms de trânsito + jitter
        media.observe(5000 + i * 33)  # relógio do host em ms (offset arbitrário)
    dues = [media.play_time((5000 + i * 33) / 1000) for i in range(len(jitter))]
    gaps = [round(b - a, 3) for a, b in itertools.pairwise(dues)]
    assert gaps == [0.033] * (len(jitter) - 1)
    # Cada quadro é exibido 200 ms depois do trânsito mínimo, logo nunca antes de chegar.
    assert abs(dues[0] - (1000.0 + 0.050 + 0.200)) < 1e-6
    media.set_delay(500)  # suavização mudou: tudo (vídeo e áudio) desloca junto
    assert abs(media.play_time(5.0) - (dues[0] + 0.3)) < 1e-6


def test_playout_shows_frames_in_order():
    shown = []
    done = threading.Event()

    def on_frame(frame):
        shown.append(frame)
        if frame == b"c":
            done.set()

    media = MediaClock(0)
    playout = VideoPlayout(lambda p: [p], on_frame, lambda: None, media)
    playout.start()
    for i, packet in enumerate([b"a", b"b", b"c"]):
        media.observe(1000 + i * 10)
        playout.push(packet, 1000 + i * 10)
    assert done.wait(2)
    playout.stop()
    # Pacotes que vencem juntos são todos decodificados, mas só o mais novo é exibido.
    assert shown[-1] == b"c"
    assert playout.frames_shown + playout.frames_skipped == 3


def _full_speed(c, now=0):
    """Passa do início suave (70% subindo a cada 2 s) até o bitrate alvo."""
    while c.current < c.base:
        c.update(now, lag=0.0, dropped=False)
        now += 2
    return now


def test_bitrate_controller_slow_start():
    c = BitrateController(4_000_000)
    assert c.current == 2_800_000  # começa em 70%
    assert c.update(0, lag=0.0, dropped=False) == 3_220_000
    assert c.update(1, lag=0.0, dropped=False) is None  # sobe a cada 2 s no início
    assert c.update(2, lag=0.0, dropped=False) == 3_703_000
    assert c.update(4, lag=0.0, dropped=False) == 4_000_000
    assert BitrateController(4_000_000, enabled=False).current == 4_000_000


def test_bitrate_controller_drops_to_measured_capacity():
    c = BitrateController(4_000_000)
    t = _full_speed(c)
    # A conexão mais lenta escoou 3 Mbps de vídeo: cai para 85% disso (não um corte cego de 30%).
    assert c.update(t, lag=0.6, dropped=False, throughput=3_000_000) == 2_550_000
    # A medida de atraso ainda cresce por 1-2 s (vem do ping): não cai de novo à toa.
    assert c.update(t + 2, lag=0.8, dropped=False, throughput=2_900_000) is None
    assert c.update(t + 4, lag=0.9, dropped=False) is None
    # Mas se a fila estourar (vídeo descartado), cai mesmo assim.
    assert c.update(t + 5, lag=0.9, dropped=True) == 1_785_000


def test_bitrate_controller_remembers_ceiling():
    """Depois de engasgar, volta rápido até perto do limite e passa dele só devagar."""
    c = BitrateController(4_000_000)
    t = _full_speed(c)
    # Uma queda corta no máximo pela metade (a medida pode ter pego só um engasgo).
    assert c.update(t, lag=0.6, dropped=False, throughput=2_000_000) == 2_000_000
    assert c.update(t + 6, lag=0.0, dropped=False) is None  # acima de 85% do teto: só sonda a cada 20 s
    c = BitrateController(4_000_000)
    t = _full_speed(c)
    c.update(t, lag=0.6, dropped=False, throughput=3_000_000)  # -> 2,55 Mbps, teto 3 Mbps
    assert c.current == 2_550_000
    assert c.update(t + 6, lag=0.0, dropped=False) is None  # já está a 85% do teto
    assert c.update(t + 20, lag=0.0, dropped=False) == 2_678_000  # sonda +5% a cada 20 s
    assert c.update(t + 26, lag=0.0, dropped=False) is None
    for step in range(30, 200, 5):  # passado 1 min, esquece o teto e volta ao alvo
        c.update(t + step, lag=0.0, dropped=False)
    assert c.current == 4_000_000


def test_bitrate_controller_without_measurement():
    c = BitrateController(4_000_000)
    t = _full_speed(c)
    assert c.update(t, lag=0.6, dropped=False) == 2_800_000  # sem medida: corta 30%
    assert c.update(t + 1, lag=0.7, dropped=False) is None  # espera 2 s entre quedas
    assert c.update(t + 2, lag=0.0, dropped=True) == 1_960_000
    for step in range(200, 260, 3):
        c.update(t + step, lag=1.0, dropped=True)
    assert c.current == c.minimum
    assert BitrateController(4_000_000, enabled=False).update(0, 5.0, True) is None


def test_bitrate_controller_waits_while_queue_drains():
    """Depois de uma queda, se o atraso já está diminuindo, não cai de novo."""
    c = BitrateController(4_000_000)
    t = _full_speed(c)
    assert c.update(t, lag=2.0, dropped=False) == 2_800_000
    assert c.update(t + 3, lag=1.2, dropped=False) is None  # esvaziando: espera
    assert c.update(t + 6, lag=0.7, dropped=False) is None
    assert c.update(t + 9, lag=0.7, dropped=False) == 1_960_000  # parou de melhorar: cai


def test_audio_follows_same_clock_as_video():
    """O áudio capturado no instante T toca no mesmo instante em que o vídeo de T é exibido."""
    import numpy as np

    from streamazap.player import BYTES_PER_SECOND, AudioPlayer

    clock = FakeClock()
    media = MediaClock(200, clock=clock)
    player = AudioPlayer(media)
    # 1 s de áudio capturado a partir de host_ts=10.000 s: cada bloco de 20 ms com valor = índice.
    for i in range(50):
        clock.now = 1000.0 + i * 0.02 + 0.08  # 80 ms de trânsito
        ts = 10_000 + i * 20
        media.observe(ts)
        player.push(np.full(960 * 2, i, np.int16).tobytes(), ts)
    video_time = media.play_time(10.400)  # quando o quadro capturado em 10,4 s aparece
    out = np.frombuffer(player.render(4 * 960, play_start=video_time), np.int16)
    assert (out == 20).mean() > 0.9  # bloco 20 = capturado em 10,400 s

    # Placa de som pedindo áudio "cedo demais" recebe silêncio até a hora certa.
    player2 = AudioPlayer(media)
    player2.push(np.full(960 * 2, 7, np.int16).tobytes(), 10_000)
    start = media.play_time(10.0) - 0.1
    out = np.frombuffer(player2.render(int(0.2 * BYTES_PER_SECOND), play_start=start), np.int16)
    half = len(out) // 2
    assert (out[: half - 400] == 0).all() and (out[half + 400 :] == 7).any()


# -- suavização automática --------------------------------------------------------------


def _feed(clock, media, start, seconds, transit, kind="video", step=0.02, jitter=lambda t: 0.0):
    """Pacotes capturados a cada `step` s (relógio do host = local) chegando com `transit` + jitter."""
    t = start
    while t < start + seconds:
        clock.now = t + transit + jitter(t)
        media.observe(int(t * 1000), kind)
        t += step
    return t


def test_auto_delay_jumps_when_audio_is_late():
    clock = FakeClock()
    media = MediaClock(200, clock=clock, auto=True)
    media.audio_lead = media.video_lead = 0.0
    t = _feed(clock, media, 1000.0, 2, 0.05, "audio")
    assert abs(media.current_delay() - 0.2) < 1e-6
    # Um pacote de áudio chega 400 ms atrasado: o áudio já está em silêncio, então sobe de uma vez.
    clock.now = t + 0.05 + 0.4
    media.observe(int(t * 1000), "audio")
    assert media.current_delay() >= 0.4 + media.AUTO_MARGIN - 1e-6
    assert media.late_events == 1


def test_auto_delay_video_late_steps_without_cutting_audio():
    clock = FakeClock()
    media = MediaClock(200, clock=clock, auto=True)
    media.audio_lead = media.video_lead = 0.0
    t = _feed(clock, media, 1000.0, 2, 0.05, "audio")
    before = media.current_delay()
    clock.now = t + 0.05 + 0.35  # só o vídeo atrasou 350 ms além do mínimo (150 ms além da suavização)
    media.observe(int(t * 1000), "video")
    # Sobe 100 ms na hora (o áudio estica sem cortar) e o resto aos poucos, a 2%/s.
    assert abs(media.current_delay() - (before + MediaClock.AUTO_STEP)) < 1e-6
    _feed(clock, media, t, 1.0, 0.05, "audio")
    assert before + MediaClock.AUTO_STEP < media.current_delay() <= 0.35 + MediaClock.AUTO_MARGIN + 1e-6


def test_auto_delay_comes_down_slowly_with_sound_and_fast_in_silence():
    for level, rate in [(0.3, MediaClock.AUTO_DOWN_RATE), (0.0, MediaClock.AUTO_FAST_DOWN_RATE)]:
        clock = FakeClock()
        media = MediaClock(800, clock=clock, auto=True)  # começou alto (ex.: depois de um engasgo)
        media.audio_lead = media.video_lead = 0.0
        media.audio_level = level
        t = _feed(clock, media, 1000.0, MediaClock.AUTO_HOLD + 1, 0.05, "audio")
        start = media.current_delay()
        _feed(clock, media, t, 4, 0.05, "audio")
        assert abs((start - media.current_delay()) - rate * 4) < 0.01
    # E nunca abaixo do pior atraso recente + margem.
    clock = FakeClock()
    media = MediaClock(800, clock=clock, auto=True)
    media.audio_lead = media.video_lead = 0.0
    media.audio_level = 0.0
    _feed(clock, media, 1000.0, 30, 0.05, "audio", jitter=lambda t: 0.12 if int(t * 50) % 100 == 0 else 0.0)
    assert abs(media.current_delay() - (0.12 + MediaClock.AUTO_MARGIN)) < 0.02


def test_manual_delay_ignores_lateness():
    clock = FakeClock()
    media = MediaClock(200, clock=clock)
    t = _feed(clock, media, 1000.0, 1, 0.05, "audio")
    clock.now = t + 0.05 + 0.5
    media.observe(int(t * 1000), "audio")
    assert media.current_delay() == 0.2


# -- áudio contínuo -----------------------------------------------------------------------


def _player_with_tone(clock, media, seconds, start_ts=10_000):
    import numpy as np

    from streamazap.player import AudioPlayer

    player = AudioPlayer(media)
    for i in range(int(seconds * 50)):
        clock.now = 1000.0 + i * 0.02 + 0.05
        ts = start_ts + i * 20
        media.observe(ts, "audio")
        tone = (np.sin(np.arange(i * 960, (i + 1) * 960) * 2 * np.pi * 440 / 48000) * 8000).astype(np.int16)
        player.push(np.repeat(tone, 2).tobytes(), ts)  # 440 Hz contínuo nos dois canais
    return player


def test_audio_follows_moving_clock_without_gaps_or_jumps():
    """A suavização mudando aos poucos não gera silêncio nem pulo: o áudio estica/encolhe."""
    import numpy as np

    clock = FakeClock()
    media = MediaClock(200, clock=clock)
    player = _player_with_tone(clock, media, 3)
    block = 480
    play = media.play_time(10.0)
    out = []
    for i in range(200):  # 2 s em blocos de 10 ms
        media.set_delay(200 + int(i * 0.15))  # +15 ms por segundo
        out.append(np.frombuffer(player.render(block * 4, play + i * block / 48000), np.int16).reshape(-1, 2))
    pcm = np.concatenate(out)[:, 0].astype(np.int32)
    assert (np.abs(pcm[block:]) > 0).mean() > 0.97  # sem buracos de silêncio
    assert np.abs(np.diff(pcm)).max() < 900  # sem saltos: um seno de 440 Hz anda no máximo ~460 por amostra
    assert player.skipped_frames == 0


def test_audio_underrun_fades_out_instead_of_clicking():
    import numpy as np

    clock = FakeClock()
    media = MediaClock(200, clock=clock)
    player = _player_with_tone(clock, media, 0.1)
    play = media.play_time(10.0)
    first = np.frombuffer(player.render(6000 * 4, play), np.int16).reshape(-1, 2)[:, 0]  # tem 4800: pede mais
    audio_end = np.flatnonzero(first)[-1]
    assert abs(int(first[audio_end])) < 300  # terminou descendo até ~0 (fade), não cortado no meio da onda
