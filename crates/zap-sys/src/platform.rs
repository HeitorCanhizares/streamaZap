//! Ajustes de processo dependentes da plataforma: resolução do timer e MTA do COM.
//! Em outros sistemas não fazem nada.

/// Resolução de timer de 1 ms enquanto viver (Windows: `timeBeginPeriod(1)` / `timeEndPeriod(1)`,
/// contados pelo sistema; vários guardas no mesmo processo podem coexistir). Outros sistemas: nada.
pub struct TimerResolution {
    active: bool,
}

impl TimerResolution {
    /// Pede resolução de 1 ms. `is_active` diz se o sistema aceitou.
    pub fn high() -> Self {
        #[cfg(windows)]
        {
            use windows::Win32::Media::{timeBeginPeriod, TIMERR_NOERROR};
            // SAFETY: chamada simples da API do sistema, sem ponteiros; é pareada com
            // `timeEndPeriod(1)` no Drop apenas quando teve sucesso.
            let ret = unsafe { timeBeginPeriod(1) };
            let active = ret == TIMERR_NOERROR;
            if !active {
                tracing::warn!(codigo = ret, "timeBeginPeriod(1) falhou; timer segue grosso");
            }
            Self { active }
        }
        #[cfg(not(windows))]
        {
            Self { active: false }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.active {
            // SAFETY: desfaz exatamente o `timeBeginPeriod(1)` bem-sucedido deste guarda.
            unsafe {
                windows::Win32::Media::timeEndPeriod(1);
            }
        }
    }
}

/// Mantém o MTA do COM vivo até o fim do processo (`CoIncrementMTAUsage` uma vez, `OnceLock`;
/// o cookie nunca é solto). Idempotente e barato depois da primeira vez. Outros sistemas: nada.
pub fn pin_com_mta() {
    #[cfg(windows)]
    {
        static PIN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        PIN.get_or_init(|| {
            // SAFETY: chamada sem ponteiros; o cookie é descartado de propósito, então o MTA
            // fica vivo até o fim do processo (nunca chamamos `CoDecrementMTAUsage`).
            match unsafe { windows::Win32::System::Com::CoIncrementMTAUsage() } {
                Ok(_cookie) => {}
                Err(e) => tracing::warn!(erro = %e, "CoIncrementMTAUsage falhou; MTA não fixado"),
            }
        });
    }
}

#[cfg(test)]
mod pin_tests {
    use super::pin_com_mta;

    #[test]
    fn pin_com_mta_idempotente() {
        for _ in 0..3 {
            std::thread::spawn(pin_com_mta)
                .join()
                .expect("pin_com_mta não deve entrar em pânico");
        }

        #[cfg(windows)]
        std::thread::spawn(|| {
            use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
            // SAFETY: inicializa e desinicializa o COM só nesta thread, de forma pareada.
            unsafe {
                let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
                assert!(hr.is_ok(), "CoInitializeEx falhou: {hr:?}");
                CoUninitialize();
            }
        })
        .join()
        .expect("thread de COM não deve entrar em pânico");
    }
}

#[cfg(test)]
mod timer_tests {
    use super::TimerResolution;

    #[cfg(windows)]
    #[test]
    fn timer_alta_resolucao_encurta_esperas() {
        let guarda = TimerResolution::high();
        // O Sender fica vivo para o recv_timeout esperar de verdade até o fim do prazo.
        let (_tx, rx) = std::sync::mpsc::channel::<()>();
        let inicio = std::time::Instant::now();
        for _ in 0..50 {
            let _ = rx.recv_timeout(std::time::Duration::from_millis(2));
        }
        let media = inicio.elapsed() / 50;
        assert!(media <= std::time::Duration::from_millis(4), "média {media:?}");
        assert!(guarda.is_active());
    }

    #[test]
    fn guardas_aninhados() {
        let a = TimerResolution::high();
        let b = TimerResolution::high();
        #[cfg(windows)]
        {
            assert!(a.is_active());
            assert!(b.is_active());
        }
        // Solta na ordem inversa à da criação.
        drop(b);
        drop(a);
    }
}
