//! Mensagens do WebSocket sobre sala, participantes e chat (`docs/api/mensagens.md`,
//! seções "Salas e participantes" e "Chat").

use tokio::sync::mpsc;
use zap_proto::ControlMsg;

use crate::{
    rooms::listing_of,
    state::{ChatEntry, Hub, Inner, RoomData},
    util::{new_id, now_iso},
};

/// O servidor guarda as últimas 200 mensagens de chat por sala.
const LIMITE_CHAT: usize = 200;
/// Tamanho do texto do chat, em caracteres.
const MAX_TEXTO_CHAT: usize = 1000;
/// Tamanho do nome do participante, em caracteres.
const MAX_NOME: usize = 32;

fn erro(code: &str, message: &str, reference: Option<String>) -> ControlMsg {
    ControlMsg::Error {
        code: code.to_string(),
        message: message.to_string(),
        reference,
    }
}

/// `chat.mensagem` de uma entrada guardada (`cliente_id` só para quem enviou).
fn chat_msg(e: &ChatEntry, client_id: Option<String>) -> ControlMsg {
    ControlMsg::ChatMessage {
        id: e.id.clone(),
        participant_id: e.participant_id.clone(),
        name: e.name.clone(),
        color: e.color.clone(),
        text: e.text.clone(),
        at: e.at.clone(),
        client_id,
    }
}

/// `salas.lista` como `who` a vê (mesma ordem de `list_rooms`: criação e depois id).
fn lista_para(inner: &Inner, who: &str) -> ControlMsg {
    let mut salas: Vec<&RoomData> = inner.rooms.values().collect();
    salas.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
    let salas = salas
        .iter()
        .filter_map(|s| serde_json::to_value(listing_of(inner, s, who)).ok())
        .collect();
    ControlMsg::RoomList { salas }
}

impl Hub {
    /// WebSocket novo de `who`: guarda `tx` em `ParticipantData::ws` (substitui o anterior) e
    /// manda `salas.lista` (formato `GET /v1/salas`).
    pub fn ws_connected(&self, who: &str, tx: mpsc::UnboundedSender<ControlMsg>) {
        let mut inner = self.inner.lock();
        let Some(p) = inner.participants.get_mut(who) else {
            return;
        };
        p.ws = Some(tx.clone());
        let lista = lista_para(&inner, who);
        // Erro = o WebSocket já fechou; o laço dele vai chamar `ws_disconnected`.
        let _ = tx.send(lista);
    }

    /// WebSocket de `who` fechou: se `tx` ainda é o registrado (`same_channel`), limpa `ws` e
    /// `ws_room`. Não remove o membro da sala (ele pode reconectar; ver `Hub::drop_participant`).
    pub fn ws_disconnected(&self, who: &str, tx: &mpsc::UnboundedSender<ControlMsg>) {
        let mut inner = self.inner.lock();
        let Some(p) = inner.participants.get_mut(who) else {
            return;
        };
        // Um WebSocket mais novo já substituiu este: não mexe no estado dele.
        if p.ws.as_ref().is_some_and(|atual| atual.same_channel(tx)) {
            p.ws = None;
            p.ws_room = None;
        }
    }

