//! Música da sala (`docs/planos/musica-da-sala.md`): ler o link do YouTube e a fila do dono.
//!
//! Tudo aqui é puro: o tempo entra como parâmetro (`Instant`), para os testes andarem o relógio.
//! A posição da sala anda sozinha a partir do início do item; ninguém espera ninguém (quem pegou
//! anúncio pula para a posição da sala quando o player volta).

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use zap_net::youtube::is_video_id;
use zap_proto::{ControlMsg, MusicItem, MusicNow, Participant};

use crate::snapshot::{MusicItemView, MusicNowView, MusicView};
use crate::util::{new_id, now_ms};

/// Itens na fila (sem contar o que está tocando).
pub const QUEUE_MAX: usize = 50;
/// O item novo começa um pouco no futuro, para os players carregarem juntos.
const ANTECEDENCIA: Duration = Duration::from_secs(2);
/// Depois da duração, espera isso antes de avançar (o fim do player e o da sala nunca batem certo).
const FOLGA_FIM: Duration = Duration::from_secs(2);
/// Um aviso de "terminou" só vale com a posição da sala a essa distância do fim.
const PERTO_DO_FIM_MS: i64 = 5_000;
/// Sem duração conhecida, "terminou" só vale depois disso tocando.
const MINIMO_SEM_DURACAO_MS: i64 = 10_000;
const TITULO_MAX: usize = 120;
const CANAL_MAX: usize = 80;
/// Título enquanto ninguém soube o de verdade (o player completa com `musica.info`).
pub const TITULO_PADRAO: &str = "Vídeo do YouTube";

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

/// Votos que fazem pular: metade da sala, arredondando para cima (pelo menos 1).
pub fn needed(presentes: usize) -> u32 {
    presentes.div_ceil(2).max(1) as u32
}

/// Por que um pedido de música foi recusado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicError {
    QueueFull,
    InvalidVideo,
    NotFound,
    NotAllowed,
}

impl MusicError {
    pub fn message(&self) -> String {
        match self {
            MusicError::QueueFull => format!("A fila já tem {QUEUE_MAX} músicas"),
            MusicError::InvalidVideo => "Link do YouTube inválido".into(),
            MusicError::NotFound => "Essa música não está mais na fila".into(),
            MusicError::NotAllowed => "Só quem pôs a música ou o dono da sala pode tirar".into(),
        }
    }
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

