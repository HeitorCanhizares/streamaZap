//! Auxiliares compartilhados pelos testes de integração novos (só Windows).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use zap_capture::{CaptureError, CapturedFrame, FrameSink};

/// Sink de teste: quadros por canal, erros acumulados num vetor.
pub struct SinkTeste {
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

pub type Erros = Arc<Mutex<Vec<String>>>;

/// Vez dos testes que duplicam o monitor (um por vez em cada processo de teste): em paralelo, o
/// segundo `DuplicateOutput` dá E_INVALIDARG e o teste pula. Segure a guarda até o fim do teste.
pub fn vez_da_duplicacao() -> std::sync::MutexGuard<'static, ()> {
    static VEZ: Mutex<()> = Mutex::new(());
    VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn novo_sink() -> (SinkTeste, Receiver<CapturedFrame>, Erros) {
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
