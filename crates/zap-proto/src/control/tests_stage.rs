//! Testes das mensagens `palco.*` e dos campos novos de `trilha.*` e `ola` (tarefa contrato-02).

use super::*;
use crate::types::AudioSource;

const OLA_BASE: &str =
    r##""t":"ola","protocolo":4,"versao_app":"3.0.0","instalacao_id":"i1","nome":"Rafa","cor":"#5CD6FF""##;

fn publicar_audio(extra: &str) -> String {
    format!(
        r#"{{"t":"trilha.publicar","local_id":"l1","tipo":"audio","titulo":"Microfone","camadas":[],"audio":{{"taxa":48000,"canais":1,"kbps":64}}{extra}}}"#
    )
}

fn fonte_de(json: &str) -> Option<AudioSource> {
    let msg: ControlMsg = serde_json::from_str(json).expect("desserializa");
    match msg {
        ControlMsg::TrackPublish { source, .. } => source,
        outro => panic!("esperava trilha.publicar, veio {outro:?}"),
    }
}

#[test]
fn palco_ligar_roundtrip() {
    let msg = roundtrip(r#"{"t":"palco.ligar","ligado":true}"#);
    assert_eq!(msg, ControlMsg::StageSet { on: true });
}

#[test]
fn palco_anfitriao_roundtrip() {
    let msg = roundtrip(r#"{"t":"palco.anfitriao","participante_id":"p_2","anfitriao":true}"#);
    assert_eq!(
        msg,
        ControlMsg::StageHost {
            participant_id: "p_2".into(),
            host: true
        }
    );
    let msg = roundtrip(r#"{"t":"palco.anfitriao","participante_id":"p_2","anfitriao":false}"#);
    assert_eq!(
        msg,
        ControlMsg::StageHost {
            participant_id: "p_2".into(),
            host: false
        }
    );
}

#[test]
fn palco_mao_propria_roundtrip() {
    let msg = roundtrip(r#"{"t":"palco.mao","levantada":true}"#);
    assert_eq!(
        msg,
        ControlMsg::StageHand {
            raised: true,
            participant_id: None
        }
    );
    let v = serde_json::to_value(&msg).expect("serializa");
    assert!(v.get("participante_id").is_none());
}

#[test]
fn palco_mao_recusa_roundtrip() {
    let msg = roundtrip(r#"{"t":"palco.mao","levantada":false,"participante_id":"p_3"}"#);
    assert_eq!(
        msg,
        ControlMsg::StageHand {
            raised: false,
            participant_id: Some("p_3".into())
        }
    );
}

#[test]
fn palco_musica_roundtrip() {
    let msg = roundtrip(r#"{"t":"palco.musica","plateia":true}"#);
    assert_eq!(msg, ControlMsg::StageMusic { audience: true });
}

#[test]
fn palco_estado_roundtrip() {
    let msg = roundtrip(
        r#"{"t":"palco.estado","ligado":true,"anfitrioes":["p_1","p_2"],"maos":["p_3"],"plateia_musica":false}"#,
    );
    assert_eq!(
        msg,
        ControlMsg::StageState {
            on: true,
            hosts: vec!["p_1".into(), "p_2".into()],
            hands: vec!["p_3".into()],
            audience_music: false
        }
    );
}

#[test]
fn palco_estado_campos_ausentes() {
    let msg: ControlMsg =
        serde_json::from_str(r#"{"t":"palco.estado","ligado":true,"anfitrioes":["p_1"]}"#).expect("desserializa");
    assert_eq!(
        msg,
        ControlMsg::StageState {
            on: true,
            hosts: vec!["p_1".into()],
            hands: vec![],
            audience_music: false
        }
    );
    // A escrita sempre emite as quatro chaves.
    let v = serde_json::to_value(&msg).expect("serializa");
    assert_eq!(v["maos"], serde_json::json!([]));
    assert_eq!(v["plateia_musica"], serde_json::json!(false));
}

#[test]
fn ola_com_recursos() {
    let msg = roundtrip(&format!(r#"{{{OLA_BASE},"recursos":["palco"]}}"#));
    match msg {
        ControlMsg::Hello { features, .. } => assert_eq!(features, vec!["palco".to_string()]),
        outro => panic!("esperava ola, veio {outro:?}"),
    }
    let msg = roundtrip(&format!("{{{OLA_BASE}}}"));
    match &msg {
        ControlMsg::Hello { features, .. } => assert!(features.is_empty()),
        outro => panic!("esperava ola, veio {outro:?}"),
    }
    let v = serde_json::to_value(&msg).expect("serializa");
    assert!(v.get("recursos").is_none());
}

#[test]
fn trilha_publicar_fonte_microfone() {
    let json = publicar_audio(r#","fonte":"microfone""#);
    let msg = roundtrip(&json);
    match msg {
        ControlMsg::TrackPublish { source, .. } => assert_eq!(source, Some(AudioSource::Mic)),
        outro => panic!("esperava trilha.publicar, veio {outro:?}"),
    }
    assert_eq!(fonte_de(&publicar_audio("")), None);
    assert_eq!(fonte_de(&publicar_audio(r#","fonte":"x""#)), None);
    assert_eq!(fonte_de(&publicar_audio(r#","fonte":3"#)), None);
    assert_eq!(fonte_de(&publicar_audio(r#","fonte":null"#)), None);
}

#[test]
fn trilha_atualizar_mudo() {
    let msg = roundtrip(r#"{"t":"trilha.atualizar","trilha_id":"t1","mudo":true}"#);
    match msg {
        ControlMsg::TrackUpdate { muted, .. } => assert_eq!(muted, Some(true)),
        outro => panic!("esperava trilha.atualizar, veio {outro:?}"),
    }
    let msg = roundtrip(r#"{"t":"trilha.atualizar","trilha_id":"t1","mudo":false}"#);
    match msg {
        ControlMsg::TrackUpdate { muted, .. } => assert_eq!(muted, Some(false)),
        outro => panic!("esperava trilha.atualizar, veio {outro:?}"),
    }
    let msg = roundtrip(r#"{"t":"trilha.atualizar","trilha_id":"t1"}"#);
    match msg {
        ControlMsg::TrackUpdate { muted, .. } => assert_eq!(muted, None),
        outro => panic!("esperava trilha.atualizar, veio {outro:?}"),
    }
}

#[test]
fn trilha_publicada_com_microfone() {
    let msg = roundtrip(
        r#"{"t":"trilha.publicada","trilha":{"id":"t1","participante_id":"p1","numero":2,"tipo":"audio","titulo":"Microfone","camadas":[],"audio":{"taxa":48000,"canais":1,"kbps":64},"fonte":"microfone","mudo":true},"local_id":"l1"}"#,
    );
    match msg {
        ControlMsg::TrackPublished { track, .. } => {
            assert_eq!(track.source, Some(AudioSource::Mic));
            assert!(track.muted);
        }
        outro => panic!("esperava trilha.publicada, veio {outro:?}"),
    }
}

#[test]
fn sala_encerrada_atualizar_app() {
    let msg = roundtrip(r#"{"t":"sala.encerrada","motivo":"atualizar_app"}"#);
    assert_eq!(
        msg,
        ControlMsg::RoomClosed {
            reason: "atualizar_app".into()
        }
    );
}

#[test]
fn erro_palco() {
    let msg = roundtrip(r#"{"t":"erro","codigo":"proibido","mensagem":"só o dono","campo":"palco"}"#);
    match msg {
        ControlMsg::Error { code, field, .. } => {
            assert_eq!(code, "proibido");
            assert_eq!(field.as_deref(), Some("palco"));
        }
        outro => panic!("esperava erro, veio {outro:?}"),
    }
    let msg = roundtrip(r#"{"t":"erro","codigo":"limite","mensagem":"devagar","tentar_em_ms":4000}"#);
    match msg {
        ControlMsg::Error { code, retry_ms, .. } => {
            assert_eq!(code, "limite");
            assert_eq!(retry_ms, Some(4000));
        }
        outro => panic!("esperava erro, veio {outro:?}"),
    }
}
