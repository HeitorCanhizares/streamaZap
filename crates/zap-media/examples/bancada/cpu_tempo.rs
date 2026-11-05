//! Tempo de CPU (usuário + núcleo) do processo, via `GetProcessTimes` (Windows).

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

/// Converte um `FILETIME` em unidades de 100 ns.
fn filetime_para_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// Tempo de CPU acumulado do processo, em 100 ns (usuário + núcleo).
pub fn tempo_cpu_processo() -> Option<u64> {
    let mut criacao = FILETIME::default();
    let mut saida = FILETIME::default();
    let mut nucleo = FILETIME::default();
    let mut usuario = FILETIME::default();
    // SAFETY: `GetCurrentProcess` devolve um pseudo-handle sempre válido para o processo atual, e
    // os quatro ponteiros apontam para `FILETIME` locais vivos durante a chamada.
    let r = unsafe { GetProcessTimes(GetCurrentProcess(), &mut criacao, &mut saida, &mut nucleo, &mut usuario) };
    match r {
        Ok(()) => Some(filetime_para_u64(&usuario).saturating_add(filetime_para_u64(&nucleo))),
        Err(_) => None,
    }
}

/// Uso de CPU em % de um núcleo entre duas leituras de `tempo_cpu_processo` (em 100 ns) e o tempo
/// de parede decorrido (em segundos). `parede_s <= 0` dá 0.
pub fn percentual_de_um_nucleo(inicio_100ns: u64, fim_100ns: u64, parede_s: f64) -> f64 {
    if parede_s.is_nan() || parede_s <= 0.0 {
        return 0.0;
    }
    let cpu_s = fim_100ns.saturating_sub(inicio_100ns) as f64 / 10_000_000.0;
    cpu_s / parede_s * 100.0
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn percentual_um_nucleo_cheio() {
        assert!((percentual_de_um_nucleo(0, 10_000_000, 1.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn percentual_meio_nucleo() {
        assert!((percentual_de_um_nucleo(0, 5_000_000, 1.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn percentual_parede_zero_da_zero() {
        assert_eq!(percentual_de_um_nucleo(0, 5_000_000, 0.0), 0.0);
        assert_eq!(percentual_de_um_nucleo(0, 5_000_000, -1.0), 0.0);
    }

    #[test]
    fn percentual_fim_menor_que_inicio_da_zero() {
        assert_eq!(percentual_de_um_nucleo(10, 5, 1.0), 0.0);
    }

    #[cfg(windows)]
    #[test]
    fn tempo_cpu_do_processo_cresce() {
        let a = tempo_cpu_processo();
        assert!(a.is_some());
        let inicio = std::time::Instant::now();
        let mut soma = 0u64;
        while inicio.elapsed() < std::time::Duration::from_millis(50) {
            for i in 0..10_000u64 {
                soma = soma.wrapping_add(std::hint::black_box(i));
            }
        }
        std::hint::black_box(soma);
        let b = tempo_cpu_processo();
        assert!(b.is_some());
        assert!(b.unwrap_or(0) >= a.unwrap_or(0));
    }
}
