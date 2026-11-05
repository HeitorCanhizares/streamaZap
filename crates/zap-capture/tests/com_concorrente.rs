//! Concorrência COM/WinRT + Media Foundation (só Windows).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Barrier};
use std::thread;

const VOLTAS: usize = 25;

#[test]
fn com_concorrente_com_winrt_e_mf() {
    // 4 (câmeras) + 4 (WGC) + 2 (janelas) threads largam juntas.
    let barreira = Arc::new(Barrier::new(10));
    let mut threads = Vec::new();

    for _ in 0..4 {
        let b = Arc::clone(&barreira);
        threads.push(thread::spawn(move || {
            b.wait();
            for _ in 0..VOLTAS {
                // Ok(vazio) ou Err (sem câmera) são válidos; só não pode dar pânico.
                let _ = zap_capture::list_cameras();
            }
        }));
    }
    for _ in 0..4 {
        let b = Arc::clone(&barreira);
        threads.push(thread::spawn(move || {
            b.wait();
            for _ in 0..VOLTAS {
                let _ = zap_capture::wgc_supported();
            }
        }));
    }
    for _ in 0..2 {
        let b = Arc::clone(&barreira);
        threads.push(thread::spawn(move || {
            b.wait();
            for _ in 0..VOLTAS {
                let _ = zap_capture::list_windows();
            }
        }));
    }

    // join().unwrap() propaga qualquer pânico de thread como falha do teste.
    for t in threads {
        t.join().unwrap();
    }
}
