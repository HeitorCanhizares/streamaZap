"""Captura de áudio por processo no Windows (WASAPI "process loopback").

Disponível no Windows 10 2004 (build 19041) ou superior. Permite:
  * capturar só o som de um aplicativo (e seus processos filhos);
  * capturar todo o som do sistema EXCETO o do próprio StreamaZap
    (evita eco quando o host também está assistindo a outra sala).

Implementado com ctypes puro chamando as vtables COM diretamente.
"""

from __future__ import annotations

import ctypes
import logging
import threading
import uuid
from ctypes import wintypes

from streamazap import config

log = logging.getLogger(__name__)

ole32 = ctypes.WinDLL("ole32")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
mmdevapi = ctypes.WinDLL("Mmdevapi")

HRESULT = ctypes.HRESULT
S_OK = 0
E_NOINTERFACE = 0x80004002 - (1 << 32)
COINIT_MULTITHREADED = 0x0

AUDCLNT_SHAREMODE_SHARED = 0
AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000
AUDCLNT_STREAMFLAGS_EVENTCALLBACK = 0x00040000
AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM = 0x80000000
AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY = 0x08000000
AUDCLNT_BUFFERFLAGS_SILENT = 0x2

AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK = 1
PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE = 0
PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE = 1
VT_BLOB = 65
VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK = "VAD\\Process_Loopback"
WAVE_FORMAT_PCM = 1
WAIT_OBJECT_0 = 0


class GUID(ctypes.Structure):
    _fields_ = [("Data1", wintypes.DWORD), ("Data2", wintypes.WORD), ("Data3", wintypes.WORD), ("Data4", ctypes.c_ubyte * 8)]

    @classmethod
    def from_str(cls, text: str) -> GUID:
        return cls.from_buffer_copy(uuid.UUID(text).bytes_le)

    def __eq__(self, other) -> bool:
        return bytes(self) == bytes(other)


IID_IUnknown = GUID.from_str("00000000-0000-0000-C000-000000000046")
IID_IAgileObject = GUID.from_str("94ea2b94-e9cc-49e0-c0ff-ee64ca8f5b90")
IID_IActivateAudioInterfaceCompletionHandler = GUID.from_str("41D949AB-9862-444A-80F6-C261334DA5EB")
IID_IAudioClient = GUID.from_str("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2")
IID_IAudioCaptureClient = GUID.from_str("C8ADBD64-E71E-48a0-A4DE-185C395CD317")


class AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS(ctypes.Structure):
    _fields_ = [("TargetProcessId", wintypes.DWORD), ("ProcessLoopbackMode", ctypes.c_int)]


class AUDIOCLIENT_ACTIVATION_PARAMS(ctypes.Structure):
    _fields_ = [("ActivationType", ctypes.c_int), ("ProcessLoopbackParams", AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS)]


class BLOB(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.ULONG), ("pBlobData", ctypes.c_void_p)]


class PROPVARIANT(ctypes.Structure):
    _fields_ = [
        ("vt", ctypes.c_ushort),
        ("wReserved1", ctypes.c_ushort),
        ("wReserved2", ctypes.c_ushort),
        ("wReserved3", ctypes.c_ushort),
        ("blob", BLOB),
    ]


class WAVEFORMATEX(ctypes.Structure):
    _fields_ = [
        ("wFormatTag", wintypes.WORD),
        ("nChannels", wintypes.WORD),
        ("nSamplesPerSec", wintypes.DWORD),
        ("nAvgBytesPerSec", wintypes.DWORD),
        ("nBlockAlign", wintypes.WORD),
        ("wBitsPerSample", wintypes.WORD),
        ("cbSize", wintypes.WORD),
    ]


