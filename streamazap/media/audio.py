"""Codificação/decodificação de áudio Opus (48 kHz estéreo, quadros de 20 ms)."""

from __future__ import annotations

import av
import numpy as np

from streamazap import config


class AudioEncoder:
    def __init__(self, bitrate: int = config.AUDIO_BITRATE):
        self._ctx = av.CodecContext.create("libopus", "w")
        self._ctx.sample_rate = config.AUDIO_RATE
        self._ctx.layout = "stereo"
        self._ctx.format = "s16"
        self._ctx.bit_rate = bitrate
        self._ctx.options = {"application": "audio", "frame_duration": "20"}
        self._ctx.open()
        self._pts = 0

    def encode(self, pcm: bytes) -> list[bytes]:
        samples = np.frombuffer(pcm, dtype=np.int16).reshape(1, -1)
        frame = av.AudioFrame.from_ndarray(samples, format="s16", layout="stereo")
        frame.sample_rate = config.AUDIO_RATE
        frame.pts = self._pts
        self._pts += samples.shape[1] // config.AUDIO_CHANNELS
        return [bytes(p) for p in self._ctx.encode(frame)]


class AudioDecoder:
    def __init__(self):
        self._ctx = av.CodecContext.create("libopus", "r")
        self._ctx.sample_rate = config.AUDIO_RATE
        self._ctx.layout = "stereo"
        self._resampler = av.AudioResampler(format="s16", layout="stereo", rate=config.AUDIO_RATE)

    def decode(self, data: bytes) -> bytes:
        """Retorna PCM s16 intercalado."""
        out = bytearray()
        for frame in self._ctx.decode(av.Packet(data)):
            for converted in self._resampler.resample(frame):
                out += converted.to_ndarray().tobytes()
        return bytes(out)
