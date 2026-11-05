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
