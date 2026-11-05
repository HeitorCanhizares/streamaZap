//! Apoio aos testes de integração (proxy UDP com rede degradada).
#![allow(dead_code)]

pub mod proxy;

/// Põe o processo de teste na mesma resolução de timer que o app usa com sala.
/// Guardar no começo de todo teste de rede: `let _timer = support::timer();`.
pub fn timer() -> zap_sys::platform::TimerResolution {
    zap_sys::platform::TimerResolution::high()
}
