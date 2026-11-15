//! Testes de `stage/apply.rs` (mesma tarefa).
use super::*;
use crate::control::ControlMsg;

fn members<'a>(v: &[(&'a str, bool)]) -> Vec<StageMember<'a>> {
    v.iter()
        .map(|&(id, has_feature)| StageMember { id, has_feature })
        .collect()
}

fn grupo() -> Vec<StageMember<'static>> {
    members(&[("d", true), ("a", true), ("b", true), ("p", true), ("q", true)])
}

fn base(on: bool) -> Stage {
    let mut st = Stage::new("d", on);
    st.hosts = vec!["a".to_string()];
    st
}

fn mao(raised: bool, who: Option<&str>) -> ControlMsg {
    ControlMsg::StageHand {
        raised,
        participant_id: who.map(String::from),
    }
}

#[test]
fn despacho_ligar() {
    let mut st = base(false);
    let r = st.apply("d", &ControlMsg::StageSet { on: true }, &grupo(), Instant::now());
    assert!(matches!(r, Some(Ok(ref c)) if c.changed));
    assert!(st.is_on());
}

#[test]
fn despacho_anfitriao() {
    let mut st = base(true);
    let msg = ControlMsg::StageHost {
        participant_id: "b".into(),
        host: true,
    };
    let r = st.apply("d", &msg, &grupo(), Instant::now());
    assert!(matches!(r, Some(Ok(_))));
    assert!(st.is_host("b"));
}

#[test]
fn despacho_mao_propria() {
    let t0 = Instant::now();
    let mut st = base(true);
    assert!(matches!(st.apply("p", &mao(true, None), &grupo(), t0), Some(Ok(_))));
    assert_eq!(st.hands(), ["p".to_string()]);
    let mut st = base(true);
    assert!(matches!(
        st.apply("p", &mao(true, Some("p")), &grupo(), t0),
        Some(Ok(_))
    ));
    assert_eq!(st.hands(), ["p".to_string()]);
}

#[test]
fn despacho_recusa() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.apply("p", &mao(true, None), &grupo(), t0);
    let r = st.apply("d", &mao(false, Some("p")), &grupo(), t0);
    assert!(matches!(r, Some(Ok(_))));
    assert!(st.hands().is_empty());
    let r = st.apply("p", &mao(true, None), &grupo(), t0 + Duration::from_secs(1));
    assert!(matches!(r, Some(Err(StageError::HandTooSoon(_)))));
}

#[test]
fn mao_de_outro_com_true() {
    let mut st = base(true);
    let r = st.apply("p", &mao(true, Some("q")), &grupo(), Instant::now());
    assert_eq!(r, Some(Err(StageError::NotYourHand)));
}

#[test]
fn recusa_por_nao_dono() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.apply("p", &mao(true, None), &grupo(), t0);
    let r = st.apply("a", &mao(false, Some("p")), &grupo(), t0);
    assert_eq!(r, Some(Err(StageError::OwnerOnly)));
}

#[test]
fn despacho_musica() {
    let mut st = base(true);
    let r = st.apply(
        "a",
        &ControlMsg::StageMusic { audience: true },
        &grupo(),
        Instant::now(),
    );
    assert!(matches!(r, Some(Ok(_))));
    assert!(st.audience_music());
}

#[test]
fn mensagem_nao_palco() {
    let mut st = base(true);
    let t = Instant::now();
    assert!(st.apply("d", &ControlMsg::Ping { ts_ms: 1 }, &grupo(), t).is_none());
    let estado = st.msg();
    assert!(st.apply("d", &estado, &grupo(), t).is_none());
}

#[test]
fn erro_vira_msg() {
    let t0 = Instant::now();
    let mut st = base(true);
    st.apply("p", &mao(true, None), &grupo(), t0);
    let Some(Err(e)) = st.apply("a", &mao(false, Some("p")), &grupo(), t0) else {
        panic!("esperava erro");
    };
    match e.to_msg(Some("r".into())) {
        ControlMsg::Error { code, .. } => assert_eq!(code, "proibido"),
        outro => panic!("mensagem inesperada: {outro:?}"),
    }
}
