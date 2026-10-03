"""Janela exibida enquanto você é o dono da sala."""

from __future__ import annotations

import threading

from PySide6.QtCore import QObject, QSettings, Qt, Signal
from PySide6.QtWidgets import (
    QApplication,
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

from streamazap import APP_NAME, firewall, netutil
from streamazap.client import JoinError, StreamViewer
from streamazap.host import StreamHost
from streamazap.ui.common import Bridge, ChatPanel, format_rtt
from streamazap.ui.host_dialog import MODE_EDIT, HostDialog


class _WatchSignals(QObject):
    joined = Signal(object)
    failed = Signal(str)
    firewall_status = Signal(object)


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
        self._watch.firewall_status.connect(self._on_firewall_status)
        self.resize(780, 560)

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
        invite = QPushButton("📋  Copiar convite")
        invite.setToolTip("Copia o endereço da sala para mandar a quem não está vendo ela na lista (Entrar por IP)")
        invite.clicked.connect(self._copy_invite)
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
        buttons.addWidget(invite)
        buttons.addWidget(stop)
        left.addLayout(buttons)
        self.chat = ChatPanel()
        self.chat.message_sent.connect(host.send_chat)

        body = QHBoxLayout()
        body.addLayout(left, 1)
        body.addWidget(self.chat, 2)
        self.firewall_label = QLabel()
        self.firewall_label.setWordWrap(True)
        self.firewall_label.setStyleSheet("color:#c0392b")
        self.firewall_fix = QPushButton("🛡️  Corrigir firewall")
        self.firewall_fix.clicked.connect(self._fix_firewall)
        self.firewall_row = QWidget()
        firewall_layout = QHBoxLayout(self.firewall_row)
        firewall_layout.setContentsMargins(0, 0, 0, 0)
        firewall_layout.addWidget(self.firewall_label, 1)
        firewall_layout.addWidget(self.firewall_fix)
        self.firewall_row.hide()

        root = QVBoxLayout()
        root.addWidget(self.header)
        root.addWidget(self.firewall_row)
        root.addLayout(body)
        central = QWidget()
        central.setLayout(root)
        self.setCentralWidget(central)

        self.bridge.chat.connect(self.chat.add_message)
        self.bridge.viewers.connect(self._on_viewers)
        self.bridge.stats.connect(self._on_stats)
        self.bridge.error.connect(self._on_error)
        self._update_header()
        self._check_firewall()

    # -- firewall: quem transmite precisa aceitar conexões de entrada ----------------
    def _check_firewall(self) -> None:
        threading.Thread(target=lambda: self._watch.firewall_status.emit(firewall.check()), daemon=True).start()

    def _on_firewall_status(self, status: firewall.FirewallStatus) -> None:
        text = status.describe()
        self.firewall_row.setVisible(bool(text))
        self.firewall_label.setText(
            f"⚠️ {text}. Seus amigos vão ver a sala, mas a conexão pode dar “tempo esgotado”." if text else ""
        )
        self.firewall_fix.setVisible(status.blocked or not status.allowed)
        self.firewall_fix.setEnabled(True)
        self.firewall_fix.setText("🛡️  Corrigir firewall")

    def _fix_firewall(self) -> None:
        self.firewall_fix.setEnabled(False)
        self.firewall_fix.setText("Corrigindo… (aceite o aviso do Windows)")

        def work():
            firewall.fix()
            self._watch.firewall_status.emit(firewall.check())

        threading.Thread(target=work, daemon=True).start()

    def _copy_invite(self) -> None:
        interfaces = netutil.ipv4_interfaces()  # Radmin primeiro
        if not interfaces:
            return
        invite = f"{interfaces[0].ip}:{self.host.port}"
        QApplication.clipboard().setText(invite)
        self.chat.add_message("", f"Convite copiado: {invite} — no StreamaZap do amigo: “Entrar por IP…”")

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
        viewer = StreamViewer(
            stream["addresses"],
            stream["port"],
            self.host.settings.host_name,
            self.host.settings.password,
            callback_request=lambda port: self.host.request_participant_callback(stream["id"], port),
        )

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
