//! Fila de música da sala (`docs/planos/musica-da-sala.md`): a regra que a fonte da sala aplica
//! (quem criou a sala no modo Local, o servidor no Online). Pura: o tempo entra como parâmetro
//! (`Instant`) e o id dos itens vem de quem chama. A posição da sala anda sozinha a partir do início
//! do item; ninguém espera ninguém (quem pegou anúncio pula para a posição da sala quando o player
//! volta). Qualquer um da sala pausa (a posição para) e muda o tempo (a posição pula).

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use crate::control::{ControlMsg, MusicItem, MusicNow};

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

/// Id de vídeo do YouTube: 11 caracteres de `[A-Za-z0-9_-]`.
pub fn is_video_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
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
    /// Buscar uma posição numa live.
    LiveSeek,
}

impl MusicError {
    pub fn message(&self) -> String {
        match self {
            MusicError::QueueFull => format!("A fila já tem {QUEUE_MAX} músicas"),
            MusicError::InvalidVideo => "Link do YouTube inválido".into(),
            MusicError::NotFound => "Essa música não está mais na fila".into(),
            MusicError::NotAllowed => "Só quem pôs a música ou o dono da sala pode tirar".into(),
            MusicError::LiveSeek => "Não dá para mudar o tempo de uma transmissão ao vivo".into(),
        }
    }
}

struct Atual {
    item: MusicItem,
    /// Posição (ms) no instante `base`; negativa = ainda vai começar (`ANTECEDENCIA`).
    pos_base: i64,
    base: Instant,
    /// Pausado para todos: a posição fica em `pos_base`.
    pausada: bool,
    votos: BTreeSet<String>,
}

