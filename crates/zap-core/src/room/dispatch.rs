//! Lógica pura do dono: o que fazer com cada mensagem de um membro.

use std::time::Instant;

use zap_proto::{ControlMsg, Participant};

use super::model::{chat_message_msg, RoomModel, TrackSpec};
use crate::types::ChatLine;
use crate::util::{new_id, now_ms};

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    All,
    Only(String),
    AllBut(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub target: Target,
    pub msg: ControlMsg,
}

/// Trata uma mensagem do membro `from` e devolve o que enviar a quem:
/// - `Ping{ts}` → `Pong{ts}` só para `from`;
/// - `ChatSend{text, client_id}` (texto vazio ou > 1000 caracteres: `Error{codigo:"chat_invalido"}`
///   só para `from`) → `chat.mensagem` para todos (com `cliente_id` só no envio para `from`:
///   devolva dois `Outgoing`: `Only(from)` com `cliente_id` e `AllBut(from)` sem);
/// - `ChatTyping{None}` → `ChatTyping{Some(from)}` para `AllBut(from)`;
/// - `TrackPublish{..}` → `model.add_track(from, ..)`; sucesso → `TrackPublished` (com `local_id`
///   só para `from`, dois `Outgoing` como no chat); falha → `Error{codigo:"trilha_invalida"}`;
/// - `TrackUpdate` → `TrackUpdated` para todos (trilha de outro dono → `Error`);
/// - `TrackRemove` → `TrackRemoved` para todos (só o dono da trilha pode);
/// - `ProfileUpdate` → atualiza nome/cor do participante → `ParticipantUpdated` para todos;
/// - `Stats{ping_ms, ..}` → atualiza `latency_ms` e `watching` não muda → `ParticipantUpdated`;
/// - `RoomLeave` e demais: lista vazia (o chamador trata a saída).
pub fn handle_member_msg(model: &mut RoomModel, from: &str, msg: ControlMsg) -> Vec<Outgoing> {
    // Remetente fora da sala (ex.: já saiu): nada a responder.
    let Some(mut autor) = model.participant(from).cloned() else {
        return Vec::new();
    };
    match msg {
        ControlMsg::Ping { ts_ms } => vec![para(Target::Only(from.into()), ControlMsg::Pong { ts_ms })],
        ControlMsg::ChatSend { text, client_id } => {
            // Conta caracteres (não bytes): acento não estoura o limite antes da hora.
            if text.trim().is_empty() || text.chars().count() > 1000 {
                return vec![erro(from, "chat_invalido", "Mensagem vazia ou longa demais")];
            }
            let line = ChatLine {
                id: new_id("m_"),
                author_id: from.into(),
                author: autor.name,
                color: autor.color,
                text,
                ts: now_ms(),
            };
            model.push_chat(line.clone());
            vec![
                para(Target::Only(from.into()), chat_message_msg(&line, Some(client_id))),
                para(Target::AllBut(from.into()), chat_message_msg(&line, None)),
            ]
        }
        ControlMsg::ChatTyping { participant_id: None } => vec![para(
            Target::AllBut(from.into()),
            ControlMsg::ChatTyping {
                participant_id: Some(from.into()),
            },
        )],
        ControlMsg::TrackPublish {
            local_id,
            kind,
            title,
            layers,
            audio,
            address,
            camera_layout,
        } => {
            let spec = TrackSpec {
                local_id: local_id.clone(),
                kind,
                title,
                layers,
                audio,
                address,
                camera_layout,
            };
            match model.add_track(from, spec) {
                // Quem publicou recebe o `local_id` para casar com o pedido; os outros, não.
                Ok(track) => vec![
                    para(
                        Target::Only(from.into()),
                        ControlMsg::TrackPublished {
                            track: track.clone(),
                            local_id: Some(local_id),
                        },
                    ),
                    para(
                        Target::AllBut(from.into()),
                        ControlMsg::TrackPublished { track, local_id: None },
                    ),
                ],
                Err(err) => vec![erro(from, "trilha_invalida", &err.to_string())],
            }
        }
        ControlMsg::TrackUpdate {
            track_id,
            title,
            layers,
            camera_layout,
        } => {
            if !dona_da_trilha(model, from, &track_id) {
                return vec![erro(
                    from,
                    "trilha_invalida",
                    "Trilha inexistente ou de outro participante",
                )];
            }
            match model.update_track(&track_id, title, layers, camera_layout) {
                Some(track) => vec![para(Target::All, ControlMsg::TrackUpdated { track })],
                None => vec![erro(
                    from,
                    "trilha_invalida",
                    "Trilha inexistente ou de outro participante",
                )],
            }
        }
        ControlMsg::TrackRemove { track_id } => {
            if !dona_da_trilha(model, from, &track_id) {
                return vec![erro(
                    from,
                    "trilha_invalida",
                    "Trilha inexistente ou de outro participante",
                )];
            }
            model.remove_track(&track_id);
            vec![para(Target::All, ControlMsg::TrackRemoved { track_id })]
        }
        ControlMsg::ProfileUpdate { name, color } => {
            if let Some(nome) = name {
                autor.name = nome;
            }
            if let Some(cor) = color {
                autor.color = cor;
            }
            model.upsert_participant(autor.clone());
            vec![para(Target::All, ControlMsg::ParticipantUpdated { participant: autor })]
        }
        ControlMsg::MusicAdd {
            video_id,
            title,
            channel,
        } => {
            let agora = Instant::now();
            let novo = zap_proto::MusicItem {
                id: new_id("mu_"),
                video_id,
                title: title.unwrap_or_default(),
                channel,
                duration_ms: None,
                live: false,
                added_by: from.into(),
                added_by_name: autor.name.clone(),
            };
            match model.music.add(novo, agora) {
                Ok(()) => vec![estado_musica(model, agora)],
                Err(e) => vec![erro(from, "musica_invalida", &e.message())],
            }
        }
        ControlMsg::MusicRemove { item_id } => {
            let agora = Instant::now();
            let eh_dono = model.room.owner_id == from;
            match model.music.remove(from, eh_dono, &item_id, agora) {
                Ok(()) => vec![estado_musica(model, agora)],
                Err(e) => vec![erro(from, "musica_invalida", &e.message())],
            }
        }
        ControlMsg::MusicSkip { item_id, vote } => {
            let agora = Instant::now();
            let presentes = model.participants.len();
            if model.music.vote(from, &item_id, vote, presentes, agora) {
                vec![estado_musica(model, agora)]
            } else {
                Vec::new()
            }
        }
        ControlMsg::MusicInfo {
            item_id,
            duration_ms,
            live,
            title,
            ended,
            failed,
        } => {
            let agora = Instant::now();
            if model
                .music
                .info(&item_id, duration_ms, live, title, ended, failed, agora)
            {
                vec![estado_musica(model, agora)]
            } else {
                Vec::new()
            }
        }
        ControlMsg::Stats { ping_ms, .. } => {
            // `watching` fica como está: só a latência muda aqui.
            autor.latency_ms = Some(ping_ms);
            model.upsert_participant(autor.clone());
            vec![para(Target::All, ControlMsg::ParticipantUpdated { participant: autor })]
        }
        _ => Vec::new(),
    }
}

