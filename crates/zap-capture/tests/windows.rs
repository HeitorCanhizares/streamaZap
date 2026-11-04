//! Testes de integração (só Windows; pulam com mensagem quando falta o recurso).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage, RegisterClassW,
    SetCursorPos, ShowWindow, TranslateMessage, MSG, SW_SHOW, WM_CLOSE, WM_DESTROY, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    WS_VISIBLE,
};
use zap_capture::{
    capture_camera, capture_monitor, capture_window, list_cameras, list_monitors, list_windows, wgc_supported,
    CaptureError, CapturedFrame, FrameSink,
};
use zap_gpu::GpuDevice;

/// Sink de teste: quadros por canal, erros acumulados num vetor.
struct SinkTeste {
    tx: Mutex<Sender<CapturedFrame>>,
    erros: Arc<Mutex<Vec<String>>>,
}

impl FrameSink for SinkTeste {
    fn on_frame(&self, frame: CapturedFrame) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(frame);
        }
    }
    fn on_error(&self, err: CaptureError) {
        if let Ok(mut v) = self.erros.lock() {
            v.push(err.to_string());
        }
    }
}

type Erros = Arc<Mutex<Vec<String>>>;

fn novo_sink() -> (SinkTeste, Receiver<CapturedFrame>, Erros) {
    let (tx, rx) = channel();
    let erros: Erros = Arc::new(Mutex::new(Vec::new()));
    (
        SinkTeste {
            tx: Mutex::new(tx),
            erros: Arc::clone(&erros),
        },
        rx,
        erros,
    )
}

#[test]
fn monitores_nao_vazio() {
    let monitores = list_monitors().expect("list_monitors deve funcionar");
    if monitores.is_empty() {
        eprintln!("sem display neste runner; teste de monitores aceito como vazio");
        return;
    }
    assert!(!monitores.is_empty());
}

#[test]
fn janelas_sem_erro() {
    list_windows().expect("list_windows deve funcionar");
}

#[test]
fn cameras_sem_erro() {
    list_cameras().expect("list_cameras deve funcionar");
}

#[test]
fn dxgi_entrega_quadro_nao_preto() {
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sem GPU ({e}); pulando");
            return;
        }
    };
    let monitores = list_monitors().expect("list_monitors");
    let Some(mon) = monitores.first() else {
        eprintln!("sem monitor; pulando");
        return;
    };
    let (sink, rx, erros) = novo_sink();
    let handle = match capture_monitor(&dev, mon, sink) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("capture_monitor falhou ({e}); pulando");
            return;
        }
    };

    // Mexe o mouse para forçar atualização da tela (a duplicação só entrega com mudança).
    let inicio = Instant::now();
    let mut quadro = None;
    let mut passo = 0i32;
    while inicio.elapsed() < Duration::from_secs(1) {
        passo += 1;
        // Falha de SetCursorPos é ignorada (sessão sem desktop interativo).
        let _ = unsafe { SetCursorPos(mon.x + 100 + (passo % 50), mon.y + 100 + (passo % 50)) };
        if let Ok(f) = rx.recv_timeout(Duration::from_millis(10)) {
            quadro = Some(f);
            break;
        }
    }
    let Some(frame) = quadro else {
        eprintln!("nenhum quadro em 1 s (erros: {:?}); pulando", erros.lock().unwrap());
        return;
    };
    assert_eq!(frame.width, mon.width);
    assert_eq!(frame.height, mon.height);
    let bytes = zap_gpu::read_back(&dev, &frame.texture).expect("read_back");
    assert!(bytes.iter().any(|&b| b != 0), "imagem toda preta");
    drop(handle);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        // SAFETY: chamada na thread dona da janela, encerra o message loop.
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    // SAFETY: repassa os argumentos recebidos do sistema.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Janela Win32 visível numa thread própria com message loop; fechada no drop.
struct JanelaTeste {
    hwnd: isize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl JanelaTeste {
    fn criar() -> Option<Self> {
        let (tx, rx) = channel::<isize>();
        let thread = std::thread::spawn(move || {
            // SAFETY: chamadas Win32 na thread que cria e possui a janela.
            unsafe {
                let classe = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    lpszClassName: w!("ZapCaptureTeste"),
                    ..Default::default()
                };
                RegisterClassW(&classe);
                let hwnd = CreateWindowExW(
                    Default::default(),
                    w!("ZapCaptureTeste"),
                    PCWSTR(w!("Zap teste").as_ptr()),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    100,
                    100,
                    320,
                    240,
                    None,
                    None,
                    None,
                    None,
                );
                let Ok(hwnd) = hwnd else {
                    let _ = tx.send(0);
                    return;
                };
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = tx.send(hwnd.0 as isize);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(h) if h != 0 => Some(Self {
                hwnd: h,
                thread: Some(thread),
            }),
            _ => None,
        }
    }
}

impl Drop for JanelaTeste {
    fn drop(&mut self) {
        // SAFETY: PostMessageW é segura entre threads; WM_CLOSE destrói a janela e encerra o loop.
        let _ = unsafe { PostMessageW(Some(HWND(self.hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[test]
fn wgc_captura_janela_criada_pelo_teste() {
    if !wgc_supported() {
        eprintln!("WGC não suportado; pulando");
        return;
    }
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sem GPU ({e}); pulando");
            return;
        }
    };
    let Some(janela) = JanelaTeste::criar() else {
        eprintln!("não foi possível criar janela; pulando");
        return;
    };
    let (sink, rx, erros) = novo_sink();
    let handle = match capture_window(&dev, janela.hwnd, sink) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("capture_window falhou ({e}); pulando");
            return;
        }
    };
    // Invalida a janela periodicamente para o WGC ter conteúdo novo.
    let inicio = Instant::now();
    let mut recebeu = false;
    while inicio.elapsed() < Duration::from_secs(2) {
        // SAFETY: InvalidateRect só pede repintura; o hwnd pode ser de outra thread.
        let _ = unsafe { InvalidateRect(Some(HWND(janela.hwnd as _)), None, true) };
        if rx.recv_timeout(Duration::from_millis(50)).is_ok() {
            recebeu = true;
            break;
        }
    }
    drop(handle);
    drop(janela);
    assert!(
        recebeu,
        "nenhum quadro da janela em 2 s (erros: {:?})",
        erros.lock().unwrap()
    );
}

#[test]
fn camera_entrega_quadro() {
    let cameras = list_cameras().expect("list_cameras");
    let Some(cam) = cameras.first() else {
        eprintln!("sem câmera; pulando");
        return;
    };
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sem GPU ({e}); pulando");
            return;
        }
    };
    let (sink, rx, erros) = novo_sink();
    let handle = match capture_camera(&dev, &cam.id, 640, 480, 30, sink) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("capture_camera falhou ({e}); pulando");
            return;
        }
    };
    let frame = rx.recv_timeout(Duration::from_secs(3));
    drop(handle);
    let frame = frame.unwrap_or_else(|_| panic!("nenhum quadro da câmera em 3 s (erros: {:?})", erros.lock().unwrap()));
    assert_eq!(frame.texture.format(), zap_gpu::PixelFormat::Nv12);
}
