import socket
import time

import numpy as np
import pytest

from streamazap import protocol
from streamazap.capture.screen import VideoSource
from streamazap.client import JoinError, StreamViewer
from streamazap.host import HostSettings, StreamHost


class FakeCapturer:
    def __init__(self, source):
        self.n = 0
        self.size = (240, 320) if source.ident == 1 else (180, 160)

    def grab(self):
        self.n += 1
        frame = np.zeros((*self.size, 4), np.uint8)
        frame[:, : (self.n * 7) % self.size[1], 1] = 255
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


def make_host(port, password="123", announce=False, source_id=1):
    settings = HostSettings(
        room_name="Teste", host_name="Host", password=password,
        source=VideoSource("monitor", source_id, "fake"), max_height=0, fps=30,
        bitrate=500_000, encoder="libx264", port=port, announce=announce,
    )
    chats = []
    h = StreamHost(settings, on_chat=lambda n, t: chats.append((n, t)), capturer_factory=FakeCapturer)
    h.chats = chats
    h.start()
    return h


@pytest.fixture
def host():
    h = make_host(48900)
    yield h
    h.stop()


def viewer_for(host, name="Amigo", password="123", **kwargs):
    return StreamViewer(["127.0.0.1"], host.port, name, password, delay_ms=50, **kwargs)


def test_viewer_receives_video_and_chat(host):
    frames, chats = [], []
    viewer = viewer_for(host, on_frame=frames.append, on_chat=lambda n, t: chats.append((n, t)))
    viewer.connect()
    viewer.start()
    try:
        assert wait_for(lambda: len(frames) >= 5)
        assert frames[-1].shape == (240, 320, 4)
        assert host.viewer_names() == ["Amigo"]
        viewer.send_chat("oi")
        assert wait_for(lambda: ("Amigo", "oi") in chats)
        assert ("Amigo", "oi") in host.chats
        assert wait_for(lambda: viewer.rtt_ms is not None, timeout=6)
    finally:
        viewer.stop()
    assert wait_for(lambda: host.viewer_names() == [])


def test_wrong_password_rejected(host):
    with pytest.raises(JoinError, match="Senha") as info:
        viewer_for(host, password="errada").connect()
    assert info.value.fatal


def test_edit_stream_live_keeps_viewer_connected(host):
    frames = []
    viewer = viewer_for(host, on_frame=frames.append)
    viewer.connect()
    viewer.start()
    try:
        assert wait_for(lambda: frames and frames[-1].shape == (240, 320, 4))
        new = HostSettings(**{**host.settings.__dict__, "source": VideoSource("window", 2, "outra"), "room_name": "Nova"})
        host.apply_settings(new)
        assert wait_for(lambda: frames[-1].shape == (180, 160, 4))
        assert host.settings.room_name == "Nova"
        assert host.viewer_names() == ["Amigo"]
    finally:
        viewer.stop()


def test_participant_stream_listed_and_watchable(host):
    infos = []
    participant = viewer_for(host, name="Ana", on_info=infos.append)
    participant.connect()
    participant.start()
    share = make_host(48920, password="123", source_id=2)  # servidor do participante
    try:
        participant.announce_stream(share.port, ["127.0.0.1"])
        assert wait_for(lambda: infos and any(s["name"] == "Ana" for s in infos[-1]["streams"]))
        stream = next(s for s in infos[-1]["streams"] if s["name"] == "Ana")
        assert stream["addresses"][0] == "127.0.0.1" and stream["port"] == share.port

        # Outro espectador assiste direto no servidor da Ana e deixa de receber mídia do host.
        frames = []
        room = viewer_for(host, name="Bia")
        room.connect()
        room.start()
        room.set_media(False)
        watch = StreamViewer(stream["addresses"], stream["port"], "Bia", "123", delay_ms=0, media=True, on_frame=frames.append)
        watch.connect()
        watch.start()
        assert wait_for(lambda: frames and frames[-1].shape == (180, 160, 4))
        assert wait_for(lambda: not next(v for v in host._authed() if v.name == "Bia").media)
        watch.stop()
        room.stop()

        participant.stop_stream()
        assert wait_for(lambda: not any(s["name"] == "Ana" for s in infos[-1]["streams"]))
    finally:
        share.stop()
        participant.stop()


def test_host_stop_sends_bye_without_reconnect():
    host = make_host(48940)
    closed, statuses = [], []
    viewer = viewer_for(host, on_closed=closed.append, on_status=statuses.append)
    viewer.connect()
    viewer.start()
    assert wait_for(lambda: host.viewer_names() == ["Amigo"])
    host.stop("Fim da live")
    assert wait_for(lambda: closed == ["Fim da live"])
    assert not any("reconectando" in s for s in statuses)
    viewer.stop()


def test_viewer_reconnects_after_connection_drop(host, monkeypatch):
    from streamazap import config

    monkeypatch.setattr(config, "RECONNECT_ATTEMPTS", 3)
    frames, statuses = [], []
    viewer = viewer_for(host, on_frame=frames.append, on_status=statuses.append)
    viewer.connect()
    viewer.start()
    try:
        assert wait_for(lambda: len(frames) > 3)
        # Derruba a conexão "pela rede" (sem BYE).
        viewer._sock.shutdown(socket.SHUT_RDWR)
        assert wait_for(lambda: any("reconectando" in s for s in statuses))
        assert wait_for(lambda: statuses[-1] == "", timeout=15)
        count = len(frames)
        assert wait_for(lambda: len(frames) > count + 3)
    finally:
        viewer.stop()


