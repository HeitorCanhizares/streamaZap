//! Tipos de `docs/api/rest.md` (campos em inglês, nomes de fio em português via serde).
use serde::{Deserialize, Serialize};
use zap_proto::plan::PlanInfo;
use zap_proto::{Access, RoomCode};

/// Onde o app faz login quando o servidor confere contas (`GET /v1/info`, campo `conta`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccountProvider {
    /// Hoje só `"supabase"`.
    #[serde(rename = "provedor")]
    pub provider: String,
    /// Ex.: `https://abcd1234.supabase.co`.
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuicInfo {
    #[serde(rename = "porta")]
    pub port: u16,
    pub alpn: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerLimits {
    #[serde(rename = "participantes_por_sala")]
    pub participants_per_room: u32,
    #[serde(rename = "trilhas_video_por_sala")]
    pub video_tracks_per_room: u32,
    #[serde(rename = "bitrate_max_kbps")]
    pub max_bitrate_kbps: u32,
}

/// `GET /v1/info`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerInfo {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "versao_servidor")]
    pub server_version: String,
    #[serde(rename = "protocolo_min")]
    pub protocol_min: u32,
    #[serde(rename = "protocolo_max")]
    pub protocol_max: u32,
    #[serde(rename = "regiao", default)]
    pub region: Option<String>,
    #[serde(rename = "requer_senha")]
    pub requires_password: bool,
    /// Sem `acesso_conta` válido, `POST /sessoes` responde `conta_necessaria`.
    #[serde(rename = "requer_conta", default)]
    pub requires_account: bool,
    /// `None` = o servidor não confere contas (o app nem mostra login).
    #[serde(rename = "conta", default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountProvider>,
    #[serde(rename = "ponte")]
    pub relay: bool,
    pub quic: QuicInfo,
    #[serde(rename = "limites")]
    pub limits: ServerLimits,
    #[serde(rename = "agora")]
    pub now: String,
}

/// Corpo de `POST /v1/sessoes`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoginRequest {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "cor")]
    pub color: String,
    #[serde(rename = "instalacao_id")]
    pub install_id: String,
    #[serde(rename = "protocolo")]
    pub protocol: u32,
    #[serde(rename = "versao_app")]
    pub app_version: String,
    #[serde(rename = "senha_servidor", default, skip_serializing_if = "Option::is_none")]
    pub server_password: Option<String>,
    /// JWT de acesso do Supabase (só quando o servidor tem `conta`).
    #[serde(rename = "acesso_conta", default, skip_serializing_if = "Option::is_none")]
    pub account_token: Option<String>,
}

/// Corpo opcional de `POST /v1/sessoes/renovar`: token novo do Supabase para atualizar o plano.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RenewRequest {
    #[serde(rename = "acesso_conta", default, skip_serializing_if = "Option::is_none")]
    pub account_token: Option<String>,
}

/// Resposta de `POST /v1/sessoes` e `/sessoes/renovar`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionResponse {
    pub token: String,
    #[serde(rename = "expira_em")]
    pub expires_at: String,
    #[serde(rename = "participante_id")]
    pub participant_id: String,
    #[serde(rename = "ip_publico", default)]
    pub public_ip: Option<String>,
    /// Só quando a sessão tem conta.
    #[serde(rename = "conta_id", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Só quando a sessão tem conta. `None` = sem limite de plano.
    #[serde(rename = "plano", default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnerInfo {
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
pub struct LastMessage {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "texto")]
    pub text: String,
    #[serde(rename = "em")]
    pub at: String,
}

/// Sala no formato da lista (`GET /v1/salas`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomListing {
    pub id: String,
    #[serde(rename = "codigo")]
    pub code: RoomCode,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "dono")]
    pub owner: OwnerInfo,
    #[serde(rename = "acesso")]
    pub access: Access,
    #[serde(rename = "criada_em")]
    pub created_at: String,
    #[serde(rename = "participantes")]
    pub participants: u32,
    #[serde(rename = "ao_vivo", default)]
    pub live: bool,
    #[serde(rename = "destaque", default)]
    pub highlight: Option<Highlight>,
    #[serde(rename = "ultima_mensagem", default)]
    pub last_message: Option<LastMessage>,
    #[serde(rename = "nao_lidas", default)]
    pub unread: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomListResponse {
    #[serde(rename = "salas")]
    pub rooms: Vec<RoomListing>,
}

/// Corpo de `POST /v1/salas`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "acesso")]
    pub access: Access,
    #[serde(rename = "senha", default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Resposta de `POST /v1/salas`: `{ "sala": {...} }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateRoomResponse {
    #[serde(rename = "sala")]
    pub room: RoomListing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParticipantBrief {
    pub id: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "cor")]
    pub color: String,
    #[serde(rename = "compartilhando", default)]
    pub sharing: Vec<String>,
}

