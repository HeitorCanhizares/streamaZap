//! Testes de `stage/admin.rs` (mesma tarefa).
use super::*;

fn members<'a>(v: &[(&'a str, bool)]) -> Vec<StageMember<'a>> {
    v.iter()
        .map(|&(id, has_feature)| StageMember { id, has_feature })
        .collect()
}

fn base(on: bool) -> Stage {
    let mut st = Stage::new("d", on);
    st.hosts = vec!["a".to_string()];
    st
}

fn grupo() -> Vec<StageMember<'static>> {
    members(&[("d", true), ("a", true), ("b", true), ("c", false)])
}

#[test]
fn ligar_com_dono_a_b_c() {
    let mut st = base(false);
    let ch = st.set_on("d", true, &grupo()).unwrap();
    assert!(ch.changed);
    assert_eq!(ch.demoted, vec!["b".to_string()]);
    assert_eq!(ch.must_leave, vec!["c".to_string()]);
    assert!(ch.voters_changed);
    assert!(!ch.drop_mics);
    assert!(st.is_on());
}

#[test]
fn ligar_de_novo() {
    let mut st = base(false);
    st.set_on("d", true, &grupo()).unwrap();
    assert_eq!(st.set_on("d", true, &grupo()).unwrap(), StageChange::default());
}

#[test]
fn desligar() {
    let mut st = base(true);
    st.hands = vec!["x".to_string()];
    let ch = st.set_on("d", false, &grupo()).unwrap();
    assert!(ch.changed);
    assert!(ch.drop_mics);
    assert!(!ch.voters_changed);
    assert!(st.hands().is_empty());
    assert!(!st.is_on());
    assert!(st.is_host("a"));
    assert_eq!(st.set_on("d", false, &grupo()).unwrap(), StageChange::default());
}

#[test]
fn so_dono_liga() {
    let mut st = base(false);
    assert_eq!(st.set_on("a", true, &grupo()), Err(StageError::OwnerOnly));
}

#[test]
fn promover() {
    let mut st = base(true);
    st.hands = vec!["b".to_string()];
    let ch = st.set_host("d", "b", true, &grupo()).unwrap();
    assert!(ch.changed);
    assert!(st.hands().is_empty());
    assert!(st.is_host("b"));
    assert_eq!(st.set_host("d", "b", true, &grupo()).unwrap(), StageChange::default());
    assert_eq!(st.set_host("d", "d", true, &grupo()).unwrap(), StageChange::default());
}

#[test]
fn tirar_com_palco_ligado() {
    let mut st = base(true);
    let ch = st.set_host("d", "a", false, &grupo()).unwrap();
    assert!(ch.changed);
    assert_eq!(ch.demoted, vec!["a".to_string()]);
    assert!(!st.is_host("a"));

    let mut st = base(false);
    let ch = st.set_host("d", "a", false, &grupo()).unwrap();
    assert!(ch.changed);
    assert!(ch.demoted.is_empty());
}

#[test]
fn tirar_dono() {
    let mut st = base(true);
    assert_eq!(
        st.set_host("d", "d", false, &grupo()),
        Err(StageError::OwnerIsAlwaysHost)
    );
    assert_eq!(
        st.set_host("d", "zz", true, &grupo()),
        Err(StageError::UnknownParticipant)
    );
    assert_eq!(st.set_host("a", "b", true, &grupo()), Err(StageError::OwnerOnly));
    assert_eq!(st.set_host("d", "b", false, &grupo()).unwrap(), StageChange::default());
}

#[test]
fn promover_zera_prazo() {
    let mut st = base(true);
    st.declined_at.insert("b".to_string(), Instant::now());
    st.last_raise.insert("b".to_string(), Instant::now());
    st.set_host("d", "b", true, &grupo()).unwrap();
    st.set_host("d", "b", false, &grupo()).unwrap();
    assert!(!st.declined_at.contains_key("b"));
    assert!(!st.last_raise.contains_key("b"));
}

#[test]
fn chave_musica() {
    let mut st = base(true);
    let ch = st.set_audience_music("a", true).unwrap();
    assert!(ch.changed);
    assert!(!ch.voters_changed);
    assert!(st.audience_music());
    let ch = st.set_audience_music("a", false).unwrap();
    assert!(ch.changed);
    assert!(ch.voters_changed);
    assert_eq!(st.set_audience_music("b", true), Err(StageError::HostOnly));
    assert_eq!(st.set_audience_music("a", false).unwrap(), StageChange::default());

    let mut off = base(false);
    assert_eq!(off.set_audience_music("d", true), Err(StageError::StageOff));
}
