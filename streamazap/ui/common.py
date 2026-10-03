"""Peças de interface reaproveitadas pelas janelas de host e espectador."""

from __future__ import annotations

import html
import time

import numpy as np
from PySide6.QtCore import QObject, QRect, Qt, Signal
from PySide6.QtGui import QColor, QImage, QPainter
from PySide6.QtWidgets import QHBoxLayout, QLineEdit, QPushButton, QTextBrowser, QVBoxLayout, QWidget


def format_rtt(rtt) -> str:
    return f"{rtt:.0f} ms" if isinstance(rtt, (int, float)) else "…"


class Bridge(QObject):
    """Leva eventos das threads de rede para a thread da interface."""

    chat = Signal(str, str)
    viewers = Signal(list)
    info = Signal(dict)
    stats = Signal(dict)
    error = Signal(str)
    closed = Signal(str)
    status = Signal(str)
    frame_ready = Signal()
    media_joined = Signal(object)
    media_failed = Signal(str)
    media_closed = Signal(str)
    share_error = Signal(str)
    share_stats = Signal(dict)


class VideoWidget(QWidget):
    """Desenha o último quadro recebido mantendo a proporção."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self._image: QImage | None = None
        self._array: np.ndarray | None = None
        self.setMinimumSize(320, 180)
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent)
        self.placeholder = "Aguardando vídeo…"
        self.overlay = ""  # aviso exibido por cima do vídeo (ex.: reconectando)

    def set_overlay(self, text: str) -> None:
        self.overlay = text
        self.update()

    def clear(self, placeholder: str) -> None:
        self._image = None
        self._array = None
        self.placeholder = placeholder
        self.update()

    def set_frame(self, bgra: np.ndarray) -> None:
        bgra = np.ascontiguousarray(bgra)
        height, width = bgra.shape[:2]
        self._array = bgra  # o QImage não copia os dados
        # RGB32 é o formato nativo do Qt: escalar RGB888 a cada quadro custa 2-3x mais.
        self._image = QImage(bgra.data, width, height, width * 4, QImage.Format.Format_RGB32)
        self.update()

    def paintEvent(self, event) -> None:
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor(0, 0, 0))
        if self._image is None:
            painter.setPen(QColor(180, 180, 180))
            painter.drawText(self.rect(), Qt.AlignmentFlag.AlignCenter, self.placeholder)
        else:
            painter.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform)
            img_w, img_h = self._image.width(), self._image.height()
            scale = min(self.width() / img_w, self.height() / img_h)
            w, h = int(img_w * scale), int(img_h * scale)
            painter.drawImage(QRect((self.width() - w) // 2, (self.height() - h) // 2, w, h), self._image)
        if self.overlay:
            band = QRect(0, 0, self.width(), 36)
            painter.fillRect(band, QColor(200, 120, 0, 220))
            painter.setPen(QColor(255, 255, 255))
            painter.drawText(band, Qt.AlignmentFlag.AlignCenter, self.overlay)


class ChatPanel(QWidget):
    message_sent = Signal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.log = QTextBrowser()
        self.log.setOpenExternalLinks(True)
        self.input = QLineEdit()
        self.input.setPlaceholderText("Mensagem…")
        send = QPushButton("Enviar")
        row = QHBoxLayout()
        row.addWidget(self.input)
        row.addWidget(send)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(self.log)
        layout.addLayout(row)
        send.clicked.connect(self._send)
        self.input.returnPressed.connect(self._send)

    def _send(self) -> None:
        text = self.input.text().strip()
        if text:
            self.message_sent.emit(text)
            self.input.clear()

    def add_message(self, name: str, text: str) -> None:
        stamp = time.strftime("%H:%M")
        if name:
            line = f"<span style='color:gray'>{stamp}</span> <b>{html.escape(name)}:</b> {html.escape(text)}"
        else:
            line = f"<span style='color:gray'>{stamp} <i>{html.escape(text)}</i></span>"
        self.log.append(line)
