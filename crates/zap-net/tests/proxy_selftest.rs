//! Roda os testes unitários do proxy de apoio (até existir tests/impaired.rs).
mod support;

#[test]
fn timer_alta_resolucao_ativa() {
    let t = support::timer();
    #[cfg(windows)]
    assert!(t.is_active());
    let _ = &t;
}
