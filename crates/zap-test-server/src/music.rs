//! Música da sala no servidor (`docs/api/mensagens.md`, "Música da sala"): no Online o servidor é
//! a fonte da fila, com a mesma regra do app (`zap_proto::music::MusicQueue`). Referência para o
//! servidor de produção.

use std::sync::atomic::Ordering;
use std::time::Instant;

use zap_proto::music::MusicQueue;
use zap_proto::{ControlMsg, MusicItem};

use crate::state::{Hub, Inner, RoomData};
use crate::util::new_id;

/// Com música tocando, o estado vai de novo a todos a cada tantos tiques de 1 s.
const REENVIO_TIQUES: u64 = 10;

/// `musica.estado` da sala agora (o limite de votos conta os membros).
pub(crate) fn estado(sala: &RoomData, agora: Instant) -> ControlMsg {
    sala.music.msg(sala.members.len(), agora)
}

fn erro(mensagem: &str) -> ControlMsg {
    ControlMsg::error("musica_invalida", mensagem, None)
}

/// Pedido de música de `who` pelo WebSocket (precisa estar na sala, como no chat). Mudou → estado
/// para todos da sala; recusado → `erro` `musica_invalida` só para quem pediu.
pub(crate) fn tratar(inner: &mut Inner, who: &str, msg: ControlMsg) {
    let Some(sala_id) = inner.participants.get(who).and_then(|p| p.ws_room.clone()) else {
        inner.send_to(
            who,
            ControlMsg::error("proibido", "entre na sala antes de mexer na música", None),
        );
        return;
    };
    let nome = inner.participants.get(who).map(|p| p.name.clone()).unwrap_or_default();
    let Some(sala) = inner.rooms.get_mut(&sala_id) else {
        return;
    };
    let agora = Instant::now();
    let presentes = sala.members.len();
    let resultado: Result<bool, String> = match msg {
        ControlMsg::MusicAdd {
            video_id,
            title,
            channel,
        } => {
            let novo = MusicItem {
                id: new_id("mu"),
                video_id,
                title: title.unwrap_or_default(),
                channel,
                duration_ms: None,
                live: false,
                added_by: who.to_string(),
                added_by_name: nome,
            };
            sala.music.add(novo, agora).map(|()| true).map_err(|e| e.message())
        }
        ControlMsg::MusicRemove { item_id } => {
            let eh_dono = sala.owner_id == who;
            sala.music
                .remove(who, eh_dono, &item_id, agora)
                .map(|()| true)
                .map_err(|e| e.message())
        }
        ControlMsg::MusicSkip { item_id, vote } => Ok(sala.music.vote(who, &item_id, vote, presentes, agora)),
        ControlMsg::MusicInfo {
            item_id,
            duration_ms,
            live,
            title,
            ended,
            failed,
        } => Ok(sala
            .music
            .info(&item_id, duration_ms, live, title, ended, failed, agora)),
        _ => Ok(false),
    };
    match resultado {
        Ok(true) => {
            let m = estado(sala, agora);
            inner.broadcast_room(&sala_id, m, None);
        }
        Ok(false) => {}
        Err(mensagem) => inner.send_to(who, erro(&mensagem)),
    }
}

/// `who` saiu de `sala`: perde o voto (o limite cai e pode pular). Devolve o estado a mandar.
pub(crate) fn saiu(sala: &mut RoomData, who: &str) -> Option<ControlMsg> {
    let agora = Instant::now();
    let presentes = sala.members.len();
    let mudou = sala.music.left(who, presentes, agora);
    (mudou || !sala.music.is_empty()).then(|| estado(sala, agora))
}

/// Sala nova: fila vazia.
pub(crate) fn nova() -> MusicQueue {
    MusicQueue::default()
}

impl Hub {
    /// Uma vez por segundo (pelo `tick_all`): avança a música das salas cujo vídeo acabou e, com
    /// música tocando, reenvia o estado a cada 10 s.
    pub fn tick_music(&self) {
        // Contador do `Hub`, não do processo: cada servidor conta os próprios tiques.
        let n = self.music_ticks.fetch_add(1, Ordering::Relaxed) + 1;
        let mut guard = self.inner.lock();
        let inner = &mut *guard;
        let agora = Instant::now();
        let mut envios = Vec::new();
        for sala in inner.rooms.values_mut() {
            let avancou = sala.music.tick(agora);
            if avancou || (n % REENVIO_TIQUES == 0 && !sala.music.is_empty()) {
                envios.push((sala.id.clone(), estado(sala, agora)));
            }
        }
        for (id, m) in envios {
            inner.broadcast_room(&id, m, None);
        }
    }
}
