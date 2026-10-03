"""Diálogo de transmissão: escolha de tela/janela, som e qualidade.

Modos:
  * MODE_CREATE: criar sala (nome, senha + mídia)
  * MODE_EDIT:   editar a sala ao vivo (pré-preenchido)
  * MODE_SHARE:  participante compartilhando dentro da sala de outra pessoa
"""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QButtonGroup,
    QCheckBox,
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

MODE_CREATE = "create"
MODE_EDIT = "edit"
MODE_SHARE = "share"

_TITLES = {MODE_CREATE: "Criar sala", MODE_EDIT: "Editar transmissão", MODE_SHARE: "Compartilhar minha tela"}
_OK_TEXT = {MODE_CREATE: "Iniciar transmissão", MODE_EDIT: "Aplicar", MODE_SHARE: "Compartilhar"}


class HostDialog(QDialog):
    def __init__(self, host_name: str, settings, parent=None, mode: str = MODE_CREATE, current: HostSettings | None = None):
        super().__init__(parent)
        self.setWindowTitle(_TITLES[mode])
        self.resize(640, 700)
        self._qsettings = settings
        self._host_name = host_name
        self._mode = mode
        self._current = current
        # Nome/senha só existem para quem é dono da sala (não para participante compartilhando).
        self._room_fields = mode != MODE_SHARE and (current is None or current.announce)

        self.room_name = QLineEdit(current.room_name if current else settings.value("room_name", f"Sala de {host_name}"))
        self.password = QLineEdit(current.password if current else "")
        self.password.setEchoMode(QLineEdit.EchoMode.Password)
        self.password.setPlaceholderText("Opcional")
        form = QFormLayout()
        if self._room_fields:
            form.addRow("Nome da sala:", self.room_name)
            form.addRow("Senha:", self.password)
        if mode == MODE_EDIT:
            note = QLabel("As mudanças valem na hora — quem está assistindo continua conectado.")
            note.setStyleSheet("color: gray")
            form.addRow(note)

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
        if current is not None:
            mode_value = current.audio_mode
        else:
            mode_value = settings.value("audio_mode", AUDIO_SYSTEM) if audio_supported() else AUDIO_NONE
        if not audio_supported():
            mode_value = AUDIO_NONE
        {AUDIO_NONE: self.audio_none, AUDIO_SYSTEM: self.audio_system, AUDIO_APPS: self.audio_apps_radio}.get(
            mode_value, self.audio_system
        ).setChecked(True)
        self.audio_apps.itemChanged.connect(lambda _: self.audio_apps_radio.setChecked(True))

        # Qualidade -------------------------------------------------------------
        self.quality = QComboBox()
        for label, *_ in config.QUALITY_PRESETS:
            self.quality.addItem(label)
        self.quality.setCurrentIndex(self._initial_quality_index())
        self.encoder = QComboBox()
        for key, label in config.ENCODER_CHOICES:
            self.encoder.addItem(label, key)
        encoder_value = current.encoder if current else settings.value("encoder", config.ENCODER_AUTO)
        self.encoder.setCurrentIndex(max(self.encoder.findData(encoder_value), 0))
        self.adaptive = QCheckBox("Qualidade adaptável — reduz sozinha se alguém estiver com a conexão lenta")
        self.adaptive.setChecked(current.adaptive if current else settings.value("adaptive", True, type=bool))
        qform = QFormLayout()
        qform.addRow("Qualidade:", self.quality)
        qform.addRow("Codificador:", self.encoder)
        qform.addRow(self.adaptive)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText(_OK_TEXT[mode])
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

    def _initial_quality_index(self) -> int:
        if self._current is not None:
            key = (self._current.max_height, self._current.fps, self._current.bitrate)
            for index, (_, max_height, fps, bitrate) in enumerate(config.QUALITY_PRESETS):
                if (max_height, fps, bitrate) == key:
                    return index
        index = int(self._qsettings.value("quality", 0))
        return index if 0 <= index < len(config.QUALITY_PRESETS) else 0

    def _load_sources(self) -> None:
        selected = self.sources.currentItem().data(Qt.ItemDataRole.UserRole) if self.sources.currentItem() else None
        if selected is None and self._current is not None:
            selected = self._current.source
        self.sources.blockSignals(True)
        self.sources.clear()
        row = 0
        windows = list_windows()
        for index, source in enumerate(list_monitors() + windows):
            prefix = "🖥  " if source.kind == "monitor" else "🗔  "
            item = QListWidgetItem(prefix + source.label)
            item.setData(Qt.ItemDataRole.UserRole, source)
            self.sources.addItem(item)
            if selected is not None and (source.kind, source.ident) == (selected.kind, selected.ident):
                row = index
        self.sources.blockSignals(False)
        self._load_apps(windows)
        if self.sources.count():
            self.sources.setCurrentRow(row)

    def _load_apps(self, windows: list) -> None:
        checked = {
            self.audio_apps.item(i).data(Qt.ItemDataRole.UserRole).name
            for i in range(self.audio_apps.count())
            if self.audio_apps.item(i).checkState() == Qt.CheckState.Checked
        }
        if not checked and self._current is not None:
            checked = {app.name for app in self._current.audio_apps}
        self.audio_apps.blockSignals(True)
        self.audio_apps.clear()
        if audio_supported():
            for app in list_audio_apps(windows):
                item = QListWidgetItem(app.label)
                item.setData(Qt.ItemDataRole.UserRole, app)
                item.setFlags(item.flags() | Qt.ItemFlag.ItemIsUserCheckable)
                item.setCheckState(Qt.CheckState.Checked if app.name in checked else Qt.CheckState.Unchecked)
                self.audio_apps.addItem(item)
        self.audio_apps.blockSignals(False)

    def _on_source_changed(self, current, previous) -> None:
        """Ao escolher uma janela, sugere compartilhar só o som daquele aplicativo."""
        if current is None or previous is None or not audio_supported():
            return  # previous None = carga inicial: mantém a escolha salva
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
            QMessageBox.warning(self, self.windowTitle(), "Escolha uma tela ou janela para compartilhar.")
            return
        if self.audio_apps_radio.isChecked() and not self._checked_apps():
            QMessageBox.warning(self, self.windowTitle(), "Marque pelo menos um aplicativo ou escolha outra opção de som.")
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
            audio_mode = AUDIO_SYSTEM
        elif self.audio_apps_radio.isChecked():
            audio_mode = AUDIO_APPS
        else:
            audio_mode = AUDIO_NONE
        _, max_height, fps, bitrate = config.QUALITY_PRESETS[self.quality.currentIndex()]
        name = self.room_name.text().strip() or f"Sala de {self._host_name}"
        if self._mode == MODE_CREATE:
            self._qsettings.setValue("room_name", name)
        self._qsettings.setValue("audio_mode", audio_mode)
        self._qsettings.setValue("quality", self.quality.currentIndex())
        self._qsettings.setValue("encoder", self.encoder.currentData())
        self._qsettings.setValue("adaptive", self.adaptive.isChecked())
        # Participante: o chamador preenche sala/senha (as da sala em que ele está).
        return HostSettings(
            room_name=name if self._room_fields else "",
            host_name=self._host_name,
            password=self.password.text() if self._room_fields else "",
            source=self.sources.currentItem().data(Qt.ItemDataRole.UserRole),
            max_height=max_height,
            fps=fps,
            bitrate=bitrate,
            encoder=self.encoder.currentData(),
            audio_mode=audio_mode,
            audio_apps=self._checked_apps() if audio_mode == AUDIO_APPS else [],
            adaptive=self.adaptive.isChecked(),
            announce=self._room_fields,
        )
