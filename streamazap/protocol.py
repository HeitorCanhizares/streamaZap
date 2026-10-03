"""Protocolo TCP do stream.

Cada mensagem é: tipo (1 byte) + tamanho (4 bytes, big-endian) + payload.
Mensagens de controle levam JSON; vídeo e áudio levam bytes comprimidos.

Handshake:
    host -> SERVER_HELLO {room, locked, nonce, version}
    cliente -> HELLO {name, auth}       (auth = HMAC-SHA256(senha, nonce))
    host -> WELCOME {...} ou REJECT {reason}
"""

from __future__ import annotations

import hashlib
import hmac
import json
import socket
import struct

SERVER_HELLO = 1
HELLO = 2
WELCOME = 3
REJECT = 4
VIDEO = 5  # payload: flags (1 byte, bit0 = keyframe) + timestamp do host (ms, 8 bytes) + H.264 annex-b
AUDIO = 6  # payload: pacote Opus
CHAT = 7  # JSON {name, text}
KEYFRAME_REQUEST = 8
INFO = 9  # JSON {host, you, viewers: [...], streams: [...]}
PING = 10  # JSON {t, rtt} cliente -> host
PONG = 11  # JSON {t} host -> cliente (eco)
SUBSCRIBE = 12  # JSON {media: bool} cliente quer (ou não) receber vídeo/áudio deste host
STREAM = 13  # JSON {port, addresses} participante anuncia que está compartilhando (port 0 = parou)
BYE = 14  # JSON {reason} host encerrou de propósito (não reconectar)

HEADER = struct.Struct("!BI")
MAX_MESSAGE = 32 * 1024 * 1024

VIDEO_FLAG_KEYFRAME = 0x01
VIDEO_HEADER = struct.Struct("!BQ")


class ProtocolError(Exception):
    pass


def encode(msg_type: int, payload: bytes = b"") -> bytes:
    return HEADER.pack(msg_type, len(payload)) + payload


def encode_json(msg_type: int, data: dict) -> bytes:
    return encode(msg_type, json.dumps(data).encode("utf-8"))


def encode_video(packet: bytes, keyframe: bool, timestamp_ms: int) -> bytes:
    flags = VIDEO_FLAG_KEYFRAME if keyframe else 0
    return encode(VIDEO, VIDEO_HEADER.pack(flags, timestamp_ms) + packet)


def decode_video(payload: bytes) -> tuple[bytes, bool, int]:
    """Retorna (pacote H.264, é keyframe, timestamp do host em ms)."""
    if len(payload) < VIDEO_HEADER.size:
        raise ProtocolError("pacote de vídeo truncado")
    flags, timestamp_ms = VIDEO_HEADER.unpack_from(payload)
    return payload[VIDEO_HEADER.size :], bool(flags & VIDEO_FLAG_KEYFRAME), timestamp_ms


def decode_json(payload: bytes) -> dict:
    try:
        data = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ProtocolError(f"JSON inválido: {exc}") from exc
    if not isinstance(data, dict):
        raise ProtocolError("JSON deve ser um objeto")
    return data


def _recv_exact(sock: socket.socket, size: int) -> bytes:
    buf = bytearray()
    while len(buf) < size:
        chunk = sock.recv(min(size - len(buf), 1 << 20))
        if not chunk:
            raise ConnectionError("conexão encerrada")
        buf += chunk
    return bytes(buf)


def recv_message(sock: socket.socket) -> tuple[int, bytes]:
    msg_type, size = HEADER.unpack(_recv_exact(sock, HEADER.size))
    if size > MAX_MESSAGE:
        raise ProtocolError(f"mensagem grande demais ({size} bytes)")
    return msg_type, _recv_exact(sock, size) if size else b""


def auth_token(password: str, nonce: str) -> str:
    return hmac.new(password.encode("utf-8"), nonce.encode("utf-8"), hashlib.sha256).hexdigest()


def check_auth(password: str, nonce: str, token: str) -> bool:
    return hmac.compare_digest(auth_token(password, nonce), token or "")
