//! Assinaturas pelo WebSocket (`docs/api/mensagens.md`, seção "Assinaturas").

use std::{collections::HashMap, sync::OnceLock};

use parking_lot::Mutex;
use zap_proto::{control::Route, ControlMsg, DelayState, TileSize, TrackKind};

use crate::{
    relay::ViewerEntry,
    state::{Hub, Inner, Via},
    util::now_unix_ms,
};

/// Intervalo mínimo entre dois `keyframe.pedir` repassados para a mesma trilha e camada.
const INTERVALO_KEYFRAME_MS: u64 = 1_000;

/// Último keyframe repassado ao publicador, por (trilha, camada), em ms.
fn ultimos_keyframes() -> &'static Mutex<HashMap<(String, u8), u64>> {
    static ULTIMOS: OnceLock<Mutex<HashMap<(String, u8), u64>>> = OnceLock::new();
    ULTIMOS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn erro(code: &str, message: &str, reference: Option<String>) -> ControlMsg {
    ControlMsg::error(code, message, reference)
}

/// Sala em que o espectador `who` está: a do WebSocket (`sala.entrar`) ou, sem ela, a do
/// ingresso da conexão QUIC (o `ola` vale como `sala.entrar`; é por aí que a prévia assina).
fn sala_do_espectador(inner: &Inner, who: &str) -> Option<String> {
    inner
        .participants
        .get(who)
        .and_then(|p| p.ws_room.clone().or_else(|| p.quic_room.clone()))
}

/// Manda `participante.atualizou` (com o `watching` atual) a todos da sala de `who`.
fn avisar_participante(inner: &Inner, sala_id: &str, who: &str) {
    let (Some(p), Some(sala)) = (inner.participants.get(who), inner.rooms.get(sala_id)) else {
        return;
    };
    let participante = inner.participant_view(p, sala);
    inner.broadcast_room(
        sala_id,
        ControlMsg::ParticipantUpdated {
            participant: participante,
        },
        None,
    );
}

/// `assinar`: cria ou atualiza o assinante da trilha e responde com a rota (ponte), pelo canal
/// do pedido (`via`). Devolve o `erro` a mandar a `who`, ou `None` se deu certo.
fn assinar(
    inner: &mut Inner,
    who: &str,
    via: Via,
    track_id: &str,
    size: TileSize,
    max_layer: Option<u8>,
    audio: Option<bool>,
) -> Option<ControlMsg> {
    let ref_ = Some(track_id.to_string());
    let Some(sala_id) = sala_do_espectador(inner, who) else {
        return Some(erro("proibido", "entre na sala antes de assinar", None));
    };
    let Some(num) = inner.participants.get(who).map(|p| p.num) else {
        return Some(erro("proibido", "participante desconhecido", None));
    };

    // Prévia: ingresso de prévia válido e o participante ainda não é membro da sala.
    let agora = now_unix_ms();
    let tem_ingresso = inner
        .tickets
        .values()
        .any(|t| t.participant_id == who && t.preview && t.expires_at_ms > agora);
    let e_membro = inner
        .rooms
        .get(&sala_id)
        .is_some_and(|s| s.members.iter().any(|m| m == who));
    let preview = tem_ingresso && !e_membro;

    let Some(sala) = inner.rooms.get_mut(&sala_id) else {
        return Some(erro("proibido", "sala inexistente", None));
    };
    let Some(trilha) = sala.tracks.iter_mut().find(|t| t.id == track_id) else {
        return Some(erro("invalido", "trilha inexistente na sala", ref_));
    };
    // Prévia só assina vídeo (`docs/api/rest.md`, prévia): qualquer outra coisa é `proibido`.
    if preview && trilha.kind == TrackKind::Audio {
        return Some(erro("proibido", "a prévia não tem áudio", ref_));
    }
    if trilha.participant_id == who && trilha.kind != TrackKind::Audio {
        return Some(erro(
            "invalido",
            "você não pode assinar a própria trilha de vídeo",
            ref_,
        ));
    }

    // Prévia: teto 2 e sem áudio (forçado em `max_layer_for` e aqui).
    trilha.relay.subscribe(ViewerEntry {
        participant_id: who.to_string(),
        num,
        size,
        max_layer_req: max_layer,
        audio: audio.unwrap_or(true) && !preview,
        preview,
        bandwidth_kbps: 0,
        state: DelayState::Normal,
        current: 0,
        pending: None,
        announced: None,
    });

    inner.reply(
        who,
        via,
        ControlMsg::MediaRoute {
            track_id: track_id.to_string(),
            route: Route::Relay,
            address: None,
        },
    );

    // Miniatura não conta como "assistindo"; principal e cheia contam.
    let assistindo = (size != TileSize::Thumbnail).then(|| track_id.to_string());
    if let Some(p) = inner.participants.get_mut(who) {
        p.watching = assistindo;
    }
    avisar_participante(inner, &sala_id, who);
    None
}

