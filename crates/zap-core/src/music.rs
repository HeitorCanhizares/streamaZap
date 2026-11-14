//! Música da sala (`docs/planos/musica-da-sala.md`), lado do app: ler o link do YouTube e o
//! espelho do estado que chega da fonte da sala. A fila em si (`MusicQueue`) é do `zap-proto`,
//! para o servidor Online aplicar a mesma regra.

use std::time::Instant;

pub use zap_proto::music::{is_video_id, needed, MusicError, MusicQueue, QUEUE_MAX, TITULO_PADRAO};
use zap_proto::{ControlMsg, MusicItem, MusicNow};

use crate::snapshot::{MusicItemView, MusicNowView, MusicView};
use crate::util::now_ms;

/// Id do vídeo a partir do que a pessoa colou: o id puro ou um link de vídeo, live, shorts,
/// embed, `youtu.be` ou `music.youtube.com`. `None` se não for do YouTube.
pub fn video_id_from_link(texto: &str) -> Option<String> {
    let t = texto.trim();
    if is_video_id(t) {
        return Some(t.to_string());
    }
    let sem_esquema = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let sem_fragmento = sem_esquema.split('#').next().unwrap_or("");
    let (host, resto) = match sem_fragmento.find(['/', '?']) {
        Some(i) => (&sem_fragmento[..i], &sem_fragmento[i..]),
        None => (sem_fragmento, ""),
    };
    let host = host.to_ascii_lowercase();
    let host = ["www.", "m.", "music."]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .unwrap_or(&host);
    let (caminho, consulta) = match resto.split_once('?') {
        Some((c, q)) => (c, q),
        None => (resto, ""),
    };
    let segmentos: Vec<&str> = caminho.split('/').filter(|s| !s.is_empty()).collect();
    let candidato = match host {
        "youtu.be" => segmentos.first().copied(),
        "youtube.com" | "youtube-nocookie.com" => match segmentos.as_slice() {
            ["watch"] => consulta.split('&').find_map(|par| par.strip_prefix("v=")),
            ["live" | "shorts" | "embed" | "v", id, ..] => Some(*id),
            _ => None,
        },
        _ => None,
    }?;
    is_video_id(candidato).then(|| candidato.to_string())
}

/// O último `musica.estado` (do dono, ou montado da própria fila no dono) e quando chegou: a
/// posição anda a partir daí.
#[derive(Debug, Clone)]
pub struct MusicMirror {
    pub queue: Vec<MusicItem>,
    pub current: Option<MusicNow>,
    pub received: Instant,
}

impl MusicMirror {
    /// De um `musica.estado`; outra mensagem vira um estado vazio.
    /// Itens com `video_id` que não é do YouTube (dono com defeito) ficam de fora: o id vai parar
    /// num endereço na interface.
    pub fn from_msg(msg: &ControlMsg, received: Instant) -> Self {
        let (queue, current) = match msg {
            ControlMsg::MusicState { queue, current } => (
                queue.iter().filter(|it| is_video_id(&it.video_id)).cloned().collect(),
                current.clone().filter(|n| is_video_id(&n.item.video_id)),
            ),
            _ => (Vec::new(), None),
        };
        Self {
            queue,
            current,
            received,
        }
    }

    /// Visão para a interface: a posição andada até `agora` (pausada, parada), com o relógio de
    /// parede do mesmo instante (`at_ms`) para a interface continuar contando.
    pub fn view(&self, agora: Instant) -> MusicView {
        let andou = agora.saturating_duration_since(self.received).as_millis() as i64;
        MusicView {
            queue: self.queue.iter().map(item_view).collect(),
            current: self.current.as_ref().map(|n| {
                let pausada = n.paused == Some(true);
                MusicNowView {
                    item: item_view(&n.item),
                    position_ms: if pausada {
                        n.position_ms
                    } else {
                        n.position_ms.saturating_add(andou)
                    },
                    at_ms: now_ms(),
                    votes: n.votes.clone(),
                    needed: n.needed,
                    paused: pausada,
                    controls: n.paused.is_some(),
                }
            }),
        }
    }
}

