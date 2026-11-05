//! Janela animada para gerar movimento na tela (bancada e testes). Só Windows.
//!
//! A duplicação de tela só entrega quadros quando a tela muda; sem movimento, bancada e testes
//! medem espera e não trabalho.

use crate::{CaptureError, MonitorInfo};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, UpdateWindow, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetWindowLongPtrW, IsWindow,
    PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage, CREATESTRUCTW,
    GWLP_USERDATA, MSG, PM_REMOVE, WM_CLOSE, WM_DESTROY, WM_NCCREATE, WM_PAINT, WM_QUIT, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

/// Janela pequena (320×200), sempre no topo, sem foco e fora da barra de tarefas, no canto
/// inferior direito de um monitor (x + largura − 340, y + altura − 240), que muda de cor a cada
/// ~8 ms: dá movimento para a duplicação de tela na bancada e nos testes. Fecha no `Drop`.
pub struct JanelaAnimada {
    /// Thread "zap-anim" (dona da janela e do laço de mensagens).
    thread: Option<JoinHandle<()>>,
    /// `HWND` como inteiro (para o `PostMessageW(WM_CLOSE)` do `Drop`).
    hwnd: isize,
    /// Pedido de parada.
    parar: Arc<AtomicBool>,
    /// Pinturas feitas.
    pinturas: Arc<AtomicU64>,
}

/// A classe só pode ser registrada uma vez por processo.
static CLASSE: Once = Once::new();

/// Converte um matiz (graus; saturação e brilho máximos) em `COLORREF` (0x00BBGGRR).
fn cor_do_matiz(hue: u32) -> COLORREF {
    let h = hue % 360;
    // HSV com s = v = 1: um canal sobe e outro desce linearmente a cada 60 graus.
    let sobe = 255 * (h % 60) / 60;
    let desce = 255 - sobe;
    let (r, g, b) = match h / 60 {
        0 => (255, sobe, 0),
        1 => (desce, 255, 0),
        2 => (0, 255, sobe),
        3 => (0, desce, 255),
        4 => (sobe, 0, 255),
        _ => (255, 0, desce),
    };
    COLORREF(r | (g << 8) | (b << 16))
}

/// Trata as mensagens da janela. O contador de pinturas desta janela fica em `GWLP_USERDATA`
/// (ponteiro do `Arc<AtomicU64>`, mantido vivo pela thread dona): não há estado global.
unsafe extern "system" fn proc_janela(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            // SAFETY: em WM_NCCREATE, `lparam` aponta para um CREATESTRUCTW válido.
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_PAINT => {
            // SAFETY: GDI com um HWND válido dentro do WM_PAINT; o ponteiro de USERDATA vem do
            // `Arc::as_ptr` da thread dona, que só o solta depois de destruir a janela.
            unsafe {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AtomicU64;
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let n = if ptr.is_null() {
                    0
                } else {
                    (*ptr).load(Ordering::Relaxed)
                };
                let pincel = CreateSolidBrush(cor_do_matiz((n.wrapping_mul(7) % 360) as u32));
                let mut area = Default::default();
                if GetClientRect(hwnd, &mut area).is_ok() {
                    FillRect(hdc, &area, pincel);
                }
                let _ = DeleteObject(pincel.into());
                let _ = EndPaint(hwnd, &ps);
                if !ptr.is_null() {
                    (*ptr).fetch_add(1, Ordering::Relaxed);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: HWND válido, chamado na thread dona da janela.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: só pede o WM_QUIT para a thread corrente.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: repassa a mensagem intacta ao tratamento padrão.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn erro_os(context: &'static str, e: impl std::fmt::Display) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.to_string(),
    }
}

/// Corpo da thread: cria a janela, avisa o HWND e roda o laço de mensagens/pintura.
fn rodar(
    x: i32,
    y: i32,
    parar: Arc<AtomicBool>,
    pinturas: Arc<AtomicU64>,
    tx: mpsc::Sender<Result<isize, CaptureError>>,
) {
    // SAFETY: chamadas Win32 simples; todos os ponteiros passados vivem durante a chamada e o
    // `Arc` de `pinturas` vive mais que a janela (destruída antes do fim desta função).
    unsafe {
        let hinst = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(Err(erro_os("GetModuleHandleW", e)));
                return;
            }
        };
        let mut falha_classe = None;
        CLASSE.call_once(|| {
            let classe = WNDCLASSW {
                lpfnWndProc: Some(proc_janela),
                hInstance: hinst.into(),
                lpszClassName: w!("ZapJanelaAnimada"),
                ..Default::default()
            };
            if RegisterClassW(&classe) == 0 {
                falha_classe = Some(erro_os("RegisterClassW", windows::core::Error::from_thread()));
            }
        });
        if let Some(e) = falha_classe {
            let _ = tx.send(Err(e));
            return;
        }
        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("ZapJanelaAnimada"),
            PCWSTR::null(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            320,
            200,
            None,
            None,
            Some(hinst.into()),
            Some(Arc::as_ptr(&pinturas) as *const std::ffi::c_void),
        ) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(Err(erro_os("CreateWindowExW", e)));
                return;
            }
        };
        let _ = tx.send(Ok(hwnd.0 as isize));

        let mut msg = MSG::default();
        'laco: while !parar.load(Ordering::Acquire) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'laco;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            let _ = UpdateWindow(hwnd);
            std::thread::sleep(Duration::from_millis(8));
        }
        // Se saímos por `parar` antes de processar o WM_CLOSE, derruba a janela aqui.
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = DestroyWindow(hwnd);
        }
    }
}

