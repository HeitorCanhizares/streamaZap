//! Envio de mensagens aos WebSockets e conversões de estado em tipos do protocolo.
//!
//! Tudo aqui roda com o `Mutex` do `Hub` já travado (recebe `&Inner`), nunca bloqueia e nunca
//! espera: o envio é `UnboundedSender::send` e erro (WebSocket já fechado) é ignorado.

use zap_proto::{ControlMsg, Participant, Role, Room, RoomMode, Track};

use crate::{
    rooms::listing_of,
    state::{Inner, ParticipantData, RoomData, TrackData},
};

impl Inner {
    /// Manda `msg` ao WebSocket de `who`, se houver.
    pub fn send_to(&self, who: &str, msg: ControlMsg) {
        if let Some(tx) = self.participants.get(who).and_then(|p| p.ws.as_ref()) {
            // Erro = WebSocket já fechado; ignorado de propósito.
            let _ = tx.send(msg);
        }
    }

    /// Manda `msg` a todos com `ws_room == room_id`, menos `except`.
    pub fn broadcast_room(&self, room_id: &str, msg: ControlMsg, except: Option<&str>) {
        for p in self.participants.values() {
            if p.ws_room.as_deref() != Some(room_id) || except == Some(p.id.as_str()) {
                continue;
            }
            if let Some(tx) = &p.ws {
                let _ = tx.send(msg.clone());
            }
        }
    }

    /// Manda `salas.lista` (formato de `GET /v1/salas`, visto por cada destinatário) a todos
    /// os participantes com WebSocket aberto.
    pub fn broadcast_room_list(&self) {
        // Mesma ordem de `list_rooms`: criação e depois id.
        let mut salas: Vec<&RoomData> = self.rooms.values().collect();
        salas.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
        for p in self.participants.values() {
            let Some(tx) = &p.ws else { continue };
            // `ultima_mensagem` depende de quem vê: a lista é montada para cada destinatário.
            let lista = salas
                .iter()
                .filter_map(|sala| serde_json::to_value(listing_of(self, sala, &p.id)).ok())
                .collect();
            let _ = tx.send(ControlMsg::RoomList { salas: lista });
        }
    }

    /// `zap_proto::Room` (modo `online`, `codigo` presente) de uma sala.
    pub fn room_view(&self, room: &RoomData) -> Room {
        Room {
            id: room.id.clone(),
            code: Some(room.code.clone()),
            name: room.name.clone(),
            owner_id: room.owner_id.clone(),
            access: room.access,
            mode: RoomMode::Online,
            created_at: room.created_at.clone(),
        }
    }

    /// `zap_proto::Participant` (`papel` = dono se for o dono da sala; `assistindo` vem de
    /// `ParticipantData::watching`).
    pub fn participant_view(&self, p: &ParticipantData, room: &RoomData) -> Participant {
        // `ParticipantData` ainda não guarda latência: fica ausente.
        let role = if p.id == room.owner_id {
            Role::Owner
        } else {
            Role::Member
        };
        Participant {
            id: p.id.clone(),
            name: p.name.clone(),
            color: p.color.clone(),
            role,
            joined_at: p.joined_at.clone(),
            app_version: p.app_version.clone(),
            protocol: p.protocol,
            latency_ms: None,
            watching: p.watching.clone(),
        }
    }

    /// `zap_proto::Track` de uma trilha publicada (sem `endereco`: só modo Local).
    pub fn track_view(&self, t: &TrackData) -> Track {
        Track {
            id: t.id.clone(),
            participant_id: t.participant_id.clone(),
            number: t.number,
            kind: t.kind,
            title: t.title.clone(),
            layers: t.layers.clone(),
            audio: t.audio.clone(),
            address: None,
        }
    }

