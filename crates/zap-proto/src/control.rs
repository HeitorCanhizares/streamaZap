//! Mensagens de controle (`docs/api/mensagens.md` e `relatorio` de `midia.md`).
//!
//! Campos opcionais: `#[serde(default, skip_serializing_if = "Option::is_none")]`.
use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayState {
    #[serde(rename = "sobrecarga")]
    Overuse,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "ociosa")]
    Underuse,
}

/// `relatorio`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// trilha, camada, seq, envio_us, chegada_us, bytes
    #[serde(rename = "chegadas", default)]
    pub arrivals: Vec<(u16, u8, u32, u32, u64, u32)>,
    /// cluster, seq, envio_us, chegada_us, bytes
    #[serde(rename = "sondas", default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<(u16, u32, u32, u64, u32)>,
    /// trilha, camada, seq
    #[serde(rename = "perdidos", default)]
    pub lost: Vec<(u16, u8, u32)>,
    #[serde(rename = "perda", default)]
    pub loss: f32,
    #[serde(rename = "banda_kbps", default, skip_serializing_if = "Option::is_none")]
    pub bandwidth_kbps: Option<u32>,
    #[serde(rename = "estado", default, skip_serializing_if = "Option::is_none")]
    pub state: Option<DelayState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_ms: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerRequestMsg {
    #[serde(rename = "camada")]
    pub layer: u8,
    pub kbps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackStats {
    #[serde(rename = "trilha_id")]
    pub track_id: String,
    #[serde(rename = "camada")]
    pub layer: u8,
    pub kbps: u32,
    pub fps: f32,
    #[serde(rename = "suavizacao_ms")]
    pub buffer_ms: u32,
    #[serde(rename = "perdidos")]
    pub lost: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    #[serde(rename = "ponte")]
    Relay,
    #[serde(rename = "direta")]
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum P2pRole {
    #[serde(rename = "cliente")]
    Client,
    #[serde(rename = "servidor")]
    Server,
}

/// Toda mensagem de controle; o campo `t` é o nome da doc.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum ControlMsg {
    // conexão e sessão
    #[serde(rename = "ping")]
    Ping { ts_ms: u64 },
    #[serde(rename = "pong")]
    Pong { ts_ms: u64 },
    #[serde(rename = "erro")]
    Error {
        #[serde(rename = "codigo")]
        code: String,
        #[serde(rename = "mensagem")]
        message: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    // aperto de mão
    #[serde(rename = "ola")]
    Hello {
        #[serde(rename = "protocolo")]
        protocol: u32,
        #[serde(rename = "versao_app")]
        app_version: String,
        #[serde(rename = "instalacao_id")]
        install_id: String,
        #[serde(rename = "nome")]
        name: String,
        #[serde(rename = "cor")]
        color: String,
        #[serde(rename = "ingresso", default, skip_serializing_if = "Option::is_none")]
        ticket: Option<String>,
        #[serde(rename = "sala_id", default, skip_serializing_if = "Option::is_none")]
        room_id: Option<String>,
        #[serde(rename = "previa", default, skip_serializing_if = "Option::is_none")]
        preview: Option<bool>,
        /// Conexão reversa (modo Local).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    #[serde(rename = "desafio")]
    Challenge {
        nonce: String,
        #[serde(rename = "acesso")]
        access: Access,
    },
    #[serde(rename = "autenticar")]
    Authenticate { hmac: String },
    #[serde(rename = "bem_vindo")]
    Welcome {
        #[serde(rename = "participante_id")]
        participant_id: String,
        #[serde(rename = "sala")]
        room: Room,
        #[serde(rename = "relogio_us", default, skip_serializing_if = "Option::is_none")]
        clock_us: Option<u64>,
        /// Base64 de 32 bytes; só no modo Local (`docs/api/mensagens.md`).
        #[serde(rename = "segredo_midia", default, skip_serializing_if = "Option::is_none")]
        media_secret: Option<String>,
    },
    #[serde(rename = "recusado")]
    Refused {
        #[serde(rename = "codigo")]
        code: String,
        #[serde(rename = "mensagem")]
        message: String,
    },
    // salas e participantes
    /// `salas` segue o formato de `GET /v1/salas` (JSON livre aqui).
    #[serde(rename = "salas.lista")]
    RoomList { salas: Vec<serde_json::Value> },
    #[serde(rename = "sala.entrar")]
    RoomJoin {
        #[serde(rename = "sala_id")]
        room_id: String,
    },
    #[serde(rename = "sala.sair")]
    RoomLeave {},
    #[serde(rename = "sala.estado")]
    RoomState {
        #[serde(rename = "sala")]
        room: Room,
        #[serde(rename = "participantes")]
        participants: Vec<Participant>,
        #[serde(rename = "trilhas")]
        tracks: Vec<Track>,
    },
    #[serde(rename = "sala.encerrada")]
    RoomClosed {
        #[serde(rename = "motivo")]
        reason: String,
    },
    #[serde(rename = "participante.entrou")]
    ParticipantJoined {
        #[serde(rename = "participante")]
        participant: Participant,
    },
    #[serde(rename = "participante.saiu")]
    ParticipantLeft {
        #[serde(rename = "participante_id")]
        participant_id: String,
        #[serde(rename = "motivo")]
        reason: String,
    },
    #[serde(rename = "participante.atualizou")]
    ParticipantUpdated {
        #[serde(rename = "participante")]
        participant: Participant,
    },
    #[serde(rename = "perfil.atualizar")]
    ProfileUpdate {
        #[serde(rename = "nome", default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(rename = "cor", default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    // chat
    #[serde(rename = "chat.enviar")]
    ChatSend {
        #[serde(rename = "texto")]
        text: String,
        #[serde(rename = "cliente_id")]
        client_id: String,
    },
    #[serde(rename = "chat.mensagem")]
    ChatMessage {
        id: String,
        #[serde(rename = "participante_id")]
        participant_id: String,
        #[serde(rename = "nome")]
        name: String,
        #[serde(rename = "cor")]
        color: String,
        #[serde(rename = "texto")]
        text: String,
        #[serde(rename = "em")]
        at: String,
        #[serde(rename = "cliente_id", default, skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
    },
    /// C→S sem campos; S→C com `participante_id`.
    #[serde(rename = "chat.digitando")]
    ChatTyping {
        #[serde(rename = "participante_id", default, skip_serializing_if = "Option::is_none")]
        participant_id: Option<String>,
    },
    // trilhas
    #[serde(rename = "trilha.publicar")]
    TrackPublish {
        local_id: String,
        #[serde(rename = "tipo")]
        kind: TrackKind,
        #[serde(rename = "titulo")]
        title: String,
        #[serde(rename = "camadas")]
        layers: Vec<LayerSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        audio: Option<AudioSpec>,
        #[serde(rename = "endereco", default, skip_serializing_if = "Option::is_none")]
        address: Option<MediaAddress>,
        /// Só trilha `camera`: canto ou solta (ausente = canto inferior direito).
        #[serde(rename = "camera", default, skip_serializing_if = "Option::is_none")]
        camera_layout: Option<CameraLayout>,
    },
    #[serde(rename = "trilha.publicada")]
    TrackPublished {
        #[serde(rename = "trilha")]
        track: Track,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        local_id: Option<String>,
    },
    #[serde(rename = "trilha.atualizar")]
    TrackUpdate {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "titulo", default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(rename = "camadas", default, skip_serializing_if = "Option::is_none")]
        layers: Option<Vec<LayerSpec>>,
        /// Troca o jeito da câmera ao vivo, sem republicar (ausente = não muda).
        #[serde(rename = "camera", default, skip_serializing_if = "Option::is_none")]
        camera_layout: Option<CameraLayout>,
    },
    #[serde(rename = "trilha.atualizada")]
    TrackUpdated {
        #[serde(rename = "trilha")]
        track: Track,
    },
    #[serde(rename = "trilha.remover")]
    TrackRemove {
        #[serde(rename = "trilha_id")]
        track_id: String,
    },
    #[serde(rename = "trilha.removida")]
    TrackRemoved {
        #[serde(rename = "trilha_id")]
        track_id: String,
    },
    #[serde(rename = "camadas.pedidas")]
    LayersRequested {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "camadas")]
        layers: Vec<LayerRequestMsg>,
    },
    /// S→C: com `camada`. C→S: só `trilha_id`.
    #[serde(rename = "keyframe.pedir")]
    KeyframeRequest {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "camada", default, skip_serializing_if = "Option::is_none")]
        layer: Option<u8>,
    },
    // assinaturas
    #[serde(rename = "assinar")]
    Subscribe {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "tamanho")]
        size: TileSize,
        #[serde(rename = "camada_max", default, skip_serializing_if = "Option::is_none")]
        max_layer: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        audio: Option<bool>,
    },
    #[serde(rename = "cancelar")]
    Unsubscribe {
        #[serde(rename = "trilha_id")]
        track_id: String,
    },
    #[serde(rename = "midia.rota")]
    MediaRoute {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "rota")]
        route: Route,
        #[serde(rename = "endereco", default, skip_serializing_if = "Option::is_none")]
        address: Option<MediaAddress>,
    },
    #[serde(rename = "camada.atual")]
    LayerCurrent {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "camada")]
        layer: u8,
        #[serde(rename = "largura")]
        width: u16,
        #[serde(rename = "altura")]
        height: u16,
        fps: u8,
        #[serde(rename = "motivo")]
        reason: String,
    },
    // rota direta e conexão reversa
    #[serde(rename = "p2p.meus_enderecos")]
    P2pMyAddresses {
        #[serde(rename = "enderecos")]
        addresses: Vec<String>,
        fingerprint: String,
    },
    #[serde(rename = "p2p.candidatos")]
    P2pCandidates {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "com")]
        with: String,
        #[serde(rename = "papel")]
        role: P2pRole,
        #[serde(rename = "enderecos")]
        addresses: Vec<String>,
        fingerprint: String,
    },
    #[serde(rename = "midia.direta_ok")]
    DirectOk {
        #[serde(rename = "trilha_id")]
        track_id: String,
    },
    #[serde(rename = "midia.direta_falhou")]
    DirectFailed {
        #[serde(rename = "trilha_id")]
        track_id: String,
    },
    #[serde(rename = "conexao.reversa")]
    ReverseConnection {
        #[serde(rename = "trilha_id")]
        track_id: String,
        #[serde(rename = "endereco")]
        address: String,
        token: String,
    },
    // estatísticas e relatório
    #[serde(rename = "estatisticas")]
    Stats {
        ping_ms: u32,
        #[serde(rename = "trilhas")]
        tracks: Vec<TrackStats>,
    },
    #[serde(rename = "relatorio")]
    Report(Report),
    // música da sala (docs/planos/musica-da-sala.md)
    /// Pôr um vídeo do YouTube na fila. Título e canal vêm de quem adiciona (oEmbed); faltando,
    /// o player completa depois com `musica.info`.
    #[serde(rename = "musica.adicionar")]
    MusicAdd {
        video_id: String,
        #[serde(rename = "titulo", default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(rename = "canal", default, skip_serializing_if = "Option::is_none")]
        channel: Option<String>,
    },
    /// Tirar um item da fila; o item que está tocando = pular direto (quem pôs ou o dono).
    #[serde(rename = "musica.remover")]
    MusicRemove { item_id: String },
    /// Voto para pular o item atual (`voto: false` desfaz).
    #[serde(rename = "musica.pular")]
    MusicSkip {
        item_id: String,
        #[serde(rename = "voto")]
        vote: bool,
    },
    /// O que o player de quem manda descobriu sobre o item.
    #[serde(rename = "musica.info")]
    MusicInfo {
        item_id: String,
        #[serde(rename = "duracao_ms", default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(rename = "ao_vivo", default, skip_serializing_if = "Option::is_none")]
        live: Option<bool>,
        #[serde(rename = "titulo", default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// O vídeo (ou a live) chegou ao fim no player.
        #[serde(rename = "terminou", default, skip_serializing_if = "std::ops::Not::not")]
        ended: bool,
        /// O player não consegue tocar o item (removido, privado, sem permissão de embed).
        #[serde(rename = "erro", default, skip_serializing_if = "std::ops::Not::not")]
        failed: bool,
    },
    /// Estado completo da música, para todos a cada mudança e para quem entra.
    #[serde(rename = "musica.estado")]
    MusicState {
        #[serde(rename = "fila")]
        queue: Vec<MusicItem>,
        #[serde(rename = "atual", default, skip_serializing_if = "Option::is_none")]
        current: Option<MusicNow>,
    },
}