/// `cancelar`: tira o espectador da trilha. Trilha desconhecida é ignorada.
fn cancelar(inner: &mut Inner, who: &str, track_id: &str) {
    let Some(sala_id) = sala_do_espectador(inner, who) else {
        return;
    };
    let Some(num) = inner.participants.get(who).map(|p| p.num) else {
        return;
    };
    let Some(sala) = inner.rooms.get_mut(&sala_id) else {
        return;
    };
    let Some(trilha) = sala.tracks.iter_mut().find(|t| t.id == track_id) else {
        return;
    };
    trilha.relay.unsubscribe(num);

    // Se era essa a trilha assistida, deixa de assistir e avisa a sala.
    let assistia = inner.participants.get(who).and_then(|p| p.watching.as_deref()) == Some(track_id);
    if assistia {
        if let Some(p) = inner.participants.get_mut(who) {
            p.watching = None;
        }
        avisar_participante(inner, &sala_id, who);
    }
}

/// `keyframe.pedir` do espectador: repassa `keyframe.pedir {trilha_id, camada}` ao publicador,
/// com a camada que o espectador recebe agora (ou a pendente), no máximo 1 por segundo por
/// (trilha, camada).
fn pedir_keyframe(inner: &mut Inner, who: &str, track_id: &str) {
    let Some(sala_id) = sala_do_espectador(inner, who) else {
        return;
    };
    let Some(num) = inner.participants.get(who).map(|p| p.num) else {
        return;
    };
    let Some(sala) = inner.rooms.get(&sala_id) else {
        return;
    };
    let Some(trilha) = sala.tracks.iter().find(|t| t.id == track_id) else {
        return;
    };
    // Quem não assina a trilha não tem camada para pedir.
    let Some(assinante) = trilha.relay.viewers.get(&num) else {
        return;
    };
    let camada = assinante.pending.unwrap_or(assinante.current);

    let agora = now_unix_ms();
    {
        let mut ultimos = ultimos_keyframes().lock();
        let chave = (track_id.to_string(), camada);
        if ultimos
            .get(&chave)
            .is_some_and(|&ultimo| agora.saturating_sub(ultimo) < INTERVALO_KEYFRAME_MS)
        {
            return;
        }
        ultimos.insert(chave, agora);
    }
    // Mensagem da mídia: vai ao publicador pelo QUIC dele.
    inner.send_quic(
        &trilha.participant_id,
        ControlMsg::KeyframeRequest {
            track_id: track_id.to_string(),
            layer: Some(camada),
        },
    );
}

