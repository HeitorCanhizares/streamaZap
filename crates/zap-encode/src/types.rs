use bytes::Bytes;

/// Descrição de um codificador encontrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderInfo {
    pub name: String,
    pub vendor: zap_gpu::Vendor,
    pub hardware: bool,
}

/// Escolha do codificador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderChoice {
    /// Hardware do mesmo fabricante da GPU, depois qualquer hardware, depois software.
    Auto,
    Hardware,
    Software,
}

#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub kbps: u32,
    /// Quadros entre keyframes (fps x 10).
    pub gop_frames: u32,
    pub choice: EncoderChoice,
}

/// Um quadro codificado.
#[derive(Debug, Clone)]
pub struct EncodedPicture {
    /// H.264 em Annex B.
    pub data: Bytes,
    pub keyframe: bool,
    /// `data` contém SPS e PPS (NAL 7 e 8).
    pub has_params: bool,
    pub capture_us: u64,
    /// Tempo dentro do codificador (submit até saída), em µs.
    pub encode_us: u32,
}
