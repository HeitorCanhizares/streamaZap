//! Testes de `stage/error.rs` (mesma tarefa).
use crate::control::ControlMsg;
use crate::stage::StageError;

fn todas() -> Vec<StageError> {
    vec![
        StageError::OwnerOnly,
        StageError::HostOnly,
        StageError::AudienceCannotPublish,
        StageError::MicOutsideStage,
        StageError::MusicLocked,
        StageError::StageOff,
        StageError::AlreadyHost,
        StageError::NotYourHand,
        StageError::UnknownParticipant,
        StageError::OwnerIsAlwaysHost,
        StageError::HandTooSoon(100),
    ]
}

#[test]
fn codigo_e_campo_por_variante() {
    let proibidos = [
        StageError::OwnerOnly,
        StageError::HostOnly,
        StageError::AudienceCannotPublish,
        StageError::MicOutsideStage,
        StageError::MusicLocked,
    ];
    for e in &proibidos {
        assert_eq!(e.code(), "proibido", "{e:?}");
        assert_eq!(e.field(), "palco", "{e:?}");
    }
    let invalidos = [StageError::StageOff, StageError::AlreadyHost, StageError::NotYourHand];
    for e in &invalidos {
        assert_eq!(e.code(), "invalido", "{e:?}");
        assert_eq!(e.field(), "palco", "{e:?}");
    }
    for e in [StageError::UnknownParticipant, StageError::OwnerIsAlwaysHost] {
        assert_eq!(e.code(), "invalido", "{e:?}");
        assert_eq!(e.field(), "participante_id", "{e:?}");
    }
    assert_eq!(StageError::HandTooSoon(100).code(), "limite");
    assert_eq!(StageError::HandTooSoon(100).field(), "palco");
    assert_eq!(todas().len(), 11);
}

#[test]
fn retry_ms_so_em_hand_too_soon() {
    assert_eq!(StageError::HandTooSoon(1234).retry_ms(), Some(1234));
    for e in todas().iter().filter(|e| !matches!(e, StageError::HandTooSoon(_))) {
        assert_eq!(e.retry_ms(), None, "{e:?}");
    }
    assert_eq!(StageError::StageOff.retry_ms(), None);
}

#[test]
fn mensagens_em_portugues() {
    assert_eq!(
        StageError::AudienceCannotPublish.to_string(),
        "No modo palco, só os anfitriões transmitem"
    );
    assert_eq!(
        StageError::MicOutsideStage.to_string(),
        "O microfone só existe no modo palco, para os anfitriões"
    );
    assert_eq!(
        StageError::MusicLocked.to_string(),
        "No palco, a música fica com os anfitriões"
    );
    assert_eq!(
        StageError::HandTooSoon(100).to_string(),
        "Espere 1 s para pedir de novo"
    );
    assert_eq!(
        StageError::HandTooSoon(4900).to_string(),
        "Espere 5 s para pedir de novo"
    );
}

#[test]
fn to_msg_limite_com_ref_e_tentar_em_ms() {
    let msg = StageError::HandTooSoon(4000).to_msg(Some("r1".into()));
    match &msg {
        ControlMsg::Error {
            code,
            reference,
            field,
            retry_ms,
            plan,
            protocol_min,
            protocol_max,
            ..
        } => {
            assert_eq!(code, "limite");
            assert_eq!(reference.as_deref(), Some("r1"));
            assert_eq!(field.as_deref(), Some("palco"));
            assert_eq!(*retry_ms, Some(4000));
            assert!(plan.is_none());
            assert!(protocol_min.is_none());
            assert!(protocol_max.is_none());
        }
        outra => panic!("esperava erro, veio {outra:?}"),
    }
    let json = serde_json::to_string(&msg).unwrap_or_default();
    assert!(json.contains("\"t\":\"erro\""), "{json}");
    assert!(json.contains("\"campo\":\"palco\""), "{json}");
}

#[test]
fn to_msg_participante_sem_ref_nem_retry() {
    let msg = StageError::OwnerIsAlwaysHost.to_msg(None);
    match msg {
        ControlMsg::Error {
            field,
            retry_ms,
            reference,
            ..
        } => {
            assert_eq!(field.as_deref(), Some("participante_id"));
            assert_eq!(retry_ms, None);
            assert_eq!(reference, None);
        }
        outra => panic!("esperava erro, veio {outra:?}"),
    }
}
