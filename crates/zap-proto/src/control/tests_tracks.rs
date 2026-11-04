use super::*;

/// Lê a chave `t` da saída serializada (o round-trip já compara o JSON inteiro;
/// isto deixa explícito que o tipo volta certo).
fn saida(msg: &ControlMsg) -> serde_json::Value {
    match serde_json::to_value(msg) {
        Ok(v) => v,
        Err(e) => panic!("serializa: {e}"),
    }
}

const FP: &str = "3f2a9c0e7b1d4a6f8e5c2b9d0a1f3e4c5b6d7a8f9e0c1b2a3d4e5f6a7b8c9d0e";

#[test]
fn trilha_publicar_minima() {
    let msg = roundtrip(
        r#"{"t":"trilha.publicar","local_id":"l1","tipo":"tela","titulo":"Tela 1","camadas":[{"camada":0,"largura":1920,"altura":1080,"fps":60,"kbps":8000}]}"#,
    );
    match &msg {
        ControlMsg::TrackPublish {
            local_id,
            kind,
            title,
            layers,
            audio,
            address,
        } => {
            assert_eq!(local_id, "l1");
            assert_eq!(*kind, TrackKind::Screen);
            assert_eq!(title, "Tela 1");
            assert_eq!(layers.len(), 1);
            assert_eq!(layers[0].kbps, 8000);
            assert!(audio.is_none());
            assert!(address.is_none());
        }
        outro => panic!("esperava trilha.publicar, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "trilha.publicar");
    assert!(v.get("audio").is_none());
    assert!(v.get("endereco").is_none());
}

#[test]
fn trilha_publicar_completa() {
    let json = format!(
        r#"{{"t":"trilha.publicar","local_id":"l2","tipo":"camera","titulo":"Câmera integrada","camadas":[{{"camada":0,"largura":1280,"altura":720,"fps":30,"kbps":3000}},{{"camada":1,"largura":640,"altura":360,"fps":30,"kbps":800}}],"audio":{{"taxa":48000,"canais":2,"kbps":128}},"endereco":{{"enderecos":["26.12.34.56","192.168.0.10"],"porta":47800,"fingerprint":"{FP}"}}}}"#
    );
    let msg = roundtrip(&json);
    match &msg {
        ControlMsg::TrackPublish {
            kind,
            layers,
            audio,
            address,
            ..
        } => {
            assert_eq!(*kind, TrackKind::Camera);
            assert_eq!(layers.len(), 2);
            match audio {
                Some(a) => assert_eq!((a.rate, a.channels, a.kbps), (48000, 2, 128)),
                None => panic!("audio deveria estar presente"),
            }
            match address {
                Some(e) => assert_eq!(e.port, 47800),
                None => panic!("endereco deveria estar presente"),
            }
        }
        outro => panic!("esperava trilha.publicar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "trilha.publicar");
}

#[test]
fn trilha_publicada() {
    // Com local_id: vai só para quem publicou.
    let msg = roundtrip(
        r#"{"t":"trilha.publicada","trilha":{"id":"t1","participante_id":"p1","numero":3,"tipo":"janela","titulo":"cs2.exe","camadas":[{"camada":0,"largura":1920,"altura":1080,"fps":60,"kbps":8000}]},"local_id":"l1"}"#,
    );
    match &msg {
        ControlMsg::TrackPublished { track, local_id } => {
            assert_eq!(track.id, "t1");
            assert_eq!(track.number, 3);
            assert_eq!(track.kind, TrackKind::Window);
            assert_eq!(local_id.as_deref(), Some("l1"));
        }
        outro => panic!("esperava trilha.publicada, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "trilha.publicada");

    // Sem local_id: é o que os demais da sala recebem.
    let sem = r#"{"t":"trilha.publicada","trilha":{"id":"t1","participante_id":"p1","numero":3,"tipo":"janela","titulo":"cs2.exe","camadas":[]}}"#;
    let msg = roundtrip(sem);
    match &msg {
        ControlMsg::TrackPublished { local_id, .. } => assert!(local_id.is_none()),
        outro => panic!("esperava trilha.publicada, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "trilha.publicada");
    assert!(v.get("local_id").is_none(), "local_id não deve sair quando é None");
}

#[test]
fn trilha_atualizar_titulo() {
    let msg = roundtrip(r#"{"t":"trilha.atualizar","trilha_id":"t1","titulo":"Tela 2"}"#);
    match &msg {
        ControlMsg::TrackUpdate {
            track_id,
            title,
            layers,
        } => {
            assert_eq!(track_id, "t1");
            assert_eq!(title.as_deref(), Some("Tela 2"));
            assert!(layers.is_none());
        }
        outro => panic!("esperava trilha.atualizar, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "trilha.atualizar");
    assert!(v.get("camadas").is_none());
}

#[test]
fn trilha_atualizar_camadas() {
    let msg = roundtrip(
        r#"{"t":"trilha.atualizar","trilha_id":"t1","camadas":[{"camada":0,"largura":1920,"altura":1080,"fps":60,"kbps":6000},{"camada":2,"largura":426,"altura":240,"fps":15,"kbps":150}]}"#,
    );
    match &msg {
        ControlMsg::TrackUpdate { title, layers, .. } => {
            assert!(title.is_none());
            match layers {
                Some(l) => {
                    assert_eq!(l.len(), 2);
                    assert_eq!(l[1].layer, 2);
                }
                None => panic!("camadas deveriam estar presentes"),
            }
        }
        outro => panic!("esperava trilha.atualizar, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "trilha.atualizar");
    assert!(v.get("titulo").is_none());
}

#[test]
fn trilha_atualizada() {
    let msg = roundtrip(
        r#"{"t":"trilha.atualizada","trilha":{"id":"t1","participante_id":"p1","numero":3,"tipo":"tela","titulo":"Tela 2","camadas":[{"camada":0,"largura":1920,"altura":1080,"fps":60,"kbps":6000}]}}"#,
    );
    match &msg {
        ControlMsg::TrackUpdated { track } => {
            assert_eq!(track.title, "Tela 2");
            assert_eq!(track.layers[0].kbps, 6000);
        }
        outro => panic!("esperava trilha.atualizada, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "trilha.atualizada");
}

#[test]
fn trilha_remover_removida() {
    let msg = roundtrip(r#"{"t":"trilha.remover","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::TrackRemove { track_id } => assert_eq!(track_id, "t1"),
        outro => panic!("esperava trilha.remover, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "trilha.remover");

    let msg = roundtrip(r#"{"t":"trilha.removida","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::TrackRemoved { track_id } => assert_eq!(track_id, "t1"),
        outro => panic!("esperava trilha.removida, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "trilha.removida");
}

#[test]
fn camadas_pedidas() {
    let msg = roundtrip(
        r#"{"t":"camadas.pedidas","trilha_id":"t1","camadas":[{"camada":0,"kbps":8000},{"camada":1,"kbps":2500}]}"#,
    );
    match &msg {
        ControlMsg::LayersRequested { track_id, layers } => {
            assert_eq!(track_id, "t1");
            assert_eq!(layers.len(), 2);
            assert_eq!(layers[0].layer, 0);
            assert_eq!(layers[1].kbps, 2500);
        }
        outro => panic!("esperava camadas.pedidas, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "camadas.pedidas");
}

#[test]
fn keyframe_pedir_servidor() {
    // S→C traz a camada que precisa de keyframe.
    let msg = roundtrip(r#"{"t":"keyframe.pedir","trilha_id":"t1","camada":1}"#);
    match &msg {
        ControlMsg::KeyframeRequest { track_id, layer } => {
            assert_eq!(track_id, "t1");
            assert_eq!(*layer, Some(1));
        }
        outro => panic!("esperava keyframe.pedir, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "keyframe.pedir");
}

#[test]
fn keyframe_pedir_cliente() {
    // C→S vem só com trilha_id; a camada fica None e não sai no JSON.
    let msg = roundtrip(r#"{"t":"keyframe.pedir","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::KeyframeRequest { track_id, layer } => {
            assert_eq!(track_id, "t1");
            assert!(layer.is_none());
        }
        outro => panic!("esperava keyframe.pedir, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "keyframe.pedir");
    assert!(v.get("camada").is_none(), "camada não deve sair quando é None");
}

#[test]
fn assinar_completo() {
    // Os três tamanhos, com teto de camada e áudio.
    for (texto, esperado) in [
        ("miniatura", TileSize::Thumbnail),
        ("principal", TileSize::Main),
        ("cheia", TileSize::Full),
    ] {
        let json = format!(r#"{{"t":"assinar","trilha_id":"t1","tamanho":"{texto}","camada_max":2,"audio":true}}"#);
        let msg = roundtrip(&json);
        match &msg {
            ControlMsg::Subscribe {
                track_id,
                size,
                max_layer,
                audio,
            } => {
                assert_eq!(track_id, "t1");
                assert_eq!(*size, esperado);
                assert_eq!(*max_layer, Some(2));
                assert_eq!(*audio, Some(true));
            }
            outro => panic!("esperava assinar, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "assinar");
    }
}

#[test]
fn assinar_minimo() {
    let msg = roundtrip(r#"{"t":"assinar","trilha_id":"t1","tamanho":"principal"}"#);
    match &msg {
        ControlMsg::Subscribe {
            size, max_layer, audio, ..
        } => {
            assert_eq!(*size, TileSize::Main);
            assert!(max_layer.is_none());
            assert!(audio.is_none());
        }
        outro => panic!("esperava assinar, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert!(v.get("camada_max").is_none());
    assert!(v.get("audio").is_none());
}

#[test]
fn cancelar() {
    let msg = roundtrip(r#"{"t":"cancelar","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::Unsubscribe { track_id } => assert_eq!(track_id, "t1"),
        outro => panic!("esperava cancelar, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "cancelar");
}

#[test]
fn midia_rota_ponte() {
    // Rota ponte não leva endereço.
    let msg = roundtrip(r#"{"t":"midia.rota","trilha_id":"t1","rota":"ponte"}"#);
    match &msg {
        ControlMsg::MediaRoute {
            track_id,
            route,
            address,
        } => {
            assert_eq!(track_id, "t1");
            assert_eq!(*route, Route::Relay);
            assert!(address.is_none());
        }
        outro => panic!("esperava midia.rota, veio {outro:?}"),
    }
    let v = saida(&msg);
    assert_eq!(v["t"], "midia.rota");
    assert!(v.get("endereco").is_none());
}

#[test]
fn midia_rota_direta() {
    let json = format!(
        r#"{{"t":"midia.rota","trilha_id":"t1","rota":"direta","endereco":{{"enderecos":["10.0.0.5:47800"],"porta":47800,"fingerprint":"{FP}"}}}}"#
    );
    let msg = roundtrip(&json);
    match &msg {
        ControlMsg::MediaRoute { route, address, .. } => {
            assert_eq!(*route, Route::Direct);
            match address {
                Some(e) => assert_eq!(e.addresses, vec!["10.0.0.5:47800".to_string()]),
                None => panic!("endereco deveria estar presente"),
            }
        }
        outro => panic!("esperava midia.rota, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "midia.rota");
}

#[test]
fn camada_atual() {
    // Um teste por motivo listado na doc.
    for motivo in ["inicio", "banda", "teto", "tamanho", "publicador"] {
        let json = format!(
            r#"{{"t":"camada.atual","trilha_id":"t1","camada":1,"largura":1280,"altura":720,"fps":30,"motivo":"{motivo}"}}"#
        );
        let msg = roundtrip(&json);
        match &msg {
            ControlMsg::LayerCurrent {
                track_id,
                layer,
                width,
                height,
                fps,
                reason,
            } => {
                assert_eq!(track_id, "t1");
                assert_eq!(*layer, 1);
                assert_eq!((*width, *height, *fps), (1280, 720, 30));
                assert_eq!(reason, motivo);
            }
            outro => panic!("esperava camada.atual, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "camada.atual");
    }
}

#[test]
fn p2p_meus_enderecos() {
    let json = format!(
        r#"{{"t":"p2p.meus_enderecos","enderecos":["192.168.0.10:47800","26.12.34.56:47800","201.10.20.30:47800"],"fingerprint":"{FP}"}}"#
    );
    let msg = roundtrip(&json);
    match &msg {
        ControlMsg::P2pMyAddresses { addresses, fingerprint } => {
            assert_eq!(addresses.len(), 3);
            assert_eq!(fingerprint, FP);
        }
        outro => panic!("esperava p2p.meus_enderecos, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "p2p.meus_enderecos");
}

#[test]
fn p2p_candidatos() {
    for (texto, esperado) in [("cliente", P2pRole::Client), ("servidor", P2pRole::Server)] {
        let json = format!(
            r#"{{"t":"p2p.candidatos","trilha_id":"t1","com":"p2","papel":"{texto}","enderecos":["10.0.0.5:47800"],"fingerprint":"{FP}"}}"#
        );
        let msg = roundtrip(&json);
        match &msg {
            ControlMsg::P2pCandidates {
                track_id,
                with,
                role,
                addresses,
                ..
            } => {
                assert_eq!(track_id, "t1");
                assert_eq!(with, "p2");
                assert_eq!(*role, esperado);
                assert_eq!(addresses.len(), 1);
            }
            outro => panic!("esperava p2p.candidatos, veio {outro:?}"),
        }
        assert_eq!(saida(&msg)["t"], "p2p.candidatos");
    }
}

#[test]
fn direta_ok_falhou() {
    let msg = roundtrip(r#"{"t":"midia.direta_ok","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::DirectOk { track_id } => assert_eq!(track_id, "t1"),
        outro => panic!("esperava midia.direta_ok, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "midia.direta_ok");

    let msg = roundtrip(r#"{"t":"midia.direta_falhou","trilha_id":"t1"}"#);
    match &msg {
        ControlMsg::DirectFailed { track_id } => assert_eq!(track_id, "t1"),
        outro => panic!("esperava midia.direta_falhou, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "midia.direta_falhou");
}

#[test]
fn conexao_reversa() {
    let json =
        format!(r#"{{"t":"conexao.reversa","trilha_id":"t1","endereco":"201.10.20.30:47800","token":"tok-{FP}"}}"#);
    let msg = roundtrip(&json);
    match &msg {
        ControlMsg::ReverseConnection {
            track_id,
            address,
            token,
        } => {
            assert_eq!(track_id, "t1");
            assert_eq!(address, "201.10.20.30:47800");
            assert!(token.starts_with("tok-"));
        }
        outro => panic!("esperava conexao.reversa, veio {outro:?}"),
    }
    assert_eq!(saida(&msg)["t"], "conexao.reversa");
}
