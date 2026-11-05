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

/// Onde converter para NV12.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConversionSpec {
    /// GPU (Video Processor) quando der; senão CPU.
    #[default]
    Auto,
    /// Só GPU: sem Video Processor a camada não liga (tenta de novo a cada 2 s, como hoje).
    Gpu,
    /// Só CPU (testes e bancada).
    Cpu,
}

/// Conversor em uso numa camada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConverterKind {
    Gpu,
    Cpu,
}

/// Placa de vídeo vista pela sonda do Diagnóstico (`probe_video_adapters`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoAdapter {
    pub name: String,
    /// O Video Processor do D3D11 converte BGRA para NV12 nesta placa. Sem ele, a conversão
    /// vai pela CPU (`ConversionSpec::Auto`).
    pub video_processor: bool,
}

/// Opções do pipeline que o núcleo não precisa conhecer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoPipelineOptions {
    pub conversion: ConversionSpec,
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
    /// Tempo de GPU do `VideoProcessorBlt` (consulta de tempo).
    pub convert_gpu: StageStats,
    /// Espera pelo lock do dispositivo antes da conversão.
    pub lock_wait: StageStats,
    /// Conversor da camada; `None` = desligada (ou backend sintético).
    pub converter: Option<ConverterKind>,
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
    /// Quadros com imagem nova entregues pela captura no último segundo.
    pub capture_new_fps: f32,
    /// Leitura da origem para a CPU: do `CopyResource` ao `Map` (µs). Vazio sem camada na CPU.
    pub cpu_readback: StageStats,
    /// Parte da leitura esperando a GPU terminar o quadro capturado antes da cópia (µs). O caminho
    /// da GPU tem a mesma espera, mas dentro da fila da GPU (aparece na codificação).
    pub cpu_frame_wait: StageStats,
    /// Threads da conversão na CPU (0 = nenhuma camada na CPU).
    pub cpu_threads: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_spec_padrao_e_auto() {
        assert_eq!(ConversionSpec::default(), ConversionSpec::Auto);
        assert_eq!(VideoPipelineOptions::default().conversion, ConversionSpec::Auto);
    }

    #[test]
    fn stats_novas_comecam_vazias() {
        assert_eq!(LayerStats::default().converter, None);
        let ps = PipelineStats::default();
        assert_eq!(ps.cpu_threads, 0);
        assert_eq!(ps.cpu_readback, StageStats::default());
        assert_eq!(ps.cpu_frame_wait, StageStats::default());
    }
}
