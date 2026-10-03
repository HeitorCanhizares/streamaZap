import socket

import pytest

from streamazap import protocol


def test_roundtrip_over_socketpair():
    a, b = socket.socketpair()
    a.sendall(protocol.encode_json(protocol.CHAT, {"text": "olá"}))
    a.sendall(protocol.encode_video(b"\x00\x01", keyframe=True))
    msg_type, payload = protocol.recv_message(b)
    assert msg_type == protocol.CHAT
    assert protocol.decode_json(payload) == {"text": "olá"}
    msg_type, payload = protocol.recv_message(b)
    assert msg_type == protocol.VIDEO
    assert protocol.decode_video(payload) == (b"\x00\x01", True)
    a.close()
    with pytest.raises(ConnectionError):
        protocol.recv_message(b)


def test_auth():
    token = protocol.auth_token("segredo", "abc")
    assert protocol.check_auth("segredo", "abc", token)
    assert not protocol.check_auth("errado", "abc", token)
    assert not protocol.check_auth("segredo", "abc", "")


def test_rejects_huge_message():
    a, b = socket.socketpair()
    a.sendall(protocol.HEADER.pack(protocol.VIDEO, protocol.MAX_MESSAGE + 1))
    with pytest.raises(protocol.ProtocolError):
        protocol.recv_message(b)