    /// Despacha uma mensagem C→S do WebSocket:
    /// - `sala.entrar {sala_id}`: exige ser membro (senão `erro` `proibido`); define `ws_room`,
    ///   manda `sala.estado` e as últimas 200 `chat.mensagem`; se for a primeira vez na sala via
    ///   WS não repete `participante.entrou` (já foi mandado pelo REST `entrar`);
    /// - `sala.sair`: limpa `ws_room` (o servidor de teste não manda `sala.saiu`: o `zap-proto` não
    ///   tem essa variante e o cliente ignora tipos desconhecidos);
    /// - `perfil.atualizar {nome?, cor?}`: atualiza e manda `participante.atualizou`;
    /// - `chat.enviar {texto, cliente_id}`: 1–1000 caracteres (senão `erro` `invalido` com
    ///   `ref = cliente_id`); guarda (limite 200) e manda `chat.mensagem` a todos da sala via WS,
    ///   com `cliente_id` só no remetente; atualiza `ultima_mensagem` e dispara `salas.lista`;
    /// - `chat.digitando`: repassa `chat.digitando {participante_id}` aos outros;
    /// - `musica.*`: fila de música da sala (`crate::music::tratar`): mudou → `musica.estado` a todos;
    /// - `estatisticas`: ignorada (a latência do participante vem do `pong` do QUIC no servidor de
    ///   produção; o de teste não a acompanha);
    /// - `p2p.*`, `conexao.reversa`, `midia.direta_*`: ignoradas (rota direta fora do escopo);
    /// - trilhas e assinaturas: delegam a `handle_track_message` (`tracks.rs`);
    /// - qualquer outra: `erro` `invalido`.
    pub fn handle_ws_message(&self, who: &str, msg: ControlMsg) {
        // Trilhas e assinaturas travam o `Mutex` sozinhas: chamadas antes de travar aqui.
        if self.handle_track_message(who, &msg) {
            return;
        }
        let mut inner = self.inner.lock();
        match msg {
            ControlMsg::RoomJoin { room_id } => entrar_pelo_ws(&mut inner, who, &room_id),
            ControlMsg::RoomLeave {} => {
                if let Some(p) = inner.participants.get_mut(who) {
                    p.ws_room = None;
                }
            }
            ControlMsg::ProfileUpdate { name, color } => atualizar_perfil(&mut inner, who, name, color),
            ControlMsg::ChatSend { text, client_id } => enviar_chat(&mut inner, who, &text, client_id),
            msg @ (ControlMsg::MusicAdd { .. }
            | ControlMsg::MusicRemove { .. }
            | ControlMsg::MusicSkip { .. }
            | ControlMsg::MusicInfo { .. }) => crate::music::tratar(&mut inner, who, msg),
            ControlMsg::ChatTyping { .. } => {
                if let Some(sala) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) {
                    inner.broadcast_room(
                        &sala,
                        ControlMsg::ChatTyping {
                            participant_id: Some(who.to_string()),
                        },
                        Some(who),
                    );
                }
            }
            ControlMsg::Ping { ts_ms } => inner.send_to(who, ControlMsg::Pong { ts_ms }),
            // Latência e rota direta ficam fora do servidor de teste (ver acima).
            ControlMsg::Pong { .. }
            | ControlMsg::Stats { .. }
            | ControlMsg::P2pMyAddresses { .. }
            | ControlMsg::DirectOk { .. }
            | ControlMsg::DirectFailed { .. }
            | ControlMsg::ReverseConnection { .. } => {}
            _ => inner.send_to(who, erro("invalido", "mensagem não suportada", None)),
        }
    }
}

/// `sala.entrar`: só membros (quem criou a sala já é membro). Manda `sala.estado` e o histórico.
fn entrar_pelo_ws(inner: &mut Inner, who: &str, room_id: &str) {
    let membro = inner
        .rooms
        .get(room_id)
        .is_some_and(|s| s.members.iter().any(|m| m == who));
    if !membro {
        inner.send_to(
            who,
            erro("proibido", "entre na sala antes (POST /v1/salas/{codigo}/entrar)", None),
        );
        return;
    }
    if let Some(p) = inner.participants.get_mut(who) {
        p.ws_room = Some(room_id.to_string());
    }
    let Some(sala) = inner.rooms.get(room_id) else {
        return;
    };
    let estado = inner.room_state_msg(sala);
    let historico: Vec<ControlMsg> = sala.chat.iter().map(|e| chat_msg(e, None)).collect();
    let musica = crate::music::estado(sala, std::time::Instant::now());
    inner.send_to(who, estado);
    for m in historico {
        inner.send_to(who, m);
    }
    // A música vai a todos: quem entra fica sabendo que a sala tem música, os outros veem o
    // limite de votos mudar.
    inner.broadcast_room(room_id, musica, None);
}

/// `perfil.atualizar`: nome com 1 a 32 caracteres; avisa a sala com `participante.atualizou`.
fn atualizar_perfil(inner: &mut Inner, who: &str, name: Option<String>, color: Option<String>) {
    if let Some(n) = &name {
        let tamanho = n.trim().chars().count();
        if tamanho == 0 || tamanho > MAX_NOME {
            inner.send_to(who, erro("invalido", "nome inválido", None));
            return;
        }
    }
    let Some(p) = inner.participants.get_mut(who) else {
        return;
    };
    if let Some(n) = name {
        p.name = n.trim().to_string();
    }
    if let Some(c) = color {
        p.color = c;
    }
    let Some(sala_id) = p.room_id.clone() else {
        return;
    };
    let (Some(p), Some(sala)) = (inner.participants.get(who), inner.rooms.get(&sala_id)) else {
        return;
    };
    let participant = inner.participant_view(p, sala);
    inner.broadcast_room(&sala_id, ControlMsg::ParticipantUpdated { participant }, None);
    inner.broadcast_room_list();
}