def test_slow_viewer_queue_is_trimmed_by_age(monkeypatch):
    """A fila de quem não consegue receber não cresce sem limite: mídia velha é descartada."""
    from streamazap import config
    from streamazap.host import _Viewer

    class FakeHost:
        congestion = 0

        def report_congestion(self):
            self.congestion += 1

    monkeypatch.setattr(config, "MAX_VIEWER_LAG", 0.05)
    a, b = socket.socketpair()
    fake = FakeHost()
    viewer = _Viewer(fake, a, "x")  # sender_loop não roda: simula rede travada
    viewer.send_video(protocol.encode_video(b"k" * 100, True, 0), keyframe=True)
    viewer.send_video(protocol.encode_video(b"p" * 100, False, 1), keyframe=False)
    viewer.send(protocol.CHAT, protocol.encode_json(protocol.CHAT, {"text": "oi"}))
    time.sleep(0.1)
    viewer.send_video(protocol.encode_video(b"p" * 100, False, 2), keyframe=False)
    assert fake.congestion == 1
    assert [item[0] for item in viewer._queue] == [protocol.CHAT]  # chat preservado, vídeo velho fora
    assert viewer.waiting_keyframe
    viewer.send_video(protocol.encode_video(b"p", False, 3), keyframe=False)
    assert len(viewer._queue) == 1  # P-frame ignorado até o próximo keyframe
    viewer.send_video(protocol.encode_video(b"k", True, 4), keyframe=True)
    assert [item[0] for item in viewer._queue] == [protocol.CHAT, protocol.VIDEO]
    a.close()
    b.close()


def test_old_client_gets_clear_update_message(host):
    """Cliente de versão antiga (sem 'version' no HELLO) recebe REJECT com instrução de atualizar."""
    sock = socket.create_connection(("127.0.0.1", host.port))
    msg_type, payload = protocol.recv_message(sock)
    nonce = protocol.decode_json(payload)["nonce"]
    sock.sendall(protocol.encode_json(protocol.HELLO, {"name": "Velho", "auth": protocol.auth_token("123", nonce)}))
    msg_type, payload = protocol.recv_message(sock)
    assert msg_type == protocol.REJECT
    assert "Atualizem" in protocol.decode_json(payload)["reason"]
    sock.close()


def test_reverse_connection_when_host_blocks_incoming(host):
    """Se a conexão direta falha (firewall do host), o host conecta de volta no espectador."""
    frames = []
    blocked_port = 1  # nada escuta aqui: simula entrada bloqueada no host
    viewer = StreamViewer(
        ["127.0.0.1"], blocked_port, "Longe", "123", delay_ms=0, on_frame=frames.append,
        callback_request=lambda port: host.connect_back("127.0.0.1", port),
    )
    viewer.connect()
    viewer.start()
    try:
        assert viewer.used_callback
        assert wait_for(lambda: len(frames) > 3)
        assert host.viewer_names() == ["Longe"]
    finally:
        viewer.stop()


def test_reverse_connection_requested_over_discovery_udp():
    """O pedido de conexão reversa vai por UDP para o socket que anuncia a sala."""
    from streamazap.discovery import Announcer, RoomBrowser, request_callback

    port = 48777
    requests = []
    browser = RoomBrowser(port=port)
    browser.start()
    info = {"name": "S", "host_name": "H", "port": 47800, "viewers": 0, "locked": False}
    announcer = Announcer(lambda: info, port=port, on_callback=lambda ip, p: requests.append((ip, p)))
    announcer.start()
    try:
        assert wait_for(lambda: browser.rooms() and browser.rooms()[0].callback_ports)
        room = browser.rooms()[0]
        request_callback(room, 40123, attempts=3)
        assert wait_for(lambda: requests)
        assert requests[0][1] == 40123 and requests[0][0] in room.addresses
        time.sleep(0.5)
        assert len(requests) == 1  # repetições do mesmo pedido são ignoradas
    finally:
        announcer.stop()
        browser.stop()


def test_reverse_connection_to_participant_via_room(host):
    """Bia não alcança o compartilhamento da Ana: o host repassa o pedido e a Ana conecta na Bia."""
    share = make_host(48960, password="123", source_id=2)
    ana = viewer_for(host, name="Ana", on_callback=share.connect_back)
    ana.connect()
    ana.start()
    infos = []
    bia_room = viewer_for(host, name="Bia", on_info=infos.append)
    bia_room.connect()
    bia_room.start()
    try:
        ana.announce_stream(share.port, ["127.0.0.1"])
        assert wait_for(lambda: infos and any(s["name"] == "Ana" for s in infos[-1]["streams"]))
        stream = next(s for s in infos[-1]["streams"] if s["name"] == "Ana")
        frames = []
        watch = StreamViewer(
            stream["addresses"], 1, "Bia", "123", delay_ms=0, on_frame=frames.append,  # porta 1: direto falha
            callback_request=lambda p: bia_room.request_stream_callback(stream["id"], p),
        )
        watch.connect()
        watch.start()
        assert watch.used_callback
        assert wait_for(lambda: frames and frames[-1].shape == (180, 160, 4))
        watch.stop()
    finally:
        bia_room.stop()
        ana.stop()
        share.stop()
