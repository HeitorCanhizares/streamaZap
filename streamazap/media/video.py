"""Codificação/decodificação H.264 com PyAV (FFmpeg).

O codificador tenta primeiro a GPU (NVIDIA NVENC, Intel QuickSync, AMD AMF)
e cai para o libx264 (CPU) se nenhuma estiver disponível.
"""

from __future__ import annotations

import logging
from fractions import Fraction

import av
import numpy as np

log = logging.getLogger(__name__)

ENCODER_AUTO = "auto"
ENCODER_CHOICES = [
    (ENCODER_AUTO, "Automático (GPU se disponível)"),
    ("h264_nvenc", "NVIDIA (NVENC)"),
    ("h264_qsv", "Intel (QuickSync)"),
    ("h264_amf", "AMD (AMF)"),
    ("libx264", "CPU (x264)"),
]

_ENCODER_OPTIONS = {
    "h264_nvenc": ("yuv420p", {"preset": "p3", "tune": "ll", "zerolatency": "1", "rc": "cbr", "forced-idr": "1"}),
    "h264_qsv": ("nv12", {"preset": "veryfast", "forced_idr": "1"}),
    "h264_amf": ("yuv420p", {"usage": "ultralowlatency", "quality": "speed", "rc": "cbr"}),
    "libx264": ("yuv420p", {"preset": "veryfast", "tune": "zerolatency", "forced-idr": "1"}),
}


def output_size(width: int, height: int, max_height: int) -> tuple[int, int]:
    """Reduz mantendo a proporção; dimensões sempre pares (exigência do yuv420p)."""
    if max_height and height > max_height:
        width = round(width * max_height / height)
        height = max_height
    return max(2, width - width % 2), max(2, height - height % 2)


class VideoEncoder:
    def __init__(self, max_height: int, fps: int, bitrate: int, encoder: str = ENCODER_AUTO):
        self.max_height = max_height
        self.fps = fps
        self.bitrate = bitrate
        self.preference = encoder
        self.codec_name: str | None = None
        self._ctx = None
        self._input_size: tuple[int, int] | None = None
        self._pix_fmt = "yuv420p"
        self._pts = 0

    def _candidates(self) -> list[str]:
        if self.preference == ENCODER_AUTO:
            return ["h264_nvenc", "h264_qsv", "h264_amf", "libx264"]
        return [self.preference] if self.preference == "libx264" else [self.preference, "libx264"]

    def _open(self, width: int, height: int) -> None:
        out_w, out_h = output_size(width, height, self.max_height)
        errors = []
        for name in self._candidates():
            pix_fmt, options = _ENCODER_OPTIONS[name]
            try:
                ctx = av.CodecContext.create(name, "w")
                ctx.width, ctx.height = out_w, out_h
                ctx.pix_fmt = pix_fmt
                ctx.time_base = Fraction(1, self.fps)
                ctx.framerate = Fraction(self.fps, 1)
                ctx.bit_rate = self.bitrate
                ctx.gop_size = self.fps * 4
                ctx.max_b_frames = 0
                ctx.options = dict(options)
                ctx.open()
            except Exception as exc:  # noqa: BLE001 - tentamos o próximo codificador
                errors.append(f"{name}: {exc}")
                continue
            self._ctx, self._pix_fmt, self.codec_name = ctx, pix_fmt, name
            self._input_size = (width, height)
            log.info("codificador %s aberto em %dx%d", name, out_w, out_h)
            return
        raise RuntimeError("Nenhum codificador H.264 disponível:\n" + "\n".join(errors))

    @property
    def size(self) -> tuple[int, int] | None:
        return (self._ctx.width, self._ctx.height) if self._ctx else None

    def encode(self, bgra: np.ndarray, force_keyframe: bool = False) -> list[tuple[bytes, bool]]:
        height, width = bgra.shape[:2]
        if self._input_size != (width, height):
            # Tamanho mudou (janela redimensionada): reabre e começa com keyframe.
            self.close()
            self._open(width, height)
            force_keyframe = True
        frame = av.VideoFrame.from_ndarray(bgra, format="bgra")
        frame = frame.reformat(width=self._ctx.width, height=self._ctx.height, format=self._pix_fmt)
        frame.pts = self._pts
        frame.time_base = self._ctx.time_base
        self._pts += 1
        if force_keyframe:
            frame.pict_type = av.video.frame.PictureType.I
        return [(bytes(p), bool(p.is_keyframe)) for p in self._ctx.encode(frame)]

    def close(self) -> None:
        self._ctx = None
        self._input_size = None


class VideoDecoder:
    def __init__(self):
        self._ctx = av.CodecContext.create("h264", "r")
        self._ctx.thread_type = "SLICE"  # threads por quadro adicionariam atraso

    def decode(self, data: bytes) -> list[np.ndarray]:
        """Retorna quadros RGB24 (altura, largura, 3)."""
        frames = self._ctx.decode(av.Packet(data))
        return [f.to_ndarray(format="rgb24") for f in frames]