impl Hub {
    /// Trata `assinar`, `cancelar` e `keyframe.pedir` (C→S). Devolve `true` se era dessa família.
    ///
    /// - `assinar {trilha_id, tamanho, camada_max?, audio?}`: a trilha precisa existir na sala do
    ///   espectador (senão `erro` `invalido`); cria/atualiza o `ViewerEntry`
    ///   (`TrackRelay::subscribe`; `audio` padrão `true`); responde `midia.rota {rota: ponte}`;
    ///   com ingresso de prévia (`TicketData::preview` ativo para o participante) força
    ///   `preview = true` (teto 2, sem áudio). Atualiza `ParticipantData::watching` quando o
    ///   tamanho é `principal`/`cheia` e manda `participante.atualizou`.
    /// - `cancelar {trilha_id}`: `unsubscribe`.
    /// - `keyframe.pedir {trilha_id}`: repassa `keyframe.pedir {trilha_id, camada}` ao publicador
    ///   para a camada atual do espectador, no máximo 1 por segundo por camada.
    pub fn handle_sub_message(&self, who: &str, msg: &ControlMsg) -> bool {
        self.handle_sub_message_via(who, msg, Via::Ws)
    }

    /// Igual a `handle_sub_message`, para um pedido que chegou por `via` (as respostas diretas
    /// voltam pelo mesmo canal). A ponte QUIC usa `Via::Quic`.
    pub fn handle_sub_message_via(&self, who: &str, msg: &ControlMsg, via: Via) -> bool {
        match msg {
            ControlMsg::Subscribe {
                track_id,
                size,
                max_layer,
                audio,
            } => {
                let mut inner = self.inner.lock();
                if let Some(e) = assinar(&mut inner, who, via, track_id, *size, *max_layer, *audio) {
                    inner.reply(who, via, e);
                }
                true
            }
            ControlMsg::Unsubscribe { track_id } => {
                cancelar(&mut self.inner.lock(), who, track_id);
                true
            }
            ControlMsg::KeyframeRequest { track_id, .. } => {
                pedir_keyframe(&mut self.inner.lock(), who, track_id);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        relay::TrackRelay,
        state::{TicketData, TrackData},
        util::new_id,
        Config,
    };
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};
    use zap_proto::{Access, LayerSpec};

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

    fn canal(inner: &mut Inner, id: &str, sala: &str) -> Rx {
        let (tx, rx) = mpsc::unbounded_channel();
        let p = inner.participants.get_mut(id).unwrap();
        p.ws = Some(tx);
        p.ws_room = Some(sala.to_string());
        rx
    }

    fn drenar(rx: &mut Rx) -> Vec<ControlMsg> {
        let mut v = Vec::new();
        while let Ok(m) = rx.try_recv() {
            v.push(m);
        }
        v
    }

    /// Códigos de todos os `erro` presentes em `msgs`.
    fn codigos(msgs: &[ControlMsg]) -> Vec<String> {
        msgs.iter()
            .filter_map(|m| match m {
                ControlMsg::Error { code, .. } => Some(code.clone()),
                _ => None,
            })
            .collect()
    }

    /// Hub com sala, Ana (dona, com WebSocket) e Bia (com WebSocket, fora dos membros).
    fn cenario() -> (Hub, String, String, String, Rx, Rx) {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let bia = entrar(&hub, "Bia");
        let req = CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        };
        let sala = hub.create_room(&ana, req).unwrap().id;
        let (ra, rb) = {
            let mut inner = hub.inner.lock();
            (canal(&mut inner, &ana, &sala), canal(&mut inner, &bia, &sala))
        };
        (hub, sala, ana, bia, ra, rb)
    }