    /// Visão para a interface: a posição andada até `agora`, com o relógio de parede do mesmo
    /// instante (`at_ms`) para a interface continuar contando.
    pub fn view(&self, agora: Instant) -> MusicView {
        let andou = agora.saturating_duration_since(self.received).as_millis() as i64;
        MusicView {
            queue: self.queue.iter().map(item_view).collect(),
            current: self.current.as_ref().map(|n| MusicNowView {
                item: item_view(&n.item),
                position_ms: n.position_ms.saturating_add(andou),
                at_ms: now_ms(),
                votes: n.votes.clone(),
                needed: n.needed,
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

struct Atual {
    item: MusicItem,
    /// Instante da posição 0 (pode estar no futuro: `ANTECEDENCIA`).
    inicio: Instant,
    votos: BTreeSet<String>,
}

/// A fila do dono da sala: a fonte da verdade da música.
#[derive(Default)]
pub struct MusicQueue {
    fila: VecDeque<MusicItem>,
    atual: Option<Atual>,
}

fn corta(texto: &str, max: usize) -> String {
    texto.trim().chars().take(max).collect()
}

impl MusicQueue {
    /// Nada tocando e nada na fila.
    pub fn is_empty(&self) -> bool {
        self.atual.is_none() && self.fila.is_empty()
    }

    /// Põe o vídeo no fim da fila; com nada tocando, ele começa já.
    pub fn add(
        &mut self,
        por: &Participant,
        video_id: &str,
        title: Option<String>,
        channel: Option<String>,
        agora: Instant,
    ) -> Result<(), MusicError> {
        if !is_video_id(video_id) {
            return Err(MusicError::InvalidVideo);
        }
        if self.fila.len() >= QUEUE_MAX {
            return Err(MusicError::QueueFull);
        }
        let title = title
            .map(|t| corta(&t, TITULO_MAX))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| TITULO_PADRAO.to_string());
        let channel = channel.map(|c| corta(&c, CANAL_MAX)).filter(|c| !c.is_empty());
        self.fila.push_back(MusicItem {
            id: new_id("mu_"),
            video_id: video_id.to_string(),
            title,
            channel,
            duration_ms: None,
            live: false,
            added_by: por.id.clone(),
            added_by_name: por.name.clone(),
        });
        if self.atual.is_none() {
            self.avancar(agora);
        }
        Ok(())
    }

    /// Tira um item da fila (quem pôs ou o dono). O que está tocando = pular direto.
    pub fn remove(&mut self, quem: &str, eh_dono: bool, item_id: &str, agora: Instant) -> Result<(), MusicError> {
        if let Some(a) = &self.atual {
            if a.item.id == item_id {
                if a.item.added_by != quem && !eh_dono {
                    return Err(MusicError::NotAllowed);
                }
                self.avancar(agora);
                return Ok(());
            }
        }
        let i = self
            .fila
            .iter()
            .position(|it| it.id == item_id)
            .ok_or(MusicError::NotFound)?;
        if self.fila[i].added_by != quem && !eh_dono {
            return Err(MusicError::NotAllowed);
        }
        self.fila.remove(i);
        Ok(())
    }

    /// Voto (ou desfazer o voto) para pular o item atual. Voto de item que já saiu é ignorado.
    /// Devolve se o estado mudou.
    pub fn vote(&mut self, quem: &str, item_id: &str, voto: bool, presentes: usize, agora: Instant) -> bool {
        let Some(a) = self.atual.as_mut() else {
            return false;
        };
        if a.item.id != item_id {
            return false;
        }
        let mudou = if voto {
            a.votos.insert(quem.to_string())
        } else {
            a.votos.remove(quem)
        };
        if a.votos.len() as u32 >= needed(presentes) {
            self.avancar(agora);
            return true;
        }
        mudou
    }

    /// O que o player de alguém descobriu do item. Devolve se o estado mudou.
    #[allow(clippy::too_many_arguments)]
    pub fn info(
        &mut self,
        item_id: &str,
        duration_ms: Option<u64>,
        live: Option<bool>,
        title: Option<String>,
        ended: bool,
        failed: bool,
        agora: Instant,
    ) -> bool {
        let posicao = self.position_ms(agora);
        let eh_atual = self.atual.as_ref().is_some_and(|a| a.item.id == item_id);
        let item = match self.atual.as_mut().filter(|a| a.item.id == item_id) {
            Some(a) => &mut a.item,
            None => match self.fila.iter_mut().find(|it| it.id == item_id) {
                Some(it) => it,
                None => return false,
            },
        };
        let mut mudou = false;
        // A maior duração vence: durante um anúncio o player pode contar a duração dele.
        if let Some(d) = duration_ms.filter(|d| *d >= 1_000) {
            if item.duration_ms.is_none_or(|atual| d > atual) {
                item.duration_ms = Some(d);
                mudou = true;
            }
        }
        if live == Some(true) && !item.live {
            item.live = true;
            mudou = true;
        }
        if let Some(t) = title.map(|t| corta(&t, TITULO_MAX)).filter(|t| !t.is_empty()) {
            if item.title == TITULO_PADRAO {
                item.title = t;
                mudou = true;
            }
        }
        if !eh_atual {
            return mudou;
        }
        let acabou = ended
            && (item.live
                || match item.duration_ms {
                    Some(d) => posicao >= d as i64 - PERTO_DO_FIM_MS,
                    None => posicao >= MINIMO_SEM_DURACAO_MS,
                });
        if failed || acabou {
            self.avancar(agora);
            return true;
        }
        mudou
    }

    /// Avança quando a posição da sala passa do fim do vídeo. Live só sai por voto, por quem pôs,
    /// pelo dono ou quando acaba. Devolve se avançou.
    pub fn tick(&mut self, agora: Instant) -> bool {
        let Some(a) = &self.atual else {
            return false;
        };
        let Some(d) = a.item.duration_ms.filter(|_| !a.item.live) else {
            return false;
        };
        if self.position_ms(agora) >= (Duration::from_millis(d) + FOLGA_FIM).as_millis() as i64 {
            self.avancar(agora);
            return true;
        }
        false
    }

    /// Alguém saiu da sala: perde o voto, e o limite cai junto (pode pular na hora).
    pub fn left(&mut self, quem: &str, presentes: usize, agora: Instant) -> bool {
        let Some(a) = self.atual.as_mut() else {
            return false;
        };
        let tirou = a.votos.remove(quem);
        if !a.votos.is_empty() && a.votos.len() as u32 >= needed(presentes) {
            self.avancar(agora);
            return true;
        }
        tirou
    }

    /// Posição da sala no item atual, em ms (negativa = ainda vai começar; 0 sem item).
    pub fn position_ms(&self, agora: Instant) -> i64 {
        match &self.atual {
            Some(a) if agora >= a.inicio => agora.duration_since(a.inicio).as_millis() as i64,
            Some(a) => -(a.inicio.duration_since(agora).as_millis() as i64),
            None => 0,
        }
    }

    /// `musica.estado` para enviar agora.
    pub fn msg(&self, presentes: usize, agora: Instant) -> ControlMsg {
        ControlMsg::MusicState {
            queue: self.fila.iter().cloned().collect(),
            current: self.atual.as_ref().map(|a| MusicNow {
                item: a.item.clone(),
                position_ms: self.position_ms(agora),
                votes: a.votos.iter().cloned().collect(),
                needed: needed(presentes),
            }),
        }
    }

    fn avancar(&mut self, agora: Instant) {
        self.atual = self.fila.pop_front().map(|item| Atual {
            item,
            inicio: agora + ANTECEDENCIA,
            votos: BTreeSet::new(),
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zap_proto::Role;

    const A: &str = "aaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbb";
    const C: &str = "ccccccccccc";

    fn pessoa(id: &str) -> Participant {
        Participant {
            id: id.into(),
            name: format!("Pessoa {id}"),
            color: "#ff0000".into(),
            role: Role::Member,
            joined_at: "2026-01-01T00:00:00Z".into(),
            app_version: "3.2.0".into(),
            protocol: 1,
            latency_ms: None,
            watching: None,
        }
    }

    fn estado(q: &MusicQueue, presentes: usize, agora: Instant) -> (Vec<MusicItem>, Option<MusicNow>) {
        match q.msg(presentes, agora) {
            ControlMsg::MusicState { queue, current } => (queue, current),
            outro => panic!("veio {outro:?}"),
        }
    }

    fn atual_video(q: &MusicQueue, agora: Instant) -> Option<String> {
        estado(q, 1, agora).1.map(|n| n.item.video_id)
    }

    fn atual_id(q: &MusicQueue, agora: Instant) -> String {
        estado(q, 1, agora).1.expect("tocando").item.id
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
    fn metade_da_sala() {
        assert_eq!(needed(0), 1);
        assert_eq!(needed(1), 1);
        assert_eq!(needed(2), 1);
        assert_eq!(needed(3), 2);
        assert_eq!(needed(4), 2);
        assert_eq!(needed(5), 3);
    }

    #[test]
    fn primeiro_item_comeca_no_futuro_e_o_resto_espera() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        assert!(q.is_empty());
        q.add(&pessoa("p_1"), A, Some("  Primeira ".into()), None, t0).unwrap();
        q.add(&pessoa("p_2"), B, None, Some("".into()), t0).unwrap();
        let (fila, atual) = estado(&q, 2, t0);
        let atual = atual.expect("tocando");
        assert_eq!(atual.item.video_id, A);
        assert_eq!(atual.item.title, "Primeira");
        assert_eq!(atual.position_ms, -2_000);
        assert_eq!(atual.needed, 1);
        assert_eq!(fila.len(), 1);
        assert_eq!(fila[0].title, TITULO_PADRAO);
        assert_eq!(fila[0].channel, None);
        assert_eq!(fila[0].added_by_name, "Pessoa p_2");
        assert_eq!(q.position_ms(t0 + Duration::from_secs(5)), 3_000);
    }

    #[test]
    fn video_invalido_e_fila_cheia() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        assert_eq!(
            q.add(&pessoa("p_1"), "x", None, None, t0),
            Err(MusicError::InvalidVideo)
        );
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap(); // começa a tocar
        for _ in 0..QUEUE_MAX {
            q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        }
        assert_eq!(q.add(&pessoa("p_1"), C, None, None, t0), Err(MusicError::QueueFull));
    }

    #[test]
    fn avanca_no_fim_com_folga_e_para_quando_acaba_a_fila() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        // Sem duração, não avança sozinho.
        assert!(!q.tick(t0 + Duration::from_secs(600)));
        assert!(q.info(&id, Some(10_000), Some(false), None, false, false, t0));
        // Começa em t0+2s; dura 10 s; folga de 2 s: avança em t0+14s.
        assert!(!q.tick(t0 + Duration::from_millis(13_900)));
        assert!(q.tick(t0 + Duration::from_millis(14_000)));
        let t1 = t0 + Duration::from_millis(14_000);
        assert_eq!(atual_video(&q, t1).as_deref(), Some(B));
        assert_eq!(q.position_ms(t1), -2_000);
        let id_b = atual_id(&q, t1);
        assert!(q.info(&id_b, Some(1_000), None, None, false, false, t1));
        assert!(q.tick(t1 + Duration::from_secs(10)));
        assert!(q.is_empty());
        assert!(!q.tick(t1 + Duration::from_secs(20)));
    }

    #[test]
    fn maior_duracao_vence_e_titulo_so_substitui_o_padrao() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, Some("Título real".into()), None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.info(&id, Some(15_000), None, Some("Do player".into()), false, false, t0));
        assert!(
            !q.info(&id, Some(15_000), None, Some("Outro".into()), false, false, t0),
            "nada mudou"
        );
        assert!(
            !q.info(&id, Some(500), None, None, false, false, t0),
            "curta demais: ignorada"
        );
        assert!(q.info(&id, Some(200_000), None, None, false, false, t0));
        let (fila, atual) = estado(&q, 1, t0);
        let atual = atual.unwrap();
        assert_eq!(atual.item.title, "Do player");
        assert_eq!(atual.item.duration_ms, Some(200_000));
        // Info de item da fila também vale (e não avança nada).
        let id_b = fila[0].id.clone();
        assert!(!q.info(&id_b, None, None, Some("Outro".into()), false, false, t0));
        assert!(q.info(&id_b, None, Some(true), None, true, false, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(A));
        assert!(!q.info("mu_nao_existe", Some(9_000), None, None, false, false, t0));
    }

    #[test]
    fn terminou_so_perto_do_fim() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        q.info(&id, Some(60_000), None, None, false, false, t0);
        // Alguém adiantado manda "terminou" com a sala em 30 s: não vale.
        let meio = t0 + Duration::from_secs(32);
        assert!(!q.info(&id, None, None, None, true, false, meio));
        assert_eq!(atual_video(&q, meio).as_deref(), Some(A));
        // A 5 s do fim vale.
        let quase = t0 + Duration::from_secs(57);
        assert!(q.info(&id, None, None, None, true, false, quase));
        assert_eq!(atual_video(&q, quase).as_deref(), Some(B));
        // Sem duração: só depois de 10 s tocando.
        let id_b = atual_id(&q, quase);
        assert!(!q.info(&id_b, None, None, None, true, false, quase + Duration::from_secs(5)));
        assert!(q.info(&id_b, None, None, None, true, false, quase + Duration::from_secs(13)));
        assert!(q.is_empty());
    }

    #[test]
    fn live_nao_acaba_sozinha_mas_sai_quando_termina() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.info(&id, Some(5_000), Some(true), None, false, false, t0));
        assert!(!q.tick(t0 + Duration::from_secs(3_600)), "live não acaba pela duração");
        let depois = t0 + Duration::from_secs(4);
        assert!(
            q.info(&id, None, None, None, true, false, depois),
            "a transmissão acabou"
        );
        assert_eq!(atual_video(&q, depois).as_deref(), Some(B));
    }

    #[test]
    fn erro_do_player_pula_na_hora() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.info(&id, None, None, None, false, true, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));
    }

    #[test]
    fn votos_com_uma_duas_tres_e_quatro_pessoas() {
        let t0 = Instant::now();
        // Sozinho na sala: o próprio voto pula.
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert_eq!(estado(&q, 1, t0).1.unwrap().needed, 1);
        assert!(q.vote("p_1", &id, true, 1, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));

        // Duas pessoas: um voto basta.
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.vote("p_2", &id, true, 2, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));

        // Três pessoas: dois votos; voto repetido não conta duas vezes; desfazer volta.
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.vote("p_2", &id, true, 3, t0));
        assert!(!q.vote("p_2", &id, true, 3, t0));
        assert_eq!(estado(&q, 3, t0).1.unwrap().votes, vec!["p_2".to_string()]);
        assert!(q.vote("p_2", &id, false, 3, t0));
        assert!(!q.vote("p_2", &id, false, 3, t0));
        assert!(q.vote("p_2", &id, true, 3, t0));
        assert!(q.vote("p_3", &id, true, 3, t0));
        assert!(q.is_empty(), "pulou e a fila acabou");

        // Quatro pessoas: dois votos. Voto de item velho é ignorado.
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(!q.vote("p_2", "mu_velho", true, 4, t0));
        assert!(q.vote("p_2", &id, true, 4, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(A));
        assert!(q.vote("p_3", &id, true, 4, t0));
        assert!(q.is_empty());
    }

    #[test]
    fn quem_sai_perde_o_voto_e_o_limite_cai() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_1"), B, None, None, t0).unwrap();
        let id = atual_id(&q, t0);
        // 5 pessoas: precisa de 3. Dois votam.
        q.vote("p_2", &id, true, 5, t0);
        q.vote("p_3", &id, true, 5, t0);
        // p_4 (sem voto) sai: 4 pessoas, precisa de 2, já tem 2: pula.
        assert!(q.left("p_4", 4, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));
        // Quem votou e saiu leva o voto.
        let id_b = atual_id(&q, t0);
        q.vote("p_2", &id_b, true, 4, t0);
        assert!(q.left("p_2", 3, t0));
        assert!(estado(&q, 3, t0).1.unwrap().votes.is_empty());
        // Sem votos, ninguém saindo pula nada.
        assert!(!q.left("p_9", 1, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));
    }

    #[test]
    fn espelho_do_membro_anda_a_posicao() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, Some("Música".into()), Some("Canal".into()), t0)
            .unwrap();
        let msg = q.msg(2, t0 + Duration::from_secs(10));
        let espelho = MusicMirror::from_msg(&msg, t0 + Duration::from_secs(10));
        let v = espelho.view(t0 + Duration::from_millis(11_500));
        let atual = v.current.expect("tocando");
        assert_eq!(atual.position_ms, 9_500, "8 s no envio + 1,5 s desde que chegou");
        assert_eq!(atual.item.title, "Música");
        assert_eq!(atual.item.channel.as_deref(), Some("Canal"));
        assert!(atual.at_ms > 0);
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
    fn remover_e_pular_direto_com_permissao() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(&pessoa("p_1"), A, None, None, t0).unwrap();
        q.add(&pessoa("p_2"), B, None, None, t0).unwrap();
        q.add(&pessoa("p_2"), C, None, None, t0).unwrap();
        let (fila, atual) = estado(&q, 3, t0);
        let (id_a, id_b, id_c) = (atual.unwrap().item.id, fila[0].id.clone(), fila[1].id.clone());
        // p_3 não pôs nada e não é dono.
        assert_eq!(q.remove("p_3", false, &id_b, t0), Err(MusicError::NotAllowed));
        assert_eq!(q.remove("p_3", false, &id_a, t0), Err(MusicError::NotAllowed));
        // Quem pôs tira da fila; o dono tira qualquer um.
        assert_eq!(q.remove("p_2", false, &id_b, t0), Ok(()));
        assert_eq!(q.remove("p_2", false, &id_b, t0), Err(MusicError::NotFound));
        // O atual: quem pôs pula direto.
        assert_eq!(q.remove("p_1", false, &id_a, t0), Ok(()));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(C));
        // O dono pula o de outra pessoa.
        assert_eq!(q.remove("p_1", true, &id_c, t0), Ok(()));
        assert!(q.is_empty());
    }
}
