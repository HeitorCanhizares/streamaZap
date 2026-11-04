//! Tipos comuns de `docs/api/mensagens.md#tipos-comuns`.
use crate::ids::RoomCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Access {
    #[serde(rename = "aberta")]
    Open,
    #[serde(rename = "senha")]
    Password,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomMode {
    #[serde(rename = "online")]
    Online,
    #[serde(rename = "local")]
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    #[serde(rename = "tela")]
    Screen,
    #[serde(rename = "janela")]
    Window,
    #[serde(rename = "camera")]
    Camera,
    #[serde(rename = "audio")]
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileSize {
    #[serde(rename = "miniatura")]
    Thumbnail,
    #[serde(rename = "principal")]
    Main,
    #[serde(rename = "cheia")]
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "dono")]
    Owner,
    #[serde(rename = "membro")]
    Member,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    #[serde(rename = "codigo", default, skip_serializing_if = "Option::is_none")]
    pub code: Option<RoomCode>,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "dono_id")]
    pub owner_id: String,
    #[serde(rename = "acesso")]
    pub access: Access,
    #[serde(rename = "modo")]
    pub mode: RoomMode,
    #[serde(rename = "criada_em")]
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "cor")]
    pub color: String,
    #[serde(rename = "papel")]
    pub role: Role,
    #[serde(rename = "entrou_em")]
    pub joined_at: String,
    #[serde(rename = "versao_app")]
    pub app_version: String,
    #[serde(rename = "protocolo")]
    pub protocol: u32,
    #[serde(rename = "latencia_ms", default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    #[serde(rename = "assistindo", default, skip_serializing_if = "Option::is_none")]
    pub watching: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerSpec {
    #[serde(rename = "camada")]
    pub layer: u8,
    #[serde(rename = "largura")]
    pub width: u16,
    #[serde(rename = "altura")]
    pub height: u16,
    pub fps: u8,
    pub kbps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSpec {
    #[serde(rename = "taxa")]
    pub rate: u32,
    #[serde(rename = "canais")]
    pub channels: u8,
    pub kbps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaAddress {
    #[serde(rename = "enderecos")]
    pub addresses: Vec<String>,
    #[serde(rename = "porta")]
    pub port: u16,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    #[serde(rename = "participante_id")]
    pub participant_id: String,
    #[serde(rename = "numero")]
    pub number: u16,
    #[serde(rename = "tipo")]
    pub kind: TrackKind,
    #[serde(rename = "titulo")]
    pub title: String,
    #[serde(rename = "camadas", default)]
    pub layers: Vec<LayerSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioSpec>,
    #[serde(rename = "endereco", default, skip_serializing_if = "Option::is_none")]
    pub address: Option<MediaAddress>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};
    use std::fmt::Debug;

    /// Desserializa `original`, reserializa e confere que o JSON volta igual.
    /// Campos opcionais ausentes na entrada precisam continuar ausentes na saída.
    fn ida_e_volta<T>(original: Value) -> T
    where
        T: Serialize + DeserializeOwned,
    {
        let tipo: T = serde_json::from_value(original.clone()).expect("desserializa");
        let de_volta = serde_json::to_value(&tipo).expect("serializa");
        assert_eq!(de_volta, original);
        tipo
    }

    /// Confere, para cada valor, o texto do JSON nos dois sentidos.
    fn confere_texto<T>(casos: &[(T, &str)])
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug + Copy,
    {
        for &(valor, texto) in casos {
            assert_eq!(serde_json::to_value(valor).expect("serializa"), json!(texto));
            let volta: T = serde_json::from_value(json!(texto)).expect("desserializa");
            assert_eq!(volta, valor);
        }
    }

    fn sala_json() -> Value {
        json!({
            "id": "s_1",
            "codigo": "ZAP-7K2Q",
            "nome": "Corrida",
            "dono_id": "p_1",
            "acesso": "senha",
            "modo": "online",
            "criada_em": "2026-01-01T00:00:00Z"
        })
    }

    fn participante_json() -> Value {
        json!({
            "id": "p_1",
            "nome": "Ana",
            "cor": "#ff0000",
            "papel": "membro",
            "entrou_em": "2026-01-01T00:00:00Z",
            "versao_app": "0.3.0",
            "protocolo": 4
        })
    }

    #[test]
    fn room_online_com_codigo() {
        let sala: Room = ida_e_volta(sala_json());
        assert_eq!(sala.id, "s_1");
        assert_eq!(sala.code.as_ref().map(RoomCode::as_str), Some("ZAP-7K2Q"));
        assert_eq!(sala.name, "Corrida");
        assert_eq!(sala.owner_id, "p_1");
        assert_eq!(sala.access, Access::Password);
        assert_eq!(sala.mode, RoomMode::Online);
        assert_eq!(sala.created_at, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn room_local_sem_codigo() {
        let json = json!({
            "id": "s_2",
            "nome": "Sofá",
            "dono_id": "p_1",
            "acesso": "aberta",
            "modo": "local",
            "criada_em": "2026-01-01T00:00:00Z"
        });
        let sala: Room = ida_e_volta(json);
        assert_eq!(sala.code, None);
        assert_eq!(sala.access, Access::Open);
        assert_eq!(sala.mode, RoomMode::Local);
        let v = serde_json::to_value(&sala).expect("serializa");
        assert!(v.get("codigo").is_none(), "codigo ausente não deve sair no JSON");
    }

    #[test]
    fn participant_opcionais() {
        // Sem latencia_ms e assistindo: viram None e não voltam no JSON.
        let p: Participant = ida_e_volta(participante_json());
        assert_eq!(p.latency_ms, None);
        assert_eq!(p.watching, None);
        let v = serde_json::to_value(&p).expect("serializa");
        assert!(v.get("latencia_ms").is_none());
        assert!(v.get("assistindo").is_none());

        // Com `assistindo: null`, vira None.
        let mut com_null = participante_json();
        com_null["assistindo"] = Value::Null;
        let p_null: Participant = serde_json::from_value(com_null).expect("desserializa");
        assert_eq!(p_null.watching, None);

        // Com valores presentes, eles são preservados.
        let mut com_valores = participante_json();
        com_valores["latencia_ms"] = json!(42);
        com_valores["assistindo"] = json!("s_1");
        let p_cheio: Participant = ida_e_volta(com_valores);
        assert_eq!(p_cheio.latency_ms, Some(42));
        assert_eq!(p_cheio.watching.as_deref(), Some("s_1"));
    }

    #[test]
    fn participant_papel() {
        let membro: Participant = ida_e_volta(participante_json());
        assert_eq!(membro.role, Role::Member);

        let mut dono_json = participante_json();
        dono_json["papel"] = json!("dono");
        let dono: Participant = ida_e_volta(dono_json);
        assert_eq!(dono.role, Role::Owner);
    }

    #[test]
    fn track_video() {
        let json = json!({
            "id": "t_1",
            "participante_id": "p_1",
            "numero": 1,
            "tipo": "tela",
            "titulo": "Tela inteira",
            "camadas": [
                {"camada": 0, "largura": 1280, "altura": 720, "fps": 30, "kbps": 2500},
                {"camada": 1, "largura": 640, "altura": 360, "fps": 15, "kbps": 800},
                {"camada": 2, "largura": 320, "altura": 180, "fps": 15, "kbps": 200}
            ],
            "endereco": {
                "enderecos": ["192.168.0.10", "fe80::1"],
                "porta": 7000,
                "fingerprint": "sha256:AB:CD"
            }
        });
        let t: Track = ida_e_volta(json);
        assert_eq!(t.id, "t_1");
        assert_eq!(t.participant_id, "p_1");
        assert_eq!(t.number, 1);
        assert_eq!(t.kind, TrackKind::Screen);
        assert_eq!(t.title, "Tela inteira");
        assert_eq!(t.layers.len(), 3);
        assert_eq!(t.layers[0].layer, 0);
        assert_eq!(t.layers[0].width, 1280);
        assert_eq!(t.layers[0].height, 720);
        assert_eq!(t.layers[0].fps, 30);
        assert_eq!(t.layers[0].kbps, 2500);
        assert_eq!(t.layers[2].kbps, 200);
        assert_eq!(t.audio, None);
        let end = t.address.expect("endereço presente");
        assert_eq!(end.addresses, vec!["192.168.0.10", "fe80::1"]);
        assert_eq!(end.port, 7000);
        assert_eq!(end.fingerprint, "sha256:AB:CD");
    }

    #[test]
    fn track_audio() {
        let json = json!({
            "id": "t_2",
            "participante_id": "p_1",
            "numero": 2,
            "tipo": "audio",
            "titulo": "Microfone",
            "audio": {"taxa": 48000, "canais": 2, "kbps": 96},
            "camadas": []
        });
        let t: Track = ida_e_volta(json);
        assert_eq!(t.kind, TrackKind::Audio);
        assert!(t.layers.is_empty());
        let audio = t.audio.clone().expect("áudio presente");
        assert_eq!(audio.rate, 48000);
        assert_eq!(audio.channels, 2);
        assert_eq!(audio.kbps, 96);
        assert_eq!(t.address, None);

        // Sem o campo `camadas`, o padrão é lista vazia.
        let sem_camadas = json!({
            "id": "t_2",
            "participante_id": "p_1",
            "numero": 2,
            "tipo": "audio",
            "titulo": "Microfone",
            "audio": {"taxa": 48000, "canais": 2, "kbps": 96}
        });
        let t2: Track = serde_json::from_value(sem_camadas).expect("desserializa");
        assert!(t2.layers.is_empty());
        assert_eq!(t2.audio, t.audio);
    }

    #[test]
    fn enums_texto() {
        confere_texto(&[
            (TrackKind::Screen, "tela"),
            (TrackKind::Window, "janela"),
            (TrackKind::Camera, "camera"),
            (TrackKind::Audio, "audio"),
        ]);
        confere_texto(&[
            (TileSize::Thumbnail, "miniatura"),
            (TileSize::Main, "principal"),
            (TileSize::Full, "cheia"),
        ]);
        confere_texto(&[(Access::Open, "aberta"), (Access::Password, "senha")]);
    }

    #[test]
    fn campos_desconhecidos_ignorados() {
        let mut com_extra = sala_json();
        com_extra["foo"] = json!(1);
        let com: Room = serde_json::from_value(com_extra).expect("campo extra é ignorado");
        let sem: Room = serde_json::from_value(sala_json()).expect("desserializa");
        assert_eq!(com, sem);
    }

    #[test]
    fn valor_invalido() {
        let mut ruim = sala_json();
        ruim["acesso"] = json!("x");
        assert!(serde_json::from_value::<Room>(ruim).is_err());
    }
}
