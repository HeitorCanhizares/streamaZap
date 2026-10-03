import numpy as np

from streamazap.capture.audio import FRAME_BYTES, AudioMixer
from streamazap.media.audio import AudioDecoder, AudioEncoder
from streamazap.media.video import VideoDecoder, VideoEncoder, output_size


def test_output_size_even_and_scaled():
    assert output_size(1920, 1080, 720) == (1280, 720)
    assert output_size(801, 601, 0) == (800, 600)


def test_video_roundtrip_and_forced_keyframe():
    encoder = VideoEncoder(360, 30, 1_000_000, "libx264")
    decoder = VideoDecoder()
    frame = np.full((480, 640, 4), 128, np.uint8)
    packets = []
    for i in range(6):
        packets += encoder.encode(frame, force_keyframe=(i == 4))
    assert packets[0][1] and packets[4][1] and not packets[1][1]
    decoded = [f for packet, _ in packets for f in decoder.decode(packet)]
    assert decoded[-1].shape == (360, 480, 3)


def test_audio_roundtrip():
    encoder, decoder = AudioEncoder(), AudioDecoder()
    pcm = (np.sin(np.arange(FRAME_BYTES // 2) / 10) * 8000).astype(np.int16).tobytes()
    out = b"".join(decoder.decode(p) for _ in range(4) for p in encoder.encode(pcm))
    assert len(out) == 4 * FRAME_BYTES


def test_mixer_prebuffers_and_mixes():
    mixer = AudioMixer(lambda frame, ts: None)
    a, b = mixer.add_source("a"), mixer.add_source("b")
    one = np.full(FRAME_BYTES // 2, 1000, np.int16).tobytes()
    a(one)
    assert mixer.mix_once() is None  # ainda enchendo o buffer
    a(one)
    b(one * 2)
    mixed = np.frombuffer(mixer.mix_once()[0], np.int16)
    assert (mixed == 2000).all()
    assert (np.frombuffer(mixer.mix_once()[0], np.int16) == 2000).all()
    loud = np.full(FRAME_BYTES // 2, 30000, np.int16).tobytes()
    a(loud)
    b(loud)
    assert (np.frombuffer(mixer.mix_once()[0], np.int16) == 32767).all()


def test_mixer_timestamps_capture_time():
    now = [100.0]
    mixer = AudioMixer(lambda frame, ts: None, clock=lambda: now[0])
    push = mixer.add_source("a")
    push(bytes(FRAME_BYTES * 3))  # 60 ms de áudio chegando em t=100 s
    pcm, ts = mixer.mix_once()
    assert ts == 100_000 - 60  # 1ª amostra foi capturada 60 ms antes de chegar
    pcm, ts = mixer.mix_once()
    assert ts == 100_000 - 40