impl JanelaAnimada {
    /// Abre e espera a primeira pintura (até 2 s).
    pub fn abrir(monitor: &MonitorInfo) -> Result<Self, CaptureError> {
        let x = monitor.x + monitor.width as i32 - 340;
        let y = monitor.y + monitor.height as i32 - 240;
        let parar = Arc::new(AtomicBool::new(false));
        let pinturas = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let thread = {
            let (parar, pinturas) = (parar.clone(), pinturas.clone());
            std::thread::Builder::new()
                .name("zap-anim".into())
                .spawn(move || rodar(x, y, parar, pinturas, tx))
                .map_err(|e| erro_os("JanelaAnimada thread", e))?
        };
        let hwnd = match rx.recv() {
            Ok(Ok(h)) => h,
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(e);
            }
            Err(_) => {
                let _ = thread.join();
                return Err(erro_os("JanelaAnimada thread", "thread encerrou sem janela"));
            }
        };
        // Com a janela criada, o Drop cuida de fechar mesmo se a primeira pintura falhar.
        let janela = Self {
            thread: Some(thread),
            hwnd,
            parar,
            pinturas,
        };
        let limite = Instant::now() + Duration::from_secs(2);
        while janela.quadros() < 1 {
            if Instant::now() >= limite {
                return Err(erro_os("JanelaAnimada primeira pintura", "sem pintura em 2 s"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(janela)
    }

    /// Pinturas feitas até agora.
    pub fn quadros(&self) -> u64 {
        self.pinturas.load(Ordering::Relaxed)
    }
}

impl Drop for JanelaAnimada {
    /// `PostMessageW(WM_CLOSE)` + join; sem pânico se a janela já fechou.
    fn drop(&mut self) {
        // SAFETY: o HWND pode já ter sido destruído; PostMessageW só devolve erro nesse caso.
        unsafe {
            let _ = PostMessageW(Some(HWND(self.hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        self.parar.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list_monitors;

    fn abrir_no_monitor_1() -> Option<JanelaAnimada> {
        let monitores = match list_monitors() {
            Ok(m) if !m.is_empty() => m,
            outra => {
                eprintln!("sem monitores ({outra:?}); teste pulado");
                return None;
            }
        };
        match JanelaAnimada::abrir(&monitores[0]) {
            Ok(j) => Some(j),
            Err(e) => {
                eprintln!("não abriu a janela ({e}); teste pulado");
                None
            }
        }
    }

    #[test]
    fn abre_e_pinta() {
        let Some(janela) = abrir_no_monitor_1() else {
            return;
        };
        std::thread::sleep(Duration::from_millis(300));
        assert!(janela.quadros() >= 10, "pinturas: {}", janela.quadros());
        let t0 = Instant::now();
        drop(janela);
        assert!(t0.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn drop_sem_pico() {
        let Some(a) = abrir_no_monitor_1() else {
            return;
        };
        drop(a);
        let b = abrir_no_monitor_1();
        assert!(b.is_some(), "segunda abertura falhou");
    }
}