/// `GET /v1/salas/{codigo}`: a lista mais `participantes_lista` (ausente em sala com senha).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomDetails {
    #[serde(flatten)]
    pub room: RoomListing,
    #[serde(rename = "participantes_lista", default)]
    pub participant_list: Option<Vec<ParticipantBrief>>,
}

/// Corpo de `POST /v1/salas/{codigo}/entrar`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JoinRequest {
    #[serde(rename = "senha", default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BridgeInfo {
    #[serde(rename = "endereco")]
    pub address: String,
    pub alpn: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JoinMedia {
    #[serde(rename = "ponte", default)]
    pub bridge: Option<BridgeInfo>,
    #[serde(rename = "direta_permitida", default)]
    pub direct_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JoinResponse {
    #[serde(rename = "sala")]
    pub room: RoomListing,
    #[serde(rename = "participante_id")]
    pub participant_id: String,
    #[serde(rename = "ingresso")]
    pub ticket: String,
    #[serde(rename = "ingresso_expira_em")]
    pub ticket_expires_at: String,
    #[serde(rename = "midia")]
    pub media: JoinMedia,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewMedia {
    #[serde(rename = "ponte")]
    pub bridge: BridgeInfo,
}

/// Resposta de `POST /v1/salas/{codigo}/previa`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewResponse {
    #[serde(rename = "ingresso")]
    pub ticket: String,
    #[serde(rename = "expira_em")]
    pub expires_at: String,
    #[serde(rename = "midia")]
    pub media: PreviewMedia,
}

/// `GET /v1/diagnostico`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerDiagnostics {
    #[serde(rename = "ip_publico")]
    pub public_ip: String,
    #[serde(rename = "porta_publica_udp", default)]
    pub public_udp_port: Option<u16>,
    #[serde(rename = "protocolo_ok")]
    pub protocol_ok: bool,
    #[serde(rename = "agora")]
    pub now: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// Exemplo de `GET /v1/info` de docs/api/rest.md.
    const INFO: &str = r##"{
      "nome": "Servidor do Heitor",
      "versao_servidor": "1.0.0",
      "protocolo_min": 4,
      "protocolo_max": 4,
      "regiao": "sa-east",
      "requer_senha": true,
      "ponte": true,
      "quic": { "porta": 7443, "alpn": "zap/4" },
      "limites": { "participantes_por_sala": 16, "trilhas_video_por_sala": 8, "bitrate_max_kbps": 12000 },
      "agora": "2026-11-04T21:30:00Z"
    }"##;

    /// Item de `GET /v1/salas`, com `destaque` e `ultima_mensagem`.
    const SALA: &str = r##"{
      "id": "s_x81Pq0",
      "codigo": "ZAP-7K2Q",
      "nome": "Sexta do Rush",
      "dono": { "id": "p_8Kq2mZ0aL1", "nome": "Marcão", "cor": "#FFB547" },
      "acesso": "aberta",
      "criada_em": "2026-11-04T21:30:00Z",
      "participantes": 4,
      "ao_vivo": true,
      "destaque": { "altura": 1080, "fps": 60 },
      "ultima_mensagem": { "nome": "Rafa", "texto": "bora, entra logo", "em": "2026-11-04T22:14:00Z" },
      "nao_lidas": 3
    }"##;

    /// Sala sem `destaque`, `ultima_mensagem` e `ao_vivo` (campos opcionais ausentes).
    const SALA_MINIMA: &str = r##"{
      "id": "s_min001",
      "codigo": "ZAP-2345",
      "nome": "Sala quieta",
      "dono": { "id": "p_1", "nome": "Ju", "cor": "#7CF2C0" },
      "acesso": "senha",
      "criada_em": "2026-11-04T21:30:00Z",
      "participantes": 1
    }"##;

    /// Detalhes de `GET /v1/salas/{codigo}` com `participantes_lista`.
    const DETALHES: &str = r##"{
      "id": "s_x81Pq0",
      "codigo": "ZAP-7K2Q",
      "nome": "Sexta do Rush",
      "dono": { "id": "p_8Kq2mZ0aL1", "nome": "Marcão", "cor": "#FFB547" },
      "acesso": "aberta",
      "criada_em": "2026-11-04T21:30:00Z",
      "participantes": 1,
      "ao_vivo": true,
      "participantes_lista": [ { "id": "p_ju", "nome": "Ju", "cor": "#7CF2C0", "compartilhando": ["camera"] } ]
    }"##;

    #[test]
    fn info_exemplo() {
        let info: ServerInfo = serde_json::from_str(INFO).unwrap();
        assert_eq!(info.name, "Servidor do Heitor");
        assert_eq!(info.server_version, "1.0.0");
        assert_eq!((info.protocol_min, info.protocol_max), (4, 4));
        assert_eq!(info.region.as_deref(), Some("sa-east"));
        assert!(info.requires_password);
        assert!(info.relay);
        assert_eq!(info.quic.port, 7443);
        assert_eq!(info.quic.alpn, "zap/4");
        assert_eq!(info.limits.participants_per_room, 16);
        assert_eq!(info.limits.video_tracks_per_room, 8);
        assert_eq!(info.limits.max_bitrate_kbps, 12000);
        assert_eq!(info.now, "2026-11-04T21:30:00Z");

        // `ponte: false` = só sinalização; `regiao` ausente vira None.
        let mut v: Value = serde_json::from_str(INFO).unwrap();
        v["ponte"] = json!(false);
        v.as_object_mut().unwrap().remove("regiao");
        let info: ServerInfo = serde_json::from_value(v).unwrap();
        assert!(!info.relay);
        assert_eq!(info.region, None);
    }

    #[test]
    fn sessao_exemplo() {
        let r: SessionResponse = serde_json::from_str(
            r##"{ "token": "tok_abc", "expira_em": "2026-11-05T21:30:00Z", "participante_id": "p_8Kq2mZ0aL1", "ip_publico": "177.12.34.56" }"##,
        )
        .unwrap();
        assert_eq!(r.token, "tok_abc");
        assert_eq!(r.expires_at, "2026-11-05T21:30:00Z");
        assert_eq!(r.participant_id, "p_8Kq2mZ0aL1");
        assert_eq!(r.public_ip.as_deref(), Some("177.12.34.56"));

        // `ip_publico` é opcional.
        let r: SessionResponse =
            serde_json::from_str(r##"{ "token": "t", "expira_em": "x", "participante_id": "p_1" }"##).unwrap();
        assert_eq!(r.public_ip, None);
    }

    #[test]
    fn sala_lista_exemplo() {
        let body = format!(r##"{{ "salas": [{SALA}, {SALA_MINIMA}] }}"##);
        let lista: RoomListResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(lista.rooms.len(), 2);

        let sala = &lista.rooms[0];
        assert_eq!(sala.id, "s_x81Pq0");
        assert_eq!(sala.code.as_str(), "ZAP-7K2Q");
        assert_eq!(sala.name, "Sexta do Rush");
        assert_eq!(sala.owner.name, "Marcão");
        assert_eq!(sala.owner.color, "#FFB547");
        assert!(matches!(sala.access, Access::Open));
        assert_eq!(sala.participants, 4);
        assert!(sala.live);
        assert_eq!(sala.unread, 3);

        let destaque = sala.highlight.as_ref().unwrap();
        assert_eq!((destaque.height, destaque.fps), (1080, 60));
        let ultima = sala.last_message.as_ref().unwrap();
        assert_eq!(ultima.name, "Rafa");
        assert_eq!(ultima.text, "bora, entra logo");
        assert_eq!(ultima.at, "2026-11-04T22:14:00Z");

        // Sem `destaque`/`ultima_mensagem`/`ao_vivo`/`nao_lidas`: None, false e 0.
        let minima = &lista.rooms[1];
        assert!(matches!(minima.access, Access::Password));
        assert!(!minima.live);
        assert!(minima.highlight.is_none());
        assert!(minima.last_message.is_none());
        assert_eq!(minima.unread, 0);
    }

    #[test]
    fn sala_detalhes_com_e_sem_participantes_lista() {
        // Com `participantes_lista` (sala aberta): flatten traz os campos da lista.
        let d: RoomDetails = serde_json::from_str(DETALHES).unwrap();
        assert_eq!(d.room.code.as_str(), "ZAP-7K2Q");
        assert_eq!(d.room.name, "Sexta do Rush");
        assert!(d.room.live);
        let participantes = d.participant_list.as_ref().unwrap();
        assert_eq!(participantes.len(), 1);
        assert_eq!(participantes[0].name, "Ju");
        assert_eq!(participantes[0].sharing, vec!["camera".to_string()]);

        // Sala com senha, antes de entrar: sem `participantes_lista`.
        let mut v: Value = serde_json::from_str(DETALHES).unwrap();
        v.as_object_mut().unwrap().remove("participantes_lista");
        v["acesso"] = json!("senha");
        let d: RoomDetails = serde_json::from_value(v).unwrap();
        assert!(d.participant_list.is_none());
        assert!(matches!(d.room.access, Access::Password));
        assert_eq!(d.room.participants, 1);
    }

    #[test]
    fn entrar_exemplo() {
        let body = format!(
            r##"{{
              "sala": {SALA},
              "participante_id": "p_3Jx7rT1bN9",
              "ingresso": "ingresso-exemplo",
              "ingresso_expira_em": "2026-11-04T22:30:00Z",
              "midia": {{
                "ponte": {{ "endereco": "zap.seudominio.com:7443", "alpn": "zap/4" }},
                "direta_permitida": true
              }}
            }}"##
        );
        let r: JoinResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(r.room.code.as_str(), "ZAP-7K2Q");
        assert_eq!(r.participant_id, "p_3Jx7rT1bN9");
        assert_eq!(r.ticket, "ingresso-exemplo");
        assert_eq!(r.ticket_expires_at, "2026-11-04T22:30:00Z");
        assert!(r.media.direct_allowed);
        let ponte = r.media.bridge.as_ref().unwrap();
        assert_eq!(ponte.address, "zap.seudominio.com:7443");
        assert_eq!(ponte.alpn, "zap/4");

        // Servidor sem ponte: `midia.ponte` ausente vira None.
        let mut v: Value = serde_json::from_str(&body).unwrap();
        v["midia"].as_object_mut().unwrap().remove("ponte");
        v["midia"]["direta_permitida"] = json!(false);
        let r: JoinResponse = serde_json::from_value(v).unwrap();
        assert!(r.media.bridge.is_none());
        assert!(!r.media.direct_allowed);

        // O servidor de produção com `--sem-ponte` manda `"ponte": null` (não omite o campo).
        let mut v: Value = serde_json::from_str(&body).unwrap();
        v["midia"]["ponte"] = Value::Null;
        let r: JoinResponse = serde_json::from_value(v).unwrap();
        assert!(r.media.bridge.is_none());
    }

    #[test]
    fn previa_exemplo() {
        let r: PreviewResponse = serde_json::from_str(
            r##"{
              "ingresso": "ingresso-previa",
              "expira_em": "2026-11-04T22:30:00Z",
              "midia": { "ponte": { "endereco": "zap.seudominio.com:7443", "alpn": "zap/4" } }
            }"##,
        )
        .unwrap();
        assert_eq!(r.ticket, "ingresso-previa");
        assert_eq!(r.expires_at, "2026-11-04T22:30:00Z");
        assert_eq!(r.media.bridge.address, "zap.seudominio.com:7443");
        assert_eq!(r.media.bridge.alpn, "zap/4");
    }

    #[test]
    fn diagnostico_exemplo() {
        let d: ServerDiagnostics = serde_json::from_str(
            r##"{ "ip_publico": "177.12.34.56", "porta_publica_udp": 51234, "protocolo_ok": true, "agora": "2026-11-04T21:30:00Z" }"##,
        )
        .unwrap();
        assert_eq!(d.public_ip, "177.12.34.56");
        assert_eq!(d.public_udp_port, Some(51234));
        assert!(d.protocol_ok);
        assert_eq!(d.now, "2026-11-04T21:30:00Z");

        // `porta_publica_udp` é opcional.
        let d: ServerDiagnostics =
            serde_json::from_str(r##"{ "ip_publico": "177.12.34.56", "protocolo_ok": false, "agora": "x" }"##).unwrap();
        assert_eq!(d.public_udp_port, None);
        assert!(!d.protocol_ok);
    }

    #[test]
    fn login_request_omite_senha_servidor_quando_none() {
        let sem = LoginRequest {
            name: "Heitor".into(),
            color: "#3BF08A".into(),
            install_id: "9f1c".into(),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
            account_token: None,
        };
        let v = serde_json::to_value(&sem).unwrap();
        let obj = v.as_object().unwrap();
        assert!(!obj.contains_key("senha_servidor"));
        assert_eq!(v["nome"], "Heitor");
        assert_eq!(v["instalacao_id"], "9f1c");
        assert_eq!(v["protocolo"], 4);
        assert_eq!(v["versao_app"], "3.0.0");

        let com = LoginRequest {
            server_password: Some("abc".into()),
            ..sem
        };
        let v = serde_json::to_value(&com).unwrap();
        assert_eq!(v["senha_servidor"], "abc");
    }

    #[test]
    fn create_room_request_serializa_acesso_senha() {
        let com_senha = CreateRoomRequest {
            name: "Sala do Heitor".into(),
            access: Access::Password,
            password: Some("1234".into()),
        };
        let v = serde_json::to_value(&com_senha).unwrap();
        assert_eq!(v["nome"], "Sala do Heitor");
        assert_eq!(v["acesso"], "senha");
        assert_eq!(v["senha"], "1234");

        let aberta = CreateRoomRequest {
            name: "Aberta".into(),
            access: Access::Open,
            password: None,
        };
        let v = serde_json::to_value(&aberta).unwrap();
        assert_eq!(v["acesso"], "aberta");
        assert!(!v.as_object().unwrap().contains_key("senha"));
    }

    #[test]
    fn campos_desconhecidos_ignorados() {
        // Campos extras (de versões futuras do servidor) não quebram a leitura.
        let mut info: Value = serde_json::from_str(INFO).unwrap();
        info["campo_novo"] = json!({ "qualquer": 1 });
        info["limites"]["algo_novo"] = json!(true);
        let info: ServerInfo = serde_json::from_value(info).unwrap();
        assert_eq!(info.limits.max_bitrate_kbps, 12000);

        let mut sala: Value = serde_json::from_str(SALA).unwrap();
        sala["dono"]["extra"] = json!("x");
        sala["extra_raiz"] = json!([1, 2, 3]);
        let sala: RoomListing = serde_json::from_value(sala).unwrap();
        assert_eq!(sala.name, "Sexta do Rush");
    }

    #[test]
    fn info_sem_conta_e_servidor_antigo() {
        // Servidor de hoje: sem `requer_conta` nem `conta`.
        let info: ServerInfo = serde_json::from_str(INFO).unwrap();
        assert!(!info.requires_account);
        assert_eq!(info.account, None);
        let v = serde_json::to_value(&info).unwrap();
        assert!(v.get("conta").is_none());
    }

    #[test]
    fn info_com_conta() {
        let mut v: Value = serde_json::from_str(INFO).unwrap();
        v["requer_conta"] = json!(true);
        v["conta"] = json!({"provedor": "supabase", "url": "https://abcd1234.supabase.co"});
        let info: ServerInfo = serde_json::from_value(v).unwrap();
        assert!(info.requires_account);
        assert_eq!(
            info.account,
            Some(AccountProvider {
                provider: "supabase".into(),
                url: "https://abcd1234.supabase.co".into(),
            })
        );
    }

    #[test]
    fn login_request_com_conta() {
        let pedido = LoginRequest {
            name: "Heitor".into(),
            color: "#3BF08A".into(),
            install_id: "9f1c".into(),
            protocol: 4,
            app_version: "3.3.0".into(),
            server_password: Some("abc".into()),
            account_token: Some("eyJ".into()),
        };
        let v = serde_json::to_value(&pedido).unwrap();
        assert_eq!(v["acesso_conta"], "eyJ");
        assert_eq!(v["senha_servidor"], "abc");
        let sem = LoginRequest {
            account_token: None,
            ..pedido
        };
        assert!(serde_json::to_value(&sem).unwrap().get("acesso_conta").is_none());
    }

    #[test]
    fn renovar_com_e_sem_conta() {
        assert_eq!(serde_json::to_value(RenewRequest::default()).unwrap(), json!({}));
        let com = RenewRequest {
            account_token: Some("eyJ".into()),
        };
        assert_eq!(serde_json::to_value(&com).unwrap(), json!({"acesso_conta": "eyJ"}));
    }

    #[test]
    fn sessao_com_conta_e_plano() {
        use zap_proto::plan::{PlanId, PlanInfo};
        let v = json!({
            "token": "tk", "expira_em": "2026-11-05T21:30:00Z", "participante_id": "p_1",
            "ip_publico": "177.12.34.56",
            "conta_id": "2b0c",
            "plano": {
                "id": "hd", "pode_entrar": true,
                "transmitir": {"altura_max": 720, "fps_max": 60, "tela_kbps": 4000, "camera_kbps": 2500}
            }
        });
        let s: SessionResponse = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(s.account_id.as_deref(), Some("2b0c"));
        assert_eq!(s.plan, Some(PlanInfo::of(PlanId::Hd)));
        assert_eq!(serde_json::to_value(&s).unwrap(), v);

        // Sem conta: nada de `conta_id` nem `plano` (sem limite de plano).
        let sem: SessionResponse =
            serde_json::from_value(json!({"token": "tk", "expira_em": "x", "participante_id": "p_1"})).unwrap();
        assert_eq!(sem.account_id, None);
        assert_eq!(sem.plan, None);
        let volta = serde_json::to_value(&sem).unwrap();
        assert!(volta.get("conta_id").is_none() && volta.get("plano").is_none());
    }
}
