"""Diálogo para criar uma sala: escolha de tela/janela, som e qualidade."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QButtonGroup,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QRadioButton,
    QVBoxLayout,
)

from streamazap import config
from streamazap.capture.audio import AUDIO_APPS, AUDIO_NONE, AUDIO_SYSTEM, audio_supported, list_audio_apps
from streamazap.capture.screen import list_monitors, list_windows
from streamazap.host import HostSettings
from streamazap.media.video import ENCODER_CHOICES


class HostDialog(QDialog):
    def __init__(self, host_name: str, settings, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Criar sala")
        self.resize(640, 680)
        self._qsettings = settings
        self._host_name = host_name

        self.room_name = QLineEdit(settings.value("room_name", f"Sala de {host_name}"))
        self.password = QLineEdit()
        self.password.setEchoMode(QLineEdit.EchoMode.Password)
        self.password.setPlaceholderText("Opcional")
        form = QFormLayout()
        form.addRow("Nome da sala:", self.room_name)
        form.addRow("Senha:", self.password)

        # Vídeo ---------------------------------------------------------------
        self.sources = QListWidget()
        refresh = QPushButton("Atualizar lista")
        refresh.clicked.connect(self._load_sources)
        video_box = QGroupBox("O que compartilhar (tela inteira ou uma janela)")
        vbox = QVBoxLayout(video_box)
        vbox.addWidget(self.sources)
        vbox.addWidget(refresh, alignment=Qt.AlignmentFlag.AlignRight)

        # Áudio ---------------------------------------------------------------
        self.audio_none = QRadioButton("Sem som")
        self.audio_system = QRadioButton("Todo o som do PC")
        self.audio_apps_radio = QRadioButton("Somente os aplicativos marcados:")
        self.audio_group = QButtonGroup(self)
        for button in (self.audio_none, self.audio_system, self.audio_apps_radio):
            self.audio_group.addButton(button)
        self.audio_apps = QListWidget()
        audio_box = QGroupBox("Som")
        abox = QVBoxLayout(audio_box)
        row = QHBoxLayout()
        row.addWidget(self.audio_none)
        row.addWidget(self.audio_system)
        abox.addLayout(row)
        abox.addWidget(self.audio_apps_radio)
        abox.addWidget(self.audio_apps)
        if not audio_supported():
            note = QLabel("Captura de som requer Windows 10 (versão 2004) ou mais recente.")
            note.setStyleSheet("color: #c60")
            abox.addWidget(note)
            for widget in (self.audio_system, self.audio_apps_radio, self.audio_apps):
                widget.setEnabled(False)
        mode = settings.value("audio_mode", AUDIO_SYSTEM) if audio_supported() else AUDIO_NONE
        {AUDIO_NONE: self.audio_none, AUDIO_SYSTEM: self.audio_system, AUDIO_APPS: self.audio_apps_radio}.get(
            mode, self.audio_system
        ).setChecked(True)
        self.audio_apps.itemChanged.connect(lambda _: self.audio_apps_radio.setChecked(True))

        # Qualidade -------------------------------------------------------------
        self.quality = QComboBox()
        for label, *_ in config.QUALITY_PRESETS:
            self.quality.addItem(label)
        self.quality.setCurrentIndex(int(settings.value("quality", 0)))
        self.encoder = QComboBox()
        for key, label in ENCODER_CHOICES:
            self.encoder.addItem(label, key)
        index = self.encoder.findData(settings.value("encoder", ENCODER_CHOICES[0][0]))
        self.encoder.setCurrentIndex(max(index, 0))
        qform = QFormLayout()
        qform.addRow("Qualidade:", self.quality)
        qform.addRow("Codificador:", self.encoder)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText("Iniciar transmissão")
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout(self)
        layout.addLayout(form)
        layout.addWidget(video_box, 3)
        layout.addWidget(audio_box, 2)
        layout.addLayout(qform)
        layout.addWidget(buttons)

        self.sources.currentItemChanged.connect(self._on_source_changed)
        self._load_sources()

    def _load_sources(self) -> None:
        self.sources.clear()
        for source in list_monitors() + list_windows():
            prefix = "🖥  " if source.kind == "monitor" else "🗔  "
            item = QListWidgetItem(prefix + source.label)
            item.setData(Qt.ItemDataRole.UserRole, source)
            self.sources.addItem(item)
        if self.sources.count():
            self.sources.setCurrentRow(0)
        self._load_apps()

    def _load_apps(self) -> None:
        checked = {self.audio_apps.item(i).data(Qt.ItemDataRole.UserRole).name
                   for i in range(self.audio_apps.count())
                   if self.audio_apps.item(i).checkState() == Qt.CheckState.Checked}
        self.audio_apps.blockSignals(True)
        self.audio_apps.clear()
        if audio_supported():
            for app in list_audio_apps():
                item = QListWidgetItem(app.label)
                item.setData(Qt.ItemDataRole.UserRole, app)
                item.setFlags(item.flags() | Qt.ItemFlag.ItemIsUserCheckable)
                item.setCheckState(Qt.CheckState.Checked if app.name in checked else Qt.CheckState.Unchecked)
                self.audio_apps.addItem(item)
        self.audio_apps.blockSignals(False)

    def _on_source_changed(self, current, _previous) -> None:
        """Ao escolher uma janela, sugere compartilhar só o som daquele aplicativo."""
        if current is None or not audio_supported():
            return
        source = current.data(Qt.ItemDataRole.UserRole)
        if source.kind != "window" or not source.process_name:
            return
        self.audio_apps.blockSignals(True)
        for i in range(self.audio_apps.count()):
            item = self.audio_apps.item(i)
            match = item.data(Qt.ItemDataRole.UserRole).name == source.process_name.lower()
            item.setCheckState(Qt.CheckState.Checked if match else Qt.CheckState.Unchecked)
        self.audio_apps.blockSignals(False)
        self.audio_apps_radio.setChecked(True)

    def accept(self) -> None:
        if self.sources.currentItem() is None:
            QMessageBox.warning(self, "Criar sala", "Escolha uma tela ou janela para compartilhar.")
            return
        if self.audio_apps_radio.isChecked() and not self._checked_apps():
            QMessageBox.warning(self, "Criar sala", "Marque pelo menos um aplicativo ou escolha outra opção de som.")
            return
        super().accept()

    def _checked_apps(self):
        return [
            self.audio_apps.item(i).data(Qt.ItemDataRole.UserRole)
            for i in range(self.audio_apps.count())
            if self.audio_apps.item(i).checkState() == Qt.CheckState.Checked
        ]

    def result_settings(self) -> HostSettings:
        if self.audio_system.isChecked():
            mode = AUDIO_SYSTEM
        elif self.audio_apps_radio.isChecked():
            mode = AUDIO_APPS
        else:
            mode = AUDIO_NONE
        _, max_height, fps, bitrate = config.QUALITY_PRESETS[self.quality.currentIndex()]
        name = self.room_name.text().strip() or f"Sala de {self._host_name}"
        self._qsettings.setValue("room_name", name)
        self._qsettings.setValue("audio_mode", mode)
        self._qsettings.setValue("quality", self.quality.currentIndex())
        self._qsettings.setValue("encoder", self.encoder.currentData())
        return HostSettings(
            room_name=name,
            host_name=self._host_name,
            password=self.password.text(),
            source=self.sources.currentItem().data(Qt.ItemDataRole.UserRole),
            max_height=max_height,
            fps=fps,
            bitrate=bitrate,
            encoder=self.encoder.currentData(),
            audio_mode=mode,
            audio_apps=self._checked_apps() if mode == AUDIO_APPS else [],
        )
