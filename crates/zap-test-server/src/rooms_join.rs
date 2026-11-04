//! Regras de salas (REST), parte 2: entrar, sair, encerrar e prévia.

use zap_net::online::{BridgeInfo, JoinMedia, JoinResponse, PreviewMedia, PreviewResponse};
use zap_proto::{Access, ControlMsg, RoomCode};

use crate::{
    rooms::listing_of,
    state::{Hub, HubError, Inner, TicketData},
    util::{iso_from_unix_ms, new_token, now_unix_ms, verify_password},
};

/// Validade do ingresso de uma entrada normal: 1 h.
const VALIDADE_INGRESSO_MS: u64 = 3_600_000;
/// Validade do ingresso de prévia: 60 s.
const VALIDADE_PREVIA_MS: u64 = 60_000;
/// Protocolo de mídia anunciado na ponte.
const ALPN: &str = "zap/4";

impl Hub {
    /// `POST /v1/salas/{codigo}/entrar`: confere senha (`senha_sala`) e lotação (`sala_cheia`),
    /// adiciona como membro (idempotente), emite ingresso de 1 h e devolve `JoinResponse`.
    /// Emite `participante.entrou` aos que estão na sala via WS.
    pub fn join_room(&self, who: &str, code: &RoomCode, password: Option<&str>) -> Result<JoinResponse, HubError> {
        let mut inner = self.inner.lock();
        if !inner.participants.contains_key(who) {
            return Err(nao_autenticado());
        }
        let sala_id = id_da_sala(&inner, code)?;

        // Confere antes de mudar qualquer coisa: senha e lotação só valem para quem não é membro.
        let sala = inner.rooms.get(&sala_id).ok_or_else(sala_nao_encontrada)?;
        let ja_membro = sala.members.iter().any(|m| m == who);
        if !ja_membro {
            if sala.access == Access::Password {
                let confere = sala
                    .password_hash
                    .as_deref()
                    .is_some_and(|hash| verify_password(password.unwrap_or(""), hash));
                if !confere {
                    return Err(HubError::new(403, "senha_sala", "Senha da sala incorreta"));
                }
            }
            if u32::try_from(sala.members.len()).unwrap_or(u32::MAX) >= self.config.participants_per_room {
                return Err(HubError::new(409, "sala_cheia", "Sala cheia"));
            }
        }

        let ingresso = new_token();
        let expira_ms = now_unix_ms() + VALIDADE_INGRESSO_MS;
        {
            let Inner {
                rooms,
                participants,
                tickets,
                ..
            } = &mut *inner;
            let sala = rooms.get_mut(&sala_id).ok_or_else(sala_nao_encontrada)?;
            if !ja_membro {
                sala.members.push(who.to_string());
            }
            sala.empty_since_ms = None;
            if let Some(p) = participants.get_mut(who) {
                p.room_id = Some(sala_id.clone());
            }
            tickets.insert(
                ingresso.clone(),
                TicketData {
                    participant_id: who.to_string(),
                    room_id: sala_id.clone(),
                    preview: false,
                    expires_at_ms: expira_ms,
                },
            );
        }

        // Só quem é novo na sala gera aviso; a lista de salas muda em ambos os casos.
        let inner = &*inner;
        let sala = inner.rooms.get(&sala_id).ok_or_else(sala_nao_encontrada)?;
        if !ja_membro {
            if let Some(p) = inner.participants.get(who) {
                let view = inner.participant_view(p, sala);
                inner.broadcast_room(&sala_id, ControlMsg::ParticipantJoined { participant: view }, Some(who));
            }
            inner.broadcast_room_list();
        }

        Ok(JoinResponse {
            room: listing_of(inner, sala, who),
            participant_id: who.to_string(),
            ticket: ingresso,
            ticket_expires_at: iso_from_unix_ms(expira_ms),
            media: JoinMedia {
                bridge: self.config.relay.then(|| self.ponte()),
                direct_allowed: self.config.direct_allowed,
            },
        })
    }

