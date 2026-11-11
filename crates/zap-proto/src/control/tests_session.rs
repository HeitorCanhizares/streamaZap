use super::*;

/// Lê a chave `t` da saída serializada de uma mensagem (o round-trip já compara o JSON inteiro;
/// isto deixa explícito que o tipo volta certo).
fn saida(msg: &ControlMsg) -> serde_json::Value {
    match serde_json::to_value(msg) {
        Ok(v) => v,
        Err(e) => panic!("serializa: {e}"),
    }
}

#[test]
fn ping_pong() {
    let msg = roundtrip(r#"{"t":"ping","ts_ms":1700000000123}"#);
    match &msg {
        ControlMsg::Ping { ts_ms } => assert_eq!(*ts_ms, 1_700_000_000_123),
        outro => panic!("esperava ping, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "ping");

    let msg = roundtrip(r#"{"t":"pong","ts_ms":1700000000123}"#);
    match &msg {
        ControlMsg::Pong { ts_ms } => assert_eq!(*ts_ms, 1_700_000_000_123),
        outro => panic!("esperava pong, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "pong");
}

#[test]
fn erro_com_ref() {
    let msg = roundtrip(r#"{"t":"erro","codigo":"atrasado","mensagem":"quadro atrasado","ref":"r1"}"#);
    match &msg {
        ControlMsg::Error {
            code,
            message,
            reference,
            ..
        } => {
            assert_eq!(code, "atrasado");
            assert_eq!(message, "quadro atrasado");
            assert_eq!(reference.as_deref(), Some("r1"));
        }
        outro => panic!("esperava erro, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "erro");
}

#[test]
fn erro_sem_ref_omite_campo() {
    let msg = roundtrip(r#"{"t":"erro","codigo":"nao_encontrado","mensagem":"sala inexistente"}"#);
    match &msg {
        ControlMsg::Error {
            code,
            message,
            reference,
            ..
        } => {
            assert_eq!(code, "nao_encontrado");
            assert_eq!(message, "sala inexistente");
            assert!(reference.is_none());
        }
        outro => panic!("esperava erro, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "erro");
    assert!(v.get("ref").is_none(), "campo ref não deve sair quando é None");
}

#[test]
fn ola_online() {
    let msg = roundtrip(
        r##"{"t":"ola","protocolo":4,"versao_app":"3.0.0","instalacao_id":"i1","nome":"Rafa","cor":"#5CD6FF","ingresso":"aW5ncmVzc28=","previa":true}"##,
    );
    match &msg {
        ControlMsg::Hello {
            protocol,
            app_version,
            install_id,
            name,
            color,
            ticket,
            room_id,
            preview,
            token,
        } => {
            assert_eq!(*protocol, 4);
            assert_eq!(app_version, "3.0.0");
            assert_eq!(install_id, "i1");
            assert_eq!(name, "Rafa");
            assert_eq!(color, "#5CD6FF");
            assert_eq!(ticket.as_deref(), Some("aW5ncmVzc28="));
            assert!(room_id.is_none());
            assert_eq!(*preview, Some(true));
            assert!(token.is_none());
        }
        outro => panic!("esperava ola, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "ola");
}

#[test]
fn ola_local() {
    let msg = roundtrip(
        r##"{"t":"ola","protocolo":4,"versao_app":"3.0.0","instalacao_id":"i1","nome":"Rafa","cor":"#5CD6FF","sala_id":"s_1"}"##,
    );
    match &msg {
        ControlMsg::Hello {
            protocol,
            install_id,
            ticket,
            room_id,
            preview,
            token,
            ..
        } => {
            assert_eq!(*protocol, 4);
            assert_eq!(install_id, "i1");
            assert!(ticket.is_none());
            assert_eq!(room_id.as_deref(), Some("s_1"));
            assert!(preview.is_none());
            assert!(token.is_none());
        }
        outro => panic!("esperava ola, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "ola");
}

#[test]
fn ola_reversa_token() {
    let msg = roundtrip(
        r##"{"t":"ola","protocolo":4,"versao_app":"3.0.0","instalacao_id":"i2","nome":"Ju","cor":"#FF8A5C","token":"dG9rZW4tZGUtdGVzdGU="}"##,
    );
    match &msg {
        ControlMsg::Hello {
            name,
            color,
            ticket,
            room_id,
            token,
            ..
        } => {
            assert_eq!(name, "Ju");
            assert_eq!(color, "#FF8A5C");
            assert!(ticket.is_none());
            assert!(room_id.is_none());
            assert_eq!(token.as_deref(), Some("dG9rZW4tZGUtdGVzdGU="));
        }
        outro => panic!("esperava ola, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "ola");
}

#[test]
fn desafio() {
    let msg = roundtrip(r#"{"t":"desafio","nonce":"bm9uY2UtZGUtdGVzdGUtZGUtMzItYnl0ZXMtYXF1aT0=","acesso":"senha"}"#);
    match &msg {
        ControlMsg::Challenge { nonce, access } => {
            assert_eq!(nonce, "bm9uY2UtZGUtdGVzdGUtZGUtMzItYnl0ZXMtYXF1aT0=");
            assert_eq!(*access, Access::Password);
        }
        outro => panic!("esperava desafio, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "desafio");
}

#[test]
fn autenticar() {
    let msg = roundtrip(r#"{"t":"autenticar","hmac":"aG1hYy1kZS10ZXN0ZS1iYXNlNjQ="}"#);
    match &msg {
        ControlMsg::Authenticate { hmac } => assert_eq!(hmac, "aG1hYy1kZS10ZXN0ZS1iYXNlNjQ="),
        outro => panic!("esperava autenticar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "autenticar");
}

#[test]
fn bem_vindo() {
    let msg = roundtrip(
        r#"{"t":"bem_vindo","participante_id":"p_2","sala":{"id":"s_1","codigo":"ZAP-7K2Q","nome":"Corrida","dono_id":"p_1","acesso":"senha","modo":"online","criada_em":"2026-01-01T00:00:00Z"},"relogio_us":123456789}"#,
    );
    match &msg {
        ControlMsg::Welcome {
            participant_id,
            room,
            clock_us,
            ..
        } => {
            assert_eq!(participant_id, "p_2");
            assert_eq!(room.id, "s_1");
            assert_eq!(room.name, "Corrida");
            assert_eq!(room.owner_id, "p_1");
            assert_eq!(room.access, Access::Password);
            assert_eq!(room.mode, RoomMode::Online);
            assert_eq!(*clock_us, Some(123_456_789));
        }
        outro => panic!("esperava bem_vindo, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "bem_vindo");
    assert_eq!(v["sala"]["modo"], "online");
}

#[test]
fn welcome_com_segredo_midia() {
    let com = roundtrip(
        r#"{"t":"bem_vindo","participante_id":"p_2","sala":{"id":"s_1","codigo":"ZAP-7K2Q","nome":"Corrida","dono_id":"p_1","acesso":"aberta","modo":"local","criada_em":"2026-01-01T00:00:00Z"},"segredo_midia":"AAAA"}"#,
    );
    match &com {
        ControlMsg::Welcome {
            media_secret, clock_us, ..
        } => {
            assert_eq!(media_secret.as_deref(), Some("AAAA"));
            assert!(clock_us.is_none());
        }
        outro => panic!("esperava bem_vindo, veio {outro:?}"),
    }
    assert_eq!(saida(&com)["segredo_midia"], "AAAA");

    // Sem o campo: None, e a saída não o inclui.
    let sem = roundtrip(
        r#"{"t":"bem_vindo","participante_id":"p_2","sala":{"id":"s_1","codigo":"ZAP-7K2Q","nome":"Corrida","dono_id":"p_1","acesso":"aberta","modo":"online","criada_em":"2026-01-01T00:00:00Z"}}"#,
    );
    match &sem {
        ControlMsg::Welcome { media_secret, .. } => assert!(media_secret.is_none()),
        outro => panic!("esperava bem_vindo, veio {outro:?}"),
    }
    assert!(saida(&sem).get("segredo_midia").is_none());
}

#[test]
fn recusado() {
    let msg = roundtrip(r#"{"t":"recusado","codigo":"senha_invalida","mensagem":"senha incorreta"}"#);
    match &msg {
        ControlMsg::Refused { code, message } => {
            assert_eq!(code, "senha_invalida");
            assert_eq!(message, "senha incorreta");
        }
        outro => panic!("esperava recusado, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "recusado");
}

#[test]
fn estatisticas() {
    let msg = roundtrip(
        r#"{"t":"estatisticas","ping_ms":42,"trilhas":[{"trilha_id":"t_1","camada":0,"kbps":5400,"fps":60.0,"suavizacao_ms":120,"perdidos":2},{"trilha_id":"t_2","camada":2,"kbps":300,"fps":15.0,"suavizacao_ms":80,"perdidos":0}]}"#,
    );
    match &msg {
        ControlMsg::Stats { ping_ms, tracks } => {
            assert_eq!(*ping_ms, 42);
            assert_eq!(tracks.len(), 2);
            assert_eq!(tracks[0].track_id, "t_1");
            assert_eq!(tracks[0].kbps, 5400);
            assert_eq!(tracks[1].layer, 2);
            assert_eq!(tracks[1].lost, 0);
        }
        outro => panic!("esperava estatisticas, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "estatisticas");
}

#[test]
fn relatorio_completo() {
    // exemplo de docs/api/midia.md, com perda 0.5 para não aparecer ruído de f32
    let msg = roundtrip(
        r#"{"t":"relatorio","chegadas":[[2,0,1841,3021440,99812345,21844],[2,0,1842,3038110,99829101,9120]],"sondas":[[7,0,3101000,99830000,1200]],"perdidos":[[2,0,1840]],"perda":0.5,"banda_kbps":5400,"estado":"normal","buffer_ms":120}"#,
    );
    match &msg {
        ControlMsg::Report(r) => {
            assert_eq!(r.arrivals.len(), 2);
            assert_eq!(r.arrivals[0], (2, 0, 1841, 3_021_440, 99_812_345, 21_844));
            assert_eq!(r.probes, vec![(7, 0, 3_101_000, 99_830_000, 1200)]);
            assert_eq!(r.lost, vec![(2, 0, 1840)]);
            assert!((r.loss - 0.5).abs() < f32::EPSILON);
            assert_eq!(r.bandwidth_kbps, Some(5400));
            assert_eq!(r.state, Some(DelayState::Normal));
            assert_eq!(r.buffer_ms, Some(120));
        }
        outro => panic!("esperava relatorio, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "relatorio");
}

#[test]
fn relatorio_minimo() {
    let msg = roundtrip(
        r#"{"t":"relatorio","chegadas":[[2,0,1841,3021440,99812345,21844]],"perdidos":[[2,0,1840]],"perda":0.5}"#,
    );
    match &msg {
        ControlMsg::Report(r) => {
            assert_eq!(r.arrivals, vec![(2, 0, 1841, 3_021_440, 99_812_345, 21_844)]);
            assert_eq!(r.lost, vec![(2, 0, 1840)]);
            assert!((r.loss - 0.5).abs() < f32::EPSILON);
            assert!(r.probes.is_empty());
            assert!(r.bandwidth_kbps.is_none());
            assert!(r.state.is_none());
            assert!(r.buffer_ms.is_none());
        }
        outro => panic!("esperava relatorio, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "relatorio");
    assert!(v.get("sondas").is_none(), "sondas vazias não saem");
    assert!(v.get("banda_kbps").is_none());
    assert!(v.get("estado").is_none());
    assert!(v.get("buffer_ms").is_none());
}

#[test]
fn relatorio_estado_valores() {
    let casos = [
        ("sobrecarga", DelayState::Overuse),
        ("normal", DelayState::Normal),
        ("ociosa", DelayState::Underuse),
    ];
    for (texto, esperado) in casos {
        let json = format!(r#"{{"t":"relatorio","chegadas":[],"perdidos":[],"perda":0.0,"estado":"{texto}"}}"#);
        let msg = roundtrip(&json);
        match &msg {
            ControlMsg::Report(r) => assert_eq!(r.state, Some(esperado), "estado {texto}"),
            outro => panic!("esperava relatorio, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "relatorio");
    }
}

#[test]
fn mensagem_desconhecida() {
    match serde_json::from_str::<ControlMsg>(r#"{"t":"xyz"}"#) {
        Err(_) => {}
        Ok(outro) => panic!("tipo desconhecido não deveria desserializar, veio {outro:?}"),
    }
}

/// Extras de um `ControlMsg::Error`, para os testes abaixo.
struct Extras {
    field: Option<String>,
    plan: Option<crate::plan::PlanId>,
    retry_ms: Option<u64>,
    protocol_min: Option<u32>,
    protocol_max: Option<u32>,
}

fn extras_do_erro(msg: &ControlMsg) -> Extras {
    match msg {
        ControlMsg::Error {
            field,
            plan,
            retry_ms,
            protocol_min,
            protocol_max,
            ..
        } => Extras {
            field: field.clone(),
            plan: *plan,
            retry_ms: *retry_ms,
            protocol_min: *protocol_min,
            protocol_max: *protocol_max,
        },
        outro => panic!("esperava erro, veio {outro:?}"),
    }
}

#[test]
fn erro_plano_insuficiente_roundtrip() {
    let msg = roundtrip(
        r#"{"t":"erro","codigo":"plano_insuficiente","mensagem":"m","ref":"l_1","plano":"hd","campo":"camadas"}"#,
    );
    let e = extras_do_erro(&msg);
    assert_eq!(e.field.as_deref(), Some("camadas"));
    assert_eq!(e.plan, Some(crate::plan::PlanId::Hd));
}

#[test]
fn erro_sem_extras_nao_ganha_chaves() {
    let msg = roundtrip(r#"{"t":"erro","codigo":"x","mensagem":"m"}"#);
    let e = extras_do_erro(&msg);
    assert!(e.field.is_none() && e.plan.is_none());
    assert!(e.retry_ms.is_none() && e.protocol_min.is_none() && e.protocol_max.is_none());
}

#[test]
fn erro_extras_ruins_viram_none_sem_derrubar() {
    let casos = [
        r#"{"t":"erro","codigo":"x","mensagem":"m","plano":"ouro"}"#,
        r#"{"t":"erro","codigo":"x","mensagem":"m","tentar_em_ms":"abc"}"#,
        r#"{"t":"erro","codigo":"x","mensagem":"m","campo":5}"#,
    ];
    for json in casos {
        let msg: ControlMsg = serde_json::from_str(json).expect("desserializa");
        match &msg {
            ControlMsg::Error { code, message, .. } => {
                assert_eq!(code, "x", "{json}");
                assert_eq!(message, "m", "{json}");
            }
            outro => panic!("esperava erro, veio {outro:?}"),
        }
        let e = extras_do_erro(&msg);
        assert!(e.field.is_none() && e.plan.is_none() && e.retry_ms.is_none(), "{json}");
    }
}

#[test]
fn erro_protocolo_min_max_e_tentar_em() {
    let msg = roundtrip(r#"{"t":"erro","codigo":"x","mensagem":"m","protocolo_min":3,"protocolo_max":4}"#);
    let e = extras_do_erro(&msg);
    assert_eq!((e.protocol_min, e.protocol_max), (Some(3), Some(4)));

    let msg = roundtrip(r#"{"t":"erro","codigo":"x","mensagem":"m","tentar_em_ms":1500}"#);
    assert_eq!(extras_do_erro(&msg).retry_ms, Some(1500));
}

#[test]
fn construtor_error_igual_ao_literal() {
    assert_eq!(
        ControlMsg::error("x", "m", Some("r".into())),
        ControlMsg::Error {
            code: "x".into(),
            message: "m".into(),
            reference: Some("r".into()),
            field: None,
            plan: None,
            retry_ms: None,
            protocol_min: None,
            protocol_max: None,
        }
    );
}
