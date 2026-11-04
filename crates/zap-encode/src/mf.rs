//! Guardião de `MFStartup`/`MFShutdown` com contagem por processo.
use std::sync::Mutex;

use windows::Win32::Media::MediaFoundation::{MFShutdown, MFStartup, MFSTARTUP_FULL, MF_VERSION};

/// Número de guardiões vivos; protegido pelo mutex para que startup/shutdown
/// nunca corram em paralelo com a contagem.
static COUNT: Mutex<u32> = Mutex::new(0);

pub(crate) struct MfGuard {
    _private: (),
}

impl MfGuard {
    /// Primeira chamada faz `MFStartup(MF_VERSION, MFSTARTUP_FULL)`; as demais só contam.
    pub(crate) fn acquire() -> Result<Self, crate::EncodeError> {
        // Mutex envenenado: a contagem continua válida, então seguimos.
        let mut count = COUNT.lock().unwrap_or_else(|p| p.into_inner());
        if *count == 0 {
            // SAFETY: chamada sem ponteiros; o mutex serializa startup/shutdown.
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|e| crate::EncodeError::os("MFStartup", &e))?;
        }
        *count += 1;
        Ok(MfGuard { _private: () })
    }
}

impl Drop for MfGuard {
    /// Última a cair faz `MFShutdown`.
    fn drop(&mut self) {
        let mut count = COUNT.lock().unwrap_or_else(|p| p.into_inner());
        *count = count.saturating_sub(1);
        if *count == 0 {
            // SAFETY: só chega aqui após o último guardião; o mutex serializa com o startup.
            let _ = unsafe { MFShutdown() };
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn acquire_duplo_e_reacquire() {
        let a = MfGuard::acquire().expect("primeiro acquire");
        let b = MfGuard::acquire().expect("segundo acquire");
        drop(b);
        drop(a);
        // Tudo liberado: um novo acquire precisa refazer o startup.
        let c = MfGuard::acquire().expect("re-acquire");
        drop(c);
    }

    #[test]
    fn threads_concorrentes() {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..50 {
                        let g = MfGuard::acquire().expect("acquire na thread");
                        drop(g);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().expect("thread terminou");
        }
    }
}
