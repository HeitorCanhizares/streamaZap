"""Constantes de rede e mídia compartilhadas entre host e espectadores."""

import sys

IS_WINDOWS = sys.platform == "win32"

# Porta TCP do stream (vídeo, áudio, chat) e porta UDP da descoberta de salas.
STREAM_PORT = 47800
DISCOVERY_PORT = 47801

# Identificador usado nos pacotes de descoberta para ignorar tráfego de outros apps.
DISCOVERY_MAGIC = "streamazap"
PROTOCOL_VERSION = 2

# Conexões pela internet (Radmin entre países) podem ter latência alta e perda
# de pacotes: timeouts generosos e reconexão automática.
CONNECT_TIMEOUT = 20.0  # por endereço
HANDSHAKE_TIMEOUT = 20.0
PING_INTERVAL = 1.0
STALL_TIMEOUT = 15.0  # sem receber nada por esse tempo = conexão caiu
VIEWER_IDLE_TIMEOUT = 30.0  # host derruba espectador que não manda ping
RECONNECT_ATTEMPTS = 6

# Suavização: atraso de reprodução padrão (ms) usado para absorver a variação da rede.
DEFAULT_PLAYOUT_DELAY_MS = 200
MAX_PLAYOUT_DELAY_MS = 1500

# Fila do host por espectador: acima desse atraso o vídeo pendente é descartado.
MAX_VIEWER_LAG = 1.0
# Buffer de envio do sistema limitado: o excesso fica na fila do app, onde dá para
# medir e descartar, em vez de virar segundos de atraso escondidos no Windows/VPN.
SEND_BUFFER_BYTES = 512 * 1024

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
