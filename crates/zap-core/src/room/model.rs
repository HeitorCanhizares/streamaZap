//! Estado puro da sala (o dono é a fonte; os membros mantêm um espelho).

use std::collections::VecDeque;

use zap_proto::{AudioSpec, ControlMsg, LayerSpec, MediaAddress, Participant, Room, Track, TrackKind};

use crate::music::{MusicMirror, MusicQueue};
use crate::snapshot::{self, MusicView, RoomView, TrackView, TrackViewKind};
use crate::types::ChatLine;
use crate::util::{iso8601_from_ms, new_id, now_ms};
use crate::CoreError;

pub const CHAT_HISTORY: usize = 200;

/// Pedido de publicação de trilha (`trilha.publicar`).
#[derive(Clone, Debug)]
pub struct TrackSpec {
    pub local_id: String,
    pub kind: TrackKind,
    pub title: String,
    pub layers: Vec<LayerSpec>,
    pub audio: Option<AudioSpec>,
    pub address: Option<MediaAddress>,
    /// Só trilha `camera`: canto ou solta (`None` = canto inferior direito para quem assiste).
    pub camera_layout: Option<zap_proto::CameraLayout>,
}

pub struct RoomModel {
    pub room: Room,
    pub participants: Vec<Participant>,
    pub tracks: Vec<Track>,
    pub chat: VecDeque<ChatLine>,
    pub unread: u32,
    pub(crate) next_number: u16,
    /// Fila de música (só o dono a usa: ele é a fonte da verdade).
    pub music: MusicQueue,
    /// Último `musica.estado` recebido do dono (só nos membros).
    pub music_mirror: Option<MusicMirror>,
}

/// Próximo número livre: maior número existente + 1 (1 se não há trilhas).
/// Satura em `u16::MAX`; `add_track` recusa emitir quando chega lá, então nada repete.
fn proximo_numero(tracks: &[Track]) -> u16 {
    tracks.iter().map(|t| t.number).max().map_or(1, |m| m.saturating_add(1))
}

