use windows::core::HRESULT;
use windows::Win32::System::Com::{CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// Mantém o MTA do COM vivo até o fim do processo (`CoIncrementMTAUsage` uma vez, `OnceLock`;
/// o cookie nunca é solto). Idempotente e barato depois da primeira vez.
pub fn pin_mta() {
    static PIN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    PIN.get_or_init(|| {
        // SAFETY: chamada FFI sem ponteiros; o cookie é descartado de propósito (o MTA fica
        // preso até o fim do processo, então nunca chamamos CoDecrementMTAUsage).
        if let Err(e) = unsafe { CoIncrementMTAUsage() } {
            tracing::warn!("CoIncrementMTAUsage falhou: {e}");
        }
    });
}

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106u32 as i32);

/// Guardião RAII: `CoInitializeEx(COINIT_MULTITHREADED)` na criação e `CoUninitialize` ao dropar
/// (só desinicializa se a inicialização teve sucesso). Não é `Send`: vale para a thread atual.
pub struct ComGuard {
    initialized: bool,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl ComGuard {
    /// Inicializa COM na thread. `RPC_E_CHANGED_MODE` conta como sucesso sem desinicializar.
    pub fn new() -> Result<Self, crate::GpuError> {
        pin_mta();
        // SAFETY: chamada FFI sem ponteiros; o par com CoUninitialize fica no Drop.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            // S_OK ou S_FALSE: ambos contam uma inicialização a desfazer.
            Ok(Self {
                initialized: true,
                _not_send: std::marker::PhantomData,
            })
        } else if hr == RPC_E_CHANGED_MODE {
            // Thread já em STA: segue sem desinicializar.
            Ok(Self {
                initialized: false,
                _not_send: std::marker::PhantomData,
            })
        } else {
            Err(crate::GpuError::Os {
                context: "CoInitializeEx",
                message: hr.message(),
            })
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: par com a CoInitializeEx bem-sucedida desta thread (ComGuard não é Send).
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_e_drop() {
        let a = ComGuard::new();
        assert!(a.is_ok());
        let b = ComGuard::new();
        assert!(b.is_ok());
    }

    #[test]
    fn new_em_outra_thread() {
        let ok = std::thread::spawn(|| ComGuard::new().is_ok()).join();
        assert_eq!(ok.ok(), Some(true));
    }

    #[test]
    fn pin_mta_idempotente() {
        let hs: Vec<_> = (0..3).map(|_| std::thread::spawn(pin_mta)).collect();
        for h in hs {
            assert!(h.join().is_ok());
        }
        pin_mta();
    }

    #[test]
    fn com_guard_new_chama_pin() {
        pin_mta();
        assert!(ComGuard::new().is_ok());
    }

    // guard_nao_e_send: PhantomData<*const ()> torna ComGuard !Send; sem teste em código.
}