def _method(ptr: ctypes.c_void_p, index: int, *argtypes):
    """Retorna a função da posição `index` na vtable do objeto COM `ptr`."""
    vtable = ctypes.cast(ptr, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
    proto = ctypes.WINFUNCTYPE(HRESULT, ctypes.c_void_p, *argtypes)
    func = proto(vtable[index])
    return lambda *args: func(ptr, *args)


def _release(ptr: ctypes.c_void_p) -> None:
    if ptr:
        vtable = ctypes.cast(ptr, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
        ctypes.WINFUNCTYPE(wintypes.ULONG, ctypes.c_void_p)(vtable[2])(ptr)


# --- objeto COM IActivateAudioInterfaceCompletionHandler feito à mão -------------

_QI = ctypes.WINFUNCTYPE(HRESULT, ctypes.c_void_p, ctypes.POINTER(GUID), ctypes.POINTER(ctypes.c_void_p))
_ADDREF = ctypes.WINFUNCTYPE(wintypes.ULONG, ctypes.c_void_p)
_RELEASE = ctypes.WINFUNCTYPE(wintypes.ULONG, ctypes.c_void_p)
_COMPLETED = ctypes.WINFUNCTYPE(HRESULT, ctypes.c_void_p, ctypes.c_void_p)


class _HandlerVtbl(ctypes.Structure):
    _fields_ = [("QueryInterface", _QI), ("AddRef", _ADDREF), ("Release", _RELEASE), ("ActivateCompleted", _COMPLETED)]


class _HandlerObject(ctypes.Structure):
    _fields_ = [("lpVtbl", ctypes.POINTER(_HandlerVtbl))]


class _CompletionHandler:
    """Implementa IActivateAudioInterfaceCompletionHandler + IAgileObject.

    O objeto Python é mantido vivo pelo chamador até a ativação terminar,
    então AddRef/Release apenas retornam contagens fictícias.
    """

    def __init__(self):
        self.done = threading.Event()
        self.operation = ctypes.c_void_p()
        self._vtbl = _HandlerVtbl(_QI(self._qi), _ADDREF(lambda this: 1), _RELEASE(lambda this: 1), _COMPLETED(self._completed))
        self.obj = _HandlerObject(ctypes.pointer(self._vtbl))

    def _qi(self, this, riid, ppv):
        iid = riid.contents
        if iid == IID_IUnknown or iid == IID_IAgileObject or iid == IID_IActivateAudioInterfaceCompletionHandler:
            ppv[0] = this
            return S_OK
        ppv[0] = None
        return E_NOINTERFACE

    def _completed(self, this, operation):
        self.operation = ctypes.c_void_p(operation)
        # Mantém a operação viva até lermos o resultado.
        vtable = ctypes.cast(self.operation, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))[0]
        ctypes.WINFUNCTYPE(wintypes.ULONG, ctypes.c_void_p)(vtable[1])(self.operation)
        self.done.set()
        return S_OK


# O Windows pode chamar Release no handler depois que a ativação termina;
# mantemos os objetos vivos para nunca liberar memória ainda referenciada.
_handlers: list[_CompletionHandler] = []

mmdevapi.ActivateAudioInterfaceAsync.restype = HRESULT
mmdevapi.ActivateAudioInterfaceAsync.argtypes = [
    wintypes.LPCWSTR,
    ctypes.POINTER(GUID),
    ctypes.POINTER(PROPVARIANT),
    ctypes.c_void_p,
    ctypes.POINTER(ctypes.c_void_p),
]
ole32.CoInitializeEx.argtypes = [ctypes.c_void_p, wintypes.DWORD]
ole32.CoInitializeEx.restype = ctypes.c_long  # não levanta exceção (S_FALSE/RPC_E_CHANGED_MODE são aceitáveis)
kernel32.CreateEventW.restype = wintypes.HANDLE
kernel32.CreateEventW.argtypes = [ctypes.c_void_p, wintypes.BOOL, wintypes.BOOL, wintypes.LPCWSTR]
kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel32.WaitForSingleObject.restype = wintypes.DWORD
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]


def _activate_process_loopback(pid: int, include_tree: bool) -> ctypes.c_void_p:
    params = AUDIOCLIENT_ACTIVATION_PARAMS()
    params.ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK
    params.ProcessLoopbackParams.TargetProcessId = pid
    params.ProcessLoopbackParams.ProcessLoopbackMode = (
        PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE if include_tree else PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE
    )
    prop = PROPVARIANT()
    prop.vt = VT_BLOB
    prop.blob.cbSize = ctypes.sizeof(params)
    prop.blob.pBlobData = ctypes.cast(ctypes.pointer(params), ctypes.c_void_p)

    handler = _CompletionHandler()
    _handlers.append(handler)
    async_op = ctypes.c_void_p()
    mmdevapi.ActivateAudioInterfaceAsync(
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
        ctypes.byref(IID_IAudioClient),
        ctypes.byref(prop),
        ctypes.addressof(handler.obj),
        ctypes.byref(async_op),
    )
    try:
        if not handler.done.wait(5.0):
            raise TimeoutError("Ativação do áudio do processo não respondeu")
        activate_hr = ctypes.c_long()
        client = ctypes.c_void_p()
        # IActivateAudioInterfaceAsyncOperation::GetActivateResult (índice 3)
        _method(handler.operation, 3, ctypes.POINTER(ctypes.c_long), ctypes.POINTER(ctypes.c_void_p))(
            ctypes.byref(activate_hr), ctypes.byref(client)
        )
        if activate_hr.value < 0:
            raise OSError(f"Falha ao ativar captura do processo {pid}: HRESULT 0x{activate_hr.value & 0xFFFFFFFF:08X}")
        return client
    finally:
        _release(handler.operation)
        _release(async_op)


