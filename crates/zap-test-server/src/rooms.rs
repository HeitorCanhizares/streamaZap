//! Regras de salas (REST): criar, listar, detalhes, entrar, sair, encerrar, prévia.

use std::collections::VecDeque;

use zap_net::online::{
    CreateRoomRequest, Highlight, LastMessage, OwnerInfo, ParticipantBrief, RoomDetails, RoomListing, ServerDiagnostics,
};
use zap_proto::{Access, RoomCode, TrackKind};

use crate::{
    state::{Hub, HubError, Inner, RoomData},
    util::{hash_password, new_id, new_room_code, now_iso, now_unix_ms},
};

/// Tamanho máximo do nome da sala, em caracteres.
const NOME_MAX_SALA: usize = 48;

impl Hub {
    /// `GET /v1/salas`: todas as salas, `nao_lidas` = 0, `ultima_mensagem` só se `viewer` é membro.
    pub fn list_rooms(&self, viewer: &str) -> Vec<RoomListing> {
        let inner = self.inner.lock();
        let mut salas: Vec<&RoomData> = inner.rooms.values().collect();
        // Ordem estável: criação (ISO ordena certo) e depois id, para empate.
        salas.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
        salas.into_iter().map(|sala| listing_of(&inner, sala, viewer)).collect()
    }

    /// `POST /v1/salas`: quem cria vira dono e membro. `acesso: senha` exige `senha` (`invalido`).
    pub fn create_room(&self, owner: &str, req: CreateRoomRequest) -> Result<RoomListing, HubError> {
        let nome = req.name.trim().to_string();
        let tamanho = nome.chars().count();
        if tamanho == 0 || tamanho > NOME_MAX_SALA {
            return Err(HubError::new(400, "invalido", "nome da sala inválido"));
        }
        let password_hash = match req.access {
            Access::Open => None,
            Access::Password => match req.password.as_deref().filter(|s| !s.is_empty()) {
                Some(senha) => Some(hash_password(senha)),
                None => return Err(HubError::new(400, "invalido", "senha da sala obrigatória")),
            },
        };

        let mut inner = self.inner.lock();
        if !inner.participants.contains_key(owner) {
            return Err(nao_autenticado());
        }
        // Código único entre as salas existentes; o espaço (31^4) é grande, a repetição é rara.
        let code = loop {
            let candidato = new_room_code();
            if !inner.rooms.values().any(|s| s.code == candidato) {
                break candidato;
            }
        };
        let id = new_id("s");
        let sala = RoomData {
            id: id.clone(),
            code,
            name: nome,
            owner_id: owner.to_string(),
            access: req.access,
            password_hash,
            created_at: now_iso(),
            members: vec![owner.to_string()],
            tracks: Vec::new(),
            chat: VecDeque::new(),
            next_track_number: 1,
            empty_since_ms: None,
        };
        let listagem = listing_of(&inner, &sala, owner);
        if let Some(dona) = inner.participants.get_mut(owner) {
            dona.room_id = Some(id.clone());
        }
        inner.rooms.insert(id, sala);
        Ok(listagem)
    }

