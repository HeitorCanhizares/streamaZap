//! Tipos de pedido e de dados trocados com a interface (espelham `app/src/bridge/types.ts`).

use bytes::Bytes;
use serde::{Deserialize, Serialize};
pub use zap_proto::{RoomMode, TileSize};

/// Identificador de um tile na interface (escolhido por ela).
pub type TileId = u32;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    #[serde(rename = "720p30")]
    #[default]
    P720p30,
    #[serde(rename = "720p60")]
    P720p60,
    #[serde(rename = "1080p30")]
    P1080p30,
    #[serde(rename = "1080p60")]
    P1080p60,
}

impl Preset {
    /// (altura, fps) do preset: 720/1080 e 30/60.
    pub fn height_fps(self) -> (u32, u32) {
        match self {
            Preset::P720p30 => (720, 30),
            Preset::P720p60 => (720, 60),
            Preset::P1080p30 => (1080, 30),
            Preset::P1080p60 => (1080, 60),
        }
    }

    pub fn to_rate(self) -> zap_rate::ladder::Preset {
        use zap_rate::ladder::Preset as R;
        match self {
            Preset::P720p30 => R::P720p30,
            Preset::P720p60 => R::P720p60,
            Preset::P1080p30 => R::P1080p30,
            Preset::P1080p60 => R::P1080p60,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EncoderPref {
    #[default]
    Auto,
    Hardware,
    Software,
}

impl EncoderPref {
    pub fn to_spec(self) -> zap_media::EncoderChoiceSpec {
        use zap_media::EncoderChoiceSpec as S;
        match self {
            EncoderPref::Auto => S::Auto,
            EncoderPref::Hardware => S::Hardware,
            EncoderPref::Software => S::Software,
        }
    }
    /// "auto" | "hardware" | "software"; qualquer outro valor vira `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "hardware" => Some(Self::Hardware),
            "software" => Some(Self::Software),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Estavel,
    Beta,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Screen,
    Window,
    Camera,
    App,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SourceItem {
    pub id: String,
    pub kind: SourceKind,
    pub name: String,
    /// Só nos apps (`app:<pid>`): o que se sabe do som dele agora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<AppSoundView>,
    /// Só nas câmeras: altura do melhor modo nativo (a interface desliga o que a câmera não faz).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<u32>,
    /// Só nas câmeras: fps do melhor modo nativo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fps: Option<u32>,
}

/// Som de um app na listagem: tocando agora, já abriu o áudio (quieto) ou só tem janela.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppSoundView {
    Playing,
    Ready,
    Silent,
}

/// Fontes disponíveis. Ids: `screen:<indice>`, `window:<hwnd>`, `camera:<id>`, `app:<pid>`,
/// `synthetic:<L>x<A>` (backend sintético, qualquer plataforma).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Sources {
    pub screens: Vec<SourceItem>,
    pub windows: Vec<SourceItem>,
    pub cameras: Vec<SourceItem>,
    pub apps: Vec<SourceItem>,
    /// "auto" | "hardware" | "software".
    pub encoders: Vec<String>,
}

/// Fonte de vídeo já interpretada (ver `sources::parse_source`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRef {
    Monitor(u32),
    Window(isize),
    Camera(String),
    Synthetic { width: u32, height: u32 },
}

/// Imagem RGBA8 (`data.len() == width * height * 4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CreateRoom {
    pub name: String,
    pub mode: RoomMode,
    pub password: Option<String>,
}

