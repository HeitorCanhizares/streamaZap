use super::*;

/// Serializa a mensagem de volta; o round-trip já compara o JSON inteiro, isto só deixa explícito
/// que a chave `t` da saída é a da doc.
fn saida(msg: &ControlMsg) -> serde_json::Value {
    match serde_json::to_value(msg) {
        Ok(v) => v,
        Err(e) => panic!("serializa: {e}"),
    }
}

#[test]
fn sala_lista() {
    let msg = roundtrip(
        r#"{"t":"salas.lista","salas":[{"id":"s_1","codigo":"ZAP-7K2Q","nome":"Corrida em Interlagos","acesso":"aberta","participantes":3}]}"#,
    );
    match &msg {
        ControlMsg::RoomList { salas } => {
            assert_eq!(salas.len(), 1);
            assert_eq!(salas[0]["nome"], "Corrida em Interlagos");
        }
        outro => panic!("esperava salas.lista, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "salas.lista");
}

#[test]
fn sala_entrar() {
    let msg = roundtrip(r#"{"t":"sala.entrar","sala_id":"s_1"}"#);
    match &msg {
        ControlMsg::RoomJoin { room_id } => assert_eq!(room_id, "s_1"),
        outro => panic!("esperava sala.entrar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "sala.entrar");
}

#[test]
fn sala_sair() {
    let msg = roundtrip(r#"{"t":"sala.sair"}"#);
    assert!(
        matches!(msg, ControlMsg::RoomLeave {}),
        "esperava sala.sair, veio {msg:?}"
    );
    let v = saida(&msg);
    assert_eq!(v["t"], "sala.sair");
    assert_eq!(
        v.as_object().map(|o| o.len()),
        Some(1),
        "sala.sair não tem campos além de `t`"
    );
}

#[test]
fn sala_estado() {
    let msg = roundtrip(
        r##"{
            "t":"sala.estado",
            "sala":{"id":"s_1","codigo":"ZAP-7K2Q","nome":"Corrida em Interlagos","dono_id":"p_1","acesso":"aberta","modo":"online","criada_em":"2026-11-04T18:00:00Z"},
            "participantes":[{"id":"p_1","nome":"Rafa","cor":"#5CD6FF","papel":"dono","entrou_em":"2026-11-04T18:00:00Z","versao_app":"3.0.0","protocolo":4,"latencia_ms":18}],
            "trilhas":[
                {"id":"t_1","participant_id":"p_1","numero":1,"tipo":"tela","titulo":"Tela 1","camadas":[{"camada":0,"largura":1920,"altura":1080,"fps":60,"kbps":8000}]},
                {"id":"t_2","participant_id":"p_1","numero":2,"tipo":"camera","titulo":"Câmera integrada","camadas":[{"camada":0,"largura":640,"altura":480,"fps":30,"kbps":1200}]},
                {"id":"t_3","participant_id":"p_1","numero":3,"tipo":"audio","titulo":"Microfone","camadas":[],"audio":{"taxa":48000,"canais":2,"kbps":128}},
                {"id":"t_4","participant_id":"p_2","numero":4,"tipo":"janela","titulo":"cs2.exe","camadas":[{"camada":0,"largura":1280,"altura":720,"fps":60,"kbps":4000}]}
            ]
        }"##,
    );
    match &msg {
        ControlMsg::RoomState {
            room,
            participants,
            tracks,
        } => {
            assert_eq!(room.name, "Corrida em Interlagos");
            assert_eq!(participants.len(), 1);
            assert_eq!(participants[0].name, "Rafa");
            assert_eq!(tracks.len(), 4);
            assert_eq!(tracks[2].kind, TrackKind::Audio);
            assert!(tracks[2].layers.is_empty(), "áudio não tem camadas de vídeo");
        }
        outro => panic!("esperava sala.estado, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "sala.estado");
}

#[test]
fn sala_encerrada() {
    for (json, esperado) in [
        (r#"{"t":"sala.encerrada","motivo":"dono"}"#, "dono"),
        (r#"{"t":"sala.encerrada","motivo":"vazia"}"#, "vazia"),
        (r#"{"t":"sala.encerrada","motivo":"servidor"}"#, "servidor"),
    ] {
        let msg = roundtrip(json);
        match &msg {
            ControlMsg::RoomClosed { reason } => assert_eq!(reason, esperado),
            outro => panic!("esperava sala.encerrada, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "sala.encerrada");
    }
}

#[test]
fn participante_entrou() {
    let msg = roundtrip(
        r##"{"t":"participante.entrou","participante":{"id":"p_2","nome":"Ju","cor":"#FF8A65","papel":"membro","entrou_em":"2026-11-04T18:05:00Z","versao_app":"3.0.0","protocolo":4}}"##,
    );
    match &msg {
        ControlMsg::ParticipantJoined { participant } => {
            assert_eq!(participant.id, "p_2");
            assert_eq!(participant.name, "Ju");
            assert_eq!(participant.role, Role::Member);
            assert!(participant.latency_ms.is_none());
        }
        outro => panic!("esperava participante.entrou, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "participante.entrou");
}

#[test]
fn participante_saiu() {
    for (json, esperado) in [
        (
            r#"{"t":"participante.saiu","participante_id":"p_2","motivo":"saiu"}"#,
            "saiu",
        ),
        (
            r#"{"t":"participante.saiu","participante_id":"p_2","motivo":"caiu"}"#,
            "caiu",
        ),
        (
            r#"{"t":"participante.saiu","participante_id":"p_2","motivo":"removido"}"#,
            "removido",
        ),
    ] {
        let msg = roundtrip(json);
        match &msg {
            ControlMsg::ParticipantLeft { participant_id, reason } => {
                assert_eq!(participant_id, "p_2");
                assert_eq!(reason, esperado);
            }
            outro => panic!("esperava participante.saiu, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "participante.saiu");
    }
}

#[test]
fn participante_atualizou() {
    let msg = roundtrip(
        r##"{"t":"participante.atualizou","participante":{"id":"p_2","nome":"Ju","cor":"#FF8A65","papel":"membro","entrou_em":"2026-11-04T18:05:00Z","versao_app":"3.0.0","protocolo":4,"latencia_ms":42,"assistindo":"t_1"}}"##,
    );
    match &msg {
        ControlMsg::ParticipantUpdated { participant } => {
            assert_eq!(participant.latency_ms, Some(42));
            assert_eq!(participant.watching.as_deref(), Some("t_1"));
        }
        outro => panic!("esperava participante.atualizou, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "participante.atualizou");
}

#[test]
fn perfil_atualizar_parcial() {
    // só nome: a cor não pode sair no JSON
    let msg = roundtrip(r#"{"t":"perfil.atualizar","nome":"Marcão"}"#);
    match &msg {
        ControlMsg::ProfileUpdate { name, color } => {
            assert_eq!(name.as_deref(), Some("Marcão"));
            assert!(color.is_none());
        }
        outro => panic!("esperava perfil.atualizar, veio {outro:?}"),
    }
    assert!(saida(&msg).get("cor").is_none(), "cor ausente não deve sair");

    // só cor: o nome não pode sair no JSON
    let msg = roundtrip(r##"{"t":"perfil.atualizar","cor":"#B388FF"}"##);
    match &msg {
        ControlMsg::ProfileUpdate { name, color } => {
            assert!(name.is_none());
            assert_eq!(color.as_deref(), Some("#B388FF"));
        }
        outro => panic!("esperava perfil.atualizar, veio {outro:?}"),
    }
    assert!(saida(&msg).get("nome").is_none(), "nome ausente não deve sair");

    // nome e cor juntos
    let msg = roundtrip(r##"{"t":"perfil.atualizar","nome":"Rafa","cor":"#5CD6FF"}"##);
    match &msg {
        ControlMsg::ProfileUpdate { name, color } => {
            assert_eq!(name.as_deref(), Some("Rafa"));
            assert_eq!(color.as_deref(), Some("#5CD6FF"));
        }
        outro => panic!("esperava perfil.atualizar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "perfil.atualizar");
}

#[test]
fn chat_enviar() {
    let msg = roundtrip(r#"{"t":"chat.enviar","texto":"bora pro S1?","cliente_id":"c_77"}"#);
    match &msg {
        ControlMsg::ChatSend { text, client_id } => {
            assert_eq!(text, "bora pro S1?");
            assert_eq!(client_id, "c_77");
        }
        outro => panic!("esperava chat.enviar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "chat.enviar");
}

#[test]
fn chat_mensagem_com_e_sem_cliente_id() {
    // com cliente_id: é a mensagem de quem enviou
    let msg = roundtrip(
        r##"{"t":"chat.mensagem","id":"m_10","participante_id":"p_1","nome":"Rafa","cor":"#5CD6FF","texto":"bora pro S1?","em":"2026-11-04T18:10:00Z","cliente_id":"c_77"}"##,
    );
    match &msg {
        ControlMsg::ChatMessage {
            id,
            participant_id,
            text,
            client_id,
            ..
        } => {
            assert_eq!(id, "m_10");
            assert_eq!(participant_id, "p_1");
            assert_eq!(text, "bora pro S1?");
            assert_eq!(client_id.as_deref(), Some("c_77"));
        }
        outro => panic!("esperava chat.mensagem, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "chat.mensagem");

    // sem cliente_id: é para os outros participantes
    let msg = roundtrip(
        r##"{"t":"chat.mensagem","id":"m_11","participante_id":"p_2","nome":"Ju","cor":"#FF8A65","texto":"chegando","em":"2026-11-04T18:10:05Z"}"##,
    );
    match &msg {
        ControlMsg::ChatMessage { client_id, name, .. } => {
            assert!(client_id.is_none());
            assert_eq!(name, "Ju");
        }
        outro => panic!("esperava chat.mensagem, veio {outro:?}"),
    }
    assert!(
        saida(&msg).get("cliente_id").is_none(),
        "cliente_id ausente não deve sair"
    );
}

#[test]
fn chat_digitando_vazio() {
    // C→S: sem campos
    let msg = roundtrip(r#"{"t":"chat.digitando"}"#);
    match &msg {
        ControlMsg::ChatTyping { participant_id } => assert!(participant_id.is_none()),
        outro => panic!("esperava chat.digitando, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "chat.digitando");
    assert!(
        v.get("participante_id").is_none(),
        "C→S não tem participante_id na saída"
    );
}

#[test]
fn chat_digitando_com_id() {
    // S→C: com participante_id de quem está digitando
    let msg = roundtrip(r#"{"t":"chat.digitando","participante_id":"p_2"}"#);
    match &msg {
        ControlMsg::ChatTyping { participant_id } => assert_eq!(participant_id.as_deref(), Some("p_2")),
        outro => panic!("esperava chat.digitando, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["participante_id"], "p_2");
}