    /// `POST /v1/salas/{codigo}/sair`: remove o membro e suas trilhas; avisa `participante.saiu`
    /// (motivo `saiu`) e `trilha.removida`. Se era o último, marca `empty_since_ms`. Sair também
    /// invalida os ingressos de `who` nessa sala (para voltar, `entrar` de novo).
    pub fn leave_room(&self, who: &str, code: &RoomCode) -> Result<(), HubError> {
        // Primeiro o trabalho de membros, sob lock; o lock é solto antes de `drop_participant_tracks`,
        // que trava de novo.
        let sala_id = {
            let mut inner = self.inner.lock();
            let sala_id = id_da_sala(&inner, code)?;
            let Inner {
                rooms,
                participants,
                tickets,
                ..
            } = &mut *inner;
            let sala = rooms.get_mut(&sala_id).ok_or_else(sala_nao_encontrada)?;
            let posicao = sala
                .members
                .iter()
                .position(|m| m == who)
                .ok_or_else(sala_nao_encontrada)?;
            sala.members.remove(posicao);
            tickets.retain(|_, t| !(t.participant_id == who && t.room_id == sala_id));
            if sala.members.is_empty() {
                sala.empty_since_ms = Some(now_unix_ms());
            }
            if let Some(p) = participants.get_mut(who) {
                p.room_id = None;
                p.ws_room = None;
                if p.quic_room.as_deref() == Some(sala_id.as_str()) {
                    p.quic_room = None;
                }
            }
            sala_id
        };

        self.drop_participant_tracks(who, &sala_id);

        let inner = self.inner.lock();
        inner.broadcast_room(
            &sala_id,
            ControlMsg::ParticipantLeft {
                participant_id: who.to_string(),
                reason: "saiu".to_string(),
            },
            None,
        );
        inner.broadcast_room_list();
        Ok(())
    }

    /// `DELETE /v1/salas/{codigo}`: só o dono (`proibido`); manda `sala.encerrada` (`dono`).
    pub fn delete_room(&self, who: &str, code: &RoomCode) -> Result<(), HubError> {
        let mut inner = self.inner.lock();
        let sala_id = id_da_sala(&inner, code)?;
        let dona = inner.rooms.get(&sala_id).is_some_and(|s| s.owner_id == who);
        if !dona {
            return Err(HubError::new(403, "proibido", "Só o dono pode encerrar a sala"));
        }

        // Avisa antes de limpar `ws_room`, que é o que define quem recebe.
        inner.broadcast_room(
            &sala_id,
            ControlMsg::RoomClosed {
                reason: "dono".to_string(),
            },
            None,
        );
        inner.rooms.remove(&sala_id);
        inner.tickets.retain(|_, t| t.room_id != sala_id);
        for p in inner.participants.values_mut() {
            if p.room_id.as_deref() == Some(sala_id.as_str()) {
                p.room_id = None;
            }
            if p.ws_room.as_deref() == Some(sala_id.as_str()) {
                p.ws_room = None;
            }
        }
        inner.broadcast_room_list();
        Ok(())
    }

    /// `POST /v1/salas/{codigo}/previa`: só `aberta` (`proibido` senão) e só com ponte (sem ponte
    /// não há prévia: `proibido`); ingresso de 60 s com `preview: true`.
    pub fn preview_room(&self, who: &str, code: &RoomCode) -> Result<PreviewResponse, HubError> {
        if !self.config.relay {
            return Err(HubError::new(
                403,
                "proibido",
                "Este servidor não faz ponte de mídia, então não há prévia",
            ));
        }
        let mut inner = self.inner.lock();
        if !inner.participants.contains_key(who) {
            return Err(nao_autenticado());
        }
        let sala_id = id_da_sala(&inner, code)?;
        let aberta = inner.rooms.get(&sala_id).is_some_and(|s| s.access == Access::Open);
        if !aberta {
            return Err(HubError::new(403, "proibido", "Prévia só em salas abertas"));
        }

        // Prévia não adiciona membro: só o ingresso com `preview: true`.
        let ingresso = new_token();
        let expira_ms = now_unix_ms() + VALIDADE_PREVIA_MS;
        inner.tickets.insert(
            ingresso.clone(),
            TicketData {
                participant_id: who.to_string(),
                room_id: sala_id,
                preview: true,
                expires_at_ms: expira_ms,
            },
        );
        Ok(PreviewResponse {
            ticket: ingresso,
            expires_at: iso_from_unix_ms(expira_ms),
            media: PreviewMedia { bridge: self.ponte() },
        })
    }

    /// Endereço da ponte QUIC para as respostas de entrada e prévia.
    fn ponte(&self) -> BridgeInfo {
        BridgeInfo {
            address: self.bridge_addr.get().cloned().unwrap_or_default(),
            alpn: ALPN.to_string(),
        }
    }
}