impl RoomModel {
    /// Sala recém-criada: só o dono, sem trilhas, `next_number = 1`.
    pub fn new(room: Room, owner: Participant) -> Self {
        Self {
            room,
            participants: vec![owner],
            tracks: Vec::new(),
            chat: VecDeque::new(),
            unread: 0,
            next_number: 1,
            music: MusicQueue::default(),
            music_mirror: None,
        }
    }
    /// Espelho de um membro a partir de `bem_vindo`/`sala.estado`.
    pub fn from_state(room: Room, participants: Vec<Participant>, tracks: Vec<Track>) -> Self {
        let next_number = proximo_numero(&tracks);
        Self {
            room,
            participants,
            tracks,
            chat: VecDeque::new(),
            unread: 0,
            next_number,
            music: MusicQueue::default(),
            music_mirror: None,
        }
    }
    /// Substitui participantes e trilhas (mantém o chat).
    /// `next_number` nunca volta atrás: números já usados nesta sala não são reaproveitados.
    pub fn replace_state(&mut self, room: Room, participants: Vec<Participant>, tracks: Vec<Track>) {
        self.room = room;
        self.participants = participants;
        self.tracks = tracks;
        self.next_number = self.next_number.max(proximo_numero(&self.tracks));
    }
    pub fn participant(&self, id: &str) -> Option<&Participant> {
        self.participants.iter().find(|p| p.id == id)
    }
    /// Insere ou substitui (chave: id).
    pub fn upsert_participant(&mut self, p: Participant) {
        match self.participants.iter().position(|x| x.id == p.id) {
            Some(i) => self.participants[i] = p,
            None => self.participants.push(p),
        }
    }
    pub fn remove_participant(&mut self, id: &str) -> Option<Participant> {
        let i = self.participants.iter().position(|p| p.id == id)?;
        Some(self.participants.remove(i))
    }
    /// Regras de `mensagens.md`: no máximo uma trilha `tela` OU `janela`, uma `camera` e uma
    /// `audio` por participante (violação → `CoreError::Invalid`). `numero` = próximo u16
    /// sequencial (começa em 1, nunca reutiliza). `id` = `new_id("t_")`.
    pub fn add_track(&mut self, participant_id: &str, spec: TrackSpec) -> Result<Track, CoreError> {
        if self.participant(participant_id).is_none() {
            return Err(CoreError::Invalid(format!(
                "participante inexistente: {participant_id}"
            )));
        }
        // Checa tudo antes de mexer em `next_number`, para um erro não gastar número.
        let kinds: Vec<TrackKind> = self
            .tracks
            .iter()
            .filter(|t| t.participant_id == participant_id)
            .map(|t| t.kind)
            .collect();
        let tem_tela_ou_janela = kinds.iter().any(|k| matches!(k, TrackKind::Screen | TrackKind::Window));
        match spec.kind {
            TrackKind::Screen | TrackKind::Window if tem_tela_ou_janela => {
                return Err(CoreError::Invalid("já existe tela ou janela publicada".into()));
            }
            TrackKind::Camera if kinds.contains(&TrackKind::Camera) => {
                return Err(CoreError::Invalid("já existe câmera publicada".into()));
            }
            TrackKind::Audio if kinds.contains(&TrackKind::Audio) => {
                return Err(CoreError::Invalid("já existe áudio publicado".into()));
            }
            _ => {}
        }
        // Sem próximo número (u16 esgotado): recusa em vez de repetir um número antigo.
        if self.next_number == u16::MAX {
            return Err(CoreError::Invalid("números de trilha esgotados".into()));
        }
        let number = self.next_number;
        self.next_number += 1;
        let track = Track {
            id: new_id("t_"),
            participant_id: participant_id.to_string(),
            number,
            kind: spec.kind,
            title: spec.title,
            layers: spec.layers,
            audio: spec.audio,
            address: spec.address,
            camera_layout: if spec.kind == TrackKind::Camera {
                spec.camera_layout
            } else {
                None
            },
        };
        self.tracks.push(track.clone());
        Ok(track)
    }
    /// Atualiza título/camadas/jeito da câmera (`None` = não muda); `None` se a trilha não existe.
    /// O jeito da câmera só vale para trilha `camera`.
    pub fn update_track(
        &mut self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<LayerSpec>>,
        camera_layout: Option<zap_proto::CameraLayout>,
    ) -> Option<Track> {
        let t = self.tracks.iter_mut().find(|t| t.id == track_id)?;
        if let Some(titulo) = title {
            t.title = titulo;
        }
        if let Some(camadas) = layers {
            t.layers = camadas;
        }
        if camera_layout.is_some() && t.kind == TrackKind::Camera {
            t.camera_layout = camera_layout;
        }
        Some(t.clone())
    }
    /// Insere ou substitui por id (lado membro).
    pub fn upsert_track(&mut self, t: Track) {
        match self.tracks.iter().position(|x| x.id == t.id) {
            Some(i) => self.tracks[i] = t,
            None => self.tracks.push(t),
        }
    }
    pub fn remove_track(&mut self, track_id: &str) -> Option<Track> {
        let i = self.tracks.iter().position(|t| t.id == track_id)?;
        Some(self.tracks.remove(i))
    }
    /// Remove todas as trilhas do participante e devolve os ids removidos.
    pub fn remove_tracks_of(&mut self, participant_id: &str) -> Vec<String> {
        let mut ids = Vec::new();
        self.tracks.retain(|t| {
            if t.participant_id == participant_id {
                ids.push(t.id.clone());
                false
            } else {
                true
            }
        });
        ids
    }
    pub fn track_by_id(&self, id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }
    /// Guarda a mensagem e descarta as mais antigas além de `CHAT_HISTORY`.
    pub fn push_chat(&mut self, line: ChatLine) {
        self.chat.push_back(line);
        while self.chat.len() > CHAT_HISTORY {
            self.chat.pop_front();
        }
    }
    /// `sala.estado` completo.
    pub fn state_msg(&self) -> ControlMsg {
        ControlMsg::RoomState {
            room: self.room.clone(),
            participants: self.participants.clone(),
            tracks: self.tracks.clone(),
        }
    }
    /// `RoomView` do snapshot: `ping_ms` = `latency_ms.unwrap_or(0)`; `sharing` = tem trilha
    /// tela/janela; `camera` = tem trilha camera; `tracks` só com vídeo (tela→Screen,
    /// janela→Window, camera→Camera), `chat` = o histórico, `id`/`name`/`mode` da sala.
    pub fn to_view(&self) -> RoomView {
        RoomView {
            id: self.room.id.clone(),
            name: self.room.name.clone(),
            mode: self.room.mode,
            code: self.room.code.as_ref().map(|c| c.to_string()),
            participants: self
                .participants
                .iter()
                .map(|p| visao_participante(&self.tracks, p))
                .collect(),
            tracks: self
                .tracks
                .iter()
                .map(|t| TrackView {
                    id: t.id.clone(),
                    owner_id: t.participant_id.clone(),
                    kind: vista_de(t.kind),
                    title: t.title.clone(),
                    number: t.number,
                    camera_layout: t.camera_layout,
                })
                .collect(),
            chat: self.chat.iter().cloned().collect(),
            unread: self.unread,
            me_id: String::new(),
            owner_id: self.room.owner_id.clone(),
            music: None,
        }
    }
    /// Música para a interface: o dono lê a própria fila; o membro, o último `musica.estado`
    /// (com a posição andada desde que chegou). `None` = sem música (sala Online, ou o dono tem
    /// um app que ainda não manda `musica.estado`).
    pub fn music_view(&self, my_id: &str, agora: std::time::Instant) -> Option<MusicView> {
        if let Some(m) = &self.music_mirror {
            return Some(m.view(agora));
        }
        let sou_dono = self.room.mode == zap_proto::RoomMode::Local && self.room.owner_id == my_id;
        sou_dono.then(|| MusicMirror::from_msg(&self.music.msg(self.participants.len(), agora), agora).view(agora))
    }
}

