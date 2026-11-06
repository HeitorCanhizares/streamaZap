//! Trilhas e assinaturas pelo WebSocket (`docs/api/mensagens.md`, seções "Trilhas" e
//! "Assinaturas") e as ações que elas disparam no `TrackRelay`.

use zap_proto::{ControlMsg, LayerSpec, TrackKind};

use crate::{
    relay::TrackRelay,
    state::{Hub, TrackData},
    util::new_id,
};

/// Limite de trilhas de vídeo por sala.
const MAX_VIDEO_TRACKS: usize = 8;

fn erro(code: &str, message: &str, reference: Option<String>) -> ControlMsg {
    ControlMsg::Error {
        code: code.to_string(),
        message: message.to_string(),
        reference,
    }
}

fn eh_tela(kind: TrackKind) -> bool {
    matches!(kind, TrackKind::Screen | TrackKind::Window)
}

/// Mesma "categoria" para a regra de uma trilha por tipo (tela e janela contam juntas).
fn mesma_categoria(a: TrackKind, b: TrackKind) -> bool {
    (eh_tela(a) && eh_tela(b)) || a == b
}

impl Hub {
    /// Trata `trilha.publicar`, `trilha.atualizar`, `trilha.remover`, `assinar`, `cancelar` e
    /// `keyframe.pedir` (C→S). Devolve `true` se a mensagem era dessa família.
    ///
    /// - `trilha.publicar`: exige estar na sala (`ws_room`); regra: no máximo uma `tela`/`janela`,
    ///   uma `camera` e uma `audio` por participante (senão `erro` `invalido`); `camadas`
    ///   ordenadas por `camada`, vídeo = 1 a 3 (áudio = vazio); número = `next_track_number`++;
    ///   cria `TrackData` com `TrackRelay::new`; manda `trilha.publicada {trilha, local_id}` ao
    ///   publicador e `trilha.publicada {trilha}` aos demais da sala; `salas.lista` atualizada.
    /// - `trilha.atualizar`: só o dono da trilha; troca título/camadas (`TrackRelay::set_layers`)
    ///   e manda `trilha.atualizada`.
    /// - `trilha.remover`: só o dono; remove, manda `trilha.removida`.
    /// - `assinar`, `cancelar` e `keyframe.pedir`: delegam a `handle_sub_message` (`subs.rs`).
    pub fn handle_track_message(&self, who: &str, msg: &ControlMsg) -> bool {
        match msg {
            ControlMsg::Subscribe { .. } | ControlMsg::Unsubscribe { .. } | ControlMsg::KeyframeRequest { .. } => {
                self.handle_sub_message(who, msg)
            }
            ControlMsg::TrackPublish {
                local_id,
                kind,
                title,
                layers,
                audio,
                camera_layout,
                ..
            } => {
                let mut inner = self.inner.lock();
                let Some(sala_id) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) else {
                    inner.send_to(who, erro("proibido", "entre na sala antes de publicar", None));
                    return true;
                };
                let ref_ = Some(local_id.clone());
                let mut camadas: Vec<LayerSpec> = layers.clone();
                camadas.sort_by_key(|l| l.layer);
                let video = *kind != TrackKind::Audio;
                let formato_ok = if video {
                    (1..=3).contains(&camadas.len())
                } else {
                    camadas.is_empty()
                };
                if !formato_ok {
                    inner.send_to(who, erro("invalido", "camadas inválidas para o tipo da trilha", ref_));
                    return true;
                }
                let Some(sala) = inner.rooms.get_mut(&sala_id) else {
                    inner.send_to(who, erro("proibido", "sala inexistente", ref_));
                    return true;
                };
                if sala
                    .tracks
                    .iter()
                    .any(|t| t.participant_id == who && mesma_categoria(t.kind, *kind))
                {
                    inner.send_to(who, erro("invalido", "você já publica uma trilha desse tipo", ref_));
                    return true;
                }
                let videos = sala.tracks.iter().filter(|t| t.kind != TrackKind::Audio).count();
                if video && videos >= MAX_VIDEO_TRACKS {
                    inner.send_to(who, erro("invalido", "limite de trilhas de vídeo da sala", ref_));
                    return true;
                }
                let number = sala.next_track_number;
                sala.next_track_number += 1;
                sala.tracks.push(TrackData {
                    id: new_id("t"),
                    number,
                    participant_id: who.to_string(),
                    kind: *kind,
                    title: title.clone(),
                    layers: camadas.clone(),
                    audio: audio.clone(),
                    camera_layout: if *kind == TrackKind::Camera {
                        *camera_layout
                    } else {
                        None
                    },
                    relay: TrackRelay::new(camadas),
                });
                let Some(view) = inner
                    .rooms
                    .get(&sala_id)
                    .and_then(|s| s.tracks.last())
                    .map(|t| inner.track_view(t))
                else {
                    return true;
                };
                inner.send_to(
                    who,
                    ControlMsg::TrackPublished {
                        track: view.clone(),
                        local_id: Some(local_id.clone()),
                    },
                );
                inner.broadcast_room(
                    &sala_id,
                    ControlMsg::TrackPublished {
                        track: view,
                        local_id: None,
                    },
                    Some(who),
                );
                inner.broadcast_room_list();
                true
            }
            ControlMsg::TrackUpdate {
                track_id,
                title,
                layers,
                camera_layout,
            } => {
                let mut inner = self.inner.lock();
                let Some(sala_id) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) else {
                    inner.send_to(who, erro("proibido", "entre na sala antes de atualizar", None));
                    return true;
                };
                let Some(sala) = inner.rooms.get_mut(&sala_id) else {
                    return true;
                };
                let Some(trilha) = sala
                    .tracks
                    .iter_mut()
                    .find(|t| &t.id == track_id && t.participant_id == who)
                else {
                    inner.send_to(who, erro("proibido", "só o dono altera a trilha", None));
                    return true;
                };
                if let Some(t) = title {
                    trilha.title = t.clone();
                }
                if camera_layout.is_some() && trilha.kind == TrackKind::Camera {
                    trilha.camera_layout = *camera_layout;
                }
                if let Some(l) = layers {
                    let mut camadas = l.clone();
                    camadas.sort_by_key(|x| x.layer);
                    trilha.layers = camadas.clone();
                    trilha.relay.set_layers(camadas);
                }
                let alvo = trilha.id.clone();
                let Some(view) = inner
                    .rooms
                    .get(&sala_id)
                    .and_then(|s| s.tracks.iter().find(|t| t.id == alvo))
                    .map(|t| inner.track_view(t))
                else {
                    return true;
                };
                inner.broadcast_room(&sala_id, ControlMsg::TrackUpdated { track: view }, None);
                true
            }
            ControlMsg::TrackRemove { track_id } => {
                let mut inner = self.inner.lock();
                let Some(sala_id) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) else {
                    inner.send_to(who, erro("proibido", "entre na sala antes de remover", None));
                    return true;
                };
                let Some(sala) = inner.rooms.get_mut(&sala_id) else {
                    return true;
                };
                let Some(pos) = sala
                    .tracks
                    .iter()
                    .position(|t| &t.id == track_id && t.participant_id == who)
                else {
                    inner.send_to(who, erro("proibido", "só o dono remove a trilha", None));
                    return true;
                };
                sala.tracks.remove(pos);
                inner.broadcast_room(
                    &sala_id,
                    ControlMsg::TrackRemoved {
                        track_id: track_id.clone(),
                    },
                    None,
                );
                inner.broadcast_room_list();
                true
            }
            _ => false,
        }
    }

    /// Remove todas as trilhas de `who` na sala `room_id` (avisando `trilha.removida`) e as
    /// assinaturas dele nas trilhas dos outros. Chamado por `leave_room` e pela queda do QUIC.
    pub fn drop_participant_tracks(&self, who: &str, room_id: &str) {
        let mut inner = self.inner.lock();
        let num = inner.participants.get(who).map(|p| p.num);
        let removidas: Vec<String> = {
            let Some(sala) = inner.rooms.get_mut(room_id) else {
                return;
            };
            let mut ids = Vec::new();
            sala.tracks.retain(|t| {
                if t.participant_id == who {
                    ids.push(t.id.clone());
                    false
                } else {
                    true
                }
            });
            if let Some(num) = num {
                for t in sala.tracks.iter_mut() {
                    t.relay.unsubscribe(num);
                }
            }
            ids
        };
        for id in removidas {
            inner.broadcast_room(room_id, ControlMsg::TrackRemoved { track_id: id }, None);
        }
        inner.broadcast_room_list();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{state::Inner, Config};
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};
    use zap_proto::Access;

    type Rx = mpsc::UnboundedReceiver<ControlMsg>;

    fn entrar(hub: &Hub, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: format!("inst-{nome}"),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
            account_token: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

    fn canal(inner: &mut Inner, id: &str, sala: Option<&str>) -> Rx {
        let (tx, rx) = mpsc::unbounded_channel();
        let p = inner.participants.get_mut(id).unwrap();
        p.ws = Some(tx);
        p.ws_room = sala.map(String::from);
        rx
    }

    fn drenar(rx: &mut Rx) -> Vec<ControlMsg> {
        let mut v = Vec::new();
        while let Ok(m) = rx.try_recv() {
            if !matches!(m, ControlMsg::RoomList { .. }) {
                v.push(m);
            }
        }
        v
    }

    /// Hub com sala, Ana e Beto dentro (com WebSocket).
    fn cenario() -> (Hub, String, String, String, Rx, Rx) {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let beto = entrar(&hub, "Beto");
        let req = CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        };
        let sala = hub.create_room(&ana, req).unwrap().id;
        let (ra, rb) = {
            let mut inner = hub.inner.lock();
            (
                canal(&mut inner, &ana, Some(&sala)),
                canal(&mut inner, &beto, Some(&sala)),
            )
        };
        (hub, sala, ana, beto, ra, rb)
    }

    fn camada(n: u8) -> LayerSpec {
        LayerSpec {
            layer: n,
            width: 640,
            height: 360,
            fps: 30,
            kbps: 500,
        }
    }

    fn publicar(local: &str, kind: TrackKind, camadas: Vec<LayerSpec>) -> ControlMsg {
        ControlMsg::TrackPublish {
            local_id: local.into(),
            kind,
            title: "t".into(),
            layers: camadas,
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    fn codigo(m: &ControlMsg) -> Option<&str> {
        match m {
            ControlMsg::Error { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    #[test]
    fn publicar_manda_trilha_publicada() {
        let (hub, _s, ana, _b, mut ra, mut rb) = cenario();
        assert!(hub.handle_track_message(&ana, &publicar("l1", TrackKind::Screen, vec![camada(1), camada(0)])));
        match &drenar(&mut ra)[0] {
            ControlMsg::TrackPublished { track, local_id } => {
                assert_eq!(local_id.as_deref(), Some("l1"));
                assert_eq!(track.number, 1);
                assert_eq!(track.layers[0].layer, 0);
            }
            outra => panic!("inesperada: {outra:?}"),
        }
        match &drenar(&mut rb)[0] {
            ControlMsg::TrackPublished { local_id, .. } => assert!(local_id.is_none()),
            outra => panic!("inesperada: {outra:?}"),
        }
        hub.handle_track_message(&ana, &publicar("l2", TrackKind::Camera, vec![camada(0)]));
        match &drenar(&mut ra)[0] {
            ControlMsg::TrackPublished { track, .. } => assert_eq!(track.number, 2),
            outra => panic!("inesperada: {outra:?}"),
        }
    }

    #[test]
    fn regra_unica_tela() {
        let (hub, _s, ana, _b, mut ra, _rb) = cenario();
        hub.handle_track_message(&ana, &publicar("l1", TrackKind::Screen, vec![camada(0)]));
        hub.handle_track_message(&ana, &publicar("l2", TrackKind::Window, vec![camada(0)]));
        hub.handle_track_message(&ana, &publicar("l3", TrackKind::Camera, vec![camada(0)]));
        hub.handle_track_message(&ana, &publicar("l4", TrackKind::Audio, vec![]));
        let msgs = drenar(&mut ra);
        assert_eq!(msgs.len(), 4);
        assert_eq!(codigo(&msgs[1]), Some("invalido"));
        assert!(matches!(msgs[0], ControlMsg::TrackPublished { .. }));
        assert!(matches!(msgs[2], ControlMsg::TrackPublished { .. }));
        assert!(matches!(msgs[3], ControlMsg::TrackPublished { .. }));
    }

    #[test]
    fn camadas_invalidas() {
        let (hub, _s, ana, _b, mut ra, _rb) = cenario();
        hub.handle_track_message(&ana, &publicar("a", TrackKind::Camera, vec![]));
        hub.handle_track_message(
            &ana,
            &publicar("b", TrackKind::Camera, vec![camada(0), camada(1), camada(2), camada(3)]),
        );
        hub.handle_track_message(&ana, &publicar("c", TrackKind::Audio, vec![camada(0)]));
        let msgs = drenar(&mut ra);
        assert_eq!(msgs.len(), 3);
        for m in &msgs {
            assert_eq!(codigo(m), Some("invalido"));
        }
        if let ControlMsg::Error { reference, .. } = &msgs[0] {
            assert_eq!(reference.as_deref(), Some("a"));
        }
    }

    #[test]
    fn fora_da_sala() {
        let (hub, _s, ana, _b, mut ra, _rb) = cenario();
        hub.inner.lock().participants.get_mut(&ana).unwrap().ws_room = None;
        hub.handle_track_message(&ana, &publicar("a", TrackKind::Camera, vec![camada(0)]));
        let msgs = drenar(&mut ra);
        assert_eq!(codigo(&msgs[0]), Some("proibido"));
    }

    #[test]
    fn outra_variante_nao_e_da_familia() {
        let (hub, _s, ana, ..) = cenario();
        assert!(!hub.handle_track_message(&ana, &ControlMsg::Pong { ts_ms: 1 }));
    }

    fn id_da_primeira(hub: &Hub, sala: &str) -> String {
        hub.inner.lock().rooms[sala].tracks[0].id.clone()
    }

    #[test]
    fn atualizar_so_dono() {
        let (hub, s, ana, beto, mut ra, mut rb) = cenario();
        hub.handle_track_message(&ana, &publicar("a", TrackKind::Camera, vec![camada(0)]));
        let id = id_da_primeira(&hub, &s);
        drenar(&mut ra);
        drenar(&mut rb);
        let upd = |t: &str| ControlMsg::TrackUpdate {
            track_id: id.clone(),
            title: Some(t.into()),
            layers: None,
            camera_layout: None,
        };
        hub.handle_track_message(&beto, &upd("x"));
        assert_eq!(codigo(&drenar(&mut rb)[0]), Some("proibido"));
        hub.handle_track_message(&ana, &upd("novo"));
        for rx in [&mut ra, &mut rb] {
            match &drenar(rx)[0] {
                ControlMsg::TrackUpdated { track } => assert_eq!(track.title, "novo"),
                outra => panic!("inesperada: {outra:?}"),
            }
        }
    }

    #[test]
    fn remover() {
        let (hub, s, ana, beto, mut ra, mut rb) = cenario();
        hub.handle_track_message(&ana, &publicar("a", TrackKind::Camera, vec![camada(0)]));
        let id = id_da_primeira(&hub, &s);
        drenar(&mut ra);
        drenar(&mut rb);
        let rem = ControlMsg::TrackRemove { track_id: id.clone() };
        hub.handle_track_message(&beto, &rem);
        assert_eq!(codigo(&drenar(&mut rb)[0]), Some("proibido"));
        hub.handle_track_message(&ana, &rem);
        assert!(matches!(drenar(&mut ra)[0], ControlMsg::TrackRemoved { .. }));
        assert!(matches!(drenar(&mut rb)[0], ControlMsg::TrackRemoved { .. }));
        assert!(hub.inner.lock().rooms[&s].tracks.is_empty());
    }

    #[test]
    fn drop_participant_tracks() {
        use crate::relay::ViewerEntry;
        use zap_proto::{DelayState, TileSize};
        let (hub, s, ana, beto, mut ra, mut rb) = cenario();
        hub.handle_track_message(&ana, &publicar("a", TrackKind::Camera, vec![camada(0)]));
        hub.handle_track_message(&beto, &publicar("b", TrackKind::Camera, vec![camada(0)]));
        drenar(&mut ra);
        drenar(&mut rb);
        // Beto assiste à trilha da Ana.
        {
            let mut inner = hub.inner.lock();
            let num = inner.participants[&beto].num;
            let sala = inner.rooms.get_mut(&s).unwrap();
            sala.tracks[0].relay.subscribe(ViewerEntry {
                participant_id: beto.clone(),
                num,
                size: TileSize::Thumbnail,
                max_layer_req: None,
                audio: true,
                preview: false,
                bandwidth_kbps: 0,
                state: DelayState::Normal,
                current: 0,
                pending: None,
                announced: None,
            });
            assert_eq!(sala.tracks[0].relay.viewers.len(), 1);
        }
        hub.drop_participant_tracks(&beto, &s);
        {
            let inner = hub.inner.lock();
            let sala = &inner.rooms[&s];
            assert_eq!(sala.tracks.len(), 1);
            assert_eq!(sala.tracks[0].participant_id, ana);
            assert!(sala.tracks[0].relay.viewers.is_empty());
        }
        assert!(matches!(drenar(&mut ra)[0], ControlMsg::TrackRemoved { .. }));
    }
}
