//! Testes de `stage/presence.rs` (mesma tarefa).
use super::*;
use crate::ControlMsg;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn base() -> Stage {
    let mut st = Stage::new("d", true);
    st.hosts = vec!["a".to_string(), "b".to_string()];
    st
}

#[test]
fn saida_tira_papel_e_mao() {
    let t0 = Instant::now();
    let mut st = base();
    st.hands = vec!["c".to_string()];
    assert!(st.left("c", t0).changed);
    assert!(st.hands().is_empty());
    assert!(st.left("a", t0).changed);
    assert!(!st.is_host("a"));
    match st.msg() {
        ControlMsg::StageState { hosts, .. } => assert_eq!(hosts, ["d", "b"]),
        outra => panic!("mensagem inesperada: {outra:?}"),
    }
}

#[test]
fn saida_de_plateia_sem_mao() {
    let mut st = base();
    assert_eq!(st.left("zzz", Instant::now()), StageChange::default());
}

#[test]
fn volta_em_59_9s() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    assert!(st.rejoined("a", "a2", t0 + ms(59_900)).changed);
    assert!(st.is_host("a2"));
    assert_eq!(st.hosts.last().map(String::as_str), Some("a2"));
}

#[test]
fn volta_em_60_1s() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    assert_eq!(st.rejoined("a", "a2", t0 + ms(60_100)), StageChange::default());
    assert!(!st.is_host("a2"));
}

#[test]
fn volta_exatamente_60s_vence() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    assert_eq!(st.rejoined("a", "a2", t0 + ms(60_000)), StageChange::default());
    assert!(!st.is_host("a2"));
}

#[test]
fn volta_mesmo_id() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    st.rejoined("a", "a", t0 + Duration::from_secs(10));
    assert!(st.is_host("a"));
}

#[test]
fn volta_so_uma_vez() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    assert!(st.rejoined("a", "a2", t0 + ms(1000)).changed);
    assert_eq!(st.rejoined("a", "a3", t0 + ms(2000)), StageChange::default());
    assert!(!st.is_host("a3"));
}

#[test]
fn dono_saindo() {
    let mut st = base();
    assert_eq!(st.left("d", Instant::now()), StageChange::default());
    assert!(st.is_host("d"));
    assert!(!st.gone_hosts.contains_key("d"));
}

#[test]
fn limpeza() {
    let t0 = Instant::now();
    let mut st = base();
    st.left("a", t0);
    st.left("b", t0 + Duration::from_secs(100));
    assert_eq!(
        st.rejoined("x", "y", t0 + Duration::from_secs(100)),
        StageChange::default()
    );
    assert!(!st.gone_hosts.contains_key("a"));
    assert!(st.gone_hosts.contains_key("b"));
}
