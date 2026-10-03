"""Captura de vídeo: monitor inteiro ou uma janela específica.

No Windows usamos GDI (BitBlt / PrintWindow) via ctypes, o que permite capturar
janelas mesmo quando estão atrás de outras e desenhar o cursor do mouse.
Em outros sistemas há apenas captura de monitor via `mss`.
"""

from __future__ import annotations

import ctypes
import os
from dataclasses import dataclass

import numpy as np

from streamazap.config import IS_WINDOWS

try:
    import psutil
except ImportError:  # pragma: no cover
    psutil = None


@dataclass(frozen=True)
class VideoSource:
    kind: str  # "monitor" ou "window"
    ident: int  # índice do monitor (base 1, como no mss) ou HWND
    title: str
    pid: int | None = None
    process_name: str | None = None
    rect: tuple[int, int, int, int] | None = None  # left, top, width, height (monitores)

    @property
    def label(self) -> str:
        if self.kind == "monitor":
            return self.title
        proc = f" — {self.process_name}" if self.process_name else ""
        return f"{self.title}{proc}"


def list_monitors() -> list[VideoSource]:
    import mss

    with mss.mss() as sct:
        monitors = sct.monitors[1:]
    return [
        VideoSource(
            "monitor",
            index,
            f"Tela {index} ({m['width']}x{m['height']})",
            rect=(m["left"], m["top"], m["width"], m["height"]),
        )
        for index, m in enumerate(monitors, start=1)
    ]


def list_windows() -> list[VideoSource]:
    if not IS_WINDOWS:
        return []
    return _win.list_windows()


def create_capturer(source: VideoSource):
    if source.kind == "window":
        if not IS_WINDOWS:
            raise RuntimeError("Captura de janela só é suportada no Windows")
        return _win.WindowCapturer(source.ident)
    if IS_WINDOWS:
        return _win.MonitorCapturer(*source.rect)
    return MssCapturer(source.ident)


class MssCapturer:
    """Captura de monitor portátil (Linux/macOS)."""

    def __init__(self, index: int):
        self._index = index
        self._sct = None

    def grab(self) -> np.ndarray | None:
        import mss

        if self._sct is None:  # o mss precisa ser criado na thread que captura
            self._sct = mss.mss()
        shot = self._sct.grab(self._sct.monitors[self._index])
        return np.asarray(shot)  # BGRA

    def close(self) -> None:
        if self._sct is not None:
            self._sct.close()
            self._sct = None


