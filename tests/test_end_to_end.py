import time

import numpy as np
import pytest

from streamazap.capture.screen import VideoSource
from streamazap.client import JoinError, StreamViewer
from streamazap.host import HostSettings, StreamHost


class FakeCapturer:
    def __init__(self, source):
        self.n = 0

    def grab(self):
        self.n += 1
        frame = np.zeros((240, 320, 4), np.uint8)
        frame[:, : (self.n * 7) % 320, 1] = 255
        return frame

    def close(self):
        pass


def wait_for(predicate, timeout=10.0):
    end = time.time() + timeout
    while time.time() < end:
        if predicate():
            return True
        time.sleep(0.05)
    return False


@pytest.fixture
def host():
    settings = HostSettings(
        room_name="Teste", host_name="Host", password="123",
        source=VideoSource("monitor", 1, "fake"), max_height=0, fps=30,
        bitrate=500_000, encoder="libx264", port=48900,
    )
    chats = []
    h = StreamHost(settings, on_chat=lambda n, t: chats.append((n, t)), capturer_factory=FakeCapturer)
    h.chats = chats
    h.start()
    yield h
    h.stop()


def test_viewer_receives_video_and_chat(host):
    frames, chats = [], []
    viewer = StreamViewer(["127.0.0.1"], host.port, "Amigo", "123",
                          on_frame=frames.append, on_chat=lambda n, t: chats.append((n, t)))
    viewer.connect()
    viewer.start()
    try:
        assert wait_for(lambda: len(frames) >= 5)
        assert frames[-1].shape == (240, 320, 3)
        assert host.viewer_names() == ["Amigo"]
        viewer.send_chat("oi")
        assert wait_for(lambda: ("Amigo", "oi") in chats)
        assert ("Amigo", "oi") in host.chats
    finally:
        viewer.stop()
    assert wait_for(lambda: host.viewer_names() == [])


def test_wrong_password_rejected(host):
    viewer = StreamViewer(["127.0.0.1"], host.port, "Intruso", "errada")
    with pytest.raises(JoinError, match="Senha"):
        viewer.connect()