/// Participante na visão do snapshot: ping, e se ele tem tela/janela ou câmera publicada.
/// `volume`/`muted` são escolhas de quem vê, não da sala: ficam ausentes.
fn visao_participante(tracks: &[Track], p: &Participant) -> snapshot::Participant {
    let donas = || tracks.iter().filter(|t| t.participant_id == p.id);
    snapshot::Participant {
        id: p.id.clone(),
        name: p.name.clone(),
        color: p.color.clone(),
        ping_ms: p.latency_ms.unwrap_or(0),
        sharing: donas().any(|t| matches!(t.kind, TrackKind::Screen | TrackKind::Window)),
        camera: donas().any(|t| t.kind == TrackKind::Camera),
        volume: None,
        muted: None,
    }
}

/// Tipo da trilha na visão (o áudio aparece para a interface achar quem compartilha só o som).
fn vista_de(kind: TrackKind) -> TrackViewKind {
    match kind {
        TrackKind::Screen => TrackViewKind::Screen,
        TrackKind::Window => TrackViewKind::Window,
        TrackKind::Camera => TrackViewKind::Camera,
        TrackKind::Audio => TrackViewKind::Audio,
    }
}

/// `chat.mensagem` a partir de uma linha (`at` = ISO 8601 de `line.ts`).
pub fn chat_message_msg(line: &ChatLine, client_id: Option<String>) -> ControlMsg {
    ControlMsg::ChatMessage {
        id: line.id.clone(),
        participant_id: line.author_id.clone(),
        name: line.author.clone(),
        color: line.color.clone(),
        text: line.text.clone(),
        at: iso8601_from_ms(line.ts),
        client_id,
    }
}

/// Inverso: `chat.mensagem` → `ChatLine` (`ts` = ms Unix de `at`; se não der para ler, agora).
pub fn chat_line_from_msg(msg: &ControlMsg) -> Option<ChatLine> {
    match msg {
        ControlMsg::ChatMessage {
            id,
            participant_id,
            name,
            color,
            text,
            at,
            ..
        } => Some(ChatLine {
            id: id.clone(),
            author_id: participant_id.clone(),
            author: name.clone(),
            color: color.clone(),
            text: text.clone(),
            ts: parse_iso_ms(at).unwrap_or_else(now_ms),
        }),
        _ => None,
    }
}