    fn camada(n: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer: n,
            width: 640,
            height: 360,
            fps: 30,
            kbps,
        }
    }

    /// Trilha publicada direto na estrutura de dados (sem passar pelo WebSocket).
    fn publicar(hub: &Hub, sala: &str, dono: &str, kind: TrackKind) -> String {
        let camadas = vec![camada(0, 2000), camada(1, 800), camada(2, 300)];
        let mut inner = hub.inner.lock();
        let s = inner.rooms.get_mut(sala).unwrap();
        let id = new_id("t");
        let number = s.next_track_number;
        s.next_track_number += 1;
        s.tracks.push(TrackData {
            id: id.clone(),
            number,
            participant_id: dono.into(),
            kind,
            title: "câmera".into(),
            layers: camadas.clone(),
            audio: None,
            relay: TrackRelay::new(camadas),
            camera_layout: None,
        });
        id
    }

    fn assinar_msg(trilha: &str, size: TileSize, max_layer: Option<u8>, audio: Option<bool>) -> ControlMsg {
        ControlMsg::Subscribe {
            track_id: trilha.into(),
            size,
            max_layer,
            audio,
        }
    }

    #[test]
    fn assinar_cria_viewer_e_responde_rota() {
        let (hub, sala, ana, bia, _ra, mut rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);

        assert!(hub.handle_sub_message(&bia, &assinar_msg(&trilha, TileSize::Main, None, None)));

        let msgs = drenar(&mut rb);
        assert!(msgs.contains(&ControlMsg::MediaRoute {
            track_id: trilha.clone(),
            route: Route::Relay,
            address: None,
        }));
        let inner = hub.inner.lock();
        let t = inner.rooms[&sala].tracks.iter().find(|t| t.id == trilha).unwrap();
        assert_eq!(t.relay.viewers.len(), 1);
        assert!(t.relay.viewers.values().all(|v| v.audio && !v.preview));
        // Principal conta como assistindo.
        assert_eq!(inner.participants[&bia].watching.as_deref(), Some(trilha.as_str()));
    }

    #[test]
    fn miniatura_nao_marca_assistindo() {
        let (hub, sala, ana, bia, _ra, mut rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);

        hub.handle_sub_message(&bia, &assinar_msg(&trilha, TileSize::Thumbnail, None, None));

        assert!(drenar(&mut rb)
            .iter()
            .any(|m| matches!(m, ControlMsg::ParticipantUpdated { .. })));
        assert_eq!(hub.inner.lock().participants[&bia].watching, None);
    }

    #[test]
    fn previa_forca_teto() {
        let (hub, sala, ana, bia, _ra, _rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);
        hub.inner.lock().tickets.insert(
            "tk-previa".into(),
            TicketData {
                participant_id: bia.clone(),
                room_id: sala.clone(),
                preview: true,
                expires_at_ms: now_unix_ms() + 60_000,
            },
        );

        hub.handle_sub_message(&bia, &assinar_msg(&trilha, TileSize::Full, Some(0), Some(true)));

        let inner = hub.inner.lock();
        let t = inner.rooms[&sala].tracks.iter().find(|t| t.id == trilha).unwrap();
        let entry = &t.relay.viewers[&inner.participants[&bia].num];
        assert!(entry.preview);
        assert!(!entry.audio);
    }

    #[test]
    fn previa_assina_pelo_quic_e_resposta_volta_pelo_quic() {
        let (hub, sala, ana, bia, _ra, mut rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);
        let audio = publicar(&hub, &sala, &ana, TrackKind::Audio);
        let (qtx, mut rq) = mpsc::unbounded_channel();
        {
            let mut inner = hub.inner.lock();
            inner.tickets.insert(
                "tk-previa".into(),
                TicketData {
                    participant_id: bia.clone(),
                    room_id: sala.clone(),
                    preview: true,
                    expires_at_ms: now_unix_ms() + 60_000,
                },
            );
            // Só QUIC: a sala vem do ingresso, não de `sala.entrar`.
            let p = inner.participants.get_mut(&bia).unwrap();
            p.ws_room = None;
            p.quic_room = Some(sala.clone());
            p.quic_ctl = Some(qtx);
        }
        drenar(&mut rb);

        let video = assinar_msg(&trilha, TileSize::Main, None, None);
        assert!(hub.handle_sub_message_via(&bia, &video, Via::Quic));
        let pelo_quic = drenar(&mut rq);
        assert!(pelo_quic
            .iter()
            .any(|m| matches!(m, ControlMsg::MediaRoute { track_id, .. } if *track_id == trilha)));

        let som = assinar_msg(&audio, TileSize::Main, None, None);
        assert!(hub.handle_sub_message_via(&bia, &som, Via::Quic));
        assert_eq!(codigos(&drenar(&mut rq)), vec!["proibido".to_string()]);
        assert!(
            !drenar(&mut rb)
                .iter()
                .any(|m| matches!(m, ControlMsg::MediaRoute { .. } | ControlMsg::Error { .. })),
            "respostas de um pedido QUIC não vão pelo WebSocket"
        );
    }

    #[test]
    fn assinar_trilha_inexistente() {
        let (hub, _sala, _ana, bia, _ra, mut rb) = cenario();

        assert!(hub.handle_sub_message(&bia, &assinar_msg("t_nao_existe", TileSize::Main, None, None)));

        let msgs = drenar(&mut rb);
        assert_eq!(codigos(&msgs), vec!["invalido".to_string()]);
        assert!(msgs.iter().any(|m| matches!(
            m,
            ControlMsg::Error { reference: Some(r), .. } if r == "t_nao_existe"
        )));
    }

    #[test]
    fn assinar_propria_trilha() {
        let (hub, sala, ana, _bia, mut ra, _rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);

        hub.handle_sub_message(&ana, &assinar_msg(&trilha, TileSize::Main, None, None));

        assert_eq!(codigos(&drenar(&mut ra)), vec!["invalido".to_string()]);
        assert!(hub.inner.lock().rooms[&sala].tracks[0].relay.viewers.is_empty());
    }

    #[test]
    fn cancelar_remove() {
        let (hub, sala, ana, bia, _ra, mut rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);
        hub.handle_sub_message(&bia, &assinar_msg(&trilha, TileSize::Main, None, None));
        drenar(&mut rb);

        assert!(hub.handle_sub_message(
            &bia,
            &ControlMsg::Unsubscribe {
                track_id: trilha.clone(),
            }
        ));

        let inner = hub.inner.lock();
        assert!(inner.rooms[&sala].tracks[0].relay.viewers.is_empty());
        assert_eq!(inner.participants[&bia].watching, None);
    }

    #[test]
    fn keyframe_pedir_repassa_ao_publicador_e_limita_1_por_s() {
        let (hub, sala, ana, bia, mut ra, mut rb) = cenario();
        let trilha = publicar(&hub, &sala, &ana, TrackKind::Camera);
        hub.handle_sub_message(&bia, &assinar_msg(&trilha, TileSize::Main, None, None));
        drenar(&mut rb);
        drenar(&mut ra);
        // `keyframe.pedir` é mensagem da mídia: chega ao publicador pelo QUIC, não pelo WebSocket.
        let (qtx, mut rq) = mpsc::unbounded_channel();
        hub.inner.lock().participants.get_mut(&ana).unwrap().quic_ctl = Some(qtx);

        let pedido = ControlMsg::KeyframeRequest {
            track_id: trilha.clone(),
            layer: None,
        };
        assert!(hub.handle_sub_message(&bia, &pedido));
        assert!(hub.handle_sub_message(&bia, &pedido));

        let pedidos: Vec<ControlMsg> = drenar(&mut rq)
            .into_iter()
            .filter(|m| matches!(m, ControlMsg::KeyframeRequest { layer: Some(_), .. }))
            .collect();
        assert_eq!(pedidos.len(), 1);
        assert!(drenar(&mut ra).is_empty(), "nada de mídia pelo WebSocket");
    }

    #[test]
    fn outra_variante_nao_e_da_familia() {
        let (hub, _sala, _ana, bia, _ra, _rb) = cenario();
        assert!(!hub.handle_sub_message(&bia, &ControlMsg::Pong { ts_ms: 1 }));
    }
}
