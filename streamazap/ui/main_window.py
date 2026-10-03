"""Tela inicial: lista de salas encontradas na rede (LAN / Radmin VPN)."""

from __future__ import annotations

import socket
import threading

from PySide6.QtCore import QObject, QSettings, Qt, QTimer, Signal
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QHBoxLayout,
    QHeaderView,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from streamazap import APP_NAME, __version__, config, netutil, updater
from streamazap.client import JoinError, StreamViewer
from streamazap.discovery import RoomBrowser
from streamazap.host import StreamHost
from streamazap.ui.host_dialog import HostDialog
from streamazap.ui.host_window import HostWindow
from streamazap.ui.update_dialog import UpdateDialog
from streamazap.ui.viewer_window import ViewerWindow


class _Signals(QObject):
    joined = Signal(object)
    join_failed = Signal(str)
    update_available = Signal(object)


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle(f"{APP_NAME} {__version__}")
        self.resize(760, 480)
        self.settings = QSettings(APP_NAME, APP_NAME)
        self.signals = _Signals()
        self.browser = RoomBrowser()
        self.browser.start()
        self._windows: list = []
        self._joining = False

        self.nick = QLineEdit(self.settings.value("nickname", socket.gethostname()))
        self.nick.setMaximumWidth(220)
        self.nick.editingFinished.connect(lambda: self.settings.setValue("nickname", self.nick.text().strip()))
        create = QPushButton("➕  Criar sala")
        create.setStyleSheet("font-weight:bold;padding:6px 14px")
        create.clicked.connect(self.create_room)
        manual = QPushButton("Entrar por IP…")
        manual.clicked.connect(self.join_manual)
        top = QHBoxLayout()
        top.addWidget(QLabel("Seu nome:"))
        top.addWidget(self.nick)
        top.addStretch()
        top.addWidget(manual)
        top.addWidget(create)

        self.rooms = QTreeWidget()
        self.rooms.setHeaderLabels(["Sala", "Host", "Assistindo", "", "Endereço"])
        self.rooms.setRootIsDecorated(False)
        self.rooms.header().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.rooms.itemDoubleClicked.connect(lambda item, _: self.join_room(item.data(0, Qt.ItemDataRole.UserRole)))
        join = QPushButton("Entrar na sala")
        join.clicked.connect(self._join_selected)

        self.empty_hint = QLabel(
            "Procurando salas na rede… As salas de amigos conectados na mesma rede do "
            "Radmin VPN (ou na mesma rede local) aparecem aqui automaticamente."
        )
        self.empty_hint.setWordWrap(True)
        self.empty_hint.setStyleSheet("color:gray")
        self.network_label = QLabel()
        self.network_label.setWordWrap(True)
        self.update_label = QLabel()
        self.update_label.setOpenExternalLinks(True)

        layout = QVBoxLayout()
        layout.addLayout(top)
        layout.addWidget(self.rooms)
        layout.addWidget(self.empty_hint)
        bottom = QHBoxLayout()
        bottom.addWidget(self.network_label, 1)
        bottom.addWidget(join)
        layout.addLayout(bottom)
        self.auto_update = QCheckBox("Atualizar automaticamente ao abrir")
        self.auto_update.setChecked(self.settings.value("auto_update", True, type=bool))
        self.auto_update.toggled.connect(lambda on: self.settings.setValue("auto_update", on))
        update_row = QHBoxLayout()
        update_row.addWidget(self.update_label, 1)
        update_row.addWidget(self.auto_update)
        layout.addLayout(update_row)
        central = QWidget()
        central.setLayout(layout)
        self.setCentralWidget(central)

        self.signals.joined.connect(self._on_joined)
        self.signals.join_failed.connect(self._on_join_failed)
        self.signals.update_available.connect(self._on_update)

        self._timer = QTimer(self)
        self._timer.timeout.connect(self.refresh)
        self._timer.start(1000)
        self.refresh()
        threading.Thread(target=self._check_update, daemon=True).start()

    # -- lista de salas ----------------------------------------------------------
    def refresh(self) -> None:
        selected = self.rooms.currentItem()
        selected_id = selected.data(0, Qt.ItemDataRole.UserRole).room_id if selected else None
        self.rooms.clear()
        for room in self.browser.rooms():
            item = QTreeWidgetItem([room.name, room.host_name, str(room.viewers), "🔒" if room.locked else "", ", ".join(room.addresses)])
            item.setData(0, Qt.ItemDataRole.UserRole, room)
            self.rooms.addTopLevelItem(item)
            if room.room_id == selected_id:
                self.rooms.setCurrentItem(item)
        self.empty_hint.setVisible(self.rooms.topLevelItemCount() == 0)

        interfaces = netutil.ipv4_interfaces()
        radmin = [i.ip for i in interfaces if i.is_radmin]
        if self.browser.error:
            text = f"<span style='color:#c0392b'>{self.browser.error}</span>"
        elif radmin:
            text = f"✅ Radmin VPN conectado: {', '.join(radmin)}"
        else:
            text = "⚠️ Radmin VPN não detectado — apenas a rede local será usada."
        others = [i.ip for i in interfaces if not i.is_radmin]
        if others:
            text += f" · Rede local: {', '.join(others)}"
        self.network_label.setText(text)

    def _join_selected(self) -> None:
        item = self.rooms.currentItem()
        if item is None:
            QMessageBox.information(self, APP_NAME, "Selecione uma sala na lista.")
            return
        self.join_room(item.data(0, Qt.ItemDataRole.UserRole))

    # -- entrar --------------------------------------------------------------------
    def _nickname(self) -> str:
        name = self.nick.text().strip() or socket.gethostname()
        self.settings.setValue("nickname", name)
        return name

    def join_room(self, room) -> None:
        password = ""
        if room.locked:
            password, ok = QInputDialog.getText(self, room.name, "Senha da sala:", QLineEdit.EchoMode.Password)
            if not ok:
                return
        self._start_join(room.addresses, room.port, password)

    def join_manual(self) -> None:
        text, ok = QInputDialog.getText(self, "Entrar por IP", "Endereço do host (ex.: 26.12.34.56 ou 26.12.34.56:47800):",
                                        text=self.settings.value("last_manual", ""))
        if not ok or not text.strip():
            return
        text = text.strip()
        self.settings.setValue("last_manual", text)
        host, _, port = text.partition(":")
        password, ok = QInputDialog.getText(self, "Entrar por IP", "Senha (deixe vazio se não houver):", QLineEdit.EchoMode.Password)
        if not ok:
            return
        try:
            port_number = int(port) if port else config.STREAM_PORT
        except ValueError:
            QMessageBox.warning(self, APP_NAME, "Porta inválida.")
            return
        self._start_join([host], port_number, password)

    def _start_join(self, addresses: list[str], port: int, password: str) -> None:
        if self._joining:
            return
        self._joining = True
        self.statusBar().showMessage("Conectando…")
        viewer = StreamViewer(addresses, port, self._nickname(), password)

        def work():
            try:
                viewer.connect()
            except JoinError as exc:
                self.signals.join_failed.emit(str(exc))
            else:
                self.signals.joined.emit(viewer)

        threading.Thread(target=work, daemon=True).start()

    def _on_joined(self, viewer: StreamViewer) -> None:
        self._joining = False
        self.statusBar().clearMessage()
        viewer.player.start()  # antes da janela, para ela poder mostrar erro de áudio
        window = ViewerWindow(viewer)
        viewer.start()
        self._keep(window)

    def _on_join_failed(self, message: str) -> None:
        self._joining = False
        self.statusBar().clearMessage()
        QMessageBox.warning(self, "Não foi possível entrar", message)

    # -- criar ---------------------------------------------------------------------
    def create_room(self) -> None:
        dialog = HostDialog(self._nickname(), self.settings, self)
        if dialog.exec() != HostDialog.DialogCode.Accepted:
            return
        host = StreamHost(dialog.result_settings())
        window = HostWindow(host)
        try:
            host.start()
        except Exception as exc:  # noqa: BLE001
            QMessageBox.critical(self, "Erro ao criar sala", str(exc))
            host.stop()
            return
        self._keep(window)

    def _keep(self, window) -> None:
        self._windows.append(window)
        window.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        window.destroyed.connect(lambda *_: self._windows.remove(window) if window in self._windows else None)
        window.show()

    # -- atualização ---------------------------------------------------------------
    def _check_update(self) -> None:
        release = updater.check_latest()
        if release:
            self.signals.update_available.emit(release)

    def _on_update(self, release: updater.Release) -> None:
        self.update_label.setText(
            f"🎉 Nova versão disponível: <a href='{release.page_url}'>{release.version} — baixar instalador</a>"
        )
        if self.auto_update.isChecked() and updater.can_auto_update(release) and not self._windows:
            self._run_update(release)

    def _run_update(self, release: updater.Release) -> None:
        dialog = UpdateDialog(release, self)
        dialog.exec()
        if dialog.error:
            QMessageBox.warning(self, "Atualização", f"Não foi possível atualizar automaticamente:\n{dialog.error}")
            return
        if not dialog.installer_path:
            return  # usuário escolheu "Agora não"
        try:
            updater.launch_installer(dialog.installer_path)
        except updater.UpdateError as exc:
            QMessageBox.warning(self, "Atualização", str(exc))
            return
        # O instalador fecha este processo, instala e reabre a nova versão.
        self.close()
        QApplication.quit()

    def closeEvent(self, event) -> None:
        for window in list(self._windows):
            window.close()
        self.browser.stop()
        super().closeEvent(event)
