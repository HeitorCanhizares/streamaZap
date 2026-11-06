//! Mensagens da música da sala (`musica.*`).
use super::*;

#[test]
fn musica_adicionar_com_e_sem_titulo() {
    let msg = roundtrip(r#"{"t":"musica.adicionar","video_id":"dQw4w9WgXcQ","titulo":"Música","canal":"Canal"}"#);
    assert_eq!(
        msg,
        ControlMsg::MusicAdd {
            video_id: "dQw4w9WgXcQ".into(),
            title: Some("Música".into()),
            channel: Some("Canal".into()),
        }
    );
    let msg = roundtrip(r#"{"t":"musica.adicionar","video_id":"dQw4w9WgXcQ"}"#);
    assert!(matches!(
        msg,
        ControlMsg::MusicAdd {
            title: None,
            channel: None,
            ..
        }
    ));
}

#[test]
fn musica_remover_e_pular() {
    assert_eq!(
        roundtrip(r#"{"t":"musica.remover","item_id":"mu_1"}"#),
        ControlMsg::MusicRemove { item_id: "mu_1".into() }
    );
    assert_eq!(
        roundtrip(r#"{"t":"musica.pular","item_id":"mu_1","voto":false}"#),
        ControlMsg::MusicSkip {
            item_id: "mu_1".into(),
            vote: false
        }
    );
}

#[test]
fn musica_info_so_com_o_que_mudou() {
    let msg = roundtrip(r#"{"t":"musica.info","item_id":"mu_1","duracao_ms":213000,"ao_vivo":false}"#);
    assert_eq!(
        msg,
        ControlMsg::MusicInfo {
            item_id: "mu_1".into(),
            duration_ms: Some(213_000),
            live: Some(false),
            title: None,
            ended: false,
            failed: false,
        }
    );
    // `terminou` e `erro` só aparecem quando verdadeiros.
    let msg = roundtrip(r#"{"t":"musica.info","item_id":"mu_1","terminou":true,"erro":true}"#);
    assert!(matches!(
        msg,
        ControlMsg::MusicInfo {
            ended: true,
            failed: true,
            ..
        }
    ));
}

#[test]
fn musica_estado_vazio_e_com_atual() {
    assert_eq!(
        roundtrip(r#"{"t":"musica.estado","fila":[]}"#),
        ControlMsg::MusicState {
            queue: vec![],
            current: None
        }
    );
    let msg = roundtrip(
        r#"{"t":"musica.estado","fila":[{"id":"mu_2","video_id":"aaaaaaaaaaa","titulo":"Próxima","ao_vivo":false,"por":"p_2","por_nome":"Ju"}],"atual":{"item":{"id":"mu_1","video_id":"bbbbbbbbbbb","titulo":"Live","duracao_ms":1000,"ao_vivo":true,"por":"p_1","por_nome":"Rafa"},"posicao_ms":-2000,"votos":["p_2"],"precisa":2}}"#,
    );
    match msg {
        ControlMsg::MusicState { queue, current } => {
            assert_eq!(queue.len(), 1);
            assert_eq!(queue[0].added_by_name, "Ju");
            let atual = current.expect("atual");
            assert!(atual.item.live);
            assert_eq!(atual.position_ms, -2000);
            assert_eq!(atual.votes, vec!["p_2".to_string()]);
            assert_eq!(atual.needed, 2);
        }
        outro => panic!("esperava musica.estado, veio {outro:?}"),
    }
}