class ProcessLoopbackCapture:
    """Captura PCM s16 estéreo 48 kHz de um processo (ou de tudo, exceto um processo).

    `on_data(bytes)` é chamado na thread de captura com blocos de áudio.
    """

    def __init__(self, pid: int, on_data, include_tree: bool = True):
        self.pid = pid
        self.include_tree = include_tree
        self._on_data = on_data
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name=f"audio-{pid}", daemon=True)
        self.error: str | None = None

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join(timeout=2)

    def _run(self) -> None:
        hr = ole32.CoInitializeEx(None, COINIT_MULTITHREADED)
        client = ctypes.c_void_p()
        capture = ctypes.c_void_p()
        event = None
        try:
            client = _activate_process_loopback(self.pid, self.include_tree)
            fmt = WAVEFORMATEX()
            fmt.wFormatTag = WAVE_FORMAT_PCM
            fmt.nChannels = config.AUDIO_CHANNELS
            fmt.nSamplesPerSec = config.AUDIO_RATE
            fmt.wBitsPerSample = 16
            fmt.nBlockAlign = fmt.nChannels * fmt.wBitsPerSample // 8
            fmt.nAvgBytesPerSec = fmt.nSamplesPerSec * fmt.nBlockAlign
            block_align = fmt.nBlockAlign

            flags = (
                AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
            )
            # IAudioClient: 3 Initialize, 10 Start, 11 Stop, 13 SetEventHandle, 14 GetService
            _method(client, 3, ctypes.c_int, wintypes.DWORD, ctypes.c_longlong, ctypes.c_longlong, ctypes.POINTER(WAVEFORMATEX), ctypes.c_void_p)(
                AUDCLNT_SHAREMODE_SHARED, flags, 200_000, 0, ctypes.byref(fmt), None
            )
            event = kernel32.CreateEventW(None, False, False, None)
            _method(client, 13, wintypes.HANDLE)(event)
            _method(client, 14, ctypes.POINTER(GUID), ctypes.POINTER(ctypes.c_void_p))(
                ctypes.byref(IID_IAudioCaptureClient), ctypes.byref(capture)
            )
            # IAudioCaptureClient: 3 GetBuffer, 4 ReleaseBuffer, 5 GetNextPacketSize
            get_buffer = _method(
                capture, 3,
                ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(wintypes.DWORD),
                ctypes.c_void_p, ctypes.c_void_p,
            )
            release_buffer = _method(capture, 4, ctypes.c_uint32)
            next_packet = _method(capture, 5, ctypes.POINTER(ctypes.c_uint32))
            _method(client, 10)()

            data_ptr = ctypes.c_void_p()
            frames = ctypes.c_uint32()
            buf_flags = wintypes.DWORD()
            packet = ctypes.c_uint32()
            while not self._stop.is_set():
                kernel32.WaitForSingleObject(event, 100)
                next_packet(ctypes.byref(packet))
                while packet.value:
                    get_buffer(ctypes.byref(data_ptr), ctypes.byref(frames), ctypes.byref(buf_flags), None, None)
                    size = frames.value * block_align
                    if buf_flags.value & AUDCLNT_BUFFERFLAGS_SILENT or not data_ptr.value:
                        chunk = bytes(size)
                    else:
                        chunk = ctypes.string_at(data_ptr.value, size)
                    release_buffer(frames.value)
                    self._on_data(chunk)
                    next_packet(ctypes.byref(packet))
            _method(client, 11)()
        except Exception as exc:  # noqa: BLE001 - reportado na interface
            self.error = str(exc)
            log.exception("captura de áudio do processo %s falhou", self.pid)
        finally:
            if event:
                kernel32.CloseHandle(event)
            _release(capture)
            _release(client)
            if hr >= 0:
                ole32.CoUninitialize()
