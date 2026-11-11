//! Lógica pura do membro: aplica ao espelho o que o dono manda.

use zap_proto::ControlMsg;

use super::model::{chat_line_from_msg, RoomModel};
use crate::music::MusicMirror;
use crate::types::ChatLine;

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Nada para a interface.
    None,
    /// Estado da sala mudou (participantes/trilhas).
    Changed,
    Chat(ChatLine),
    TrackRemoved(String),
    ParticipantLeft {
        id: String,
        reason: String,
    },
    Closed(String),
    Error {
        code: String,
        message: String,
    },
}

/// `RoomState` → `replace_state` (Changed); `ParticipantJoined/Updated` → upsert (Changed);
/// `ParticipantLeft` → remove participante e suas trilhas (ParticipantLeft); `TrackPublished/
/// Updated` → upsert (Changed); `TrackRemoved` → remove (TrackRemoved); `ChatMessage` →
/// `push_chat` + `unread += 1` (Chat); `RoomClosed{motivo}` → Closed; `Error` → Error;
/// `Ping` e demais → None (o chamador responde ao ping).
pub fn apply_owner_msg(model: &mut RoomModel, msg: ControlMsg) -> Effect {
    match msg {
        ControlMsg::RoomState {
            room,
            participants,
            tracks,
        } => {
            model.replace_state(room, participants, tracks);
            Effect::Changed
        }
        ControlMsg::ParticipantJoined { participant } | ControlMsg::ParticipantUpdated { participant } => {
            model.upsert_participant(participant);
            Effect::Changed
        }
        ControlMsg::ParticipantLeft { participant_id, reason } => {
            // Quem saiu leva as trilhas junto; saída de quem não está no espelho não quebra.
            model.remove_participant(&participant_id);
            model.remove_tracks_of(&participant_id);
            Effect::ParticipantLeft {
                id: participant_id,
                reason,
            }
        }
        ControlMsg::TrackPublished { track, .. } | ControlMsg::TrackUpdated { track } => {
            model.upsert_track(track);
            Effect::Changed
        }
        ControlMsg::TrackRemoved { track_id } => {
            model.remove_track(&track_id);
            Effect::TrackRemoved(track_id)
        }
        msg @ ControlMsg::ChatMessage { .. } => match chat_line_from_msg(&msg) {
            Some(line) => {
                model.push_chat(line.clone());
                model.unread = model.unread.saturating_add(1);
                Effect::Chat(line)
            }
            None => Effect::None,
        },
        msg @ ControlMsg::MusicState { .. } => {
            model.music_mirror = Some(MusicMirror::from_msg(&msg, std::time::Instant::now()));
            Effect::Changed
        }
        ControlMsg::RoomClosed { reason } => Effect::Closed(reason),
        ControlMsg::Error { code, message, .. } => Effect::Error { code, message },
        _ => Effect::None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zap_proto::{Access, Participant, Role, Room, RoomMode, Track, TrackKind};

    fn sala(nome: &str) -> Room {
        Room {
            id: "s_1".into(),
            code: None,
            name: nome.into(),
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

    fn trilha(id: &str, dono: &str, numero: u16, kind: TrackKind) -> Track {
        Track {
            id: id.into(),
            participant_id: dono.into(),
            number: numero,
            kind,
            title: format!("t{numero}"),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    /// Espelho de membro: sala `s_1`, dono `p_1` e uma trilha dele.
    fn espelho() -> RoomModel {
        RoomModel::from_state(
            sala("Corrida"),
            vec![participante("p_1")],
            vec![trilha("t_1", "p_1", 1, TrackKind::Camera)],
        )
    }

    fn chat(id: &str) -> ControlMsg {
        ControlMsg::ChatMessage {
            id: id.into(),
            participant_id: "p_1".into(),
            name: "Ana".into(),
            color: "#00ff00".into(),
            text: format!("texto {id}"),
            at: "2026-01-01T00:00:00Z".into(),
            client_id: None,
        }
    }

    #[test]
    fn room_state_substitui() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::RoomState {
                room: sala("Nova"),
                participants: vec![participante("p_1"), participante("p_2")],
                tracks: vec![trilha("t_9", "p_2", 3, TrackKind::Audio)],
            },
        );
        assert_eq!(efeito, Effect::Changed);
        assert_eq!(m.room.name, "Nova");
        assert_eq!(m.participants.len(), 2);
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].id, "t_9");
    }

    #[test]
    fn entrou_e_saiu() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::ParticipantJoined {
                participant: participante("p_2"),
            },
        );
        assert_eq!(efeito, Effect::Changed);
        assert!(m.participant("p_2").is_some());

        // Trilha do p_2 entra antes da saída para checar que ela some junto.
        apply_owner_msg(
            &mut m,
            ControlMsg::TrackPublished {
                track: trilha("t_2", "p_2", 2, TrackKind::Screen),
                local_id: None,
            },
        );
        assert!(m.track_by_id("t_2").is_some());

        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::ParticipantLeft {
                participant_id: "p_2".into(),
                reason: "saiu".into(),
            },
        );
        assert_eq!(
            efeito,
            Effect::ParticipantLeft {
                id: "p_2".into(),
                reason: "saiu".into(),
            }
        );
        assert!(m.participant("p_2").is_none());
        assert!(m.track_by_id("t_2").is_none());
        // A trilha do dono continua.
        assert!(m.track_by_id("t_1").is_some());
    }

    #[test]
    fn trilha_publicada_atualizada_removida() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::TrackPublished {
                track: trilha("t_2", "p_1", 2, TrackKind::Audio),
                local_id: Some("local".into()),
            },
        );
        assert_eq!(efeito, Effect::Changed);
        assert!(m.track_by_id("t_2").is_some());

        let mut atualizada = trilha("t_2", "p_1", 2, TrackKind::Audio);
        atualizada.title = "Novo título".into();
        let efeito = apply_owner_msg(&mut m, ControlMsg::TrackUpdated { track: atualizada });
        assert_eq!(efeito, Effect::Changed);
        assert_eq!(m.track_by_id("t_2").unwrap().title, "Novo título");
        // Atualizar não duplica a trilha.
        assert_eq!(m.tracks.len(), 2);

        let efeito = apply_owner_msg(&mut m, ControlMsg::TrackRemoved { track_id: "t_2".into() });
        assert_eq!(efeito, Effect::TrackRemoved("t_2".into()));
        assert!(m.track_by_id("t_2").is_none());
    }

    #[test]
    fn chat_conta_nao_lidas_e_devolve_linha() {
        let mut m = espelho();
        let efeito = apply_owner_msg(&mut m, chat("c_1"));
        match efeito {
            Effect::Chat(linha) => {
                assert_eq!(linha.id, "c_1");
                assert_eq!(linha.author, "Ana");
                assert_eq!(linha.text, "texto c_1");
            }
            outro => panic!("esperava Chat, veio {outro:?}"),
        }
        assert_eq!(m.unread, 1);
        assert_eq!(m.chat.len(), 1);

        apply_owner_msg(&mut m, chat("c_2"));
        assert_eq!(m.unread, 2);
        assert_eq!(m.chat.len(), 2);
    }

    #[test]
    fn sala_encerrada_devolve_motivo() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::RoomClosed {
                reason: "dono saiu".into(),
            },
        );
        assert_eq!(efeito, Effect::Closed("dono saiu".into()));
    }

    #[test]
    fn erro_vira_effect_error() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::error("sem_permissao", "não pode", Some("t_1".into())),
        );
        assert_eq!(
            efeito,
            Effect::Error {
                code: "sem_permissao".into(),
                message: "não pode".into(),
            }
        );
    }

    #[test]
    fn ping_e_none() {
        let mut m = espelho();
        let antes = m.tracks.len();
        let efeito = apply_owner_msg(&mut m, ControlMsg::Ping { ts_ms: 42 });
        assert_eq!(efeito, Effect::None);
        assert_eq!(m.tracks.len(), antes);
        assert_eq!(m.unread, 0);
    }

    #[test]
    fn saida_de_desconhecido_nao_quebra() {
        let mut m = espelho();
        let efeito = apply_owner_msg(
            &mut m,
            ControlMsg::ParticipantLeft {
                participant_id: "fantasma".into(),
                reason: "caiu".into(),
            },
        );
        assert_eq!(
            efeito,
            Effect::ParticipantLeft {
                id: "fantasma".into(),
                reason: "caiu".into(),
            }
        );
        assert_eq!(m.participants.len(), 1);
        assert_eq!(m.tracks.len(), 1);

        // Remover trilha que não existe também não quebra.
        let efeito = apply_owner_msg(&mut m, ControlMsg::TrackRemoved { track_id: "x".into() });
        assert_eq!(efeito, Effect::TrackRemoved("x".into()));
    }
}
