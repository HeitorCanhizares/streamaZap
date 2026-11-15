//! Testes de `stage/hands.rs` (mesma tarefa).
use super::*;

fn base(on: bool) -> Stage {
    let mut st = Stage::new("d", on);
    st.hosts = vec!["a".to_string()];
    st
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn levantar_e_baixar() {
    let t0 = Instant::now();
    let mut st = base(true);
    assert!(st.raise_hand("p", true, t0).unwrap().changed);
    assert_eq!(st.hands(), ["p".to_string()]);
    assert!(st.raise_hand("p", false, t0).unwrap().changed);
    assert!(st.hands().is_empty());
}

#[test]
fn levantar_duas_vezes_igual() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.raise_hand("p", true, t0).unwrap();
    assert_eq!(st.raise_hand("p", true, t0 + ms(10)).unwrap(), StageChange::default());
    assert_eq!(st.hands(), ["p".to_string()]);
}

#[test]
fn intervalo_5s() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.raise_hand("p", true, t0).unwrap();
    st.raise_hand("p", false, t0).unwrap();
    assert_eq!(
        st.raise_hand("p", true, t0 + ms(4900)),
        Err(StageError::HandTooSoon(100))
    );
    assert!(st.raise_hand("p", true, t0 + ms(5000)).unwrap().changed);
}

#[test]
fn recusa_30s() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.raise_hand("p", true, t0).unwrap();
    let t1 = t0 + ms(1000);
    let ch = st.decline("d", "p", t1).unwrap();
    assert!(ch.changed);
    assert!(st.hands().is_empty());
    assert_eq!(
        st.raise_hand("p", true, t1 + ms(29900)),
        Err(StageError::HandTooSoon(100))
    );
    assert!(st.raise_hand("p", true, t1 + ms(30000)).unwrap().changed);
}

#[test]
fn ordem_de_chegada() {
    let t0 = Instant::now();
    let mut st = base(true);
    for p in ["p", "q", "r"] {
        st.raise_hand(p, true, t0).unwrap();
    }
    assert_eq!(st.hands(), ["p", "q", "r"].map(String::from));
}

#[test]
fn anfitriao_levantando() {
    let t0 = Instant::now();
    let mut st = base(true);
    assert_eq!(st.raise_hand("a", true, t0), Err(StageError::AlreadyHost));
    assert_eq!(st.raise_hand("d", true, t0), Err(StageError::AlreadyHost));
    assert_eq!(st.raise_hand("a", false, t0).unwrap(), StageChange::default());
}

#[test]
fn palco_desligado() {
    let t0 = Instant::now();
    let mut st = base(false);
    assert_eq!(st.raise_hand("p", true, t0), Err(StageError::StageOff));
    assert_eq!(st.decline("d", "p", t0), Err(StageError::StageOff));
}

#[test]
fn so_dono_recusa() {
    let t0 = Instant::now();
    let mut st = base(true);
    assert_eq!(st.decline("a", "p", t0), Err(StageError::OwnerOnly));
}

#[test]
fn recusar_quem_nao_levantou() {
    let t0 = Instant::now();
    let mut st = base(true);
    assert_eq!(st.decline("d", "z", t0).unwrap(), StageChange::default());
    assert!(st.declined_at.is_empty());
}