/// Lê `YYYY-MM-DDTHH:MM:SSZ` (o formato de `iso8601_from_ms`) para ms Unix. `None` se não bate.
fn parse_iso_ms(texto: &str) -> Option<u64> {
    let corpo = texto.strip_suffix('Z')?;
    let (data, hora) = corpo.split_once('T')?;
    let data: Vec<i64> = data.split('-').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let hora: Vec<i64> = hora.split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (&[ano, mes, dia], &[h, mi, s]) = (data.as_slice(), hora.as_slice()) else {
        return None;
    };
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return None;
    }
    if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&s) {
        return None;
    }
    // Dias desde 1970-01-01 (algoritmo `days_from_civil` de Howard Hinnant, domínio público).
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = if mes > 2 { mes - 3 } else { mes + 9 };
    let doy = (153 * mp + 2) / 5 + dia - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    let segundos = dias * 86_400 + h * 3600 + mi * 60 + s;
    u64::try_from(segundos).ok()?.checked_mul(1000)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zap_proto::{Access, Role, RoomMode};

    fn sala() -> Room {
        Room {
            id: "s_1".into(),
            code: None,
            name: "Corrida".into(),
            owner_id: "p_1".into(),
            access: Access::Open,
            mode: RoomMode::Online,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn participante(id: &str) -> Participant {
        Participant {
            id: id.into(),
            name: format!("Pessoa {id}"),
            color: "#ff0000".into(),
            role: Role::Member,
            joined_at: "2026-01-01T00:00:00Z".into(),
            app_version: "1.0.0".into(),
            protocol: 1,
            latency_ms: None,
            watching: None,
        }
    }

    fn spec(kind: TrackKind) -> TrackSpec {
        TrackSpec {
            local_id: "local".into(),
            kind,
            title: "Trilha".into(),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    fn camada(layer: u8) -> LayerSpec {
        LayerSpec {
            layer,
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 4000,
        }
    }

    fn modelo() -> RoomModel {
        RoomModel::new(sala(), participante("p_1"))
    }

    fn linha(id: &str, ts: u64) -> ChatLine {
        ChatLine {
            id: id.into(),
            author_id: "p_1".into(),
            author: "Ana".into(),
            color: "#00ff00".into(),
            text: format!("texto {id}"),
            ts,
        }
    }

    fn track_de(id: &str, owner: &str, number: u16, kind: TrackKind) -> Track {
        Track {
            id: id.into(),
            participant_id: owner.into(),
            number,
            kind,
            title: format!("t{number}"),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    #[test]
    fn numeros_sequenciais_e_nunca_reutilizados() {
        let mut m = modelo();
        let a = m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        let b = m.add_track("p_1", spec(TrackKind::Camera)).unwrap();
        let c = m.add_track("p_1", spec(TrackKind::Audio)).unwrap();
        assert_eq!((a.number, b.number, c.number), (1, 2, 3));
        assert!(a.id.starts_with("t_"));
        // Remover a trilha 3 não libera o número 3.
        m.remove_track(&c.id).unwrap();
        let d = m.add_track("p_1", spec(TrackKind::Audio)).unwrap();
        assert_eq!(d.number, 4);
    }

    #[test]
    fn tela_e_janela_se_excluem() {
        let mut m = modelo();
        m.upsert_participant(participante("p_2"));
        m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        assert!(matches!(
            m.add_track("p_1", spec(TrackKind::Window)),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            m.add_track("p_1", spec(TrackKind::Screen)),
            Err(CoreError::Invalid(_))
        ));
        // Outro participante pode ter a sua janela.
        assert!(m.add_track("p_2", spec(TrackKind::Window)).is_ok());
        // Erro não gasta número.
        assert_eq!(m.next_number, 3);
    }

    #[test]
    fn uma_camera_e_um_audio_por_participante() {
        let mut m = modelo();
        m.add_track("p_1", spec(TrackKind::Camera)).unwrap();
        assert!(matches!(
            m.add_track("p_1", spec(TrackKind::Camera)),
            Err(CoreError::Invalid(_))
        ));
        m.add_track("p_1", spec(TrackKind::Audio)).unwrap();
        assert!(matches!(
            m.add_track("p_1", spec(TrackKind::Audio)),
            Err(CoreError::Invalid(_))
        ));
        // Câmera e tela podem coexistir.
        m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        assert_eq!(m.tracks.len(), 3);
    }

    #[test]
    fn participante_inexistente_erro() {
        let mut m = modelo();
        assert!(matches!(
            m.add_track("p_fantasma", spec(TrackKind::Camera)),
            Err(CoreError::Invalid(_))
        ));
        assert!(m.tracks.is_empty());
    }

    #[test]
    fn remove_tracks_of_devolve_ids() {
        let mut m = modelo();
        m.upsert_participant(participante("p_2"));
        let a = m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        let b = m.add_track("p_1", spec(TrackKind::Audio)).unwrap();
        let outra = m.add_track("p_2", spec(TrackKind::Camera)).unwrap();
        let ids = m.remove_tracks_of("p_1");
        assert_eq!(ids, vec![a.id, b.id]);
        assert_eq!(m.tracks.len(), 1);
        assert_eq!(m.tracks[0].id, outra.id);
        assert!(m.remove_tracks_of("p_1").is_empty());
    }

    #[test]
    fn chat_limite_200() {
        let mut m = modelo();
        for i in 0..201u64 {
            m.push_chat(linha(&format!("c{i}"), i * 1000));
        }
        assert_eq!(m.chat.len(), CHAT_HISTORY);
        assert_eq!(m.chat.front().unwrap().id, "c1");
        assert_eq!(m.chat.back().unwrap().id, "c200");
    }

    #[test]
    fn state_msg_contem_tudo() {
        let mut m = modelo();
        m.upsert_participant(participante("p_2"));
        m.add_track("p_2", spec(TrackKind::Camera)).unwrap();
        match m.state_msg() {
            ControlMsg::RoomState {
                room,
                participants,
                tracks,
            } => {
                assert_eq!(room.id, "s_1");
                assert_eq!(participants.len(), 2);
                assert_eq!(tracks.len(), 1);
                assert_eq!(tracks[0].participant_id, "p_2");
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
    }

    #[test]
    fn to_view_flags_sharing_camera_e_ping() {
        let mut m = modelo();
        let mut p2 = participante("p_2");
        p2.latency_ms = Some(42);
        m.upsert_participant(p2);
        m.add_track("p_2", spec(TrackKind::Screen)).unwrap();
        m.add_track("p_2", spec(TrackKind::Camera)).unwrap();
        m.add_track("p_2", spec(TrackKind::Audio)).unwrap();
        m.unread = 3;
        m.push_chat(linha("c1", 1_767_225_600_000));
        let v = m.to_view();
        assert_eq!(v.id, "s_1");
        assert_eq!(v.mode, RoomMode::Online);
        // Tela, câmera e áudio aparecem com o tipo certo.
        let kinds: Vec<TrackViewKind> = v.tracks.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![TrackViewKind::Screen, TrackViewKind::Camera, TrackViewKind::Audio]
        );
        assert!(v.tracks.iter().all(|t| t.owner_id == "p_2"));
        // Latência (ping) do participante preservada.
        let p = v.participants.iter().find(|p| p.id == "p_2").unwrap();
        assert_eq!(p.ping_ms, 42);
        assert!(p.sharing);
        assert!(p.camera);
        let outro = v.participants.iter().find(|p| p.id == "p_1").unwrap();
        assert_eq!((outro.ping_ms, outro.sharing, outro.camera), (0, false, false));
        assert_eq!(v.chat.len(), 1);
        assert_eq!(v.unread, 3);
    }

    #[test]
    fn from_state_continua_numeracao() {
        let tracks = vec![
            track_de("t_a", "p_1", 5, TrackKind::Screen),
            track_de("t_b", "p_1", 2, TrackKind::Audio),
        ];
        let mut m = RoomModel::from_state(sala(), vec![participante("p_1")], tracks);
        let nova = m.add_track("p_1", spec(TrackKind::Camera)).unwrap();
        assert_eq!(nova.number, 6);
        // Estado vazio começa em 1.
        let vazio = RoomModel::from_state(sala(), Vec::new(), Vec::new());
        assert_eq!(vazio.next_number, 1);
    }

    #[test]
    fn replace_state_nao_reaproveita_numero() {
        let mut m = modelo();
        m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        m.add_track("p_1", spec(TrackKind::Audio)).unwrap();
        // Estado novo sem trilhas: o número 3 não volta.
        m.replace_state(sala(), vec![participante("p_1")], Vec::new());
        let t = m.add_track("p_1", spec(TrackKind::Camera)).unwrap();
        assert_eq!(t.number, 3);
    }

    #[test]
    fn chat_msg_ida_e_volta() {
        let l = linha("c_1", 1_767_225_661_000);
        let msg = chat_message_msg(&l, Some("cli_9".into()));
        match &msg {
            ControlMsg::ChatMessage { at, client_id, .. } => {
                assert_eq!(at, "2026-01-01T00:01:01Z");
                assert_eq!(client_id.as_deref(), Some("cli_9"));
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
        assert_eq!(chat_line_from_msg(&msg), Some(l));
        // Outra mensagem não vira linha de chat.
        assert_eq!(chat_line_from_msg(&ControlMsg::RoomLeave {}), None);
    }

    #[test]
    fn chat_at_invalido_usa_agora() {
        let msg = ControlMsg::ChatMessage {
            id: "c".into(),
            participant_id: "p".into(),
            name: "n".into(),
            color: "#000".into(),
            text: "x".into(),
            at: "ontem".into(),
            client_id: None,
        };
        let antes = now_ms();
        let l = chat_line_from_msg(&msg).unwrap();
        assert!(l.ts >= antes && l.ts <= now_ms());
    }

    #[test]
    fn update_track_muda_camadas() {
        let mut m = modelo();
        let t = m.add_track("p_1", spec(TrackKind::Screen)).unwrap();
        let novo = m
            .update_track(&t.id, None, Some(vec![camada(0), camada(1)]), None)
            .unwrap();
        assert_eq!(novo.layers.len(), 2);
        assert_eq!(novo.title, "Trilha");
        let novo = m.update_track(&t.id, Some("Nova".into()), None, None).unwrap();
        assert_eq!(novo.title, "Nova");
        assert_eq!(novo.layers.len(), 2);
        assert_eq!(m.track_by_id(&t.id).unwrap().layers.len(), 2);
        assert!(m.update_track("t_nao_existe", None, None, None).is_none());
    }

    #[test]
    fn jeito_da_camera_guardado_e_trocado_ao_vivo() {
        use zap_proto::{CameraLayout, Corner};
        let mut m = modelo();
        let canto = CameraLayout::Corner {
            corner: Corner::TopLeft,
        };
        let cam = m
            .add_track(
                "p_1",
                TrackSpec {
                    camera_layout: Some(canto),
                    ..spec(TrackKind::Camera)
                },
            )
            .unwrap();
        assert_eq!(cam.camera_layout, Some(canto));
        // Só o jeito muda: título e camadas ficam.
        let novo = m.update_track(&cam.id, None, None, Some(CameraLayout::Free)).unwrap();
        assert_eq!(novo.camera_layout, Some(CameraLayout::Free));
        assert_eq!(novo.title, "Trilha");
        // `None` em `trilha.atualizar` não apaga o jeito.
        let igual = m.update_track(&cam.id, Some("Câmera".into()), None, None).unwrap();
        assert_eq!(igual.camera_layout, Some(CameraLayout::Free));
    }

    #[test]
    fn jeito_da_camera_ignorado_fora_da_camera() {
        let mut m = modelo();
        let tela = m
            .add_track(
                "p_1",
                TrackSpec {
                    camera_layout: Some(zap_proto::CameraLayout::Free),
                    ..spec(TrackKind::Screen)
                },
            )
            .unwrap();
        assert_eq!(tela.camera_layout, None);
        let novo = m
            .update_track(&tela.id, None, None, Some(zap_proto::CameraLayout::Free))
            .unwrap();
        assert_eq!(novo.camera_layout, None);
    }

    #[test]
    fn room_view_expoe_camera_layout_da_camera() {
        use zap_proto::CameraLayout;
        let mut m = modelo();
        let cam = m
            .add_track(
                "p_1",
                TrackSpec {
                    camera_layout: Some(CameraLayout::Free),
                    ..spec(TrackKind::Camera)
                },
            )
            .unwrap();
        let tela = m
            .add_track(
                "p_1",
                TrackSpec {
                    camera_layout: Some(CameraLayout::Free),
                    ..spec(TrackKind::Screen)
                },
            )
            .unwrap();
        let view = m.to_view();
        let v_cam = view.tracks.iter().find(|t| t.id == cam.id).unwrap();
        let v_tela = view.tracks.iter().find(|t| t.id == tela.id).unwrap();
        assert_eq!(v_cam.camera_layout, Some(CameraLayout::Free));
        assert_eq!(v_tela.camera_layout, None);
    }

    #[test]
    fn room_view_camera_sem_campo_fica_none() {
        let mut m = modelo();
        let cam = m.add_track("p_1", spec(TrackKind::Camera)).unwrap();
        let view = m.to_view();
        let v_cam = view.tracks.iter().find(|t| t.id == cam.id).unwrap();
        assert_eq!(v_cam.camera_layout, None);
    }
}