fn item_view(it: &MusicItem) -> MusicItemView {
    MusicItemView {
        id: it.id.clone(),
        video_id: it.video_id.clone(),
        title: it.title.clone(),
        channel: it.channel.clone(),
        duration_ms: it.duration_ms,
        live: it.live,
        added_by: it.added_by.clone(),
        added_by_name: it.added_by_name.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    const A: &str = "aaaaaaaaaaa";

    fn novo(video: &str) -> MusicItem {
        MusicItem {
            id: "mu_1".into(),
            video_id: video.into(),
            title: "Música".into(),
            channel: Some("Canal".into()),
            duration_ms: None,
            live: false,
            added_by: "p_1".into(),
            added_by_name: "Pessoa p_1".into(),
        }
    }

    #[test]
    fn links_do_youtube() {
        let casos = [
            ("dQw4w9WgXcQ", Some("dQw4w9WgXcQ")),
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", Some("dQw4w9WgXcQ")),
            (
                "https://youtube.com/watch?list=PL1&v=dQw4w9WgXcQ&t=42s",
                Some("dQw4w9WgXcQ"),
            ),
            (
                "http://m.youtube.com/watch?v=dQw4w9WgXcQ#comentarios",
                Some("dQw4w9WgXcQ"),
            ),
            (
                "https://music.youtube.com/watch?v=dQw4w9WgXcQ&si=abc",
                Some("dQw4w9WgXcQ"),
            ),
            ("youtube.com/watch?v=dQw4w9WgXcQ", Some("dQw4w9WgXcQ")),
            ("https://youtu.be/dQw4w9WgXcQ?si=xyz", Some("dQw4w9WgXcQ")),
            (
                "https://www.youtube.com/live/jfKfPfyJRdk?feature=share",
                Some("jfKfPfyJRdk"),
            ),
            ("https://www.youtube.com/shorts/dQw4w9WgXcQ", Some("dQw4w9WgXcQ")),
            (
                "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
                Some("dQw4w9WgXcQ"),
            ),
            ("  https://YouTube.com/watch?v=dQw4w9WgXcQ  ", Some("dQw4w9WgXcQ")),
            ("https://www.youtube.com/playlist?list=PL123", None),
            ("https://www.youtube.com/@canal", None),
            ("https://vimeo.com/watch?v=dQw4w9WgXcQ", None),
            ("https://notyoutube.com/watch?v=dQw4w9WgXcQ", None),
            ("https://youtu.be/curto", None),
            ("", None),
            ("oi", None),
        ];
        for (link, esperado) in casos {
            assert_eq!(video_id_from_link(link).as_deref(), esperado, "{link}");
        }
    }

    #[test]
    fn espelho_do_membro_anda_a_posicao() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(novo(A), t0).unwrap();
        let msg = q.msg(2, t0 + Duration::from_secs(10));
        let espelho = MusicMirror::from_msg(&msg, t0 + Duration::from_secs(10));
        let v = espelho.view(t0 + Duration::from_millis(11_500));
        let atual = v.current.expect("tocando");
        assert_eq!(atual.position_ms, 9_500, "8 s no envio + 1,5 s desde que chegou");
        assert_eq!(atual.item.title, "Música");
        assert_eq!(atual.item.channel.as_deref(), Some("Canal"));
        assert!(atual.at_ms > 0);
        assert!(!atual.paused && atual.controls, "fonte nova: dá para pausar e buscar");
        // Outra mensagem vira estado vazio.
        let vazio = MusicMirror::from_msg(&ControlMsg::Ping { ts_ms: 1 }, t0).view(t0);
        assert!(vazio.queue.is_empty() && vazio.current.is_none());
        // Id que não é do YouTube não entra no espelho.
        let mut ruim = msg.clone();
        if let ControlMsg::MusicState { queue, current } = &mut ruim {
            let mut item = current.as_ref().unwrap().item.clone();
            item.video_id = "../../x?a=1".into();
            queue.push(item.clone());
            current.as_mut().unwrap().item = item;
        }
        let filtrado = MusicMirror::from_msg(&ruim, t0).view(t0);
        assert!(filtrado.queue.is_empty() && filtrado.current.is_none());
    }

    #[test]
    fn espelho_pausado_nao_anda_e_fonte_antiga_nao_tem_controles() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(novo(A), t0).unwrap();
        q.pause("mu_1", true, t0 + Duration::from_secs(10));
        let msg = q.msg(2, t0 + Duration::from_secs(20));
        let espelho = MusicMirror::from_msg(&msg, t0 + Duration::from_secs(20));
        let atual = espelho.view(t0 + Duration::from_secs(50)).current.expect("tocando");
        assert_eq!(atual.position_ms, 8_000, "parada onde pausou");
        assert!(atual.paused && atual.controls);
        // Fonte antiga: sem `pausada` no estado.
        let mut antigo = msg;
        if let ControlMsg::MusicState { current, .. } = &mut antigo {
            current.as_mut().unwrap().paused = None;
        }
        let atual = MusicMirror::from_msg(&antigo, t0).view(t0).current.expect("tocando");
        assert!(!atual.paused && !atual.controls);
    }
}