/// Id da sala com esse código, ou `sala_nao_encontrada`.
fn id_da_sala(inner: &Inner, code: &RoomCode) -> Result<String, HubError> {
    inner
        .rooms
        .values()
        .find(|s| s.code == *code)
        .map(|s| s.id.clone())
        .ok_or_else(sala_nao_encontrada)
}

fn sala_nao_encontrada() -> HubError {
    HubError::new(404, "sala_nao_encontrada", "Sala não encontrada")
}

fn nao_autenticado() -> HubError {
    HubError::new(401, "nao_autenticado", "Sessão inválida ou expirada")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{util::now_unix_ms, Config};
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};

    /// Entra um participante novo (protocolo 4, sem senha de servidor) e devolve o id.
    fn entrar(hub: &Hub, instalacao: &str, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: instalacao.into(),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

    /// Cria uma sala com `dono` e devolve (id, código).
    fn criar(hub: &Hub, dono: &str, acesso: Access, senha: Option<&str>) -> (String, RoomCode) {
        let req = CreateRoomRequest {
            name: "Sala".into(),
            access: acesso,
            password: senha.map(String::from),
        };
        let listagem = hub.create_room(dono, req).unwrap();
        (listagem.id, listagem.code)
    }

    /// Registra um canal de WebSocket para `id` (com `ws_room` dado) e devolve o receptor.
    fn canal(hub: &Hub, id: &str, ws_room: Option<&str>) -> mpsc::UnboundedReceiver<ControlMsg> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut inner = hub.inner.lock();
        let p = inner.participants.get_mut(id).unwrap();
        p.ws = Some(tx);
        p.ws_room = ws_room.map(String::from);
        rx
    }

    /// Tira tudo o que já chegou ao canal.
    fn drenar(rx: &mut mpsc::UnboundedReceiver<ControlMsg>) -> Vec<ControlMsg> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[test]
    fn entrar_sala_aberta() {
        let hub = Hub::new(Config::default());
        let _ = hub.bridge_addr.set("127.0.0.1:7443".to_string());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (sala_id, codigo) = criar(&hub, &ana, Access::Open, None);
        let mut rx_ana = canal(&hub, &ana, Some(&sala_id));

        let resp = hub.join_room(&bia, &codigo, None).unwrap();
        assert!(!resp.ticket.is_empty());
        assert_eq!(resp.participant_id, bia);
        assert_eq!(resp.room.participants, 2);
        let ponte = resp.media.bridge.expect("relay ligado: ponte presente");
        assert_eq!(ponte.address, "127.0.0.1:7443");
        assert_eq!(ponte.alpn, "zap/4");
        assert!(resp.media.direct_allowed);

        {
            let inner = hub.inner.lock();
            let ingresso = &inner.tickets[&resp.ticket];
            assert_eq!(ingresso.participant_id, bia);
            assert!(!ingresso.preview);
            assert_eq!(inner.participants[&bia].room_id.as_deref(), Some(sala_id.as_str()));
        }
        let msgs = drenar(&mut rx_ana);
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ControlMsg::ParticipantJoined { participant } if participant.id == bia)));
    }

    #[test]
    fn entrar_idempotente() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (sala_id, codigo) = criar(&hub, &ana, Access::Open, None);
        let mut rx_ana = canal(&hub, &ana, Some(&sala_id));

        hub.join_room(&bia, &codigo, None).unwrap();
        let de_novo = hub.join_room(&bia, &codigo, None).unwrap();
        assert_eq!(de_novo.room.participants, 2);
        assert_eq!(hub.inner.lock().rooms[&sala_id].members, vec![ana.clone(), bia.clone()]);
        // Só o primeiro ingresso avisa que alguém entrou.
        let avisos = drenar(&mut rx_ana)
            .iter()
            .filter(|m| matches!(m, ControlMsg::ParticipantJoined { .. }))
            .count();
        assert_eq!(avisos, 1);
    }

    #[test]
    fn senha_errada() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (_, codigo) = criar(&hub, &ana, Access::Password, Some("segredo"));

        let sem = hub.join_room(&bia, &codigo, None).unwrap_err();
        assert_eq!((sem.status, sem.code), (403, "senha_sala"));
        let errada = hub.join_room(&bia, &codigo, Some("errada")).unwrap_err();
        assert_eq!((errada.status, errada.code), (403, "senha_sala"));
        assert!(hub.join_room(&bia, &codigo, Some("segredo")).is_ok());
    }

    #[test]
    fn sala_cheia() {
        let hub = Hub::new(Config {
            participants_per_room: 1,
            ..Config::default()
        });
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (_, codigo) = criar(&hub, &ana, Access::Open, None);

        // Quem já é membro entra de novo sem problema, mesmo com a sala cheia.
        assert!(hub.join_room(&ana, &codigo, None).is_ok());
        let erro = hub.join_room(&bia, &codigo, None).unwrap_err();
        assert_eq!((erro.status, erro.code), (409, "sala_cheia"));
    }

    #[test]
    fn sair() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (sala_id, codigo) = criar(&hub, &ana, Access::Open, None);
        let ingresso = hub.join_room(&bia, &codigo, None).unwrap().ticket;
        let mut rx_ana = canal(&hub, &ana, Some(&sala_id));

        hub.leave_room(&bia, &codigo).unwrap();
        assert!(
            !hub.inner.lock().tickets.contains_key(&ingresso),
            "sair invalida o ingresso da sala"
        );
        let msgs = drenar(&mut rx_ana);
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ControlMsg::ParticipantLeft { participant_id, reason } if participant_id == &bia && reason == "saiu")));
        {
            let inner = hub.inner.lock();
            assert_eq!(inner.rooms[&sala_id].members, vec![ana.clone()]);
            assert!(inner.rooms[&sala_id].empty_since_ms.is_none());
            assert!(inner.participants[&bia].room_id.is_none());
        }

        // Último a sair: a sala fica marcada como vazia.
        hub.leave_room(&ana, &codigo).unwrap();
        assert!(hub.inner.lock().rooms[&sala_id].empty_since_ms.is_some());
        // Quem não é membro não consegue sair.
        let erro = hub.leave_room(&bia, &codigo).unwrap_err();
        assert_eq!(erro.code, "sala_nao_encontrada");
    }

    #[test]
    fn encerrar_so_dono() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (sala_id, codigo) = criar(&hub, &ana, Access::Open, None);
        hub.join_room(&bia, &codigo, None).unwrap();
        let mut rx_ana = canal(&hub, &ana, Some(&sala_id));
        let mut rx_bia = canal(&hub, &bia, Some(&sala_id));

        let erro = hub.delete_room(&bia, &codigo).unwrap_err();
        assert_eq!((erro.status, erro.code), (403, "proibido"));
        assert!(drenar(&mut rx_bia).is_empty(), "a tentativa recusada não avisa ninguém");

        hub.delete_room(&ana, &codigo).unwrap();
        for rx in [&mut rx_ana, &mut rx_bia] {
            let msgs = drenar(rx);
            assert!(msgs
                .iter()
                .any(|m| matches!(m, ControlMsg::RoomClosed { reason } if reason == "dono")));
        }
        let inner = hub.inner.lock();
        assert!(inner.rooms.is_empty());
        assert!(inner.participants[&bia].room_id.is_none());
        assert!(inner.participants[&bia].ws_room.is_none());
    }

    #[test]
    fn previa_so_aberta() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");

        let (_, codigo_fechada) = criar(&hub, &ana, Access::Password, Some("segredo"));
        let erro = hub.preview_room(&bia, &codigo_fechada).unwrap_err();
        assert_eq!((erro.status, erro.code), (403, "proibido"));

        let (sala_id, codigo_aberta) = criar(&hub, &ana, Access::Open, None);
        let previa = hub.preview_room(&bia, &codigo_aberta).unwrap();
        assert!(!previa.ticket.is_empty());
        let inner = hub.inner.lock();
        let ingresso = &inner.tickets[&previa.ticket];
        assert!(ingresso.preview);
        assert_eq!(ingresso.participant_id, bia);
        assert_eq!(ingresso.room_id, sala_id);
        assert!(ingresso.expires_at_ms <= now_unix_ms() + 60_000);
        // Prévia não vira participação.
        assert!(!inner.rooms[&sala_id].members.contains(&bia));
        assert!(inner.participants[&bia].room_id.is_none());
    }

    #[test]
    fn previa_sem_ponte_e_proibida() {
        let hub = Hub::new(Config {
            relay: false,
            ..Config::default()
        });
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let (_, codigo) = criar(&hub, &ana, Access::Open, None);
        let erro = hub.preview_room(&bia, &codigo).unwrap_err();
        assert_eq!((erro.status, erro.code), (403, "proibido"));
        // Entrar continua funcionando, só sem `midia.ponte`.
        assert!(hub.join_room(&bia, &codigo, None).unwrap().media.bridge.is_none());
    }
}