    /// `sala.estado` completo: sala, participantes (membros) e trilhas.
    pub fn room_state_msg(&self, room: &RoomData) -> ControlMsg {
        // Só membros que existem no estado, na ordem de entrada da sala.
        let participants = room
            .members
            .iter()
            .filter_map(|m| self.participants.get(m))
            .map(|p| self.participant_view(p, room))
            .collect();
        let tracks = room.tracks.iter().map(|t| self.track_view(t)).collect();
        ControlMsg::RoomState {
            room: self.room_view(room),
            participants,
            tracks,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{state::Hub, Config};
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};
    use zap_proto::Access;

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

    /// Registra um canal de WebSocket para `id` (com `ws_room` dado) e devolve o receptor.
    fn canal(inner: &mut Inner, id: &str, ws_room: Option<&str>) -> mpsc::UnboundedReceiver<ControlMsg> {
        let (tx, rx) = mpsc::unbounded_channel();
        let p = inner.participants.get_mut(id).unwrap();
        p.ws = Some(tx);
        p.ws_room = ws_room.map(String::from);
        rx
    }

    /// Cria uma sala aberta com `dono` e devolve o id dela.
    fn criar_sala(hub: &Hub, dono: &str, nome: &str) -> String {
        let req = CreateRoomRequest {
            name: nome.into(),
            access: Access::Open,
            password: None,
        };
        hub.create_room(dono, req).unwrap().id
    }

    #[test]
    fn send_to_entrega() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let mut inner = hub.inner.lock();
        let mut rx = canal(&mut inner, &ana, None);

        inner.send_to(&ana, ControlMsg::Pong { ts_ms: 1 });
        assert_eq!(rx.try_recv().unwrap(), ControlMsg::Pong { ts_ms: 1 });

        // Participante desconhecido e WebSocket fechado não causam pânico.
        inner.send_to("p_inexistente", ControlMsg::Pong { ts_ms: 2 });
        drop(rx);
        inner.send_to(&ana, ControlMsg::Pong { ts_ms: 3 });
    }

    #[test]
    fn broadcast_room_exclui_except() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let caio = entrar(&hub, "inst-caio", "Caio");
        let sala1 = criar_sala(&hub, &ana, "Sala 1");
        let sala2 = criar_sala(&hub, &caio, "Sala 2");

        let mut inner = hub.inner.lock();
        let mut rx_ana = canal(&mut inner, &ana, Some(&sala1));
        let mut rx_bia = canal(&mut inner, &bia, Some(&sala1));
        let mut rx_caio = canal(&mut inner, &caio, Some(&sala2));

        inner.broadcast_room(&sala1, ControlMsg::Pong { ts_ms: 7 }, Some(&ana));

        assert!(rx_ana.try_recv().is_err(), "o `except` não pode receber");
        assert_eq!(rx_bia.try_recv().unwrap(), ControlMsg::Pong { ts_ms: 7 });
        assert!(rx_caio.try_recv().is_err(), "outra sala não recebe");
    }

    #[test]
    fn room_view_online() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let listagem = hub
            .create_room(
                &ana,
                CreateRoomRequest {
                    name: "Sala".into(),
                    access: Access::Open,
                    password: None,
                },
            )
            .unwrap();
        let inner = hub.inner.lock();
        let sala = &inner.rooms[&listagem.id];
        let view = inner.room_view(sala);
        assert_eq!(view.mode, RoomMode::Online);
        assert_eq!(view.code, Some(listagem.code.clone()));
        assert_eq!(view.owner_id, ana);
        assert_eq!(view.name, "Sala");
    }

    #[test]
    fn participant_view_dono() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        let sala_id = criar_sala(&hub, &ana, "Sala");
        let mut inner = hub.inner.lock();
        inner.rooms.get_mut(&sala_id).unwrap().members.push(bia.clone());

        let sala = &inner.rooms[&sala_id];
        let dona = inner.participant_view(&inner.participants[&ana], sala);
        assert_eq!(dona.role, Role::Owner);
        assert_eq!(dona.latency_ms, None);
        assert_eq!(dona.watching, None);
        let membro = inner.participant_view(&inner.participants[&bia], sala);
        assert_eq!(membro.role, Role::Member);
    }

    #[test]
    fn room_state_msg_participante_sem_trilhas() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let sala_id = criar_sala(&hub, &ana, "Sala");
        let inner = hub.inner.lock();
        let sala = &inner.rooms[&sala_id];

        match inner.room_state_msg(sala) {
            ControlMsg::RoomState {
                room,
                participants,
                tracks,
            } => {
                assert_eq!(room.id, sala_id);
                assert_eq!(participants.len(), 1);
                assert_eq!(participants[0].id, ana);
                assert!(tracks.is_empty());
            }
            outra => panic!("mensagem inesperada: {outra:?}"),
        }
    }

    #[test]
    fn broadcast_room_list_cada_ws_recebe_uma_sala() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "inst-ana", "Ana");
        let bia = entrar(&hub, "inst-bia", "Bia");
        criar_sala(&hub, &ana, "Sala");

        let mut inner = hub.inner.lock();
        let mut rx_ana = canal(&mut inner, &ana, None);
        let mut rx_bia = canal(&mut inner, &bia, None);

        inner.broadcast_room_list();

        for rx in [&mut rx_ana, &mut rx_bia] {
            match rx.try_recv().unwrap() {
                ControlMsg::RoomList { salas } => assert_eq!(salas.len(), 1),
                outra => panic!("mensagem inesperada: {outra:?}"),
            }
        }
    }
}