/// Um vídeo da fila de música da sala.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicItem {
    pub id: String,
    /// Id do vídeo no YouTube (11 caracteres).
    pub video_id: String,
    #[serde(rename = "titulo")]
    pub title: String,
    #[serde(rename = "canal", default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(rename = "duracao_ms", default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "ao_vivo", default)]
    pub live: bool,
    /// `participante_id` de quem adicionou.
    #[serde(rename = "por")]
    pub added_by: String,
    #[serde(rename = "por_nome")]
    pub added_by_name: String,
}

/// O item tocando agora.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicNow {
    pub item: MusicItem,
    /// Posição da sala no instante do envio; negativa = ainda vai começar.
    #[serde(rename = "posicao_ms")]
    pub position_ms: i64,
    /// `participante_id` de quem votou para pular.
    #[serde(rename = "votos", default)]
    pub votes: Vec<String>,
    /// Votos que fazem pular (metade da sala, arredondando para cima).
    #[serde(rename = "precisa")]
    pub needed: u32,
}

#[cfg(test)]
pub(crate) fn roundtrip(json: &str) -> ControlMsg {
    let msg: ControlMsg = serde_json::from_str(json).expect("desserializa");
    let back = serde_json::to_value(&msg).expect("serializa");
    let orig: serde_json::Value = serde_json::from_str(json).expect("json");
    assert_eq!(back, orig, "round-trip difere");
    msg
}

#[cfg(test)]
mod tests_music;
#[cfg(test)]
mod tests_rooms; // tarefa proto-06
#[cfg(test)]
mod tests_session; // tarefa proto-05
#[cfg(test)]
mod tests_tracks; // tarefa proto-07
