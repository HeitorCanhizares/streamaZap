use std::sync::OnceLock;
use std::time::Instant;

#[cfg(windows)]
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Relógio monotônico do processo em microssegundos (mesmo relógio para captura, conversão e
/// codificação). Windows: `QueryPerformanceCounter`/`QueryPerformanceFrequency`; outros: `Instant`.
#[cfg(windows)]
pub fn now_us() -> u64 {
    let freq = qpc_freq();
    if freq > 0 {
        let mut counter: i64 = 0;
        // SAFETY: `counter` é um i64 local vivo durante a chamada; QPC só escreve nele.
        let ok = unsafe { QueryPerformanceCounter(&mut counter) }.is_ok();
        if ok && counter >= 0 {
            // u128 evita estouro: counter * 1e6 passa de u64 após algumas horas de uptime.
            let us = (counter as u128) * 1_000_000 / (freq as u128);
            return u64::try_from(us).unwrap_or(u64::MAX);
        }
    }
    // Sem QPC utilizável: cai para o relógio do Rust (origem própria, mas ainda monotônico).
    instant_us()
}

/// Relógio monotônico do processo em microssegundos (mesmo relógio para captura, conversão e
/// codificação). Windows: `QueryPerformanceCounter`/`QueryPerformanceFrequency`; outros: `Instant`.
#[cfg(not(windows))]
pub fn now_us() -> u64 {
    instant_us()
}

/// Frequência do QPC, lida uma única vez. Zero indica falha (usa-se o fallback).
#[cfg(windows)]
fn qpc_freq() -> i64 {
    static FREQ: OnceLock<i64> = OnceLock::new();
    *FREQ.get_or_init(|| {
        let mut freq: i64 = 0;
        // SAFETY: `freq` é um i64 local vivo durante a chamada; QPF só escreve nele.
        match unsafe { QueryPerformanceFrequency(&mut freq) } {
            Ok(()) => freq,
            Err(_) => 0,
        }
    })
}

/// Origem do relógio de fallback: o primeiro uso define o zero.
fn instant_us() -> u64 {
    static ORIGEM: OnceLock<Instant> = OnceLock::new();
    let origem = ORIGEM.get_or_init(Instant::now);
    u64::try_from(origem.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_us_monotonico() {
        let mut anterior = now_us();
        for _ in 0..100 {
            let atual = now_us();
            assert!(atual >= anterior, "relógio retrocedeu: {anterior} -> {atual}");
            anterior = atual;
        }
    }

    #[test]
    fn now_us_avanca() {
        let inicio = now_us();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let decorrido = now_us() - inicio;
        assert!(
            (15_000..=200_000).contains(&decorrido),
            "decorrido fora da faixa: {decorrido} µs"
        );
    }
}
