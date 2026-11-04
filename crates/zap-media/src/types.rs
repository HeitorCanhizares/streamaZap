use bytes::Bytes;
use zap_proto::LayerSpec;

/// Um quadro de vídeo codificado de uma camada.
#[derive(Clone, Debug)]
pub struct EncodedFrame {
    pub track: u16,
    pub layer: u8,
    /// Contador por camada, começa em 0 e cresce 1 por quadro.
    pub seq: u32,
    pub keyframe: bool,
    /// O dado traz SPS/PPS (NAL 7/8).
    pub has_params: bool,
    pub width: u16,
    pub height: u16,
    pub capture_us: u64,
    /// H.264 Annex B.
    pub data: Bytes,
}

/// Um quadro de áudio Opus de 20 ms.
#[derive(Clone, Debug)]
pub struct EncodedAudio {
    pub track: u16,
    pub seq: u32,
    pub capture_us: u64,
    pub data: Bytes,
}

/// Monitor a capturar: índice 1.. de `zap_capture::MonitorInfo::index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorRef(pub u32);

#[derive(Clone, Debug)]
pub enum VideoSourceSpec {
    Monitor(MonitorRef),
    /// HWND como `isize`.
    Window(isize),
    /// Id (symbolic link) de `zap_capture::CameraInfo`.
    Camera(String),
    /// Gera quadros codificados falsos (todas as plataformas).
    Synthetic {
        width: u32,
        height: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EncoderChoiceSpec {
    #[default]
    Auto,
    Hardware,
    Software,
}

#[derive(Clone, Debug)]
pub struct VideoPipelineConfig {
    pub track: u16,
    pub source: VideoSourceSpec,
    pub ladder: Vec<LayerSpec>,
    pub encoder: EncoderChoiceSpec,
}

/// Média e p95 de uma etapa, em microssegundos.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StageStats {
    pub avg_us: f64,
    pub p95_us: f64,
}

#[derive(Clone, Debug, Default)]
pub struct LayerStats {
    pub layer: u8,
    /// Camada ligada por `set_requests`.
    pub enabled: bool,
    /// Quadros codificados por segundo (janela de ~1 s).
    pub fps: f32,
    /// Bitrate real saído, kbps (janela de ~1 s).
    pub kbps: f32,
    pub frames: u64,
    pub convert: StageStats,
    pub encode: StageStats,
}

#[derive(Clone, Debug, Default)]
pub struct PipelineStats {
    pub layers: Vec<LayerStats>,
    /// Captura -> chegada na thread de vídeo.
    pub capture: StageStats,
    /// Quadros de captura descartados pelo slot "o mais novo vence".
    pub dropped_capture: u64,
    /// Soma dos descartes dos codificadores (fila cheia).
    pub dropped_encode: u64,
    /// Nome do codificador em uso ("synthetic" no backend sintético).
    pub encoder_name: String,
}
