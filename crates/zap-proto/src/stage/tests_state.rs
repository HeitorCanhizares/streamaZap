//! Testes de `stage/state.rs` (mesma tarefa).
use super::*;
use crate::control::ControlMsg;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn is_host_dono_sempre() {
    let st = Stage::new("d", false);
    assert!(st.is_host("d"));
    assert!(!st.is_host("x"));
}

#[test]
fn is_host_lista() {
    let mut st = Stage::new("d", false);
    st.hosts = s(&["a"]);
    assert!(st.is_host("a"));
    assert!(!st.is_host("b"));
}

#[test]
fn msg_dono_primeiro() {
    let mut st = Stage::new("d", true);
    st.hosts = s(&["b", "a"]);
    st.hands = s(&["c"]);
    st.audience_music = true;
    assert_eq!(
        st.msg(),
        ControlMsg::StageState {
            on: true,
            hosts: s(&["d", "b", "a"]),
            hands: s(&["c"]),
            audience_music: true,
        }
    );
}

#[test]
fn from_msg_ida_e_volta() {
    let mut st = Stage::new("d", true);
    st.hosts = s(&["b", "a"]);
    st.hands = s(&["c"]);
    st.audience_music = true;
    let back = Stage::from_msg("d", &st.msg()).expect("StageState");
    assert_eq!(back.msg(), st.msg());
}

#[test]
fn from_msg_ignora_dono_e_duplicatas() {
    let m = ControlMsg::StageState {
        on: true,
        hosts: s(&["d", "a", "a", "b"]),
        hands: s(&["c", "c"]),
        audience_music: false,
    };
    let st = Stage::from_msg("d", &m).expect("StageState");
    match st.msg() {
        ControlMsg::StageState { hosts, hands, .. } => {
            assert_eq!(hosts, s(&["d", "a", "b"]));
            assert_eq!(hands, s(&["c"]));
        }
        _ => panic!("esperava StageState"),
    }
}

#[test]
fn from_msg_outra_mensagem_none() {
    assert!(Stage::from_msg("d", &ControlMsg::StageSet { on: true }).is_none());
    assert!(Stage::from_msg("d", &ControlMsg::Ping { ts_ms: 0 }).is_none());
}