if IS_WINDOWS:
    from ctypes import wintypes

    class _Win:
        SRCCOPY = 0x00CC0020
        PW_CLIENTONLY = 0x1
        PW_RENDERFULLCONTENT = 0x2
        DI_NORMAL = 0x3
        CURSOR_SHOWING = 0x1
        GW_OWNER = 4
        GWL_EXSTYLE = -20
        WS_EX_TOOLWINDOW = 0x00000080
        DWMWA_CLOAKED = 14

        class BITMAPINFOHEADER(ctypes.Structure):
            _fields_ = [
                ("biSize", wintypes.DWORD),
                ("biWidth", wintypes.LONG),
                ("biHeight", wintypes.LONG),
                ("biPlanes", wintypes.WORD),
                ("biBitCount", wintypes.WORD),
                ("biCompression", wintypes.DWORD),
                ("biSizeImage", wintypes.DWORD),
                ("biXPelsPerMeter", wintypes.LONG),
                ("biYPelsPerMeter", wintypes.LONG),
                ("biClrUsed", wintypes.DWORD),
                ("biClrImportant", wintypes.DWORD),
            ]

        class CURSORINFO(ctypes.Structure):
            _fields_ = [
                ("cbSize", wintypes.DWORD),
                ("flags", wintypes.DWORD),
                ("hCursor", wintypes.HANDLE),
                ("ptScreenPos", wintypes.POINT),
            ]

        class ICONINFO(ctypes.Structure):
            _fields_ = [
                ("fIcon", wintypes.BOOL),
                ("xHotspot", wintypes.DWORD),
                ("yHotspot", wintypes.DWORD),
                ("hbmMask", wintypes.HBITMAP),
                ("hbmColor", wintypes.HBITMAP),
            ]

        def __init__(self):
            self.user32 = ctypes.WinDLL("user32", use_last_error=True)
            self.gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
            self.dwmapi = ctypes.WinDLL("dwmapi")
            u, g = self.user32, self.gdi32
            u.GetDC.restype = wintypes.HDC
            u.GetDC.argtypes = [wintypes.HWND]
            u.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
            u.PrintWindow.argtypes = [wintypes.HWND, wintypes.HDC, wintypes.UINT]
            u.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
            u.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
            u.IsWindow.argtypes = [wintypes.HWND]
            u.IsIconic.argtypes = [wintypes.HWND]
            u.IsWindowVisible.argtypes = [wintypes.HWND]
            u.GetWindow.restype = wintypes.HWND
            u.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
            u.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
            u.GetWindowTextLengthW.argtypes = [wintypes.HWND]
            u.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
            u.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
            u.GetCursorInfo.argtypes = [ctypes.POINTER(self.CURSORINFO)]
            u.GetIconInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(self.ICONINFO)]
            u.DrawIconEx.argtypes = [
                wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.HANDLE,
                ctypes.c_int, ctypes.c_int, wintypes.UINT, wintypes.HBRUSH, wintypes.UINT,
            ]
            g.CreateCompatibleDC.restype = wintypes.HDC
            g.CreateCompatibleDC.argtypes = [wintypes.HDC]
            g.CreateDIBSection.restype = wintypes.HBITMAP
            g.CreateDIBSection.argtypes = [
                wintypes.HDC, ctypes.c_void_p, wintypes.UINT,
                ctypes.POINTER(ctypes.c_void_p), wintypes.HANDLE, wintypes.DWORD,
            ]
            g.SelectObject.restype = wintypes.HGDIOBJ
            g.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
            g.DeleteObject.argtypes = [wintypes.HGDIOBJ]
            g.DeleteDC.argtypes = [wintypes.HDC]
            g.BitBlt.argtypes = [
                wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.DWORD,
            ]
            self.dwmapi.DwmGetWindowAttribute.argtypes = [
                wintypes.HWND, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD,
            ]
            self._hotspots: dict[int, tuple[int, int]] = {}
            self.EnumWindowsProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

        # -- enumeração de janelas ------------------------------------------------
        def _is_cloaked(self, hwnd) -> bool:
            cloaked = ctypes.c_int(0)
            res = self.dwmapi.DwmGetWindowAttribute(hwnd, self.DWMWA_CLOAKED, ctypes.byref(cloaked), 4)
            return res == 0 and cloaked.value != 0

        def window_pid(self, hwnd) -> int:
            pid = wintypes.DWORD()
            self.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            return pid.value

        def list_windows(self) -> list[VideoSource]:
            u = self.user32
            own_pid = os.getpid()
            found: list[VideoSource] = []

            def callback(hwnd, _):
                if not u.IsWindowVisible(hwnd) or u.GetWindow(hwnd, self.GW_OWNER):
                    return True
                if u.GetWindowLongW(hwnd, self.GWL_EXSTYLE) & self.WS_EX_TOOLWINDOW:
                    return True
                length = u.GetWindowTextLengthW(hwnd)
                if length == 0 or self._is_cloaked(hwnd):
                    return True
                buf = ctypes.create_unicode_buffer(length + 1)
                u.GetWindowTextW(hwnd, buf, length + 1)
                pid = self.window_pid(hwnd)
                if pid == own_pid:
                    return True
                name = None
                if psutil is not None:
                    try:
                        name = psutil.Process(pid).name()
                    except (psutil.Error, OSError):
                        pass
                found.append(VideoSource("window", int(hwnd), buf.value, pid=pid, process_name=name))
                return True

            u.EnumWindows(self.EnumWindowsProc(callback), 0)
            return found

        # -- cursor ---------------------------------------------------------------
        def draw_cursor(self, hdc, origin_x: int, origin_y: int) -> None:
            info = self.CURSORINFO()
            info.cbSize = ctypes.sizeof(info)
            if not self.user32.GetCursorInfo(ctypes.byref(info)):
                return
            if not (info.flags & self.CURSOR_SHOWING) or not info.hCursor:
                return
            handle = int(info.hCursor)
            hotspot = self._hotspots.get(handle)
            if hotspot is None:
                icon = self.ICONINFO()
                if self.user32.GetIconInfo(info.hCursor, ctypes.byref(icon)):
                    hotspot = (icon.xHotspot, icon.yHotspot)
                    for bmp in (icon.hbmMask, icon.hbmColor):
                        if bmp:
                            self.gdi32.DeleteObject(bmp)
                else:
                    hotspot = (0, 0)
                self._hotspots[handle] = hotspot
            x = info.ptScreenPos.x - origin_x - hotspot[0]
            y = info.ptScreenPos.y - origin_y - hotspot[1]
            self.user32.DrawIconEx(hdc, x, y, info.hCursor, 0, 0, 0, None, self.DI_NORMAL)

    _win_api = _Win()

    class _DibBuffer:
        """Bitmap DIB 32 bits cujo conteúdo é acessível direto como array numpy."""

        def __init__(self, width: int, height: int):
            self.width, self.height = width, height
            header = _Win.BITMAPINFOHEADER()
            header.biSize = ctypes.sizeof(header)
            header.biWidth = width
            header.biHeight = -height  # top-down
            header.biPlanes = 1
            header.biBitCount = 32
            self.hdc = _win_api.gdi32.CreateCompatibleDC(None)
            bits = ctypes.c_void_p()
            self.hbmp = _win_api.gdi32.CreateDIBSection(self.hdc, ctypes.byref(header), 0, ctypes.byref(bits), None, 0)
            if not self.hbmp:
                _win_api.gdi32.DeleteDC(self.hdc)
                raise OSError("CreateDIBSection falhou")
            self._old = _win_api.gdi32.SelectObject(self.hdc, self.hbmp)
            buf = (ctypes.c_uint8 * (width * height * 4)).from_address(bits.value)
            self.array = np.ctypeslib.as_array(buf).reshape(height, width, 4)

        def snapshot(self) -> np.ndarray:
            frame = self.array.copy()
            frame[:, :, 3] = 255
            return frame

        def close(self) -> None:
            _win_api.gdi32.SelectObject(self.hdc, self._old)
            _win_api.gdi32.DeleteObject(self.hbmp)
            _win_api.gdi32.DeleteDC(self.hdc)

    class _WinMonitorCapturer:
        def __init__(self, left: int, top: int, width: int, height: int):
            self.left, self.top = left, top
            self._dib = _DibBuffer(width, height)

        def grab(self) -> np.ndarray | None:
            screen = _win_api.user32.GetDC(None)
            try:
                _win_api.gdi32.BitBlt(
                    self._dib.hdc, 0, 0, self._dib.width, self._dib.height,
                    screen, self.left, self.top, _Win.SRCCOPY,
                )
            finally:
                _win_api.user32.ReleaseDC(None, screen)
            _win_api.draw_cursor(self._dib.hdc, self.left, self.top)
            return self._dib.snapshot()

        def close(self) -> None:
            self._dib.close()

    class _WinWindowCapturer:
        def __init__(self, hwnd: int):
            self.hwnd = hwnd
            self._dib: _DibBuffer | None = None

        def grab(self) -> np.ndarray | None:
            u = _win_api.user32
            if not u.IsWindow(self.hwnd):
                raise RuntimeError("A janela compartilhada foi fechada")
            if u.IsIconic(self.hwnd):
                return None  # minimizada: mantém o último quadro
            rect = wintypes.RECT()
            u.GetClientRect(self.hwnd, ctypes.byref(rect))
            width, height = rect.right - rect.left, rect.bottom - rect.top
            if width < 2 or height < 2:
                return None
            if self._dib is None or (self._dib.width, self._dib.height) != (width, height):
                if self._dib is not None:
                    self._dib.close()
                self._dib = _DibBuffer(width, height)
            if not u.PrintWindow(self.hwnd, self._dib.hdc, _Win.PW_CLIENTONLY | _Win.PW_RENDERFULLCONTENT):
                return None
            origin = wintypes.POINT(0, 0)
            u.ClientToScreen(self.hwnd, ctypes.byref(origin))
            _win_api.draw_cursor(self._dib.hdc, origin.x, origin.y)
            return self._dib.snapshot()

        def close(self) -> None:
            if self._dib is not None:
                self._dib.close()
                self._dib = None

    class _WinNamespace:
        list_windows = staticmethod(_win_api.list_windows)
        window_pid = staticmethod(_win_api.window_pid)
        MonitorCapturer = _WinMonitorCapturer
        WindowCapturer = _WinWindowCapturer

    _win = _WinNamespace()
