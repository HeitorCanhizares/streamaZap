//! Compatibilidade com a v3.5.1 (tarefa contrato-11): tipos congelados da 3.5 leem o JSON da 3.6.
//!
//! Os tipos `Old*` abaixo são cópias fiéis do que a 3.5.1 (commit 6aa7f18) desserializava, sem
//! `deny_unknown_fields`. Provam a decisão D4: nada de valor novo em enum existente, só campos
//! novos que o app antigo ignora.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde::Deserialize;
use serde_json::{json, Value};
use zap_proto::{Access, AudioSource, AudioSpec, ControlMsg, Participant, Role, Room, RoomMode, Track, TrackKind};

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
enum OldAccess {
    #[serde(rename = "aberta")]
    Open,
    #[serde(rename = "senha")]
    Password,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
enum OldRoomMode {
    #[serde(rename = "online")]
    Online,
    #[serde(rename = "local")]
    Local,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
enum OldTrackKind {
    #[serde(rename = "tela")]
    Screen,
    #[serde(rename = "janela")]
    Window,
    #[serde(rename = "camera")]
    Camera,
    #[serde(rename = "audio")]
    Audio,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
enum OldRole {
    #[serde(rename = "dono")]
    Owner,
    #[serde(rename = "membro")]
    Member,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldRoom {
    id: String,
    #[serde(rename = "codigo", default)]
    code: Option<String>,
    #[serde(rename = "nome")]
    name: String,
    #[serde(rename = "dono_id")]
    owner_id: String,
    #[serde(rename = "acesso")]
    access: OldAccess,
    #[serde(rename = "modo")]
    mode: OldRoomMode,
    #[serde(rename = "criada_em")]
    created_at: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldParticipant {
    id: String,
    #[serde(rename = "nome")]
    name: String,
    #[serde(rename = "cor")]
    color: String,
    #[serde(rename = "papel")]
    role: OldRole,
    #[serde(rename = "entrou_em")]
    joined_at: String,
    #[serde(rename = "versao_app")]
    app_version: String,
    #[serde(rename = "protocolo")]
    protocol: u32,
    #[serde(rename = "latencia_ms", default)]
    latency_ms: Option<u32>,
    #[serde(rename = "assistindo", default)]
    watching: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldLayerSpec {
    #[serde(rename = "camada")]
    layer: u8,
    #[serde(rename = "largura")]
    width: u16,
    #[serde(rename = "altura")]
    height: u16,
    fps: u8,
    kbps: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldAudioSpec {
    #[serde(rename = "taxa")]
    rate: u32,
    #[serde(rename = "canais")]
    channels: u8,
    kbps: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldMediaAddress {
    #[serde(rename = "enderecos")]
    addresses: Vec<String>,
    #[serde(rename = "porta")]
    port: u16,
    fingerprint: String,
}

/// Lê o campo `camera` sem falhar, como na 3.5.1 (aqui guardado cru: só importa não derrubar).
fn old_tolerant<'de, D>(d: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Value>::deserialize(d)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldTrack {
    id: String,
    #[serde(rename = "participante_id")]
    participant_id: String,
    #[serde(rename = "numero")]
    number: u16,
    #[serde(rename = "tipo")]
    kind: OldTrackKind,
    #[serde(rename = "titulo")]
    title: String,
    #[serde(rename = "camadas", default)]
    layers: Vec<OldLayerSpec>,
    #[serde(default)]
    audio: Option<OldAudioSpec>,
    #[serde(rename = "endereco", default)]
    address: Option<OldMediaAddress>,
    #[serde(rename = "camera", default, deserialize_with = "old_tolerant")]
    camera_layout: Option<Value>,
}

/// Só as mensagens que a 3.6 mexe; o resto da 3.5.1 não importa aqui.
#[derive(Debug, Deserialize)]
#[serde(tag = "t")]
#[allow(dead_code)]
enum OldMsg {
    #[serde(rename = "sala.estado")]
    RoomState {
        #[serde(rename = "sala")]
        room: OldRoom,
        #[serde(rename = "participantes")]
        participants: Vec<OldParticipant>,
        #[serde(rename = "trilhas")]
        tracks: Vec<OldTrack>,
    },
    #[serde(rename = "trilha.publicada")]
    TrackPublished {
        #[serde(rename = "trilha")]
        track: OldTrack,
        #[serde(default)]
        local_id: Option<String>,
    },
    #[serde(rename = "trilha.atualizada")]
    TrackUpdated {
        #[serde(rename = "trilha")]
        track: OldTrack,
    },
    #[serde(rename = "participante.entrou")]
    ParticipantJoined {
        #[serde(rename = "participante")]
        participant: OldParticipant,
    },
    #[serde(rename = "participante.atualizou")]
    ParticipantUpdated {
        #[serde(rename = "participante")]
        participant: OldParticipant,
    },
    #[serde(rename = "sala.encerrada")]
    RoomClosed {
        #[serde(rename = "motivo")]
        reason: String,
    },
}

fn sala() -> Room {
    Room {
        id: "s_1".into(),
        code: None,
        name: "Palco".into(),
        owner_id: "p_1".into(),
        access: Access::Open,
        mode: RoomMode::Local,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn pessoa(features: &[&str]) -> Participant {
    Participant {
        id: "p_1".into(),
        name: "Ana".into(),
        color: "#ff0000".into(),
        role: Role::Owner,
        joined_at: "2026-01-01T00:00:00Z".into(),
        app_version: "3.6.0".into(),
        protocol: 4,
        latency_ms: None,
        watching: None,
        features: features.iter().map(|s| s.to_string()).collect(),
    }
}

fn trilha_audio(id: &str, numero: u16, fonte: Option<AudioSource>, mudo: bool) -> Track {
    Track {
        id: id.into(),
        participant_id: "p_1".into(),
        number: numero,
        kind: TrackKind::Audio,
        title: "Som".into(),
        layers: vec![],
        audio: Some(AudioSpec {
            rate: 48000,
            channels: 2,
            kbps: 96,
        }),
        address: None,
        camera_layout: None,
        source: fonte,
        muted: mudo,
    }
}

fn mic_mudo() -> Track {
    trilha_audio("t_2", 2, Some(AudioSource::Mic), true)
}

fn para_antigo(msg: &ControlMsg) -> Result<OldMsg, serde_json::Error> {
    let v = serde_json::to_value(msg).expect("serializa");
    serde_json::from_value::<OldMsg>(v)
}

#[test]
fn estado_com_recursos_e_microfone() {
    let msg = ControlMsg::RoomState {
        room: sala(),
        participants: vec![pessoa(&["palco"])],
        tracks: vec![trilha_audio("t_1", 1, None, false), mic_mudo()],
    };
    let velho = para_antigo(&msg).expect("3.5 lê o estado da 3.6");
    let OldMsg::RoomState {
        room,
        participants,
        tracks,
    } = velho
    else {
        panic!("variante errada");
    };
    assert_eq!(room.id, "s_1");
    assert_eq!(participants[0].id, "p_1");
    assert_eq!(participants[0].role, OldRole::Owner);
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].id, "t_1");
    assert_eq!(tracks[1].id, "t_2");
    assert!(tracks.iter().all(|t| t.kind == OldTrackKind::Audio));
    assert_eq!(tracks[1].audio.as_ref().map(|a| a.rate), Some(48000));
}

#[test]
fn publicada_e_atualizada_microfone() {
    let pub_msg = ControlMsg::TrackPublished {
        track: mic_mudo(),
        local_id: Some("l_1".into()),
    };
    match para_antigo(&pub_msg).expect("lê publicada") {
        OldMsg::TrackPublished { track, local_id } => {
            assert_eq!(track.id, "t_2");
            assert_eq!(track.kind, OldTrackKind::Audio);
            assert_eq!(local_id.as_deref(), Some("l_1"));
        }
        outro => panic!("variante errada: {outro:?}"),
    }
    let upd = ControlMsg::TrackUpdated { track: mic_mudo() };
    match para_antigo(&upd).expect("lê atualizada") {
        OldMsg::TrackUpdated { track } => assert_eq!(track.id, "t_2"),
        outro => panic!("variante errada: {outro:?}"),
    }
}

#[test]
fn participante_entrou_com_recursos() {
    let entrou = ControlMsg::ParticipantJoined {
        participant: pessoa(&["palco", "microfone"]),
    };
    match para_antigo(&entrou).expect("lê entrou") {
        OldMsg::ParticipantJoined { participant } => assert_eq!(participant.name, "Ana"),
        outro => panic!("variante errada: {outro:?}"),
    }
    let atualizou = ControlMsg::ParticipantUpdated {
        participant: pessoa(&["palco"]),
    };
    match para_antigo(&atualizou).expect("lê atualizou") {
        OldMsg::ParticipantUpdated { participant } => assert_eq!(participant.id, "p_1"),
        outro => panic!("variante errada: {outro:?}"),
    }
}

#[test]
fn sala_encerrada_atualizar_app() {
    let v = json!({"t": "sala.encerrada", "motivo": "atualizar_app"});
    match serde_json::from_value::<OldMsg>(v).expect("lê encerrada") {
        OldMsg::RoomClosed { reason } => assert_eq!(reason, "atualizar_app"),
        outro => panic!("variante errada: {outro:?}"),
    }
}

#[test]
fn palco_estado_e_desconhecido_para_o_antigo() {
    let msg = ControlMsg::StageState {
        on: true,
        hosts: vec!["p_1".into()],
        hands: vec![],
        audience_music: false,
    };
    let erro = para_antigo(&msg).expect_err("3.5 não conhece palco.estado");
    let texto = erro.to_string();
    assert!(texto.contains("unknown variant"), "erro: {texto}");
    assert!(texto.contains("palco.estado"), "erro: {texto}");
}

#[test]
fn palco_estado_o_atual_le_como_stage_state() {
    let v = json!({"t": "palco.estado", "ligado": true, "anfitrioes": ["p_1"]});
    match serde_json::from_value::<ControlMsg>(v).expect("lê palco.estado") {
        ControlMsg::StageState {
            on,
            hosts,
            hands,
            audience_music,
        } => {
            assert!(on);
            assert_eq!(hosts, vec!["p_1".to_string()]);
            assert!(hands.is_empty());
            assert!(!audience_music);
        }
        outro => panic!("variante errada: {outro:?}"),
    }
}

/// Confere que cada valor atual lê no enum congelado. Os `match` exaustivos fazem a compilação
/// falhar se alguém acrescentar variante, forçando a revisitar a decisão D4.
#[test]
fn enum_existente_nao_ganhou_valor() {
    for k in [
        TrackKind::Screen,
        TrackKind::Window,
        TrackKind::Camera,
        TrackKind::Audio,
    ] {
        match k {
            TrackKind::Screen | TrackKind::Window | TrackKind::Camera | TrackKind::Audio => {}
        }
        let v = serde_json::to_value(k).expect("serializa");
        serde_json::from_value::<OldTrackKind>(v).expect("TrackKind lê no congelado");
    }
    for r in [Role::Owner, Role::Member] {
        match r {
            Role::Owner | Role::Member => {}
        }
        let v = serde_json::to_value(r).expect("serializa");
        serde_json::from_value::<OldRole>(v).expect("Role lê no congelado");
    }
    for a in [Access::Open, Access::Password] {
        match a {
            Access::Open | Access::Password => {}
        }
        let v = serde_json::to_value(a).expect("serializa");
        serde_json::from_value::<OldAccess>(v).expect("Access lê no congelado");
    }
    for m in [RoomMode::Online, RoomMode::Local] {
        match m {
            RoomMode::Online | RoomMode::Local => {}
        }
        let v = serde_json::to_value(m).expect("serializa");
        serde_json::from_value::<OldRoomMode>(v).expect("RoomMode lê no congelado");
    }
    // Os valores congelados também não podem ter sobrado sem par (evita aviso de variante morta).
    assert_ne!(OldTrackKind::Screen, OldTrackKind::Window);
    assert_ne!(OldTrackKind::Camera, OldTrackKind::Audio);
    assert_ne!(OldAccess::Open, OldAccess::Password);
    assert_ne!(OldRoomMode::Online, OldRoomMode::Local);
    assert_ne!(OldRole::Owner, OldRole::Member);
}