/// `agora - antes` em ms, negativo se `agora` vier antes.
fn ms_entre(antes: Instant, agora: Instant) -> i64 {
    if agora >= antes {
        agora.duration_since(antes).as_millis() as i64
    } else {
        -(antes.duration_since(agora).as_millis() as i64)
    }
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

    /// Põe o vídeo no fim da fila; com nada tocando, ele começa já. Quem chama dá o `id` do
    /// item e quem pôs (`added_by`, `added_by_name`); título e canal são aparados (título vazio
    /// vira [`TITULO_PADRAO`]); duração e live começam vazios (o player conta com `musica.info`).
    pub fn add(&mut self, novo: MusicItem, agora: Instant) -> Result<(), MusicError> {
        let MusicItem {
            id,
            video_id,
            title,
            channel,
            added_by,
            added_by_name,
            ..
        } = novo;
        if !is_video_id(&video_id) {
            return Err(MusicError::InvalidVideo);
        }
        if self.fila.len() >= QUEUE_MAX {
            return Err(MusicError::QueueFull);
        }
        let title = Some(corta(&title, TITULO_MAX))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| TITULO_PADRAO.to_string());
        let channel = channel.map(|c| corta(&c, CANAL_MAX)).filter(|c| !c.is_empty());
        self.fila.push_back(MusicItem {
            id,
            video_id,
            title,
            channel,
            duration_ms: None,
            live: false,
            added_by,
            added_by_name,
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

    /// Pausa (`true`) ou retoma (`false`) o item atual, para todos; retomar continua da posição em
    /// que parou. Item que não é o atual (a fila andou no meio do caminho) é ignorado. Devolve se
    /// o estado mudou.
    pub fn pause(&mut self, item_id: &str, pausada: bool, agora: Instant) -> bool {
        let posicao = self.position_ms(agora);
        let Some(a) = self.atual.as_mut().filter(|a| a.item.id == item_id) else {
            return false;
        };
        if a.pausada == pausada {
            return false;
        }
        a.pos_base = posicao;
        a.base = agora;
        a.pausada = pausada;
        true
    }

    /// Leva o item atual para `posicao_ms` (até a duração, se conhecida); pausado, continua
    /// pausado. Live é recusada. Item que não é o atual é ignorado (`Ok(false)`).
    pub fn seek(&mut self, item_id: &str, posicao_ms: u64, agora: Instant) -> Result<bool, MusicError> {
        let Some(a) = self.atual.as_mut().filter(|a| a.item.id == item_id) else {
            return Ok(false);
        };
        if a.item.live {
            return Err(MusicError::LiveSeek);
        }
        let alvo = a.item.duration_ms.map_or(posicao_ms, |d| posicao_ms.min(d));
        a.pos_base = i64::try_from(alvo).unwrap_or(i64::MAX);
        a.base = agora;
        Ok(true)
    }

    /// O item atual está pausado para todos.
    pub fn is_paused(&self) -> bool {
        self.atual.as_ref().is_some_and(|a| a.pausada)
    }

    /// Avança quando a posição da sala passa do fim do vídeo (pausado, não anda). Live só sai por
    /// voto, por quem pôs, pelo dono ou quando acaba. Devolve se avançou.
    pub fn tick(&mut self, agora: Instant) -> bool {
        let Some(a) = &self.atual else {
            return false;
        };
        if a.pausada {
            return false;
        }
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

    /// Posição da sala no item atual, em ms (negativa = ainda vai começar; 0 sem item). Pausado,
    /// fica parada.
    pub fn position_ms(&self, agora: Instant) -> i64 {
        match &self.atual {
            Some(a) if a.pausada => a.pos_base,
            Some(a) => a.pos_base.saturating_add(ms_entre(a.base, agora)),
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
                paused: Some(a.pausada),
            }),
        }
    }

    fn avancar(&mut self, agora: Instant) {
        self.atual = self.fila.pop_front().map(|item| Atual {
            item,
            pos_base: -(ANTECEDENCIA.as_millis() as i64),
            base: agora,
            pausada: false,
            votos: BTreeSet::new(),
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbb";
    const C: &str = "ccccccccccc";

    /// Item novo como o despacho monta: id sequencial, quem pôs = "Pessoa <id>".
    fn item(por: &str, video: &str, title: Option<&str>, channel: Option<&str>) -> MusicItem {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        MusicItem {
            id: format!("mu_{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            video_id: video.into(),
            title: title.unwrap_or("").into(),
            channel: channel.map(Into::into),
            duration_ms: None,
            live: false,
            added_by: por.into(),
            added_by_name: format!("Pessoa {por}"),
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
    fn id_de_video() {
        assert!(is_video_id("dQw4w9WgXcQ"));
        assert!(is_video_id("a-b_c1234XY"));
        assert!(!is_video_id("dQw4w9WgXc"), "10 caracteres");
        assert!(!is_video_id("dQw4w9WgXcQQ"), "12 caracteres");
        assert!(!is_video_id("dQw4w9WgX&Q"), "caractere de URL");
        assert!(!is_video_id(""));
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
        q.add(item("p_1", A, Some("  Primeira "), None), t0).unwrap();
        q.add(item("p_2", B, None, Some("")), t0).unwrap();
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
        assert_eq!(q.add(item("p_1", "x", None, None), t0), Err(MusicError::InvalidVideo));
        q.add(item("p_1", A, None, None), t0).unwrap(); // começa a tocar
        for _ in 0..QUEUE_MAX {
            q.add(item("p_1", B, None, None), t0).unwrap();
        }
        assert_eq!(q.add(item("p_1", C, None, None), t0), Err(MusicError::QueueFull));
    }

    #[test]
    fn avanca_no_fim_com_folga_e_para_quando_acaba_a_fila() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, Some("Título real"), None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.info(&id, None, None, None, false, true, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));
    }

    #[test]
    fn votos_com_uma_duas_tres_e_quatro_pessoas() {
        let t0 = Instant::now();
        // Sozinho na sala: o próprio voto pula.
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        assert_eq!(estado(&q, 1, t0).1.unwrap().needed, 1);
        assert!(q.vote("p_1", &id, true, 1, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));

        // Duas pessoas: um voto basta.
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        assert!(q.vote("p_2", &id, true, 2, t0));
        assert_eq!(atual_video(&q, t0).as_deref(), Some(B));

        // Três pessoas: dois votos; voto repetido não conta duas vezes; desfazer volta.
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
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
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
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
    fn pausar_para_a_posicao_e_o_fim_e_retomar_continua() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        q.info(&id, Some(60_000), None, None, false, false, t0);
        assert_eq!(
            estado(&q, 1, t0).1.unwrap().paused,
            Some(false),
            "a fonte nova sempre diz"
        );
        assert!(!q.is_paused());

        // Pausa com a sala em 10 s (começou em t0+2s): a posição para.
        let t_pausa = t0 + Duration::from_secs(12);
        assert!(q.pause(&id, true, t_pausa));
        assert!(!q.pause(&id, true, t_pausa), "já estava pausada");
        assert!(q.is_paused());
        let muito_depois = t0 + Duration::from_secs(600);
        assert_eq!(q.position_ms(muito_depois), 10_000);
        let atual = estado(&q, 1, muito_depois).1.unwrap();
        assert_eq!((atual.position_ms, atual.paused), (10_000, Some(true)));
        assert!(!q.tick(muito_depois), "pausada, o fim não chega");

        // Retoma de onde parou; o fim conta a partir daí (faltam 50 s + 2 s de folga).
        assert!(q.pause(&id, false, muito_depois));
        assert!(!q.pause(&id, false, muito_depois), "já estava tocando");
        assert_eq!(q.position_ms(muito_depois + Duration::from_secs(5)), 15_000);
        assert!(!q.tick(muito_depois + Duration::from_millis(51_900)));
        assert!(q.tick(muito_depois + Duration::from_secs(52)));
        assert_eq!(atual_video(&q, muito_depois).as_deref(), Some(B));
    }

    #[test]
    fn pausar_item_que_nao_e_o_atual_e_ignorado() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        assert!(!q.pause("mu_1", true, t0), "nada tocando");
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let fila = estado(&q, 1, t0).0;
        assert!(!q.pause(&fila[0].id, true, t0), "item da fila");
        assert!(!q.pause("mu_velho", true, t0));
        assert!(!q.is_paused());
    }

    #[test]
    fn o_proximo_item_comeca_tocando() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        let t1 = t0 + Duration::from_secs(30);
        assert!(q.pause(&id, true, t1));
        // Pausada, o voto pula do mesmo jeito.
        assert!(q.vote("p_1", &id, true, 1, t1));
        let atual = estado(&q, 1, t1).1.unwrap();
        assert_eq!(atual.item.video_id, B);
        assert_eq!((atual.position_ms, atual.paused), (-2_000, Some(false)));
        assert_eq!(q.position_ms(t1 + Duration::from_secs(3)), 1_000);
    }

    #[test]
    fn buscar_muda_a_posicao_para_todos() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_1", B, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        // Sem duração conhecida: vale qualquer posição.
        let t1 = t0 + Duration::from_secs(5);
        assert_eq!(q.seek(&id, 400_000, t1), Ok(true));
        assert_eq!(q.position_ms(t1), 400_000);
        q.info(&id, Some(60_000), None, None, false, false, t1);
        // Voltar: a posição anda a partir do ponto novo.
        assert_eq!(q.seek(&id, 30_000, t1), Ok(true));
        assert_eq!(q.position_ms(t1 + Duration::from_secs(5)), 35_000);
        // Pausada, busca e continua pausada.
        assert!(q.pause(&id, true, t1));
        assert_eq!(q.seek(&id, 45_000, t1), Ok(true));
        assert!(q.is_paused());
        assert_eq!(q.position_ms(t1 + Duration::from_secs(60)), 45_000);
        assert!(q.pause(&id, false, t1));
        // Além do fim vira a duração; a fila avança 2 s depois.
        assert_eq!(q.seek(&id, 90_000, t1), Ok(true));
        assert_eq!(q.position_ms(t1), 60_000);
        assert!(!q.tick(t1 + Duration::from_millis(1_900)));
        assert!(q.tick(t1 + Duration::from_secs(2)));
        assert_eq!(atual_video(&q, t1).as_deref(), Some(B));
        // O item que saiu não muda mais nada.
        assert_eq!(q.seek(&id, 1_000, t1), Ok(false));
    }

    #[test]
    fn live_pausa_mas_nao_busca() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        let id = atual_id(&q, t0);
        q.info(&id, None, Some(true), None, false, false, t0);
        assert_eq!(q.seek(&id, 10_000, t0), Err(MusicError::LiveSeek));
        assert!(q.pause(&id, true, t0));
        assert!(q.is_paused());
    }

    #[test]
    fn remover_e_pular_direto_com_permissao() {
        let t0 = Instant::now();
        let mut q = MusicQueue::default();
        q.add(item("p_1", A, None, None), t0).unwrap();
        q.add(item("p_2", B, None, None), t0).unwrap();
        q.add(item("p_2", C, None, None), t0).unwrap();
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
