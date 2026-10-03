"""Diálogo de progresso da atualização automática."""

from __future__ import annotations

import threading

from PySide6.QtCore import QObject, Qt, Signal
from PySide6.QtWidgets import QDialog, QLabel, QProgressBar, QPushButton, QVBoxLayout

from streamazap import __version__, updater


class _Signals(QObject):
    progress = Signal(int, int)
    finished = Signal(str)
    failed = Signal(str)


class UpdateDialog(QDialog):
    """Baixa o instalador da nova versão; ao terminar, `installer_path` fica preenchido."""

    def __init__(self, release: updater.Release, parent=None):
        super().__init__(parent)
        self.release = release
        self.installer_path: str | None = None
        self.error: str | None = None
        self._cancel = threading.Event()
        self.setWindowTitle("Atualizando o StreamaZap")
        self.setWindowFlag(Qt.WindowType.WindowContextHelpButtonHint, False)
        self.setMinimumWidth(420)

        self.label = QLabel(f"Baixando a versão <b>{release.version}</b> (você está na {__version__})…")
        self.bar = QProgressBar()
        self.bar.setRange(0, 0)
        cancel = QPushButton("Agora não")
        cancel.clicked.connect(self.reject)
        layout = QVBoxLayout(self)
        layout.addWidget(self.label)
        layout.addWidget(self.bar)
        layout.addWidget(cancel, alignment=Qt.AlignmentFlag.AlignRight)

        self.signals = _Signals()
        self.signals.progress.connect(self._on_progress)
        self.signals.finished.connect(self._on_finished)
        self.signals.failed.connect(self._on_failed)
        threading.Thread(target=self._work, name="update-download", daemon=True).start()

    def _work(self) -> None:
        try:
            path = updater.download(self.release, self.signals.progress.emit, self._cancel)
        except updater.UpdateCancelled:
            return
        except Exception as exc:  # noqa: BLE001 - mostrado ao usuário
            self.signals.failed.emit(str(exc))
            return
        self.signals.finished.emit(str(path))

    def _on_progress(self, done: int, total: int) -> None:
        if total:
            self.bar.setRange(0, 1000)
            self.bar.setValue(int(done * 1000 / total))
            self.bar.setFormat(f"{done / 1e6:.1f} de {total / 1e6:.1f} MB")

    def _on_finished(self, path: str) -> None:
        self.installer_path = path
        self.accept()

    def _on_failed(self, message: str) -> None:
        self.error = message
        self.reject()

    def reject(self) -> None:
        self._cancel.set()
        super().reject()
