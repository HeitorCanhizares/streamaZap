//! Relógio monotônico do processo em µs (mesma fonte do `zap_gpu::now_us`).

use std::sync::OnceLock;

#[cfg(windows)]
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

#[cfg(not(windows))]
use std::time::Instant;

/// QueryPerformanceCounter convertido para µs no Windows; `Instant` desde a primeira chamada nos demais.
pub fn now_us() -> u64 {
    agora_us()
}

/// Converte contagens de um relógio de `freq` Hz para µs sem estourar u64.
///
/// Separa segundos inteiros e resto, para que `ticks * 1_000_000` nunca seja calculado direto.
#[cfg(windows)]
fn ticks_para_us(ticks: u64, freq: u64) -> u64 {
    (ticks / freq) * 1_000_000 + (ticks % freq) * 1_000_000 / freq
}

/// Frequência do QPC, lida uma vez. Nunca zero, para não dividir por zero.
#[cfg(windows)]
fn freq_qpc() -> u64 {
    static FREQ: OnceLock<u64> = OnceLock::new();
    *FREQ.get_or_init(|| {
        let mut f: i64 = 0;
        // SAFETY: ponteiro válido para um i64 local; a API só escreve nesse valor.
        // QPF não falha no Windows XP+; se falhar, f fica 0 e o max(1) evita divisão por zero.
        let _ = unsafe { QueryPerformanceFrequency(&mut f) };
        u64::try_from(f).unwrap_or(1).max(1)
    })
}

/// Contagem atual do QPC em ticks.
#[cfg(windows)]
fn ticks_qpc() -> u64 {
    let mut t: i64 = 0;
    // SAFETY: ponteiro válido para um i64 local; a API só escreve nesse valor.
    let _ = unsafe { QueryPerformanceCounter(&mut t) };
    u64::try_from(t).unwrap_or(0)
}

#[cfg(windows)]
fn agora_us() -> u64 {
    ticks_para_us(ticks_qpc(), freq_qpc())
}

/// Fora do Windows: tempo decorrido desde a primeira chamada, em µs.
#[cfg(not(windows))]
fn agora_us() -> u64 {
    static INICIO: OnceLock<Instant> = OnceLock::new();
    let inicio = *INICIO.get_or_init(Instant::now);
    u64::try_from(inicio.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn now_us_nao_decresce() {
        let mut anterior = now_us();
        for _ in 0..1000 {
            let atual = now_us();
            assert!(atual >= anterior, "relógio retrocedeu: {anterior} -> {atual}");
            anterior = atual;
        }
    }

    #[test]
    fn now_us_avanca_com_sleep() {
        let antes = now_us();
        std::thread::sleep(Duration::from_millis(20));
        let diff = now_us() - antes;
        assert!((15_000..200_000).contains(&diff), "diferença fora da faixa: {diff} µs");
    }
}
