"""Janela exibida enquanto você transmite."""

from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QHBoxLayout, QLabel, QListWidget, QMainWindow, QMessageBox, QPushButton, QVBoxLayout, QWidget

from streamazap import netutil
from streamazap.host import StreamHost
from streamazap.ui.common import Bridge, ChatPanel


class HostWindow(QMainWindow):
    finished = Signal()

    def __init__(self, host: StreamHost, parent=None):
        super().__init__(parent)
        self.host = host
        settings = host.settings
        self.setWindowTitle(f"Transmitindo — {settings.room_name}")
        self.resize(720, 480)

        self.bridge = Bridge()
        host.on_chat = self.bridge.chat.emit
        host.on_viewers = self.bridge.viewers.emit
        host.on_stats = self.bridge.stats.emit
        host.on_error = self.bridge.error.emit

        addresses = ", ".join(
            f"{i.ip}{' (Radmin)' if i.is_radmin else ''}" for i in netutil.ipv4_interfaces()
        )
        header = QLabel(
            f"<b>{settings.room_name}</b><br>"
            f"Compartilhando: {settings.source.label}<br>"
            f"Seus endereços: {addresses or '—'} · porta {host.port}"
        )
        header.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        header.setWordWrap(True)
        self.stats = QLabel("Iniciando…")
        self.viewers = QListWidget()
        stop = QPushButton("Encerrar sala")
        stop.setStyleSheet("background:#c0392b;color:white;padding:6px 14px")
        stop.clicked.connect(self.close)

        left = QVBoxLayout()
        left.addWidget(QLabel("Espectadores:"))
        left.addWidget(self.viewers)
        left.addWidget(self.stats)
        left.addWidget(stop)
        self.chat = ChatPanel()
        self.chat.message_sent.connect(host.send_chat)

        body = QHBoxLayout()
        body.addLayout(left, 1)
        body.addWidget(self.chat, 2)
        root = QVBoxLayout()
        root.addWidget(header)
        root.addLayout(body)
        central = QWidget()
        central.setLayout(root)
        self.setCentralWidget(central)

        self.bridge.chat.connect(self.chat.add_message)
        self.bridge.viewers.connect(self._on_viewers)
        self.bridge.stats.connect(self._on_stats)
        self.bridge.error.connect(self._on_error)

    def _on_viewers(self, names: list) -> None:
        self.viewers.clear()
        self.viewers.addItems(names)

    def _on_stats(self, stats: dict) -> None:
        size = stats.get("size")
        resolution = f"{size[0]}x{size[1]}" if size else "—"
        text = f"{resolution} · {stats['fps']:.0f} fps · {stats['kbps']:.0f} kbps · {stats.get('encoder') or '—'}"
        errors = stats.get("audio_errors") or []
        if errors:
            text += "<br><span style='color:#c0392b'>Som: " + "; ".join(errors) + "</span>"
        self.stats.setText(text)

    def _on_error(self, message: str) -> None:
        QMessageBox.critical(self, "Transmissão interrompida", message)
        self.close()

    def closeEvent(self, event) -> None:
        self.host.stop()
        self.finished.emit()
        super().closeEvent(event)
