//! Conversões entre os tipos da API do servidor (`zap_net::online`), do protocolo
//! (`zap_proto`) e da interface (`crate::snapshot`).

use zap_net::online::RoomListing as NetListing;
use zap_proto::{Access, RoomCode, RoomMode};

use crate::snapshot::RoomListing as UiListing;

/// Listagem do servidor → listagem da interface. Regras:
/// `room_id` = `id` do servidor; `name`; `host` = nome do dono; `participants`;
/// `has_password` = `acesso == Password`; `mode` = `RoomMode::Online`; `addr` = `Some(codigo)`
/// (ex.: `"ZAP-7K2Q"`); `live_title` = `Some("<altura>p<fps>")` quando `live` com `highlight`;
/// `quality` = `Some("<altura>p<fps>")` com `highlight`; `last_message` = `"<nome>: <texto>"`;
/// `last_time` = `"HH:MM"` de `ultima_mensagem.em` (UTC, ISO-8601; ilegível → `None`);
/// `unread` = `Some(n)` só se `n > 0`; demais campos `None`.
pub fn listing_to_ui(l: &NetListing) -> UiListing {
    let mut ui = UiListing::basic(
        l.id.clone(),
        l.name.clone(),
        l.owner.name.clone(),
        l.participants,
        matches!(l.access, Access::Password),
        RoomMode::Online,
    );
    // O código é o endereço que a interface mostra e copia (ex.: "ZAP-7K2Q").
    ui.addr = Some(l.code.to_string());

    if let Some(destaque) = &l.highlight {
        let qualidade = format!("{}p{}", destaque.height, destaque.fps);
        // O título "ao vivo" só faz sentido com a transmissão no ar.
        if l.live {
            ui.live_title = Some(qualidade.clone());
        }
        ui.quality = Some(qualidade);
    }

    if let Some(msg) = &l.last_message {
        ui.last_message = Some(format!("{}: {}", msg.name, msg.text));
        ui.last_time = hora_do_instante(&msg.at);
    }

    // Só mostra a contagem quando há mensagem não lida; zero fica de fora.
    if l.unread > 0 {
        ui.unread = Some(l.unread);
    }

    ui
}

/// Extrai `HH:MM` de um instante ISO-8601 em UTC (`2026-11-04T22:14:05Z` → `22:14`).
/// Formato fora do esperado → `None` (a interface simplesmente não mostra a hora).
fn hora_do_instante(at: &str) -> Option<String> {
    let b = at.as_bytes();
    // `T` na posição 10 e `:` na 13 indicam o formato `AAAA-MM-DDTHH:MM`.
    if b.len() < 16 || b[10] != b'T' || b[13] != b':' {
        return None;
    }
    let hora = at.get(11..16)?;
    // Só dígitos (e o `:` do meio): se não for, o texto não é uma hora legível.
    let legivel = hora
        .bytes()
        .enumerate()
        .all(|(i, c)| if i == 2 { c == b':' } else { c.is_ascii_digit() });
    legivel.then(|| hora.to_string())
}

/// `salas.lista` do WebSocket (`serde_json::Value` de cada sala, formato de `GET /v1/salas`)
/// → listagens. Sala que não desserializa é ignorada (com `tracing::debug!`), nunca erro.
pub fn parse_ws_rooms(values: &[serde_json::Value]) -> Vec<NetListing> {
    values
        .iter()
        .filter_map(|v| match serde_json::from_value::<NetListing>(v.clone()) {
            Ok(listing) => Some(listing),
            Err(e) => {
                tracing::debug!("sala ignorada na lista do servidor: {e}");
                None
            }
        })
        .collect()
}

/// Listagem do servidor → `zap_proto::Room` (para `RoomModel`): `id`, `code = Some(codigo)`,
/// `name`, `owner_id = dono.id`, `access`, `mode = Online`, `created_at = criada_em`.
pub fn listing_to_room(l: &NetListing) -> zap_proto::Room {
    zap_proto::Room {
        id: l.id.clone(),
        code: Some(l.code.clone()),
        name: l.name.clone(),
        owner_id: l.owner.id.clone(),
        access: l.access,
        mode: RoomMode::Online,
        created_at: l.created_at.clone(),
    }
}

