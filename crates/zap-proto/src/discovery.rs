//! Anúncios da descoberta local (UDP 47801).
use crate::error::ProtoError;
use crate::types::MediaAddress;
use crate::version::{DISCOVERY_MAGIC, MAX_DISCOVERY_DATAGRAM, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomSummary {
    pub id: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "acesso")]
    pub access: crate::types::Access,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnerSummary {
    pub id: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "cor")]
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    #[serde(rename = "altura")]
    pub height: u16,
    pub fps: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomAnnouncement {
    #[serde(rename = "sala")]
    pub room: RoomSummary,
    #[serde(rename = "dono")]
    pub owner: OwnerSummary,
    #[serde(rename = "endereco")]
    pub address: MediaAddress,
    #[serde(rename = "participantes")]
    pub participants: u32,
    #[serde(rename = "destaque", default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Highlight>,
    #[serde(rename = "versao_app")]
    pub app_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresenceAnnouncement {
    #[serde(rename = "instalacao_id")]
    pub install_id: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "versao_app")]
    pub app_version: String,
    #[serde(rename = "ouve", default)]
    pub hears: Vec<String>,
}

/// Serializa com `tipo` = "sala" | "presenca". Os campos `app` e `v` são
/// acrescentados por `encode_announcement`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo")]
pub enum Announcement {
    #[serde(rename = "sala")]
    Room(RoomAnnouncement),
    #[serde(rename = "presenca")]
    Presence(PresenceAnnouncement),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedAnnouncement {
    Ok(Announcement),
    OtherVersion { v: u32, app_version: Option<String> },
}

/// JSON com `app`, `v` e `tipo`; erro `TooLarge` se passar de 1200 bytes.
pub fn encode_announcement(a: &Announcement) -> Result<Vec<u8>, ProtoError> {
    let mut value = serde_json::to_value(a)?;
    // `app` e `v` ficam fora do enum: todo anúncio carrega a assinatura do app
    // e a versão do protocolo, para quem ouve poder descartar pacote alheio.
    if let Some(obj) = value.as_object_mut() {
        obj.insert("app".to_owned(), DISCOVERY_MAGIC.into());
        obj.insert("v".to_owned(), PROTOCOL_VERSION.into());
    }
    let bytes = serde_json::to_vec(&value)?;
    if bytes.len() > MAX_DISCOVERY_DATAGRAM {
        return Err(ProtoError::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

/// `None` = não é do streamaZap (JSON inválido ou `app` diferente).
pub fn parse_announcement(buf: &[u8]) -> Option<ParsedAnnouncement> {
    let value: serde_json::Value = serde_json::from_slice(buf).ok()?;
    let obj = value.as_object()?;
    if obj.get("app")?.as_str()? != DISCOVERY_MAGIC {
        return None;
    }
    let v = obj.get("v")?.as_u64()?;
    if v != u64::from(PROTOCOL_VERSION) {
        // Outra versão: não tentamos decodificar, só avisamos que existe.
        let app_version = obj
            .get("versao_app")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        // Satura em vez de truncar: um `v` absurdo continua sendo "outra versão".
        let v = u32::try_from(v).unwrap_or(u32::MAX);
        return Some(ParsedAnnouncement::OtherVersion { v, app_version });
    }
    // `app` e `v` são ignorados pelo enum (serde ignora campos desconhecidos).
    serde_json::from_value::<Announcement>(value)
        .ok()
        .map(ParsedAnnouncement::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Access;

    fn sala(nome: &str, destaque: Option<Highlight>) -> Announcement {
        Announcement::Room(RoomAnnouncement {
            room: RoomSummary {
                id: "s_1".into(),
                name: nome.into(),
                access: Access::Open,
            },
            owner: OwnerSummary {
                id: "u_1".into(),
                name: "Rafa".into(),
                color: "#5CD6FF".into(),
            },
            address: MediaAddress {
                addresses: vec!["26.9.77.140".into()],
                port: 47800,
                fingerprint: "ab12".into(),
            },
            participants: 3,
            highlight: destaque,
            app_version: "3.0.0".into(),
        })
    }

    fn presenca(ouve: Vec<String>) -> Announcement {
        Announcement::Presence(PresenceAnnouncement {
            install_id: "i_1".into(),
            name: "Heitor".into(),
            app_version: "3.0.0".into(),
            hears: ouve,
        })
    }

    fn destaque() -> Option<Highlight> {
        Some(Highlight { height: 1080, fps: 60 })
    }

    fn json_de(a: &Announcement) -> serde_json::Value {
        let bytes = encode_announcement(a).expect("encode");
        serde_json::from_slice(&bytes).expect("json")
    }

    #[test]
    fn sala_roundtrip() {
        let a = sala("Corrida em Interlagos", destaque());
        let bytes = encode_announcement(&a).expect("encode");
        assert_eq!(parse_announcement(&bytes), Some(ParsedAnnouncement::Ok(a)));
        let v = json_de(&sala("x", destaque()));
        assert_eq!(v["app"], "streamazap");
        assert_eq!(v["v"], 4);
        assert_eq!(v["tipo"], "sala");
    }

    #[test]
    fn presenca_roundtrip() {
        for ouve in [vec!["a".to_owned(), "b".to_owned()], vec![]] {
            let a = presenca(ouve);
            let bytes = encode_announcement(&a).expect("encode");
            assert_eq!(parse_announcement(&bytes), Some(ParsedAnnouncement::Ok(a)));
        }
        let v = json_de(&presenca(vec![]));
        assert_eq!(v["tipo"], "presenca");
        assert_eq!(v["app"], "streamazap");
        assert_eq!(v["v"], 4);
    }

    #[test]
    fn sem_destaque_omite_campo() {
        let v = json_de(&sala("x", None));
        assert!(v.get("destaque").is_none());
        let bytes = encode_announcement(&sala("x", None)).expect("encode");
        assert_eq!(
            parse_announcement(&bytes),
            Some(ParsedAnnouncement::Ok(sala("x", None)))
        );
    }

    #[test]
    fn exemplo_da_doc() {
        let sala_doc = br##"{ "app": "streamazap", "v": 4, "tipo": "sala",
          "sala": { "id": "s_1", "nome": "Corrida em Interlagos", "acesso": "aberta" },
          "dono": { "id": "u_1", "nome": "Rafa", "cor": "#5CD6FF" },
          "endereco": { "enderecos": ["26.9.77.140"], "porta": 47800, "fingerprint": "ab12" },
          "participantes": 3, "destaque": { "altura": 1080, "fps": 60 }, "versao_app": "3.0.0" }"##;
        assert_eq!(
            parse_announcement(sala_doc),
            Some(ParsedAnnouncement::Ok(sala("Corrida em Interlagos", destaque())))
        );

        let presenca_doc = br##"{ "app": "streamazap", "v": 4, "tipo": "presenca",
          "instalacao_id": "i_1", "nome": "Heitor", "versao_app": "3.0.0",
          "ouve": ["i_2", "i_3"] }"##;
        assert_eq!(
            parse_announcement(presenca_doc),
            Some(ParsedAnnouncement::Ok(presenca(vec!["i_2".into(), "i_3".into()])))
        );
    }

    #[test]
    fn app_diferente() {
        let buf = br#"{"app":"outro","v":4,"tipo":"sala"}"#;
        assert_eq!(parse_announcement(buf), None);
    }

    #[test]
    fn json_invalido() {
        assert_eq!(parse_announcement(b"nao e json"), None);
    }

    #[test]
    fn outra_versao() {
        let com = br#"{"app":"streamazap","v":5,"tipo":"sala","versao_app":"9.0.0"}"#;
        assert_eq!(
            parse_announcement(com),
            Some(ParsedAnnouncement::OtherVersion {
                v: 5,
                app_version: Some("9.0.0".into())
            })
        );
        let sem = br#"{"app":"streamazap","v":5,"tipo":"sala"}"#;
        assert_eq!(
            parse_announcement(sem),
            Some(ParsedAnnouncement::OtherVersion {
                v: 5,
                app_version: None
            })
        );
    }

    #[test]
    fn v4_com_tipo_desconhecido() {
        let buf = br#"{"app":"streamazap","v":4,"tipo":"zzz"}"#;
        assert_eq!(parse_announcement(buf), None);
    }

    #[test]
    fn grande_demais() {
        let a = sala(&"x".repeat(1500), None);
        assert!(matches!(
            encode_announcement(&a),
            Err(ProtoError::TooLarge(n)) if n > MAX_DISCOVERY_DATAGRAM
        ));
    }
}
