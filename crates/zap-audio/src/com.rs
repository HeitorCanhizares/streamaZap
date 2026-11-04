//! Guardião RAII de COM por thread (`CoInitializeEx(COINIT_MULTITHREADED)`).

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

pub struct ComGuard(bool);

impl ComGuard {
    pub fn new() -> Self {
        // S_OK e S_FALSE exigem CoUninitialize; RPC_E_CHANGED_MODE não.
        // SAFETY: chamada simples de inicialização de COM na thread atual.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComGuard(hr.is_ok())
    }
}

impl Default for ComGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: pareado com o CoInitializeEx bem-sucedido do `new`.
            unsafe { CoUninitialize() };
        }
    }
}