/// `chat.enviar`: valida, guarda (últimas 200) e entrega a todos da sala (`cliente_id` só ao
/// remetente). Depois a `salas.lista` muda (`ultima_mensagem`).
fn enviar_chat(inner: &mut Inner, who: &str, text: &str, client_id: String) {
    let Some(sala_id) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) else {
        inner.send_to(who, erro("proibido", "entre na sala antes de mandar mensagem", None));
        return;
    };
    let texto = text.trim();
    let tamanho = texto.chars().count();
    if tamanho == 0 || tamanho > MAX_TEXTO_CHAT {
        inner.send_to(
            who,
            erro(
                "invalido",
                "a mensagem precisa ter de 1 a 1000 caracteres",
                Some(client_id),
            ),
        );
        return;
    }
    let Some((name, color)) = inner.participants.get(who).map(|p| (p.name.clone(), p.color.clone())) else {
        return;
    };
    let entrada = ChatEntry {
        id: new_id("m"),
        participant_id: who.to_string(),
        name,
        color,
        text: texto.to_string(),
        at: now_iso(),
    };
    let Some(sala) = inner.rooms.get_mut(&sala_id) else {
        return;
    };
    sala.chat.push_back(entrada.clone());
    while sala.chat.len() > LIMITE_CHAT {
        sala.chat.pop_front();
    }
    inner.send_to(who, chat_msg(&entrada, Some(client_id)));
    inner.broadcast_room(&sala_id, chat_msg(&entrada, None), Some(who));
    inner.broadcast_room_list();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Config;
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

    fn conectar(hub: &Hub, who: &str) -> (mpsc::UnboundedSender<ControlMsg>, Rx) {
        let (tx, rx) = mpsc::unbounded_channel();
        hub.ws_connected(who, tx.clone());
        (tx, rx)
    }

    fn drenar(rx: &mut Rx) -> Vec<ControlMsg> {
        let mut v = Vec::new();
        while let Ok(m) = rx.try_recv() {
            v.push(m);
        }
        v
    }

    fn codigos(msgs: &[ControlMsg]) -> Vec<String> {
        msgs.iter()
            .filter_map(|m| match m {
                ControlMsg::Error { code, .. } => Some(code.clone()),
                _ => None,
            })
            .collect()
    }

    /// Ana (dona) e Bia (membro) com WebSocket aberto e `sala.entrar` feito.
    fn sala_com_dois() -> (Hub, String, String, String, Rx, Rx) {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let bia = entrar(&hub, "Bia");
        let sala = hub
            .create_room(
                &ana,
                CreateRoomRequest {
                    name: "Sala".into(),
                    access: Access::Open,
                    password: None,
                },
            )
            .unwrap();
        hub.join_room(&bia, &sala.code, None).unwrap();
        let (_ta, mut ra) = conectar(&hub, &ana);
        let (_tb, mut rb) = conectar(&hub, &bia);
        hub.handle_ws_message(
            &ana,
            ControlMsg::RoomJoin {
                room_id: sala.id.clone(),
            },
        );
        hub.handle_ws_message(
            &bia,
            ControlMsg::RoomJoin {
                room_id: sala.id.clone(),
            },
        );
        drenar(&mut ra);
        drenar(&mut rb);
        // Os `tx` de teste saem de escopo, mas o `Hub` guarda uma cópia: os canais seguem vivos.
        (hub, sala.id, ana, bia, ra, rb)
    }

    #[test]
    fn conectar_recebe_lista() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        hub.create_room(
            &ana,
            CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            },
        )
        .unwrap();
        let (_tx, mut rx) = conectar(&hub, &ana);
        match drenar(&mut rx).as_slice() {
            [ControlMsg::RoomList { salas }] => assert_eq!(salas.len(), 1),
            outro => panic!("esperava salas.lista: {outro:?}"),
        }
    }

    #[test]
    fn sala_entrar_manda_estado() {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let bia = entrar(&hub, "Bia");
        let sala = hub
            .create_room(
                &ana,
                CreateRoomRequest {
                    name: "Sala".into(),
                    access: Access::Open,
                    password: None,
                },
            )
            .unwrap();
        let (_ta, mut ra) = conectar(&hub, &ana);
        hub.handle_ws_message(
            &ana,
            ControlMsg::RoomJoin {
                room_id: sala.id.clone(),
            },
        );
        hub.handle_ws_message(
            &ana,
            ControlMsg::ChatSend {
                text: "primeira".into(),
                client_id: "c0".into(),
            },
        );
        drenar(&mut ra);

        // Não-membro: proibido.
        let (_tb, mut rb) = conectar(&hub, &bia);
        drenar(&mut rb);
        hub.handle_ws_message(
            &bia,
            ControlMsg::RoomJoin {
                room_id: sala.id.clone(),
            },
        );
        assert_eq!(codigos(&drenar(&mut rb)), vec!["proibido".to_string()]);

        // Membro: estado com ele e o histórico do chat.
        hub.join_room(&bia, &sala.code, None).unwrap();
        drenar(&mut rb);
        hub.handle_ws_message(
            &bia,
            ControlMsg::RoomJoin {
                room_id: sala.id.clone(),
            },
        );
        let msgs = drenar(&mut rb);
        match msgs.first() {
            Some(ControlMsg::RoomState { participants, .. }) => {
                assert!(participants.iter().any(|p| p.id == bia));
            }
            outro => panic!("esperava sala.estado: {outro:?}"),
        }
        assert!(msgs
            .iter()
            .any(|m| matches!(m, ControlMsg::ChatMessage { text, client_id: None, .. } if text == "primeira")));
    }

    #[test]
    fn chat_ida_e_volta() {
        let (hub, _sala, ana, _bia, mut ra, mut rb) = sala_com_dois();
        hub.handle_ws_message(
            &ana,
            ControlMsg::ChatSend {
                text: "oi".into(),
                client_id: "c1".into(),
            },
        );
        assert!(drenar(&mut ra).iter().any(|m| matches!(
            m,
            ControlMsg::ChatMessage { text, client_id: Some(c), .. } if text == "oi" && c == "c1"
        )));
        assert!(drenar(&mut rb).iter().any(|m| matches!(
            m,
            ControlMsg::ChatMessage { text, client_id: None, .. } if text == "oi"
        )));

        for texto in [String::new(), "x".repeat(1001)] {
            hub.handle_ws_message(
                &ana,
                ControlMsg::ChatSend {
                    text: texto,
                    client_id: "c2".into(),
                },
            );
            let msgs = drenar(&mut ra);
            assert!(msgs.iter().any(|m| matches!(
                m,
                ControlMsg::Error { code, reference: Some(r), .. } if code == "invalido" && r == "c2"
            )));
        }
        assert!(
            !drenar(&mut rb)
                .iter()
                .any(|m| matches!(m, ControlMsg::ChatMessage { .. })),
            "mensagem recusada não chega a ninguém"
        );
    }

    #[test]
    fn chat_limite_200() {
        let (hub, sala, ana, _bia, _ra, _rb) = sala_com_dois();
        for i in 0..201 {
            hub.handle_ws_message(
                &ana,
                ControlMsg::ChatSend {
                    text: format!("m{i}"),
                    client_id: format!("c{i}"),
                },
            );
        }
        let inner = hub.inner.lock();
        let chat = &inner.rooms[&sala].chat;
        assert_eq!(chat.len(), 200);
        assert_eq!(chat.front().unwrap().text, "m1", "a mais antiga saiu");
    }

    #[test]
    fn perfil_atualiza() {
        let (hub, _sala, ana, _bia, _ra, mut rb) = sala_com_dois();
        hub.handle_ws_message(
            &ana,
            ControlMsg::ProfileUpdate {
                name: Some("Ana Maria".into()),
                color: None,
            },
        );
        assert!(drenar(&mut rb).iter().any(|m| matches!(
            m,
            ControlMsg::ParticipantUpdated { participant } if participant.name == "Ana Maria"
        )));
    }

    #[test]
    fn digitando() {
        let (hub, _sala, ana, bia, mut ra, mut rb) = sala_com_dois();
        hub.handle_ws_message(&ana, ControlMsg::ChatTyping { participant_id: None });
        assert!(drenar(&mut ra).is_empty(), "quem digita não recebe o próprio aviso");
        assert!(drenar(&mut rb).iter().any(|m| matches!(
            m,
            ControlMsg::ChatTyping { participant_id: Some(p) } if *p == ana
        )));
        let _ = bia;
    }

    #[test]
    fn ping_pong() {
        let (hub, _sala, ana, _bia, mut ra, _rb) = sala_com_dois();
        hub.handle_ws_message(&ana, ControlMsg::Ping { ts_ms: 42 });
        assert_eq!(drenar(&mut ra), vec![ControlMsg::Pong { ts_ms: 42 }]);
    }

    #[test]
    fn mensagem_nao_suportada() {
        let (hub, _sala, ana, _bia, mut ra, _rb) = sala_com_dois();
        hub.handle_ws_message(
            &ana,
            ControlMsg::Refused {
                code: "x".into(),
                message: "y".into(),
            },
        );
        assert_eq!(codigos(&drenar(&mut ra)), vec!["invalido".to_string()]);
    }

    #[test]
    fn ws_disconnected_canal_antigo_nao_limpa() {
        let (hub, sala, ana, _bia, _ra, _rb) = sala_com_dois();
        let antigo = hub.inner.lock().participants[&ana].ws.clone().unwrap();
        let (_novo, _rn) = conectar(&hub, &ana);
        hub.handle_ws_message(&ana, ControlMsg::RoomJoin { room_id: sala.clone() });

        hub.ws_disconnected(&ana, &antigo);
        {
            let inner = hub.inner.lock();
            assert!(inner.participants[&ana].ws.is_some(), "o canal novo continua");
            assert_eq!(inner.participants[&ana].ws_room.as_deref(), Some(sala.as_str()));
        }

        let atual = hub.inner.lock().participants[&ana].ws.clone().unwrap();
        hub.ws_disconnected(&ana, &atual);
        let inner = hub.inner.lock();
        assert!(inner.participants[&ana].ws.is_none());
        assert!(inner.participants[&ana].ws_room.is_none());
    }

    fn estado_musica(msgs: &[ControlMsg]) -> Option<(Vec<zap_proto::MusicItem>, Option<zap_proto::MusicNow>)> {
        msgs.iter().rev().find_map(|m| match m {
            ControlMsg::MusicState { queue, current } => Some((queue.clone(), current.clone())),
            _ => None,
        })
    }

    #[test]
    fn musica_entrar_ja_traz_o_estado() {
        let (hub, sala, _ana, bia, _ra, mut rb) = sala_com_dois();
        hub.handle_ws_message(&bia, ControlMsg::RoomJoin { room_id: sala });
        let (fila, atual) = estado_musica(&drenar(&mut rb)).expect("musica.estado ao entrar");
        assert!(fila.is_empty() && atual.is_none());
    }

    #[test]
    fn musica_adicionar_votar_e_permissao() {
        let (hub, _sala, ana, bia, mut ra, mut rb) = sala_com_dois();
        hub.handle_ws_message(
            &bia,
            ControlMsg::MusicAdd {
                video_id: "aaaaaaaaaaa".into(),
                title: Some("Primeira".into()),
                channel: None,
            },
        );
        // Os dois recebem o mesmo estado: Bia pôs, 2 pessoas, 1 voto pula.
        let (_, atual_a) = estado_musica(&drenar(&mut ra)).expect("Ana recebeu");
        let (_, atual_b) = estado_musica(&drenar(&mut rb)).expect("Bia recebeu");
        let atual = atual_a.expect("tocando");
        assert_eq!(Some(&atual), atual_b.as_ref());
        assert_eq!(atual.item.added_by, bia);
        assert_eq!(atual.item.added_by_name, "Bia");
        assert_eq!(atual.needed, 1);

        // Link ruim: erro só para quem pediu.
        hub.handle_ws_message(
            &ana,
            ControlMsg::MusicAdd {
                video_id: "curto".into(),
                title: None,
                channel: None,
            },
        );
        assert_eq!(codigos(&drenar(&mut ra)), vec!["musica_invalida".to_string()]);
        assert!(drenar(&mut rb).is_empty());

        // O voto de Ana (2 pessoas) pula: a fila acaba.
        hub.handle_ws_message(
            &ana,
            ControlMsg::MusicSkip {
                item_id: atual.item.id.clone(),
                vote: true,
            },
        );
        let (fila, atual) = estado_musica(&drenar(&mut rb)).expect("estado depois do voto");
        assert!(fila.is_empty() && atual.is_none());
    }

    #[test]
    fn musica_fora_da_sala_e_proibido() {
        let hub = Hub::new(Config::default());
        let caio = entrar(&hub, "Caio");
        let (_tc, mut rc) = conectar(&hub, &caio);
        drenar(&mut rc);
        hub.handle_ws_message(&caio, ControlMsg::MusicRemove { item_id: "mu_1".into() });
        assert_eq!(codigos(&drenar(&mut rc)), vec!["proibido".to_string()]);
    }
}
