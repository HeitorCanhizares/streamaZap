"""Janela de quem está na sala assistindo (e, se quiser, compartilhando também).

room_mode=True: conexão com o dono da sala (chat, participantes, troca de stream,
compartilhar a própria tela). room_mode=False: só assiste um stream (usado pelo
host para ver um participante).
"""

from __future__ import annotations

import threading

from PySide6.QtCore import QSettings, Qt
from PySide6.QtGui import QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSlider,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from streamazap import APP_NAME, config
from streamazap.client import JoinError, StreamViewer
from streamazap.host import StreamHost, local_addresses
from streamazap.ui.common import Bridge, ChatPanel, VideoWidget, format_network, format_rtt
from streamazap.ui.host_dialog import MODE_EDIT, MODE_SHARE, HostDialog

HOST_STREAM = "host"


class ViewerWindow(QMainWindow):
    def __init__(self, viewer: StreamViewer, parent=None, room_mode: bool = True):
        super().__init__(parent)
        self.room = viewer
        self.room_mode = room_mode
        self.media: StreamViewer | None = None  # conexão extra ao assistir um participante
        self.share: StreamHost | None = None  # nosso compartilhamento dentro da sala
        self.settings = QSettings(APP_NAME, APP_NAME)
        self._current_stream = HOST_STREAM
        self._streams: list[dict] = []
        self._latest = None
        self._latest_lock = threading.Lock()
        self._chat_visible = True
        self.setWindowTitle(f"{viewer.room_name} — {viewer.host_name}")
        self.resize(1280, 760)

        delay = int(self.settings.value("playout_delay", config.DEFAULT_PLAYOUT_DELAY_MS))
        auto = self.settings.value("playout_auto", True, type=bool)
        viewer.set_delay(delay)
        viewer.set_auto_delay(auto)

        self.bridge = Bridge()
        viewer.on_frame = lambda frame: self._on_frame_thread(viewer, frame)
        viewer.on_chat = self.bridge.chat.emit
        viewer.on_info = self.bridge.info.emit
        viewer.on_closed = self.bridge.closed.emit
        viewer.on_status = self.bridge.status.emit
        viewer.on_stats = lambda stats: self.bridge.stats.emit(stats) if self.media is None else None
        # Alguém não alcança nosso compartilhamento: o host pede que a gente conecte nele.
        viewer.on_callback = lambda ip, port: self.share.connect_back(ip, port) if self.share else None

        # Vídeo + lateral ---------------------------------------------------------
        self.video = VideoWidget()
        self.video.mouseDoubleClickEvent = lambda event: self.toggle_fullscreen()
        self.chat = ChatPanel()
        self.chat.message_sent.connect(viewer.send_chat)
        self.viewers_label = QLabel()
        self.viewers_label.setWordWrap(True)
        self.stream_select = QComboBox()
        self.stream_select.activated.connect(self._on_stream_selected)
        self.share_button = QPushButton("📺  Compartilhar minha tela")
        self.share_button.clicked.connect(self._toggle_share)
        self.share_edit = QPushButton("✏️  Editar")
        self.share_edit.clicked.connect(self._edit_share)
        self.share_edit.hide()
        self.share_status = QLabel()
        self.share_status.setWordWrap(True)
        self.share_status.setStyleSheet("color: gray")

        side = QWidget()
        side.setMinimumWidth(280)
        side_layout = QVBoxLayout(side)
        side_layout.addWidget(QLabel("Assistindo:"))
        side_layout.addWidget(self.stream_select)
        share_row = QHBoxLayout()
        share_row.addWidget(self.share_button, 1)
        share_row.addWidget(self.share_edit)
        side_layout.addLayout(share_row)
        side_layout.addWidget(self.share_status)
        side_layout.addWidget(self.viewers_label)
        side_layout.addWidget(self.chat, 1)
        self.side = side

        self.splitter = QSplitter()
        self.splitter.addWidget(self.video)
        self.splitter.addWidget(side)
        self.splitter.setStretchFactor(0, 4)
        self.splitter.setStretchFactor(1, 0)
        self.splitter.setSizes([1000, 300])

        # Barra inferior ----------------------------------------------------------
        self.mute = QPushButton("🔊")
        self.mute.setCheckable(True)
        self.mute.toggled.connect(self._on_mute)
        self.volume = QSlider(Qt.Orientation.Horizontal)
        self.volume.setRange(0, 150)
        self.volume.setValue(100)
        self.volume.setMaximumWidth(140)
        self.volume.valueChanged.connect(self._apply_audio)
        self.smooth = QSlider(Qt.Orientation.Horizontal)
        self.smooth.setRange(0, config.MAX_PLAYOUT_DELAY_MS // 50)
        self.smooth.setValue(delay // 50)
        self.smooth.setMaximumWidth(160)
        self.smooth.setToolTip(
            "Atraso proposital para absorver as variações da rede.\n"
            "Mais alto = mais liso (bom para conexões de longe); mais baixo = menos atraso."
        )
        self.smooth_label = QLabel()
        self.smooth_auto = QCheckBox("Auto")
        self.smooth_auto.setToolTip(
            "Ajusta a suavização sozinho: sobe quando a rede engasga (para não travar) e desce\n"
            "devagar quando ela estabiliza (menos atraso). O som acompanha sem cortes."
        )
        self.smooth_auto.setChecked(auto)
        self.smooth_auto.toggled.connect(self._on_smooth_auto)
        self.smooth.valueChanged.connect(self._on_smooth)
        self._on_smooth(self.smooth.value(), save=False)
        self.smooth.setEnabled(not auto)
        self.net_label = QLabel()
        self.net_label.setStyleSheet("color: gray")
        chat_toggle = QPushButton("Painel")
        chat_toggle.setCheckable(True)
        chat_toggle.setChecked(True)
        chat_toggle.toggled.connect(side.setVisible)
        fullscreen = QPushButton("Tela cheia (F11)")
        fullscreen.clicked.connect(self.toggle_fullscreen)
        leave = QPushButton("Sair da sala" if room_mode else "Fechar")
        leave.clicked.connect(self.close)
        self.controls = QWidget()
        bar = QHBoxLayout(self.controls)
        bar.setContentsMargins(8, 4, 8, 6)
        bar.addWidget(self.mute)
        bar.addWidget(self.volume)
        if viewer.player.error:
            bar.addWidget(QLabel(viewer.player.error))
        bar.addSpacing(12)
        bar.addWidget(self.smooth_label)
        bar.addWidget(self.smooth)
        bar.addWidget(self.smooth_auto)
        bar.addSpacing(12)
        bar.addWidget(self.net_label)
        bar.addStretch()
        if room_mode:
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
        if not room_mode:
            side.hide()

        QShortcut(QKeySequence("F11"), self, self.toggle_fullscreen)
        QShortcut(QKeySequence("Esc"), self, self._exit_fullscreen)

        b = self.bridge
        b.frame_ready.connect(self._show_latest, Qt.ConnectionType.QueuedConnection)
        b.chat.connect(self.chat.add_message)
        b.info.connect(self._on_info)
        b.closed.connect(self._on_closed)
        b.status.connect(self.video.set_overlay)
        b.stats.connect(self._on_stats)
        b.media_joined.connect(self._on_media_joined)
        b.media_failed.connect(self._on_media_failed)
        b.media_closed.connect(self._on_media_closed)
        b.share_error.connect(self._on_share_error)
        b.share_stats.connect(self._on_share_stats)
        self._rebuild_stream_list()

    # -- quadros (chamado nas threads de rede) ---------------------------------------
    def _active_viewer(self) -> StreamViewer:
        return self.media or self.room

    def _on_frame_thread(self, source: StreamViewer, frame) -> None:
        if source is not self._active_viewer():
            return
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

    # -- controles -----------------------------------------------------------------
    def _on_mute(self, muted: bool) -> None:
        self.mute.setText("🔇" if muted else "🔊")
        self._apply_audio()

    def _apply_audio(self, *_) -> None:
        for viewer in (self.room, self.media):
            if viewer is not None:
                viewer.player.muted = self.mute.isChecked()
                viewer.player.volume = self.volume.value() / 100

    def _on_smooth(self, value: int, save: bool = True) -> None:
        delay = value * 50
        if not self.smooth_auto.isChecked():
            self.smooth_label.setText(f"Suavização: {delay} ms")
        for viewer in (self.room, self.media):
            if viewer is not None:
                viewer.set_delay(delay)
        if save:
            self.settings.setValue("playout_delay", delay)

    def _on_smooth_auto(self, enabled: bool) -> None:
        self.smooth.setEnabled(not enabled)
        for viewer in (self.room, self.media):
            if viewer is not None:
                viewer.set_auto_delay(enabled)
        self.settings.setValue("playout_auto", enabled)
        if enabled:
            self.smooth_label.setText("Suavização: auto")
        else:
            self._on_smooth(self.smooth.value(), save=False)

    def _on_stats(self, stats: dict) -> None:
        if stats.get("auto") and self.smooth_auto.isChecked():
            self.smooth_label.setText(f"Suavização: auto · {stats.get('delay_ms', 0)} ms")
        mbps = stats.get("kbps", 0) / 1000
        reverse = " · 🔁 conexão reversa" if stats.get("reverse") else ""
        self.net_label.setText(
            f"📶 {format_rtt(stats.get('rtt'))} · {mbps:.1f} Mbps · buffer {stats.get('buffer_ms', 0)} ms{reverse}"
        )

    # -- sala: participantes e streams ---------------------------------------------
    def _on_info(self, info: dict) -> None:
        lines = [f"<b>Host:</b> {info.get('host', '')}"]
        for v in info.get("viewers", []):
            you = " (você)" if v.get("id") == self.room.viewer_id else ""
            sharing = " 📺" if v.get("sharing") else ""
            lines.append(f"• {v['name']}{you}{sharing} <span style='color:gray'>{format_network(v)}</span>")
        self.viewers_label.setText("<br>".join(lines))
        self._streams = [s for s in info.get("streams", []) if s.get("id") != self.room.viewer_id]
        self._rebuild_stream_list()
        if self._current_stream != HOST_STREAM and not any(s["id"] == self._current_stream for s in self._streams):
            self._switch_to_host("O compartilhamento que você assistia terminou.")

    def _rebuild_stream_list(self) -> None:
        self.stream_select.blockSignals(True)
        self.stream_select.clear()
        self.stream_select.addItem(f"📺 {self.room.host_name} (host)", HOST_STREAM)
        for stream in self._streams:
            if stream["id"] != HOST_STREAM:
                self.stream_select.addItem(f"📺 {stream['name']}", stream["id"])
        self.stream_select.setCurrentIndex(max(self.stream_select.findData(self._current_stream), 0))
        self.stream_select.blockSignals(False)

    def _on_stream_selected(self, index: int) -> None:
        stream_id = self.stream_select.itemData(index)
        if stream_id == self._current_stream:
            return
        if stream_id == HOST_STREAM:
            self._switch_to_host()
            return
        stream = next((s for s in self._streams if s["id"] == stream_id), None)
        if stream is None:
            return
        self.video.set_overlay(f"Conectando no compartilhamento de {stream['name']}…")
        viewer = StreamViewer(
            stream["addresses"],
            stream["port"],
            self.room.name,
            self.room.password,
            delay_ms=self.room.delay_ms,
            callback_request=lambda port: self.room.request_stream_callback(stream_id, port),
        )
        viewer.stream_id = stream_id

        def work():
            try:
                viewer.connect()
            except JoinError as exc:
                self.bridge.media_failed.emit(str(exc))
            else:
                self.bridge.media_joined.emit(viewer)

        threading.Thread(target=work, daemon=True).start()

    def _on_media_joined(self, viewer: StreamViewer) -> None:
        old = self.media
        viewer.on_frame = lambda frame: self._on_frame_thread(viewer, frame)
        viewer.on_closed = lambda reason: self.bridge.media_closed.emit(reason)
        viewer.on_status = self.bridge.status.emit
        viewer.on_stats = self.bridge.stats.emit
        viewer.set_auto_delay(self.smooth_auto.isChecked())
        self.media = viewer
        self._current_stream = viewer.stream_id
        self.room.set_media(False)  # para de receber o vídeo do host (economiza banda)
        viewer.start()
        self._apply_audio()
        if old is not None:
            old.stop()
        self.video.set_overlay("")
        self._rebuild_stream_list()

    def _on_media_failed(self, message: str) -> None:
        self.video.set_overlay("")
        self._rebuild_stream_list()
        QMessageBox.warning(self, "Não foi possível assistir", message)

    def _on_media_closed(self, reason: str) -> None:
        self._switch_to_host(reason)

    def _switch_to_host(self, message: str = "") -> None:
        media, self.media = self.media, None
        self._current_stream = HOST_STREAM
        if media is not None:
            media.stop()
        self.room.set_media(True)
        self.video.set_overlay("")
        self._rebuild_stream_list()
        if message:
            self.chat.add_message("", message)

    # -- compartilhar a própria tela ------------------------------------------------
    def _toggle_share(self) -> None:
        if self.share is not None:
            self._stop_share()
            return
        dialog = HostDialog(self.room.name, self.settings, self, mode=MODE_SHARE)
        if dialog.exec() != HostDialog.DialogCode.Accepted:
            return
        settings = dialog.result_settings()
        settings.room_name = self.room.room_name
        settings.password = self.room.password
        settings.announce = False
        settings.port = config.STREAM_PORT + 1
        share = StreamHost(settings, on_error=self.bridge.share_error.emit, on_stats=self.bridge.share_stats.emit)
        try:
            share.start()
        except Exception as exc:  # noqa: BLE001
            QMessageBox.critical(self, "Compartilhar", str(exc))
            share.stop()
            return
        self.share = share
        self.room.announce_stream(share.port, local_addresses())
        self.share_button.setText("⏹  Parar de compartilhar")
        self.share_edit.show()
        self.share_status.setText(f"Compartilhando: {settings.source.label}")

    def _edit_share(self) -> None:
        if self.share is None:
            return
        dialog = HostDialog(self.room.name, self.settings, self, mode=MODE_EDIT, current=self.share.settings)
        if dialog.exec() == HostDialog.DialogCode.Accepted:
            self.share.apply_settings(dialog.result_settings())
            self.share_status.setText(f"Compartilhando: {self.share.settings.source.label}")

    def _stop_share(self) -> None:
        share, self.share = self.share, None
        if share is not None:
            self.room.stop_stream()
            share.stop("Parou de compartilhar")
        self.share_button.setText("📺  Compartilhar minha tela")
        self.share_edit.hide()
        self.share_status.setText("")

    def _on_share_error(self, message: str) -> None:
        self._stop_share()
        QMessageBox.warning(self, "Compartilhamento interrompido", message)

    def _on_share_stats(self, stats: dict) -> None:
        if self.share is None:
            return
        watching = len(self.share.viewer_names())
        text = f"Compartilhando: {self.share.settings.source.label}<br>{stats['fps']:.0f} fps · {stats['kbps']:.0f} kbps · {watching} assistindo"
        if stats.get("idle"):
            text = f"Compartilhando: {self.share.settings.source.label}<br>ninguém assistindo — captura pausada"
        if stats.get("target_kbps", 0) < stats.get("base_kbps", 0) and not stats.get("starting"):
            text += f"<br><span style='color:#c87f0a'>qualidade reduzida p/ {stats['target_kbps']} kbps (conexão lenta)</span>"
        self.share_status.setText(text)

    # -- janela --------------------------------------------------------------------
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
            self.side.setVisible(self._chat_visible and self.room_mode)

    def _on_closed(self, reason: str) -> None:
        self._exit_fullscreen()
        QMessageBox.information(self, "Sala encerrada" if self.room_mode else "Transmissão encerrada", reason)
        self.close()

    def closeEvent(self, event) -> None:
        self._stop_share()
        if self.media is not None:
            self.media.stop()
        self.room.stop()
        super().closeEvent(event)
