//! Thread de eventos do MFT de hardware: `GetEvent` bloqueante fora do laço principal.
use crossbeam_channel::{Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use windows::core::Interface;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaEventGenerator, IMFShutdown, IMFTransform, MEError, MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    MF_EVENT_FLAG_NO_WAIT, MF_E_NO_EVENTS_AVAILABLE, MF_E_SHUTDOWN,
};

/// Evento do MFT já reduzido a dois números.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MftEvent {
    /// `MediaEventType` (ex.: `METransformNeedInput.0 as u32`).
    pub kind: u32,
    /// `HRESULT` do evento (`GetStatus`); 0 se não conseguiu ler.
    pub status: i32,
}

/// Embrulho para mover interfaces COM para a thread de eventos.
struct SendCom<T>(T);
// SAFETY: o MFT de hardware é MTA e thread-safe para `GetEvent`; usar
// `IMFMediaEventGenerator` de outra thread é o uso previsto da interface.
unsafe impl<T> Send for SendCom<T> {}

/// Dono da thread `zap-encode-ev` e do canal de eventos.
pub(crate) struct EventThread {
    pub(crate) rx: Receiver<MftEvent>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl EventThread {
    /// Cria o canal `bounded(64)` e a thread. Se o MFT tem `IMFShutdown`, a thread faz
    /// `GetEvent(0)` bloqueante; senão `GetEvent(MF_EVENT_FLAG_NO_WAIT)` + `sleep(1 ms)` olhando
    /// `stop`. Sai em `MF_E_SHUTDOWN`, erro de `GetEvent` ou canal fechado.
    pub(crate) fn spawn(mft: &IMFTransform) -> Result<Self, crate::EncodeError> {
        let (tx, rx) = crossbeam_channel::bounded(64);
        let gen: IMFMediaEventGenerator = mft
            .cast()
            .map_err(|e| crate::EncodeError::os("IMFMediaEventGenerator", &e))?;
        let bloqueante = mft.cast::<IMFShutdown>().is_ok();
        let stop = Arc::new(AtomicBool::new(false));

        let com = SendCom(gen);
        let stop_thread = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("zap-encode-ev".into())
            .spawn(move || {
                let com = com;
                laco_eventos(com.0, bloqueante, tx, stop_thread);
            })
            .map_err(|e| crate::EncodeError::Stopped(format!("thread de eventos: {e}")))?;

        Ok(Self {
            rx,
            stop,
            handle: Some(handle),
        })
    }

    /// Para a thread: marca `stop`, chama `IMFShutdown::Shutdown` (se houver) e dá `join`.
    /// Deve ser chamada antes dos `ProcessMessage` finais de encerramento. Idempotente.
    pub(crate) fn shutdown(&mut self, mft: &IMFTransform) {
        self.stop.store(true, Ordering::SeqCst);
        if self.handle.is_none() {
            return;
        }
        if let Ok(shut) = mft.cast::<IMFShutdown>() {
            // SAFETY: chamada COM sem ponteiros; destrava o `GetEvent` bloqueante.
            if let Err(e) = unsafe { shut.Shutdown() } {
                tracing::debug!("IMFShutdown::Shutdown falhou: {e}");
            }
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for EventThread {
    /// Só marca `stop`: sem o `mft` não dá para destravar o `GetEvent` bloqueante, então não
    /// há `join` aqui. O `hw.rs` sempre chama `shutdown` explicitamente antes do drop.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Corpo da thread: lê eventos e os envia até erro, shutdown, `stop` ou canal fechado.
fn laco_eventos(gen: IMFMediaEventGenerator, bloqueante: bool, tx: Sender<MftEvent>, stop: Arc<AtomicBool>) {
    let _com = match zap_gpu::ComGuard::new() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("thread de eventos sem COM: {e}");
            return;
        }
    };
    let flags = if bloqueante {
        MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(0)
    } else {
        MF_EVENT_FLAG_NO_WAIT
    };
    while !stop.load(Ordering::SeqCst) {
        // SAFETY: chamada COM sem ponteiros do chamador; `gen` vive durante o laço.
        match unsafe { gen.GetEvent(flags) } {
            Ok(ev) => {
                // SAFETY: `ev` é um IMFMediaEvent válido recém devolvido.
                let kind = unsafe { ev.GetType() }.unwrap_or(MEError.0 as u32);
                // SAFETY: idem.
                let status = unsafe { ev.GetStatus() }.map(|h| h.0).unwrap_or(0);
                if tx.send(MftEvent { kind, status }).is_err() {
                    return;
                }
            }
            Err(e) if !bloqueante && e.code() == MF_E_NO_EVENTS_AVAILABLE => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                if e.code() == MF_E_SHUTDOWN {
                    tracing::debug!("MFT encerrado; thread de eventos saindo");
                } else {
                    tracing::error!("GetEvent falhou: {e}");
                }
                let code = if e.code().0 == 0 { E_FAIL } else { e.code() };
                let r = tx.try_send(MftEvent {
                    kind: MEError.0 as u32,
                    status: code.0,
                });
                if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = r {
                    tracing::debug!("evento de erro descartado");
                }
                return;
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::mf::MfGuard;
    use std::time::{Duration, Instant};
    use windows::Win32::Media::MediaFoundation::MF_TRANSFORM_ASYNC;

    /// Ativa o primeiro MFT de hardware assíncrono; `None` pula o teste.
    fn mft_assincrono() -> Option<(MfGuard, zap_gpu::ComGuard, IMFTransform)> {
        let com = zap_gpu::ComGuard::new().ok()?;
        let mf = MfGuard::acquire().ok()?;
        let cands = crate::enumerate::enumerate(true).ok()?;
        for c in cands {
            // SAFETY: ativação COM sem ponteiros do chamador.
            let mft: IMFTransform = match unsafe { c.activate.ActivateObject() } {
                Ok(m) => m,
                Err(_) => continue,
            };
            // SAFETY: GetAttributes/GetUINT32 não têm ponteiros do chamador.
            let assinc = unsafe { mft.GetAttributes() }
                .ok()
                .and_then(|a| unsafe { a.GetUINT32(&MF_TRANSFORM_ASYNC) }.ok())
                .unwrap_or(0);
            if assinc != 0 {
                return Some((mf, com, mft));
            }
        }
        None
    }

    #[test]
    fn spawn_e_shutdown_nao_travam() {
        let Some((_mf, _com, mft)) = mft_assincrono() else {
            eprintln!("pulado: sem MFT de hardware assíncrono");
            return;
        };
        let t0 = Instant::now();
        let mut et = EventThread::spawn(&mft).expect("spawn");
        et.shutdown(&mft);
        assert!(t0.elapsed() < Duration::from_secs(2), "shutdown demorou");
    }

    #[test]
    fn shutdown_idempotente() {
        let Some((_mf, _com, mft)) = mft_assincrono() else {
            eprintln!("pulado: sem MFT de hardware assíncrono");
            return;
        };
        let mut et = EventThread::spawn(&mft).expect("spawn");
        et.shutdown(&mft);
        et.shutdown(&mft);
        assert!(et.handle.is_none());
    }
}