/// Entrada de membro: adiciona ao modelo e devolve `participante.entrou` para `AllBut(p.id)`.
/// (O `sala.estado` e o histórico de chat para o novo membro são enviados pelo chamador.)
pub fn on_member_joined(model: &mut RoomModel, p: Participant) -> Vec<Outgoing> {
    let id = p.id.clone();
    model.upsert_participant(p.clone());
    vec![
        para(Target::AllBut(id), ControlMsg::ParticipantJoined { participant: p }),
        // Para todos: o novo recebe a música (e sabe que a sala tem), os outros veem o limite
        // de votos mudar.
        estado_musica(model, Instant::now()),
    ]
}

/// Saída/queda: `trilha.removida` para cada trilha dele e `participante.saiu{motivo}` para todos
/// (`motivo`: "saiu" | "caiu" | "removido"). Se o participante não existe: vazio.
pub fn on_member_left(model: &mut RoomModel, id: &str, reason: &str) -> Vec<Outgoing> {
    if model.remove_participant(id).is_none() {
        return Vec::new();
    }
    // Trilhas primeiro, depois o participante: quem recebe vê as trilhas sumirem antes.
    let mut saida: Vec<Outgoing> = model
        .remove_tracks_of(id)
        .into_iter()
        .map(|track_id| para(Target::All, ControlMsg::TrackRemoved { track_id }))
        .collect();
    saida.push(para(
        Target::All,
        ControlMsg::ParticipantLeft {
            participant_id: id.into(),
            reason: reason.into(),
        },
    ));
    // Quem saiu perde o voto e o limite cai (pode pular na hora).
    let agora = Instant::now();
    model.music.left(id, model.participants.len(), agora);
    saida.push(estado_musica(model, agora));
    saida
}

