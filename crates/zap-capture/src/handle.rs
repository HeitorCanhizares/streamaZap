use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Controla uma captura em andamento; ao ser descartado, pede parada e espera a thread.
pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureHandle {
    /// `stop` é a mesma flag que a thread de captura consulta a cada volta.
    pub fn new(stop: Arc<AtomicBool>, thread: JoinHandle<()>) -> Self {
        Self {
            stop,
            thread: Some(thread),
        }
    }
    /// Pede parada sem esperar.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureHandle;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// Thread de teste: roda até `stop` ficar true e marca `terminou` ao sair.
    fn thread_de_captura(stop: Arc<AtomicBool>, terminou: Arc<AtomicBool>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            terminou.store(true, Ordering::Release);
        })
    }

    #[test]
    fn drop_para_a_thread() {
        let stop = Arc::new(AtomicBool::new(false));
        let terminou = Arc::new(AtomicBool::new(false));
        let t = thread_de_captura(Arc::clone(&stop), Arc::clone(&terminou));
        let handle = CaptureHandle::new(Arc::clone(&stop), t);

        drop(handle);

        // O drop pede parada e espera a thread (join), então ao voltar ela já terminou.
        assert!(stop.load(Ordering::Acquire));
        assert!(terminou.load(Ordering::Acquire));
    }

    #[test]
    fn request_stop_nao_espera() {
        let stop = Arc::new(AtomicBool::new(false));
        let terminou = Arc::new(AtomicBool::new(false));
        let t = thread_de_captura(Arc::clone(&stop), Arc::clone(&terminou));
        let handle = CaptureHandle::new(Arc::clone(&stop), t);

        handle.request_stop();

        // A flag foi levantada; a thread pode ainda estar viva neste ponto, então não afirmamos `terminou`.
        assert!(stop.load(Ordering::Acquire));

        // O drop posterior encerra a thread de qualquer forma.
        drop(handle);
        assert!(terminou.load(Ordering::Acquire));
    }

    #[test]
    fn drop_com_thread_que_ja_terminou_nao_trava() {
        let stop = Arc::new(AtomicBool::new(false));
        let t = thread::spawn(|| {});
        // Espera a thread acabar antes do drop para cobrir o caso de join em thread já finalizada.
        while !t.is_finished() {
            thread::sleep(Duration::from_millis(1));
        }
        let handle = CaptureHandle::new(Arc::clone(&stop), t);

        drop(handle);

        assert!(stop.load(Ordering::Acquire));
    }
}
