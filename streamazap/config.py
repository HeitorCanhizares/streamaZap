"""Constantes de rede e mídia compartilhadas entre host e espectadores."""

import sys

IS_WINDOWS = sys.platform == "win32"

# Porta TCP do stream (vídeo, áudio, chat) e porta UDP da descoberta de salas.
STREAM_PORT = 47800
DISCOVERY_PORT = 47801

# Identificador usado nos pacotes de descoberta para ignorar tráfego de outros apps.
DISCOVERY_MAGIC = "streamazap"
PROTOCOL_VERSION = 1

ANNOUNCE_INTERVAL = 1.5  # segundos entre anúncios do host
ROOM_TIMEOUT = 5.0  # sala some da lista após esse tempo sem anúncio

# Áudio: PCM 16 bits estéreo 48 kHz, quadros de 20 ms (tamanho exigido pelo Opus).
AUDIO_RATE = 48000
AUDIO_CHANNELS = 2
AUDIO_FRAME_SAMPLES = 960
AUDIO_BITRATE = 128_000

# Presets de qualidade exibidos na interface: (rótulo, altura máxima, fps, bitrate).
QUALITY_PRESETS = [
    ("720p 30fps (leve, ideal p/ Radmin)", 720, 30, 2_500_000),
    ("720p 60fps", 720, 60, 4_000_000),
    ("1080p 30fps", 1080, 30, 5_000_000),
    ("1080p 60fps", 1080, 60, 8_000_000),
    ("Original 30fps", 0, 30, 8_000_000),
]