    /// `GET /v1/salas/{codigo}`. `participantes_lista` some em sala com senha para não-membros.
    pub fn room_details(&self, viewer: &str, code: &RoomCode) -> Result<RoomDetails, HubError> {
        let inner = self.inner.lock();
        let sala = inner
            .rooms
            .values()
            .find(|s| s.code == *code)
            .ok_or_else(|| HubError::new(404, "sala_nao_encontrada", "Sala não encontrada"))?;
        let room = listing_of(&inner, sala, viewer);
        let lista_visivel = sala.access == Access::Open || sala.members.iter().any(|m| m == viewer);
        let participant_list = if lista_visivel {
            Some(
                sala.members
                    .iter()
                    .map(|m| participante_breve(&inner, sala, m))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        Ok(RoomDetails { room, participant_list })
    }

    /// `GET /v1/diagnostico` para o participante.
    pub fn diagnostics(&self, who: &str) -> Result<ServerDiagnostics, HubError> {
        let inner = self.inner.lock();
        let quem = inner.participants.get(who).ok_or_else(nao_autenticado)?;
        Ok(ServerDiagnostics {
            public_ip: quem.public_ip.clone(),
            public_udp_port: quem.quic_port,
            protocol_ok: true,
            now: now_iso(),
        })
    }

    /// Apaga salas vazias há mais de `after_ms` (a regra real é 2 min).
    pub fn sweep(&self, after_ms: u64) {
        let agora = now_unix_ms();
        let mut inner = self.inner.lock();
        let vencidas: Vec<String> = inner
            .rooms
            .values()
            .filter(|s| s.empty_since_ms.is_some_and(|t| agora.saturating_sub(t) >= after_ms))
            .map(|s| s.id.clone())
            .collect();
        if vencidas.is_empty() {
            return;
        }
        inner.rooms.retain(|id, _| !vencidas.contains(id));
        // Quem ainda apontava para uma sala apagada fica sem sala (REST e WebSocket).
        for participante in inner.participants.values_mut() {
            if participante.room_id.as_ref().is_some_and(|r| vencidas.contains(r)) {
                participante.room_id = None;
            }
            if participante.ws_room.as_ref().is_some_and(|r| vencidas.contains(r)) {
                participante.ws_room = None;
            }
        }
        // Ingressos emitidos para a sala deixam de servir.
        inner.tickets.retain(|_, t| !vencidas.contains(&t.room_id));
    }
}

/// Converte uma sala em `RoomListing` (formato de `GET /v1/salas`). `viewer` decide se
/// `ultima_mensagem` aparece (só para membros). Usada também por `signaling` e `rooms_join`.
pub fn listing_of(inner: &Inner, room: &RoomData, viewer: &str) -> RoomListing {
    // Dono que sumiu da lista de participantes aparece sem nome e com cor neutra.
    let owner = match inner.participants.get(&room.owner_id) {
        Some(p) => OwnerInfo {
            id: room.owner_id.clone(),
            name: p.name.clone(),
            color: p.color.clone(),
        },
        None => OwnerInfo {
            id: room.owner_id.clone(),
            name: String::new(),
            color: "#FFFFFF".to_string(),
        },
    };
    // Vídeo "de destaque" é a trilha de vídeo de menor número; áudio não conta.
    let primeira_video = room
        .tracks
        .iter()
        .filter(|t| t.kind != TrackKind::Audio)
        .min_by_key(|t| t.number);
    let highlight = primeira_video.and_then(|t| t.layers.first()).map(|camada| Highlight {
        height: camada.height,
        fps: camada.fps,
    });
    let last_message = if room.members.iter().any(|m| m == viewer) {
        room.chat.back().map(|entrada| LastMessage {
            name: entrada.name.clone(),
            text: entrada.text.clone(),
            at: entrada.at.clone(),
        })
    } else {
        None
    };
    RoomListing {
        id: room.id.clone(),
        code: room.code.clone(),
        name: room.name.clone(),
        owner,
        access: room.access,
        created_at: room.created_at.clone(),
        participants: u32::try_from(room.members.len()).unwrap_or(u32::MAX),
        live: primeira_video.is_some(),
        highlight,
        last_message,
        unread: 0,
    }
}

/// Participante como aparece em `participantes_lista`, com as trilhas que ele publica na sala.
fn participante_breve(inner: &Inner, sala: &RoomData, id: &str) -> ParticipantBrief {
    let (name, color) = match inner.participants.get(id) {
        Some(p) => (p.name.clone(), p.color.clone()),
        None => (String::new(), "#FFFFFF".to_string()),
    };
    let sharing = sala
        .tracks
        .iter()
        .filter(|t| t.participant_id == id)
        .map(|t| tipo_do_fio(t.kind).to_string())
        .collect();
    ParticipantBrief {
        id: id.to_string(),
        name,
        color,
        sharing,
    }
}

/// Tipo de trilha como aparece no fio (`compartilhando`).
fn tipo_do_fio(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Screen => "tela",
        TrackKind::Window => "janela",
        TrackKind::Camera => "camera",
        TrackKind::Audio => "audio",
    }
}

fn nao_autenticado() -> HubError {
    HubError::new(401, "nao_autenticado", "Sessão inválida ou expirada")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{util::now_unix_ms, Config};
    use zap_net::online::LoginRequest;

    /// Entra um participante novo (protocolo 4, sem senha de servidor) e devolve o id.
    fn entrar(hub: &Hub, instalacao: &str, nome: &str, ip: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: instalacao.into(),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
        };
        hub.login(pedido, ip).unwrap().participant_id
    }

    fn pedido(nome: &str, acesso: Access, senha: Option<&str>) -> CreateRoomRequest {
        CreateRoomRequest {
            name: nome.into(),
            access: acesso,
            password: senha.map(String::from),
        }
    }

