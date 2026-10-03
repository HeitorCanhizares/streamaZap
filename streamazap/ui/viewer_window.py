"""Janela de quem está assistindo a uma sala."""

from __future__ import annotations

import threading

from PySide6.QtCore import Qt
from PySide6.QtGui import QKeySequence, QShortcut
from PySide6.QtWidgets import QHBoxLayout, QLabel, QMainWindow, QMessageBox, QPushButton, QSlider, QSplitter, QVBoxLayout, QWidget

from streamazap.client import StreamViewer
from streamazap.ui.common import Bridge, ChatPanel, VideoWidget


class ViewerWindow(QMainWindow):
    def __init__(self, viewer: StreamViewer, parent=None):
        super().__init__(parent)
        self.viewer = viewer
        self.setWindowTitle(f"{viewer.room_name} — {viewer.host_name}")
        self.resize(1280, 760)

        self.bridge = Bridge()
        self._latest = None
        self._latest_lock = threading.Lock()
        viewer.on_frame = self._on_frame_thread
        viewer.on_chat = self.bridge.chat.emit
        viewer.on_info = self.bridge.info.emit
        viewer.on_closed = self.bridge.closed.emit

        self.video = VideoWidget()
        self.video.mouseDoubleClickEvent = lambda event: self.toggle_fullscreen()
        self.chat = ChatPanel()
        self.chat.message_sent.connect(self._send_chat)
        self.viewers_label = QLabel()
        self.viewers_label.setWordWrap(True)
        side = QWidget()
        side.setMinimumWidth(260)
        side_layout = QVBoxLayout(side)
        side_layout.addWidget(self.viewers_label)
        side_layout.addWidget(self.chat)

        self.splitter = QSplitter()
        self.splitter.addWidget(self.video)
        self.splitter.addWidget(side)
        self.splitter.setStretchFactor(0, 4)
        self.splitter.setStretchFactor(1, 0)
        self.splitter.setSizes([1000, 280])
        self.side = side

        self.mute = QPushButton("🔊")
        self.mute.setCheckable(True)
        self.mute.toggled.connect(self._on_mute)
        self.volume = QSlider(Qt.Orientation.Horizontal)
        self.volume.setRange(0, 150)
        self.volume.setValue(100)
        self.volume.setMaximumWidth(160)
        self.volume.valueChanged.connect(lambda v: setattr(viewer.player, "volume", v / 100))
        chat_toggle = QPushButton("Chat")
        chat_toggle.setCheckable(True)
        chat_toggle.setChecked(True)
        chat_toggle.toggled.connect(side.setVisible)
        fullscreen = QPushButton("Tela cheia (F11)")
        fullscreen.clicked.connect(self.toggle_fullscreen)
        leave = QPushButton("Sair da sala")
        leave.clicked.connect(self.close)
        self.controls = QWidget()
        bar = QHBoxLayout(self.controls)
        bar.setContentsMargins(8, 4, 8, 6)
        bar.addWidget(self.mute)
        bar.addWidget(self.volume)
        if viewer.player.error:
            bar.addWidget(QLabel(viewer.player.error))
        bar.addStretch()
        bar.addWidget(chat_toggle)
        bar.addWidget(fullscreen)
        bar.addWidget(leave)

        root = QVBoxLayout()
        root.setContentsMargins(0, 0, 0, 0)
        root.addWidget(self.splitter, 1)
        root.addWidget(self.controls, 0)
        central = QWidget()
        central.setLayout(root)
        self.setCentralWidget(central)

        QShortcut(QKeySequence("F11"), self, self.toggle_fullscreen)
        QShortcut(QKeySequence("Esc"), self, self._exit_fullscreen)

        self.bridge.frame_ready.connect(self._show_latest, Qt.ConnectionType.QueuedConnection)
        self.bridge.chat.connect(self.chat.add_message)
        self.bridge.info.connect(self._on_info)
        self.bridge.closed.connect(self._on_closed)
        self._chat_visible = True

    # Chamado na thread de rede: guarda só o quadro mais recente para não acumular atraso.
    def _on_frame_thread(self, frame) -> None:
        with self._latest_lock:
            pending = self._latest is not None
            self._latest = frame
        if not pending:
            self.bridge.frame_ready.emit()

    def _show_latest(self) -> None:
        with self._latest_lock:
            frame, self._latest = self._latest, None
        if frame is not None:
            self.video.set_frame(frame)

    def _send_chat(self, text: str) -> None:
        self.viewer.send_chat(text)

    def _on_mute(self, muted: bool) -> None:
        self.viewer.player.muted = muted
        self.mute.setText("🔇" if muted else "🔊")

    def _on_info(self, info: dict) -> None:
        names = ", ".join(info.get("viewers", []))
        self.viewers_label.setText(f"<b>Host:</b> {info.get('host', '')}<br><b>Assistindo:</b> {names}")

    def toggle_fullscreen(self) -> None:
        if self.isFullScreen():
            self._exit_fullscreen()
        else:
            self._chat_visible = self.side.isVisible()
            self.side.hide()
            self.controls.hide()
            self.showFullScreen()

    def _exit_fullscreen(self) -> None:
        if self.isFullScreen():
            self.showNormal()
            self.controls.show()
            self.side.setVisible(self._chat_visible)

    def _on_closed(self, reason: str) -> None:
        self._exit_fullscreen()
        QMessageBox.information(self, "Sala encerrada", reason)
        self.close()

    def closeEvent(self, event) -> None:
        self.viewer.stop()
        super().closeEvent(event)
