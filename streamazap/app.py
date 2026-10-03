"""Ponto de entrada da aplicação gráfica."""

from __future__ import annotations

import logging
import os
import sys
from pathlib import Path

from streamazap import APP_NAME


def _log_path() -> Path:
    base = os.environ.get("LOCALAPPDATA") or os.path.join(Path.home(), ".local", "share")
    path = Path(base) / APP_NAME
    path.mkdir(parents=True, exist_ok=True)
    return path / "streamazap.log"


def main() -> int:
    handlers: list[logging.Handler] = [logging.FileHandler(_log_path(), encoding="utf-8")]
    if sys.stderr is not None:  # no executável sem console o stderr é None
        handlers.append(logging.StreamHandler())
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s", handlers=handlers)

    from PySide6.QtGui import QIcon
    from PySide6.QtWidgets import QApplication

    from streamazap.ui.main_window import MainWindow

    app = QApplication(sys.argv)
    app.setApplicationName(APP_NAME)
    app.setStyle("Fusion")
    icon_path = Path(__file__).resolve().parent / "assets" / "icon.png"
    if icon_path.exists():
        app.setWindowIcon(QIcon(str(icon_path)))
    if sys.platform == "win32":
        # Faz a barra de tarefas usar o ícone do app (e não o do python.exe).
        import ctypes

        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("StreamaZap.App")
    window = MainWindow()
    window.show()
    return app.exec()
