//! Capturas não podem depender do endereço do `GpuDevice` (só Windows).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use common::novo_sink;
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
use zap_capture::{
    capture_camera, capture_monitor, capture_window, list_cameras, list_monitors, list_windows, wgc_supported,
};
use zap_gpu::GpuDevice;

#[test]
fn capture_monitor_sobrevive_a_mover_e_soltar_o_dispositivo() {
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

    // Move o dispositivo para o heap e solta o original: a captura não pode depender do endereço.
    let caixa = Box::new(dev);
    drop(caixa);

    // Mexe o mouse para forçar quadros (a duplicação só entrega com mudança na tela).
    let inicio = Instant::now();
    let mut quadros = 0;
    let mut passo = 0i32;
    while inicio.elapsed() < Duration::from_secs(3) {
        passo += 1;
        let (dx, dy) = if passo % 2 == 0 { (100, 100) } else { (200, 160) };
        // Falha de SetCursorPos é ignorada (sessão sem desktop interativo).
        let _ = unsafe { SetCursorPos(mon.x + dx, mon.y + dy) };
        if rx.recv_timeout(Duration::from_millis(50)).is_ok() {
            quadros += 1;
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    drop(handle);

    let erros = erros.lock().unwrap().clone();
    if quadros == 0 {
        let sem_sessao = erros.iter().any(|e| {
            let e = e.to_lowercase();
            e.contains("duplica") || e.contains("suportado") || e.contains("not supported")
        });
        if sem_sessao {
            eprintln!("ambiente sem sessão interativa ({erros:?}); pulando");
            return;
        }
    }
    assert!(
        quadros >= 1,
        "nenhum quadro em 3 s após mover/soltar o dispositivo (erros: {erros:?})"
    );
    assert!(erros.is_empty(), "erros no sink: {erros:?}");
}

/// Cria o dispositivo padrão ou devolve `None` (sem GPU) com aviso.
fn novo_dispositivo() -> Option<GpuDevice> {
    match GpuDevice::create_default() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("sem GPU ({e}); pulando");
            None
        }
    }
}

#[test]
fn capture_camera_sobrevive_a_mover_e_soltar_o_dispositivo() {
    let Some(dev) = novo_dispositivo() else { return };
    let cams = match list_cameras() {
        Ok(c) if !c.is_empty() => c,
        _ => {
            eprintln!("sem câmera; pulando");
            return;
        }
    };
    let (sink, rx, erros) = novo_sink();
    let handle = match capture_camera(&dev, &cams[0].id, 640, 480, 30, sink) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("capture_camera falhou ({e}); pulando");
            return;
        }
    };

    // Move o dispositivo para o heap e solta o original.
    let caixa = Box::new(dev);
    drop(caixa);

    let quadros = usize::from(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    drop(handle);

    let erros = erros.lock().unwrap().clone();
    assert!(
        quadros >= 1,
        "nenhum quadro da câmera em 5 s após mover/soltar o dispositivo (erros: {erros:?})"
    );
}

#[test]
fn capture_window_sobrevive_a_mover_e_soltar_o_dispositivo() {
    if !wgc_supported() {
        eprintln!("WGC não suportado; pulando");
        return;
    }
    let Some(dev) = novo_dispositivo() else { return };
    let janelas = match list_windows() {
        Ok(j) if !j.is_empty() => j,
        _ => {
            eprintln!("sem janela visível; pulando");
            return;
        }
    };
    let (sink, rx, erros) = novo_sink();
    let handle = match capture_window(&dev, janelas[0].hwnd, sink) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("capture_window falhou ({e}); pulando");
            return;
        }
    };

    let caixa = Box::new(dev);
    drop(caixa);

    let quadros = usize::from(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    // A asserção forte é este drop retornar sem travar nem derrubar o processo.
    drop(handle);

    let erros = erros.lock().unwrap().clone();
    if quadros == 0 {
        // Janela parada pode não gerar quadro: não é falha.
        eprintln!("janela sem quadros em 5 s (erros: {erros:?}); sem falha");
    }
}
