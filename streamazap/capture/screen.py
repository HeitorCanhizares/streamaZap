"""Captura de vídeo: monitor inteiro ou uma janela específica.

No Windows o monitor é capturado pela Desktop Duplication API (DXGI), com o GDI
(BitBlt) de reserva; janelas usam PrintWindow, que as captura mesmo atrás de
outras. O cursor do mouse é desenhado por cima via GDI, tudo por ctypes.
Em outros sistemas há apenas captura de monitor via `mss`.
"""

from __future__ import annotations

import ctypes
import logging
import os
import time
import uuid
from dataclasses import dataclass

import numpy as np

from streamazap.config import IS_WINDOWS

try:
    import psutil
except ImportError:  # pragma: no cover
    psutil = None

log = logging.getLogger(__name__)


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
        try:
            return _win.DxgiMonitorCapturer(*source.rect)
        except OSError as exc:  # sessão remota, driver, monitor girado...
            log.info("Desktop Duplication indisponível (%s); capturando o monitor por BitBlt", exc)
            return _win.MonitorCapturer(*source.rect)
    return MssCapturer(source.ident)


class MssCapturer:
    """Captura de monitor portátil (Linux/macOS)."""

    backend = "mss"

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
            # O alfa (lixo do GDI) não importa: a conversão para YUV no codificador o descarta.
            return self.array.copy()

        def close(self) -> None:
            _win_api.gdi32.SelectObject(self.hdc, self._old)
            _win_api.gdi32.DeleteObject(self.hbmp)
            _win_api.gdi32.DeleteDC(self.hdc)

    class _WinMonitorCapturer:
        backend = "BitBlt"

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

    # --- Desktop Duplication (DXGI + Direct3D 11), vtables COM chamadas direto -------

    _HRESULT = ctypes.HRESULT  # levanta OSError em falha
    _DXGI_ERROR_ACCESS_LOST = 0x887A0026 - (1 << 32)
    _DXGI_ERROR_WAIT_TIMEOUT = 0x887A0027 - (1 << 32)
    _DXGI_FORMAT_B8G8R8A8_UNORM = 87
    _D3D11_USAGE_STAGING = 3
    _D3D11_CPU_ACCESS_READ = 0x20000
    _D3D11_MAP_READ = 1
    _D3D11_SDK_VERSION = 7

    def _iid(text: str):
        return (ctypes.c_ubyte * 16).from_buffer_copy(uuid.UUID(text).bytes_le)

    _IID_IDXGIFactory1 = _iid("770aae78-f26f-4dba-a829-253c83d1b387")
    _IID_IDXGIOutput1 = _iid("00cddea8-939b-4b83-a340-a685226666cc")
    _IID_ID3D11Texture2D = _iid("6f15aaf2-d208-4e89-9ab4-489535d34f9c")

    class _DXGI_OUTPUT_DESC(ctypes.Structure):
        _fields_ = [
            ("DeviceName", wintypes.WCHAR * 32),
            ("DesktopCoordinates", wintypes.RECT),
            ("AttachedToDesktop", wintypes.BOOL),
            ("Rotation", wintypes.UINT),
            ("Monitor", wintypes.HANDLE),
        ]

    class _DXGI_OUTDUPL_DESC(ctypes.Structure):
        _fields_ = [
            ("Width", wintypes.UINT),  # DXGI_MODE_DESC
            ("Height", wintypes.UINT),
            ("RefreshNumerator", wintypes.UINT),
            ("RefreshDenominator", wintypes.UINT),
            ("Format", wintypes.UINT),
            ("ScanlineOrdering", wintypes.UINT),
            ("Scaling", wintypes.UINT),
            ("Rotation", wintypes.UINT),
            ("DesktopImageInSystemMemory", wintypes.BOOL),
        ]

    class _DXGI_OUTDUPL_FRAME_INFO(ctypes.Structure):
        _fields_ = [
            ("LastPresentTime", ctypes.c_longlong),
            ("LastMouseUpdateTime", ctypes.c_longlong),
            ("AccumulatedFrames", wintypes.UINT),
            ("RectsCoalesced", wintypes.BOOL),
            ("ProtectedContentMaskedOut", wintypes.BOOL),
            ("PointerPosition", wintypes.POINT),
            ("PointerVisible", wintypes.BOOL),
            ("TotalMetadataBufferSize", wintypes.UINT),
            ("PointerShapeBufferSize", wintypes.UINT),
        ]

    class _D3D11_TEXTURE2D_DESC(ctypes.Structure):
        _fields_ = [
            ("Width", wintypes.UINT),
            ("Height", wintypes.UINT),
            ("MipLevels", wintypes.UINT),
            ("ArraySize", wintypes.UINT),
            ("Format", wintypes.UINT),
            ("SampleCount", wintypes.UINT),
            ("SampleQuality", wintypes.UINT),
            ("Usage", wintypes.UINT),
            ("BindFlags", wintypes.UINT),
            ("CPUAccessFlags", wintypes.UINT),
            ("MiscFlags", wintypes.UINT),
        ]

    class _D3D11_MAPPED_SUBRESOURCE(ctypes.Structure):
        _fields_ = [("pData", ctypes.c_void_p), ("RowPitch", wintypes.UINT), ("DepthPitch", wintypes.UINT)]

    def _com(ptr, index: int, restype, *argtypes):
        """Método `index` da vtable do objeto COM `ptr`."""
        vtable = ctypes.cast(ptr, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
        func = ctypes.WINFUNCTYPE(restype, ctypes.c_void_p, *argtypes)(vtable[index])
        return lambda *args: func(ptr, *args)

    def _com_release(ptr) -> None:
        if ptr:
            _com(ptr, 2, wintypes.ULONG)()

    def _com_query(ptr, iid) -> ctypes.c_void_p:
        out = ctypes.c_void_p()
        _com(ptr, 0, _HRESULT, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(ctypes.byref(iid), ctypes.byref(out))
        return out

    class _DxgiMonitorCapturer:
        """Captura de monitor pela Desktop Duplication API.

        O BitBlt da tela faz o DWM copiar a imagem da GPU para a CPU a cada quadro e costuma
        ser o passo mais lento da transmissão. Aqui a GPU entrega a imagem já composta, e só
        quando algo muda. O cursor não vem na imagem: é desenhado por cima com o código do GDI.
        Qualquer falha cai para o BitBlt (perder o acesso, ex.: aviso do UAC, é temporário).
        """

        @property
        def backend(self) -> str:
            return "BitBlt" if self._broken or not self._dupl else "DXGI"

        def __init__(self, left: int, top: int, width: int, height: int):
            self._gdi = _WinMonitorCapturer(left, top, width, height)  # reserva e dono do DIB
            self._device = ctypes.c_void_p()
            self._context = ctypes.c_void_p()
            self._output = ctypes.c_void_p()  # IDXGIOutput1
            self._dupl = ctypes.c_void_p()
            self._staging = ctypes.c_void_p()
            self._staged = False  # o staging já tem uma imagem da GPU
            self._broken = False
            self._retry_at = 0.0
            try:
                self._open()
            except BaseException:
                self.close()
                raise

        def _open(self) -> None:
            dxgi, d3d11 = ctypes.WinDLL("dxgi"), ctypes.WinDLL("d3d11")
            dxgi.CreateDXGIFactory1.restype = _HRESULT
            dxgi.CreateDXGIFactory1.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
            d3d11.D3D11CreateDevice.restype = _HRESULT
            d3d11.D3D11CreateDevice.argtypes = [
                ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, wintypes.UINT, ctypes.c_void_p, wintypes.UINT,
                wintypes.UINT, ctypes.POINTER(ctypes.c_void_p), ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p),
            ]
            factory = ctypes.c_void_p()
            dxgi.CreateDXGIFactory1(ctypes.byref(_IID_IDXGIFactory1), ctypes.byref(factory))
            try:
                adapter, output = self._find_output(factory)
            finally:
                _com_release(factory)
            try:
                # O dispositivo precisa ser da placa que controla o monitor (notebooks com 2 GPUs).
                d3d11.D3D11CreateDevice(
                    adapter, 0, None, 0, None, 0, _D3D11_SDK_VERSION,
                    ctypes.byref(self._device), None, ctypes.byref(self._context),
                )
                self._output = _com_query(output, _IID_IDXGIOutput1)
            finally:
                _com_release(output)
                _com_release(adapter)
            ctx = self._context
            self._copy_resource = _com(ctx, 47, None, ctypes.c_void_p, ctypes.c_void_p)
            self._map = _com(ctx, 14, _HRESULT, ctypes.c_void_p, wintypes.UINT, ctypes.c_int, wintypes.UINT,
                             ctypes.POINTER(_D3D11_MAPPED_SUBRESOURCE))
            self._unmap = _com(ctx, 15, None, ctypes.c_void_p, wintypes.UINT)
            self._duplicate()

        def _find_output(self, factory):
            g = self._gdi
            target = (g.left, g.top, g.left + g._dib.width, g.top + g._dib.height)
            enum_adapters = _com(factory, 12, ctypes.c_long, wintypes.UINT, ctypes.POINTER(ctypes.c_void_p))
            for i in range(16):
                adapter = ctypes.c_void_p()
                if enum_adapters(i, ctypes.byref(adapter)) < 0:
                    break
                enum_outputs = _com(adapter, 7, ctypes.c_long, wintypes.UINT, ctypes.POINTER(ctypes.c_void_p))
                for j in range(16):
                    output = ctypes.c_void_p()
                    if enum_outputs(j, ctypes.byref(output)) < 0:
                        break
                    r = self._output_desc(output).DesktopCoordinates
                    if (r.left, r.top, r.right, r.bottom) == target:
                        return adapter, output
                    _com_release(output)
                _com_release(adapter)
            raise OSError(f"monitor {target} não encontrado entre as saídas DXGI")

        @staticmethod
        def _output_desc(output) -> _DXGI_OUTPUT_DESC:
            desc = _DXGI_OUTPUT_DESC()
            _com(output, 7, _HRESULT, ctypes.POINTER(_DXGI_OUTPUT_DESC))(ctypes.byref(desc))
            return desc

        def _duplicate(self) -> None:
            """(Re)cria a duplicação; acompanha mudança de resolução do monitor."""
            self._release_duplication()
            try:
                self._setup_duplication()
            except BaseException:
                self._release_duplication()  # nunca fica uma duplicação pela metade
                raise

        def _setup_duplication(self) -> None:
            dupl = ctypes.c_void_p()
            _com(self._output, 22, _HRESULT, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(
                self._device, ctypes.byref(dupl)
            )
            self._dupl = dupl
            desc = _DXGI_OUTDUPL_DESC()
            _com(dupl, 7, None, ctypes.POINTER(_DXGI_OUTDUPL_DESC))(ctypes.byref(desc))
            if desc.Rotation > 1:  # 0 = não informado, 1 = normal
                raise OSError("monitor girado")
            r = self._output_desc(self._output).DesktopCoordinates
            rect = (r.left, r.top, desc.Width, desc.Height)
            g = self._gdi
            if rect != (g.left, g.top, g._dib.width, g._dib.height):
                self._gdi = _WinMonitorCapturer(*rect)
                g.close()
            texture = _D3D11_TEXTURE2D_DESC(
                Width=desc.Width, Height=desc.Height, MipLevels=1, ArraySize=1, Format=_DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleCount=1, Usage=_D3D11_USAGE_STAGING, CPUAccessFlags=_D3D11_CPU_ACCESS_READ,
            )
            _com(self._device, 5, _HRESULT, ctypes.POINTER(_D3D11_TEXTURE2D_DESC), ctypes.c_void_p,
                 ctypes.POINTER(ctypes.c_void_p))(ctypes.byref(texture), None, ctypes.byref(self._staging))
            self._acquire = _com(dupl, 8, ctypes.c_long, wintypes.UINT, ctypes.POINTER(_DXGI_OUTDUPL_FRAME_INFO),
                                 ctypes.POINTER(ctypes.c_void_p))
            self._release_frame = _com(dupl, 14, ctypes.c_long)

        def _release_duplication(self) -> None:
            _com_release(self._staging)
            _com_release(self._dupl)
            self._staging, self._dupl = ctypes.c_void_p(), ctypes.c_void_p()
            self._staged = False

        def grab(self) -> np.ndarray | None:
            if not self._broken:
                try:
                    frame = self._grab_dxgi()
                except OSError as exc:
                    log.warning("captura DXGI falhou (%s); usando BitBlt", exc)
                    self._broken = True
                    self._release_duplication()
                else:
                    if frame is not None:
                        return frame
            return self._gdi.grab()

        def _grab_dxgi(self) -> np.ndarray | None:
            """Quadro pela GPU, ou None para usar o BitBlt desta vez."""
            if not self._dupl:
                if time.monotonic() < self._retry_at:
                    return None
                try:
                    self._duplicate()
                except OSError as exc:  # ex.: área de trabalho segura do UAC aberta
                    log.debug("Desktop Duplication indisponível por enquanto: %s", exc)
                    self._retry_at = time.monotonic() + 1.0
                    return None
            info = _DXGI_OUTDUPL_FRAME_INFO()
            resource = ctypes.c_void_p()
            hr = self._acquire(0, ctypes.byref(info), ctypes.byref(resource))
            dib = self._gdi._dib
            if hr == _DXGI_ERROR_WAIT_TIMEOUT:
                # Nada mudou, nem o mouse: o DIB já tem a tela atual (a não ser que nunca
                # tenha chegado imagem da GPU, como na tela parada logo após começar).
                return dib.snapshot() if self._staged else None
            if hr == _DXGI_ERROR_ACCESS_LOST:  # troca de resolução, UAC, jogo em tela cheia
                self._release_duplication()
                return None
            if hr < 0:
                raise OSError(f"AcquireNextFrame: HRESULT 0x{hr & 0xFFFFFFFF:08X}")
            try:
                if info.LastPresentTime:  # imagem nova (0 = só o mouse mexeu)
                    texture = _com_query(resource, _IID_ID3D11Texture2D)
                    try:
                        self._copy_resource(self._staging, texture)
                    finally:
                        _com_release(texture)
                    self._staged = True
            finally:
                _com_release(resource)
                self._release_frame()
            if not self._staged:
                return None
            mapped = _D3D11_MAPPED_SUBRESOURCE()
            self._map(self._staging, 0, _D3D11_MAP_READ, 0, ctypes.byref(mapped))
            try:
                height, row = dib.height, dib.width * 4
                src = (ctypes.c_uint8 * (mapped.RowPitch * height)).from_address(mapped.pData)
                np.copyto(dib.array.reshape(height, row), np.frombuffer(src, np.uint8).reshape(height, mapped.RowPitch)[:, :row])
            finally:
                self._unmap(self._staging, 0)
            _win_api.draw_cursor(dib.hdc, self._gdi.left, self._gdi.top)
            return dib.snapshot()

        def close(self) -> None:
            self._release_duplication()
            for name in ("_output", "_context", "_device"):
                _com_release(getattr(self, name))
                setattr(self, name, ctypes.c_void_p())
            self._gdi.close()

    class _WinWindowCapturer:
        backend = "PrintWindow"

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
        DxgiMonitorCapturer = _DxgiMonitorCapturer
        WindowCapturer = _WinWindowCapturer

    _win = _WinNamespace()
