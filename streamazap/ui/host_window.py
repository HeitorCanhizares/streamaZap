"""Janela exibida enquanto você é o dono da sala."""

from __future__ import annotations

import threading

from PySide6.QtCore import QObject, QSettings, Qt, Signal
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from streamazap import APP_NAME, netutil
from streamazap.client import JoinError, StreamViewer
from streamazap.host import StreamHost
from streamazap.ui.common import Bridge, ChatPanel, format_rtt
from streamazap.ui.host_dialog import MODE_EDIT, HostDialog


class _WatchSignals(QObject):
    joined = Signal(object)
    failed = Signal(str)


class HostWindow(QMainWindow):
    finished = Signal()

    def __init__(self, host: StreamHost, parent=None):
        super().__init__(parent)
        self.host = host
        self._children: list = []
        self.bridge = Bridge()
        host.on_chat = self.bridge.chat.emit
        host.on_viewers = self.bridge.viewers.emit
        host.on_stats = self.bridge.stats.emit
        host.on_error = self.bridge.error.emit
        self._watch = _WatchSignals()
        self._watch.joined.connect(self._open_watch_window)
        self._watch.failed.connect(lambda msg: QMessageBox.warning(self, "Assistir", msg))
        self.resize(780, 520)

        self.header = QLabel()
        self.header.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        self.header.setWordWrap(True)
        self.stats = QLabel("Iniciando…")
        self.stats.setWordWrap(True)
        self.viewers = QListWidget()
        self.streams = QListWidget()
        self.streams.itemDoubleClicked.connect(self._watch_stream)
        watch = QPushButton("Assistir")
        watch.clicked.connect(lambda: self._watch_stream(self.streams.currentItem()))
        edit = QPushButton("✏️  Editar transmissão")
        edit.clicked.connect(self.edit)
        stop = QPushButton("Encerrar sala")
        stop.setStyleSheet("background:#c0392b;color:white;padding:6px 14px")
        stop.clicked.connect(self.close)

        left = QVBoxLayout()
        left.addWidget(QLabel("Espectadores (latência):"))
        left.addWidget(self.viewers, 2)
        left.addWidget(QLabel("Outros compartilhando na sala:"))
        left.addWidget(self.streams, 1)
        left.addWidget(watch)
        left.addWidget(self.stats)
        buttons = QHBoxLayout()
        buttons.addWidget(edit)
        buttons.addWidget(stop)
        left.addLayout(buttons)
        self.chat = ChatPanel()
        self.chat.message_sent.connect(host.send_chat)

        body = QHBoxLayout()
        body.addLayout(left, 1)
        body.addWidget(self.chat, 2)
        root = QVBoxLayout()
        root.addWidget(self.header)
        root.addLayout(body)
        central = QWidget()
        central.setLayout(root)
        self.setCentralWidget(central)

        self.bridge.chat.connect(self.chat.add_message)
        self.bridge.viewers.connect(self._on_viewers)
        self.bridge.stats.connect(self._on_stats)
        self.bridge.error.connect(self._on_error)
        self._update_header()

    def _update_header(self) -> None:
        s = self.host.settings
        self.setWindowTitle(f"Transmitindo — {s.room_name}")
        addresses = ", ".join(f"{i.ip}{' (Radmin)' if i.is_radmin else ''}" for i in netutil.ipv4_interfaces())
        lock = " 🔒" if s.password else ""
        self.header.setText(
            f"<b>{s.room_name}</b>{lock}<br>"
            f"Compartilhando: {s.source.label if s.source else '—'}<br>"
            f"Seus endereços: {addresses or '—'} · porta {self.host.port}"
        )

    def edit(self) -> None:
        dialog = HostDialog(self.host.settings.host_name, QSettings(APP_NAME, APP_NAME), self, mode=MODE_EDIT, current=self.host.settings)
        if dialog.exec() != HostDialog.DialogCode.Accepted:
            return
        self.host.apply_settings(dialog.result_settings())
        self._update_header()
        self.chat.add_message("", "Transmissão atualizada")

    def _on_viewers(self, viewers: list) -> None:
        self.viewers.clear()
        for v in viewers:
            sharing = " · 📺 compartilhando" if v.get("sharing") else ""
            self.viewers.addItem(f"{v['name']}  ({format_rtt(v.get('rtt'))}){sharing}")
        selected = self.streams.currentItem().data(Qt.ItemDataRole.UserRole)["id"] if self.streams.currentItem() else None
        self.streams.clear()
        for stream in self.host.streams():
            if stream["id"] == "host":
                continue
            item = QListWidgetItem(f"📺 {stream['name']}")
            item.setData(Qt.ItemDataRole.UserRole, stream)
            self.streams.addItem(item)
            if stream["id"] == selected:
                self.streams.setCurrentItem(item)

    def _on_stats(self, stats: dict) -> None:
        size = stats.get("size")
        resolution = f"{size[0]}x{size[1]}" if size else "—"
        text = f"{resolution} · {stats['fps']:.0f} fps · {stats['kbps']:.0f} kbps · {stats.get('encoder') or '—'}"
        if stats.get("target_kbps", 0) < stats.get("base_kbps", 0):
            text += (
                f"<br><span style='color:#c87f0a'>Qualidade reduzida para {stats['target_kbps']} kbps "
                f"(alguém com conexão lenta, atraso {stats.get('lag', 0):.1f}s)</span>"
            )
        errors = stats.get("audio_errors") or []
        if errors:
            text += "<br><span style='color:#c0392b'>Som: " + "; ".join(errors) + "</span>"
        self.stats.setText(text)

    def _on_error(self, message: str) -> None:
        QMessageBox.critical(self, "Erro na captura", f"{message}\n\nUse “Editar transmissão” para escolher outra tela ou janela.")

    def _watch_stream(self, item) -> None:
        if item is None:
            return
        stream = item.data(Qt.ItemDataRole.UserRole)
        viewer = StreamViewer(stream["addresses"], stream["port"], self.host.settings.host_name, self.host.settings.password)

        def work():
            try:
                viewer.connect()
            except JoinError as exc:
                self._watch.failed.emit(str(exc))
            else:
                self._watch.joined.emit(viewer)

        threading.Thread(target=work, daemon=True).start()

    def _open_watch_window(self, viewer: StreamViewer) -> None:
        from streamazap.ui.viewer_window import ViewerWindow

        window = ViewerWindow(viewer, room_mode=False)
        window.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        self._children.append(window)
        window.destroyed.connect(lambda *_: self._children.remove(window) if window in self._children else None)
        viewer.start()
        window.show()

    def closeEvent(self, event) -> None:
        for window in list(self._children):
            window.close()
        self.host.stop("O host encerrou a sala")
        self.finished.emit()
        super().closeEvent(event)
