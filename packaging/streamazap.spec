# -*- mode: python ; coding: utf-8 -*-
# Gera dist/StreamaZap/ (pasta com StreamaZap.exe). Rodar a partir da raiz do repositório:
#   pyinstaller packaging/streamazap.spec --noconfirm
import os

from PyInstaller.utils.hooks import collect_dynamic_libs, collect_submodules

hiddenimports = collect_submodules("av") + ["streamazap.capture.audio_win"]
binaries = collect_dynamic_libs("av")

a = Analysis(
    [os.path.join(SPECPATH, "launcher.py")],
    pathex=[os.path.dirname(SPECPATH)],
    binaries=binaries,
    hiddenimports=hiddenimports,
    excludes=["tkinter", "PySide6.QtNetwork", "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtPdf"],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="StreamaZap",
    console=False,
    upx=False,
)
coll = COLLECT(exe, a.binaries, a.datas, name="StreamaZap", upx=False)