/// Normaliza o código digitado: tira espaços, põe em maiúsculas, aceita sem o prefixo
/// (`"7k2q"` → `ZAP-7K2Q`; `"zap-7k2q"` e `" ZAP 7K2Q "` idem). Formato final inválido
/// (4 caracteres do alfabeto de `RoomCode::ALPHABET` depois de `ZAP-`) → `CoreError::Invalid`
/// com a mensagem `"Código de sala inválido"`.
pub fn parse_code(input: &str) -> Result<RoomCode, crate::CoreError> {
    // Sem espaços e em maiúsculas: "zap 7k2q" vira "ZAP7K2Q".
    let limpo: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();

    // "ZAP-" já está; "ZAP" + 4 caracteres é o prefixo sem traço; senão é só o corpo.
    let corpo = limpo
        .strip_prefix("ZAP-")
        .or_else(|| limpo.strip_prefix("ZAP").filter(|r| r.len() == 4))
        .unwrap_or(&limpo);

    RoomCode::parse(&format!("ZAP-{corpo}")).map_err(|_| crate::CoreError::Invalid("Código de sala inválido".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoreError;
    use serde_json::json;

    fn sala_completa() -> serde_json::Value {
        json!({
            "id": "s_x81Pq0",
            "codigo": "ZAP-7K2Q",
            "nome": "Sexta do Rush",
            "dono": { "id": "p_8Kq2mZ0aL1", "nome": "Marcão", "cor": "#FFB547" },
            "acesso": "aberta",
            "criada_em": "2026-11-04T21:30:00Z",
            "participantes": 4,
            "ao_vivo": true,
            "destaque": { "altura": 1080, "fps": 60 },
            "ultima_mensagem": { "nome": "Rafa", "texto": "bora", "em": "2026-11-04T22:14:05Z" },
            "nao_lidas": 3
        })
    }

    fn sala_minima() -> serde_json::Value {
        json!({
            "id": "s_min001",
            "codigo": "ZAP-2345",
            "nome": "Sala quieta",
            "dono": { "id": "p_1", "nome": "Ju", "cor": "#7CF2C0" },
            "acesso": "senha",
            "criada_em": "2026-11-04T21:30:00Z",
            "participantes": 1
        })
    }

    fn listing(v: serde_json::Value) -> NetListing {
        serde_json::from_value(v).expect("listagem de teste válida")
    }

    #[test]
    fn listing_basica() {
        let ui = listing_to_ui(&listing(sala_minima()));
        assert_eq!(ui.room_id, "s_min001");
        assert_eq!(ui.name, "Sala quieta");
        assert_eq!(ui.host, "Ju");
        assert_eq!(ui.participants, 1);
        assert!(ui.has_password);
        assert_eq!(ui.mode, RoomMode::Online);
        assert_eq!(ui.addr.as_deref(), Some("ZAP-2345"));
        assert_eq!(ui.live_title, None);
        assert_eq!(ui.quality, None);
        assert_eq!(ui.last_message, None);
        assert_eq!(ui.last_time, None);
        assert_eq!(ui.unread, None);
        assert_eq!(ui.scene, None);
        assert_eq!(ui.typing, None);
        assert_eq!(ui.viewers, None);
    }

    #[test]
    fn listing_com_destaque_e_chat() {
        let ui = listing_to_ui(&listing(sala_completa()));
        assert!(!ui.has_password);
        assert_eq!(ui.quality.as_deref(), Some("1080p60"));
        assert_eq!(ui.live_title.as_deref(), Some("1080p60"));
        assert_eq!(ui.last_message.as_deref(), Some("Rafa: bora"));
        assert_eq!(ui.last_time.as_deref(), Some("22:14"));
        assert_eq!(ui.unread, Some(3));

        // `live_title` só com ao_vivo; `quality` continua com o destaque.
        let mut v = sala_completa();
        v["ao_vivo"] = json!(false);
        let ui = listing_to_ui(&listing(v));
        assert_eq!(ui.live_title, None);
        assert_eq!(ui.quality.as_deref(), Some("1080p60"));

        // `unread` omitido quando zero.
        let mut v = sala_completa();
        v["nao_lidas"] = json!(0);
        assert_eq!(listing_to_ui(&listing(v)).unread, None);
    }

    #[test]
    fn hora_ilegivel_vira_none() {
        assert_eq!(hora_do_instante("ontem"), None);
        assert_eq!(hora_do_instante("2026-11-04 22:14:05Z"), None);
        assert_eq!(hora_do_instante("2026-11-04T22:1x:05Z"), None);
        assert_eq!(hora_do_instante(""), None);
        assert_eq!(hora_do_instante("2026-11-04T22:14:05Z").as_deref(), Some("22:14"));

        let mut v = sala_completa();
        v["ultima_mensagem"]["em"] = json!("isso não é uma data");
        let ui = listing_to_ui(&listing(v));
        assert_eq!(ui.last_time, None);
        assert_eq!(ui.last_message.as_deref(), Some("Rafa: bora"));
    }

    #[test]
    fn parse_ws_rooms_ignora_invalidos() {
        let valores = vec![sala_completa(), json!({ "x": 1 })];
        let salas = parse_ws_rooms(&valores);
        assert_eq!(salas.len(), 1);
        assert_eq!(salas[0].code.as_str(), "ZAP-7K2Q");
    }

    #[test]
    fn listing_to_room_campos() {
        let room = listing_to_room(&listing(sala_completa()));
        assert_eq!(room.id, "s_x81Pq0");
        assert_eq!(room.code.as_ref().map(RoomCode::as_str), Some("ZAP-7K2Q"));
        assert_eq!(room.name, "Sexta do Rush");
        assert_eq!(room.owner_id, "p_8Kq2mZ0aL1");
        assert_eq!(room.access, Access::Open);
        assert_eq!(room.mode, RoomMode::Online);
        assert_eq!(room.created_at, "2026-11-04T21:30:00Z");

        let room = listing_to_room(&listing(sala_minima()));
        assert_eq!(room.access, Access::Password);
    }

    #[test]
    fn parse_code_aceita_variacoes() {
        for entrada in ["7k2q", "zap-7k2q", " ZAP 7K2Q ", "zap 7k2q", "ZAP-7K2Q"] {
            let code = parse_code(entrada).expect("código válido");
            assert_eq!(code.as_str(), "ZAP-7K2Q", "entrada {entrada:?}");
        }
    }

    #[test]
    fn parse_code_recusa() {
        for entrada in ["ZAP-12", "ZAP-0000", ""] {
            match parse_code(entrada) {
                Err(CoreError::Invalid(msg)) => {
                    assert_eq!(msg, "Código de sala inválido", "entrada {entrada:?}")
                }
                outro => panic!("deveria recusar {entrada:?}, veio {outro:?}"),
            }
        }
    }
}
