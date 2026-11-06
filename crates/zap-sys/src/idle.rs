//! Tempo ocioso do usuário (sem teclado nem mouse), usado para decidir quando é seguro
//! instalar uma atualização sozinho.

use std::time::Duration;

/// Há quanto tempo o usuário não mexe no teclado nem no mouse.
///
/// No Windows usa `GetLastInputInfo` + `GetTickCount`. Em outros sistemas, ou se a chamada
/// falhar, devolve `Duration::ZERO` (nunca considera o PC ocioso por engano).
pub fn idle_time() -> Duration {
    idle_time_impl()
}

#[cfg(windows)]
fn idle_time_impl() -> Duration {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` é um LASTINPUTINFO válido com `cbSize` preenchido.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return Duration::ZERO;
    }
    // SAFETY: GetTickCount não tem pré-condições.
    let agora = unsafe { GetTickCount() };
    // `wrapping_sub` cobre a virada do contador de 32 bits (a cada ~49,7 dias).
    Duration::from_millis(u64::from(agora.wrapping_sub(info.dwTime)))
}

#[cfg(not(windows))]
fn idle_time_impl() -> Duration {
    Duration::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_time_e_plausivel() {
        assert!(idle_time() < Duration::from_secs(86400));
    }
}