    #[test]
    fn criar_e_listar() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "203.0.113.7");
        let sala = hub
            .create_room(&dona, pedido("Sexta do Rush", Access::Open, None))
            .unwrap();
        assert!(sala.id.starts_with("s_"));
        assert!(sala.code.as_str().starts_with("ZAP-"));
        assert_eq!(sala.code.as_str().len(), 8);
        assert_eq!(sala.owner.id, dona);
        assert_eq!(sala.owner.name, "Ana");
        assert_eq!(sala.participants, 1);
        assert!(!sala.live);
        assert_eq!(sala.highlight, None);
        assert_eq!(sala.unread, 0);

        let lista = hub.list_rooms(&dona);
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, sala.id);
        assert_eq!(lista[0].code, sala.code);
    }

    #[test]
    fn sala_senha_exige_senha() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "ip");
        let sem = hub
            .create_room(&dona, pedido("Privada", Access::Password, None))
            .unwrap_err();
        assert_eq!((sem.status, sem.code), (400, "invalido"));
        let vazia = hub
            .create_room(&dona, pedido("Privada", Access::Password, Some("")))
            .unwrap_err();
        assert_eq!((vazia.status, vazia.code), (400, "invalido"));
        let com = hub.create_room(&dona, pedido("Privada", Access::Password, Some("segredo")));
        assert!(com.is_ok());
    }

    #[test]
    fn nome_invalido() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "ip");
        let vazio = hub.create_room(&dona, pedido("   ", Access::Open, None)).unwrap_err();
        assert_eq!((vazio.status, vazio.code), (400, "invalido"));
        let longo = "x".repeat(49);
        let grande = hub.create_room(&dona, pedido(&longo, Access::Open, None)).unwrap_err();
        assert_eq!((grande.status, grande.code), (400, "invalido"));
        let limite = "x".repeat(48);
        assert!(hub.create_room(&dona, pedido(&limite, Access::Open, None)).is_ok());
    }

    #[test]
    fn detalhes_sala_senha() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "ip");
        let outra = entrar(&hub, "inst-2", "Bia", "ip");
        let sala = hub
            .create_room(&dona, pedido("Privada", Access::Password, Some("segredo")))
            .unwrap();

        let de_fora = hub.room_details(&outra, &sala.code).unwrap();
        assert_eq!(de_fora.room.id, sala.id);
        assert!(de_fora.participant_list.is_none());

        let de_dentro = hub.room_details(&dona, &sala.code).unwrap();
        let lista = de_dentro.participant_list.expect("dono é membro");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, dona);
        assert_eq!(lista[0].name, "Ana");
        assert!(lista[0].sharing.is_empty());
    }

    #[test]
    fn detalhes_inexistente() {
        let hub = Hub::new(Config::default());
        let quem = entrar(&hub, "inst-1", "Ana", "ip");
        let codigo = RoomCode::parse("ZAP-2222").unwrap();
        let erro = hub.room_details(&quem, &codigo).unwrap_err();
        assert_eq!((erro.status, erro.code), (404, "sala_nao_encontrada"));
    }

    #[test]
    fn diagnostico() {
        let hub = Hub::new(Config::default());
        let quem = entrar(&hub, "inst-1", "Ana", "203.0.113.7");
        let diag = hub.diagnostics(&quem).unwrap();
        assert_eq!(diag.public_ip, "203.0.113.7");
        assert!(diag.protocol_ok);
        assert!(!diag.now.is_empty());
        assert_eq!(diag.public_udp_port, None);
    }

    #[test]
    fn sweep() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "ip");
        let sala = hub.create_room(&dona, pedido("Velha", Access::Open, None)).unwrap();
        // Sala esvaziada há 10 s: some com `sweep(1)`.
        let antigo = now_unix_ms() - 10_000;
        hub.inner.lock().rooms.get_mut(&sala.id).unwrap().empty_since_ms = Some(antigo);
        hub.sweep(1);
        assert!(hub.list_rooms(&dona).is_empty());
        assert!(hub.inner.lock().participants[&dona].room_id.is_none());
    }

    #[test]
    fn sweep_respeita_tempo() {
        let hub = Hub::new(Config::default());
        let dona = entrar(&hub, "inst-1", "Ana", "ip");
        let sala = hub.create_room(&dona, pedido("Recente", Access::Open, None)).unwrap();
        hub.inner.lock().rooms.get_mut(&sala.id).unwrap().empty_since_ms = Some(now_unix_ms());
        hub.sweep(60_000);
        assert_eq!(hub.list_rooms(&dona).len(), 1);
    }
}