/// `musica.estado` de agora, para todos.
fn estado_musica(model: &RoomModel, agora: Instant) -> Outgoing {
    para(Target::All, model.music.msg(model.participants.len(), agora))
}

fn para(target: Target, msg: ControlMsg) -> Outgoing {
    Outgoing { target, msg }
}

fn erro(to: &str, codigo: &str, mensagem: &str) -> Outgoing {
    para(Target::Only(to.into()), ControlMsg::error(codigo, mensagem, None))
}

/// Só o dono da trilha pode mexer nela (trilha inexistente também é `false`).
fn dona_da_trilha(model: &RoomModel, from: &str, track_id: &str) -> bool {
    model.track_by_id(track_id).is_some_and(|t| t.participant_id == from)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zap_proto::{Access, LayerSpec, Role, Room, RoomMode, TrackKind};

    fn sala() -> Room {
        Room {
            id: "s_1".into(),
            code: None,
            name: "Corrida".into(),
            owner_id: "p_1".into(),
            access: Access::Open,
            mode: RoomMode::Online,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn participante(id: &str) -> Participant {
        Participant {
            id: id.into(),
            name: format!("Pessoa {id}"),
            color: "#ff0000".into(),
            role: Role::Member,
            joined_at: "2026-01-01T00:00:00Z".into(),
            app_version: "1.0.0".into(),
            protocol: 1,
            latency_ms: None,
            watching: None,
        }
    }

    /// Dono `p_1` e membros `p_2` e `p_3`.
    fn modelo() -> RoomModel {
        let mut m = RoomModel::new(sala(), participante("p_1"));
        m.upsert_participant(participante("p_2"));
        m.upsert_participant(participante("p_3"));
        m
    }

    fn publicar_msg(local: &str, kind: TrackKind) -> ControlMsg {
        ControlMsg::TrackPublish {
            local_id: local.into(),
            kind,
            title: "Trilha".into(),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    /// Publica direto no modelo (para montar cenários sem passar pelo despacho).
    fn publicar(m: &mut RoomModel, dono: &str, kind: TrackKind) -> String {
        let spec = TrackSpec {
            local_id: "local".into(),
            kind,
            title: "Trilha".into(),
            layers: vec![LayerSpec {
                layer: 0,
                width: 1280,
                height: 720,
                fps: 30,
                kbps: 4000,
            }],
            audio: None,
            address: None,
            camera_layout: None,
        };
        m.add_track(dono, spec).unwrap().id
    }

    fn so_erro(out: &[Outgoing], para_quem: &str, codigo: &str) -> bool {
        out.len() == 1
            && out[0].target == Target::Only(para_quem.into())
            && matches!(&out[0].msg, ControlMsg::Error { code, .. } if code == codigo)
    }

    #[test]
    fn ping_vira_pong() {
        let mut m = modelo();
        let out = handle_member_msg(&mut m, "p_2", ControlMsg::Ping { ts_ms: 7 });
        assert_eq!(
            out,
            vec![Outgoing {
                target: Target::Only("p_2".into()),
                msg: ControlMsg::Pong { ts_ms: 7 },
            }]
        );
    }

    #[test]
    fn chat_valido_tem_dois_outgoing_com_cliente_id_so_para_remetente() {
        let mut m = modelo();
        let out = handle_member_msg(
            &mut m,
            "p_2",
            ControlMsg::ChatSend {
                text: "oi".into(),
                client_id: "cli_1".into(),
            },
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target, Target::Only("p_2".into()));
        assert_eq!(out[1].target, Target::AllBut("p_2".into()));
        match (&out[0].msg, &out[1].msg) {
            (
                ControlMsg::ChatMessage {
                    id,
                    participant_id,
                    name,
                    text,
                    client_id: a,
                    ..
                },
                ControlMsg::ChatMessage { client_id: b, .. },
            ) => {
                assert!(id.starts_with("m_"));
                assert_eq!(participant_id, "p_2");
                assert_eq!(name, "Pessoa p_2");
                assert_eq!(text, "oi");
                assert_eq!(a.as_deref(), Some("cli_1"));
                assert!(b.is_none());
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
        assert_eq!(m.chat.len(), 1);
    }

    #[test]
    fn chat_vazio_e_longo_erro() {
        let mut m = modelo();
        for texto in ["   ".to_string(), "x".repeat(1001)] {
            let out = handle_member_msg(
                &mut m,
                "p_2",
                ControlMsg::ChatSend {
                    text: texto,
                    client_id: "c".into(),
                },
            );
            assert!(so_erro(&out, "p_2", "chat_invalido"));
        }
        assert!(m.chat.is_empty());
        // 1000 caracteres acentuados passam: conta caracteres, não bytes.
        let out = handle_member_msg(
            &mut m,
            "p_2",
            ControlMsg::ChatSend {
                text: "é".repeat(1000),
                client_id: "c".into(),
            },
        );
        assert_eq!(out.len(), 2);
        assert_eq!(m.chat.len(), 1);
    }

    #[test]
    fn trilha_publicada_numero_e_local_id() {
        let mut m = modelo();
        let out = handle_member_msg(&mut m, "p_2", publicar_msg("loc_1", TrackKind::Camera));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target, Target::Only("p_2".into()));
        assert_eq!(out[1].target, Target::AllBut("p_2".into()));
        match (&out[0].msg, &out[1].msg) {
            (
                ControlMsg::TrackPublished { track: a, local_id: la },
                ControlMsg::TrackPublished { track: b, local_id: lb },
            ) => {
                assert_eq!(a.number, 1);
                assert_eq!(a.participant_id, "p_2");
                assert_eq!(a.id, b.id);
                assert_eq!(la.as_deref(), Some("loc_1"));
                assert!(lb.is_none());
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
    }

    #[test]
    fn segunda_tela_erro() {
        let mut m = modelo();
        handle_member_msg(&mut m, "p_2", publicar_msg("l1", TrackKind::Screen));
        let out = handle_member_msg(&mut m, "p_2", publicar_msg("l2", TrackKind::Window));
        assert!(so_erro(&out, "p_2", "trilha_invalida"));
        assert_eq!(m.tracks.len(), 1);
    }

    #[test]
    fn update_de_trilha_alheia_erro() {
        let mut m = modelo();
        let id = publicar(&mut m, "p_2", TrackKind::Camera);
        let alheia = ControlMsg::TrackUpdate {
            track_id: id.clone(),
            title: Some("Invadida".into()),
            layers: None,
            camera_layout: None,
        };
        let out = handle_member_msg(&mut m, "p_3", alheia);
        assert!(so_erro(&out, "p_3", "trilha_invalida"));
        assert_eq!(m.track_by_id(&id).unwrap().title, "Trilha");

        let propria = ControlMsg::TrackUpdate {
            track_id: id.clone(),
            title: Some("Nova".into()),
            layers: None,
            camera_layout: None,
        };
        let out = handle_member_msg(&mut m, "p_2", propria);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, Target::All);
        assert!(matches!(&out[0].msg, ControlMsg::TrackUpdated { track } if track.title == "Nova"));
        assert_eq!(m.track_by_id(&id).unwrap().title, "Nova");
    }

    #[test]
    fn remove_de_trilha_propria_vai_para_todos() {
        let mut m = modelo();
        let id = publicar(&mut m, "p_2", TrackKind::Camera);
        let out = handle_member_msg(&mut m, "p_3", ControlMsg::TrackRemove { track_id: id.clone() });
        assert!(so_erro(&out, "p_3", "trilha_invalida"));
        assert!(m.track_by_id(&id).is_some());

        let out = handle_member_msg(&mut m, "p_2", ControlMsg::TrackRemove { track_id: id.clone() });
        assert_eq!(
            out,
            vec![Outgoing {
                target: Target::All,
                msg: ControlMsg::TrackRemoved { track_id: id.clone() },
            }]
        );
        assert!(m.track_by_id(&id).is_none());
    }

    #[test]
    fn perfil_atualiza_nome() {
        let mut m = modelo();
        let out = handle_member_msg(
            &mut m,
            "p_2",
            ControlMsg::ProfileUpdate {
                name: Some("Bia".into()),
                color: None,
            },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, Target::All);
        match &out[0].msg {
            ControlMsg::ParticipantUpdated { participant } => {
                assert_eq!(participant.id, "p_2");
                assert_eq!(participant.name, "Bia");
                assert_eq!(participant.color, "#ff0000");
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
        assert_eq!(m.participant("p_2").unwrap().name, "Bia");
    }

    #[test]
    fn stats_atualiza_latencia() {
        let mut m = modelo();
        let out = handle_member_msg(
            &mut m,
            "p_2",
            ControlMsg::Stats {
                ping_ms: 42,
                tracks: Vec::new(),
            },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, Target::All);
        assert!(matches!(
            &out[0].msg,
            ControlMsg::ParticipantUpdated { participant } if participant.latency_ms == Some(42)
        ));
        assert_eq!(m.participant("p_2").unwrap().latency_ms, Some(42));
    }

    #[test]
    fn entrada_notifica_os_outros() {
        let mut m = RoomModel::new(sala(), participante("p_1"));
        let out = on_member_joined(&mut m, participante("p_2"));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target, Target::AllBut("p_2".into()));
        // A música vai para todos, até vazia: o novo fica sabendo que a sala tem música.
        assert_eq!(out[1].target, Target::All);
        assert!(matches!(&out[1].msg, ControlMsg::MusicState { queue, current: None } if queue.is_empty()));
        assert!(matches!(
            &out[0].msg,
            ControlMsg::ParticipantJoined { participant } if participant.id == "p_2"
        ));
        assert!(m.participant("p_2").is_some());
    }

    #[test]
    fn saida_remove_trilhas_e_notifica() {
        let mut m = modelo();
        let cam = publicar(&mut m, "p_2", TrackKind::Camera);
        let aud = publicar(&mut m, "p_2", TrackKind::Audio);
        let outra = publicar(&mut m, "p_3", TrackKind::Camera);

        let out = on_member_left(&mut m, "p_2", "caiu");
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|o| o.target == Target::All));
        assert!(matches!(out[3].msg, ControlMsg::MusicState { .. }));
        // Trilhas primeiro (na ordem em que estavam), participante por último.
        assert_eq!(out[0].msg, ControlMsg::TrackRemoved { track_id: cam.clone() });
        assert_eq!(out[1].msg, ControlMsg::TrackRemoved { track_id: aud.clone() });
        assert_eq!(
            out[2].msg,
            ControlMsg::ParticipantLeft {
                participant_id: "p_2".into(),
                reason: "caiu".into(),
            }
        );
        assert!(m.participant("p_2").is_none());
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].id, outra);
    }

    #[test]
    fn saida_de_desconhecido_vazia() {
        let mut m = modelo();
        let id = publicar(&mut m, "p_2", TrackKind::Camera);
        let out = on_member_left(&mut m, "p_fantasma", "saiu");
        assert!(out.is_empty());
        assert_eq!(m.participants.len(), 3);
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].id, id);
    }

    fn musica_de(out: &[Outgoing]) -> (Vec<zap_proto::MusicItem>, Option<zap_proto::MusicNow>) {
        match out {
            [Outgoing {
                target: Target::All,
                msg: ControlMsg::MusicState { queue, current },
            }] => (queue.clone(), current.clone()),
            outro => panic!("esperava só musica.estado para todos, veio {outro:?}"),
        }
    }

    fn adicionar(video: &str) -> ControlMsg {
        ControlMsg::MusicAdd {
            video_id: video.into(),
            title: Some("Música".into()),
            channel: None,
        }
    }

    #[test]
    fn musica_adicionar_avisa_todos_e_recusa_link_ruim() {
        let mut m = modelo();
        let (fila, atual) = musica_de(&handle_member_msg(&mut m, "p_2", adicionar("aaaaaaaaaaa")));
        assert!(fila.is_empty());
        let atual = atual.expect("começou a tocar");
        assert_eq!(atual.item.added_by, "p_2");
        assert_eq!(atual.item.added_by_name, "Pessoa p_2");
        assert_eq!(atual.needed, 2, "3 pessoas: precisa de 2");

        let out = handle_member_msg(&mut m, "p_2", adicionar("curto"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, Target::Only("p_2".into()));
        assert!(matches!(&out[0].msg, ControlMsg::Error { code, .. } if code == "musica_invalida"));
    }

    #[test]
    fn musica_voto_e_permissao_de_remover() {
        let mut m = modelo();
        let (_, atual) = musica_de(&handle_member_msg(&mut m, "p_2", adicionar("aaaaaaaaaaa")));
        let id = atual.unwrap().item.id;
        handle_member_msg(&mut m, "p_2", adicionar("bbbbbbbbbbb"));
        // p_3 não pôs a música nem é dono: não pula direto.
        let out = handle_member_msg(&mut m, "p_3", ControlMsg::MusicRemove { item_id: id.clone() });
        assert!(matches!(&out[0].msg, ControlMsg::Error { .. }));
        // Um voto de 2: só muda os votos. Voto repetido: nada a enviar.
        let (_, atual) = musica_de(&handle_member_msg(
            &mut m,
            "p_3",
            ControlMsg::MusicSkip {
                item_id: id.clone(),
                vote: true,
            },
        ));
        assert_eq!(atual.unwrap().votes, vec!["p_3".to_string()]);
        assert!(handle_member_msg(
            &mut m,
            "p_3",
            ControlMsg::MusicSkip {
                item_id: id.clone(),
                vote: true
            }
        )
        .is_empty());
        // O dono (p_1) pula direto.
        let (_, atual) = musica_de(&handle_member_msg(
            &mut m,
            "p_1",
            ControlMsg::MusicRemove { item_id: id },
        ));
        assert_eq!(atual.unwrap().item.video_id, "bbbbbbbbbbb");
    }

    #[test]
    fn musica_info_de_quem_nao_esta_na_sala_e_ignorada() {
        let mut m = modelo();
        let (_, atual) = musica_de(&handle_member_msg(&mut m, "p_2", adicionar("aaaaaaaaaaa")));
        let id = atual.unwrap().item.id;
        let info = ControlMsg::MusicInfo {
            item_id: id.clone(),
            duration_ms: Some(60_000),
            live: None,
            title: None,
            ended: false,
            failed: true,
        };
        assert!(handle_member_msg(&mut m, "p_fantasma", info.clone()).is_empty());
        // De alguém da sala, o erro do player pula na hora (e a fila acaba).
        let (_, atual) = musica_de(&handle_member_msg(&mut m, "p_3", info));
        assert!(atual.is_none());
    }

    #[test]
    fn saida_tira_o_voto_e_pode_pular() {
        let mut m = modelo();
        let (_, atual) = musica_de(&handle_member_msg(&mut m, "p_2", adicionar("aaaaaaaaaaa")));
        let id = atual.unwrap().item.id;
        handle_member_msg(&mut m, "p_2", adicionar("bbbbbbbbbbb"));
        // 3 pessoas, precisa de 2: p_2 vota.
        handle_member_msg(
            &mut m,
            "p_2",
            ControlMsg::MusicSkip {
                item_id: id,
                vote: true,
            },
        );
        // p_3 sai: 2 pessoas, precisa de 1, já tem 1: pula.
        let out = on_member_left(&mut m, "p_3", "saiu");
        match &out.last().unwrap().msg {
            ControlMsg::MusicState {
                current: Some(atual), ..
            } => assert_eq!(atual.item.video_id, "bbbbbbbbbbb"),
            outro => panic!("veio {outro:?}"),
        }
    }
}
