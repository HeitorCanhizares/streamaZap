//! Prioridade de thread (única fonte de `unsafe` do crate).
#![allow(unsafe_code)]

/// Windows: `SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL)`; falha só é
/// logada (`tracing::debug!`). Outras plataformas: não faz nada.
#[cfg(windows)]
pub fn raise_current_thread() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};

    // SAFETY: altera só a prioridade da thread atual; o pseudo-handle de `GetCurrentThread`
    // é sempre válido e não precisa ser fechado.
    if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) } {
        tracing::debug!("SetThreadPriority falhou: {e}");
    }
}

/// Outras plataformas: não faz nada.
#[cfg(not(windows))]
pub fn raise_current_thread() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn nao_entra_em_panico() {
        std::thread::spawn(raise_current_thread).join().unwrap();
    }
}