/// `"none"`, `"all"` ou lista de ids `app:<pid>`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum AudioChoice {
    Keyword(String),
    Apps(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ShareRequest {
    pub source: String,
    pub audio: AudioChoice,
    pub preset: Preset,
    /// "auto" | "hardware" | "software".
    pub encoder: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum JoinTarget {
    Discovered {
        room_id: String,
    },
    Address {
        ip: String,
        port: u16,
    },
    /// Sala Online pelo código (`ZAP-XXXX`; maiúsculas/minúsculas e espaços são tolerados).
    Code {
        code: String,
    },
}

/// Mensagem de chat entregue à interface (`UiSink::chat`) e guardada no `RoomView`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub id: String,
    pub author_id: String,
    pub author: String,
    pub color: String,
    pub text: String,
    /// Milissegundos desde a época Unix.
    pub ts: u64,
}

/// Quadro de vídeo no instante de exibir.
#[derive(Clone, Debug)]
pub struct VideoPacket {
    /// Número da trilha (`Track::number`).
    pub track: u16,
    pub layer: u8,
    pub keyframe: bool,
    pub width: u16,
    pub height: u16,
    pub seq: u32,
    pub capture_us: u64,
    /// H.264 Annex B.
    pub data: Bytes,
}

impl VideoPacket {
    pub const WIRE_HEADER_LEN: usize = 24;

    /// Formato para o JS (little-endian): `u16 trilha, u8 camada, u8 flags (bit0 keyframe),
    /// u16 largura, u16 altura, u32 seq, u64 captura_us, u32 reservado(0)` + `data`.
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_HEADER_LEN + self.data.len());
        out.extend_from_slice(&self.track.to_le_bytes());
        out.push(self.layer);
        // bit0 = keyframe; os demais bits ficam reservados.
        out.push(u8::from(self.keyframe));
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out.extend_from_slice(&self.capture_us.to_le_bytes());
        // Campo reservado de 4 bytes, sempre zero.
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pacote(keyframe: bool, data: Vec<u8>) -> VideoPacket {
        VideoPacket {
            track: 0x0102,
            layer: 2,
            keyframe,
            width: 1920,
            height: 1080,
            seq: 7,
            capture_us: 0x1122_3344_5566_7788,
            data: Bytes::from(data),
        }
    }

    #[test]
    fn to_wire_cabecalho() {
        let bytes = pacote(true, vec![1, 2, 3]).to_wire();
        assert_eq!(bytes.len(), 27);
        assert_eq!(&bytes[0..2], &[0x02, 0x01]);
        assert_eq!(bytes[2], 2);
        assert_eq!(bytes[3], 1);
        assert_eq!(&bytes[4..6], &1920u16.to_le_bytes());
        assert_eq!(&bytes[6..8], &1080u16.to_le_bytes());
        assert_eq!(&bytes[8..12], &7u32.to_le_bytes());
        assert_eq!(&bytes[12..20], &0x1122_3344_5566_7788u64.to_le_bytes());
        assert_eq!(&bytes[20..24], &[0, 0, 0, 0]);
        assert_eq!(&bytes[24..], &[1, 2, 3]);
    }

    #[test]
    fn to_wire_delta_flags_zero() {
        let bytes = pacote(false, vec![9]).to_wire();
        assert_eq!(bytes[3], 0);
    }

    #[test]
    fn preset_serde() {
        let json = serde_json::to_string(&Preset::P1080p60).unwrap();
        assert_eq!(json, "\"1080p60\"");

        let none: AudioChoice = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(none, AudioChoice::Keyword("none".into()));
        let all: AudioChoice = serde_json::from_str("\"all\"").unwrap();
        assert_eq!(all, AudioChoice::Keyword("all".into()));
        let apps: AudioChoice = serde_json::from_str("[\"app:12\"]").unwrap();
        assert_eq!(apps, AudioChoice::Apps(vec!["app:12".into()]));

        let sala: JoinTarget = serde_json::from_str("{\"room_id\":\"s_1\"}").unwrap();
        assert_eq!(sala, JoinTarget::Discovered { room_id: "s_1".into() });
        let end: JoinTarget = serde_json::from_str("{\"ip\":\"1.2.3.4\",\"port\":47800}").unwrap();
        assert_eq!(
            end,
            JoinTarget::Address {
                ip: "1.2.3.4".into(),
                port: 47800
            }
        );
    }
}
