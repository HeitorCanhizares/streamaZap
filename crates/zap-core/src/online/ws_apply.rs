//! Aplica mensagens do WebSocket da sala ao `RoomModel` (puro, sem I/O).

use zap_proto::ControlMsg;

use crate::music::MusicMirror;
use crate::plan_gate::{PlanAction, PlanRefusal};
use crate::room::{chat_line_from_msg, RoomEvent, RoomModel};

/// Aplica `msg` ao modelo e devolve os eventos para o `Core`:
/// - `RoomState{room, participants, tracks}` → `replace_state` + `Changed`;
/// - `ParticipantJoined`/`ParticipantUpdated` → `upsert_participant` + `Changed`;
/// - `ParticipantLeft{participant_id}` → `remove_participant` + `remove_tracks_of`
///   (um `TrackRemoved(id)` por trilha) + `Changed`;
/// - `TrackPublished{track}`/`TrackUpdated{track}` → `upsert_track` + `Changed`;
/// - `TrackRemoved{track_id}` → `remove_track` (se existia: `RoomEvent::TrackRemoved(id)`) +
///   `Changed`;
/// - `ChatMessage` → `chat_line_from_msg`; se `Some(linha)`: `push_chat(linha.clone())` +
///   `RoomEvent::Chat(linha)`; mensagem com o mesmo `id` já no histórico é ignorada;
/// - `RoomClosed{reason}` → `RoomEvent::Closed(reason)` (o motivo vai como está:
///   `dono`/`vazia`/`servidor`);
/// - `Error{code: "plano_insuficiente"}` → `RoomEvent::PlanRefused` (plano do `erro`, senão
///   `PlanId::None`; ação `Publish`; `campo` e mensagem do servidor);
/// - `Error{message}` (outros códigos) → `RoomEvent::Error(message)`;
/// - `MusicState` → espelho da música (`MusicMirror`) + `Changed`;
/// - qualquer outra (ping, camadas.pedidas, midia.rota, ...) → vazio (tratadas em `room_events`).
pub fn apply_ws_msg(model: &mut RoomModel, msg: ControlMsg) -> Vec<RoomEvent> {
    // Chat tem regra própria (dedupe por id), então sai antes do `match`.
    if let Some(linha) = chat_line_from_msg(&msg) {
        if model.chat.iter().any(|c| c.id == linha.id) {
            return Vec::new();
        }
        model.push_chat(linha.clone());
        return vec![RoomEvent::Chat(linha)];
    }
    match msg {
        ControlMsg::RoomState {
            room,
            participants,
            tracks,
        } => {
            model.replace_state(room, participants, tracks);
            vec![RoomEvent::Changed]
        }
        ControlMsg::ParticipantJoined { participant } | ControlMsg::ParticipantUpdated { participant } => {
            model.upsert_participant(participant);
            vec![RoomEvent::Changed]
        }
        ControlMsg::ParticipantLeft { participant_id, .. } => {
            model.remove_participant(&participant_id);
            // Trilhas do participante que saiu: cada uma avisa para encerrar assinaturas.
            let mut eventos: Vec<RoomEvent> = model
                .remove_tracks_of(&participant_id)
                .into_iter()
                .map(RoomEvent::TrackRemoved)
                .collect();
            eventos.push(RoomEvent::Changed);
            eventos
        }
        ControlMsg::TrackPublished { track, .. } | ControlMsg::TrackUpdated { track } => {
            model.upsert_track(track);
            vec![RoomEvent::Changed]
        }
        ControlMsg::TrackRemoved { track_id } => {
            // Trilha desconhecida: nada a fazer, nem `Changed`.
            if model.remove_track(&track_id).is_none() {
                return Vec::new();
            }
            vec![RoomEvent::TrackRemoved(track_id), RoomEvent::Changed]
        }
        ControlMsg::RoomClosed { reason } => vec![RoomEvent::Closed(reason)],
        // Recusa por plano sem `ref` (com `ref` de publicação pendente, `room_events` já tratou).
        ControlMsg::Error {
            code,
            message,
            plan,
            field,
            ..
        } if code == "plano_insuficiente" => vec![RoomEvent::PlanRefused(PlanRefusal {
            plan: plan.unwrap_or_default(),
            action: PlanAction::Publish,
            field,
            message,
        })],
        ControlMsg::Error { message, .. } => vec![RoomEvent::Error(message)],
        // No Online o servidor é a fonte da música: guarda o estado como o membro no Local.
        msg @ ControlMsg::MusicState { .. } => {
            model.music_mirror = Some(MusicMirror::from_msg(&msg, std::time::Instant::now()));
            vec![RoomEvent::Changed]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::ChatLine;
    use zap_proto::{Access, Participant, Role, Room, RoomMode, Track, TrackKind};

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

    fn trilha(id: &str, owner: &str, number: u16, kind: TrackKind) -> Track {
        Track {
            id: id.into(),
            participant_id: owner.into(),
            number,
            kind,
            title: format!("t{number}"),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    fn modelo() -> RoomModel {
        RoomModel::new(sala(), participante("p_1"))
    }

    fn chat_msg(id: &str, texto: &str) -> ControlMsg {
        ControlMsg::ChatMessage {
            id: id.into(),
            participant_id: "p_2".into(),
            name: "Ana".into(),
            color: "#00ff00".into(),
            text: texto.into(),
            at: "2026-01-01T00:00:00Z".into(),
            client_id: None,
        }
    }

    #[test]
    fn room_state_substitui_e_mantem_chat() {
        let mut m = modelo();
        m.push_chat(ChatLine {
            id: "c_0".into(),
            author_id: "p_1".into(),
            author: "Dono".into(),
            color: "#fff".into(),
            text: "antes".into(),
            ts: 1,
        });
        let msg = ControlMsg::RoomState {
            room: sala(),
            participants: vec![participante("p_2"), participante("p_3")],
            tracks: vec![trilha("t_1", "p_2", 1, TrackKind::Camera)],
        };
        let evs = apply_ws_msg(&mut m, msg);
        assert!(matches!(evs.as_slice(), [RoomEvent::Changed]));
        assert_eq!(m.participants.len(), 2);
        assert!(m.participant("p_1").is_none());
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.chat.len(), 1);
    }

    #[test]
    fn participante_entrou_atualizou_saiu() {
        let mut m = modelo();
        let evs = apply_ws_msg(
            &mut m,
            ControlMsg::ParticipantJoined {
                participant: participante("p_2"),
            },
        );
        assert!(matches!(evs.as_slice(), [RoomEvent::Changed]));
        assert!(m.participant("p_2").is_some());

        let mut atualizado = participante("p_2");
        atualizado.name = "Nova".into();
        let evs = apply_ws_msg(
            &mut m,
            ControlMsg::ParticipantUpdated {
                participant: atualizado,
            },
        );
        assert!(matches!(evs.as_slice(), [RoomEvent::Changed]));
        assert_eq!(m.participants.len(), 2);
        assert_eq!(m.participant("p_2").unwrap().name, "Nova");

        // Saiu: duas trilhas dele somem e cada uma emite TrackRemoved, antes do Changed.
        m.upsert_track(trilha("t_1", "p_2", 1, TrackKind::Screen));
        m.upsert_track(trilha("t_2", "p_2", 2, TrackKind::Camera));
        m.upsert_track(trilha("t_3", "p_1", 3, TrackKind::Camera));
        let evs = apply_ws_msg(
            &mut m,
            ControlMsg::ParticipantLeft {
                participant_id: "p_2".into(),
                reason: "saiu".into(),
            },
        );
        assert!(m.participant("p_2").is_none());
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].id, "t_3");
        assert_eq!(evs.len(), 3);
        assert!(matches!(&evs[0], RoomEvent::TrackRemoved(id) if id == "t_1"));
        assert!(matches!(&evs[1], RoomEvent::TrackRemoved(id) if id == "t_2"));
        assert!(matches!(&evs[2], RoomEvent::Changed));
    }

    #[test]
    fn trilha_publicada_atualizada_removida() {
        let mut m = modelo();
        let evs = apply_ws_msg(
            &mut m,
            ControlMsg::TrackPublished {
                track: trilha("t_1", "p_1", 1, TrackKind::Screen),
                local_id: Some("local".into()),
            },
        );
        assert!(matches!(evs.as_slice(), [RoomEvent::Changed]));
        assert_eq!(m.tracks.len(), 1);

        let mut trocada = trilha("t_1", "p_1", 1, TrackKind::Screen);
        trocada.title = "Novo título".into();
        let evs = apply_ws_msg(&mut m, ControlMsg::TrackUpdated { track: trocada });
        assert!(matches!(evs.as_slice(), [RoomEvent::Changed]));
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].title, "Novo título");

        let evs = apply_ws_msg(&mut m, ControlMsg::TrackRemoved { track_id: "t_1".into() });
        assert!(m.tracks.is_empty());
        assert!(matches!(evs.as_slice(), [RoomEvent::TrackRemoved(id), RoomEvent::Changed] if id == "t_1"));

        // Trilha que não existe: nada é emitido.
        let evs = apply_ws_msg(
            &mut m,
            ControlMsg::TrackRemoved {
                track_id: "t_inexistente".into(),
            },
        );
        assert!(evs.is_empty());
    }

    #[test]
    fn chat_novo_gera_evento_e_entra_no_historico() {
        let mut m = modelo();
        let evs = apply_ws_msg(&mut m, chat_msg("c_1", "oi"));
        assert_eq!(m.chat.len(), 1);
        assert!(matches!(&evs[..], [RoomEvent::Chat(linha)] if linha.id == "c_1" && linha.text == "oi"));
        assert_eq!(m.chat[0].author_id, "p_2");
        assert_eq!(m.chat[0].ts, 1_767_225_600_000);
    }

    #[test]
    fn chat_duplicado_ignorado() {
        let mut m = modelo();
        apply_ws_msg(&mut m, chat_msg("c_1", "oi"));
        let evs = apply_ws_msg(&mut m, chat_msg("c_1", "oi de novo"));
        assert!(evs.is_empty());
        assert_eq!(m.chat.len(), 1);
    }

    #[test]
    fn sala_encerrada_gera_closed_com_motivo() {
        let mut m = modelo();
        let evs = apply_ws_msg(&mut m, ControlMsg::RoomClosed { reason: "dono".into() });
        assert!(matches!(evs.as_slice(), [RoomEvent::Closed(motivo)] if motivo == "dono"));
    }

    #[test]
    fn erro_vira_evento_error() {
        let mut m = modelo();
        let evs = apply_ws_msg(&mut m, ControlMsg::error("x", "falhou", None));
        assert!(matches!(evs.as_slice(), [RoomEvent::Error(texto)] if texto == "falhou"));
    }

    #[test]
    fn plano_insuficiente_sem_plano_vira_plan_refused_none() {
        let mut m = modelo();
        let evs = apply_ws_msg(&mut m, ControlMsg::error("plano_insuficiente", "acima do plano", None));
        match evs.as_slice() {
            [RoomEvent::PlanRefused(p)] => {
                assert_eq!(p.plan, zap_proto::plan::PlanId::None);
                assert_eq!(p.action, PlanAction::Publish);
                assert_eq!(p.field, None);
                assert_eq!(p.message, "acima do plano");
            }
            outro => panic!("esperava PlanRefused, veio {outro:?}"),
        }
    }

    #[test]
    fn mensagem_desconhecida_nao_faz_nada() {
        let mut m = modelo();
        assert!(apply_ws_msg(&mut m, ControlMsg::Ping { ts_ms: 1 }).is_empty());
        assert!(apply_ws_msg(
            &mut m,
            ControlMsg::LayersRequested {
                track_id: "t_1".into(),
                layers: Vec::new(),
            },
        )
        .is_empty());
        assert_eq!(m.participants.len(), 1);
        assert!(m.tracks.is_empty());
        assert!(m.chat.is_empty());
    }
}
