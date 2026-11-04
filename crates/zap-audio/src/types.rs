use crate::FRAME_SAMPLES;

/// Aplicativo com sessão de áudio ativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioApp {
    pub pid: u32,
    /// Nome do executável, ex.: "cs2.exe".
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioSelection {
    None,
    /// Tudo menos o próprio processo.
    All,
    Apps(Vec<u32>),
}

/// Quadro de 20 ms já mixado.
pub struct CapturedAudio {
    pub pcm: Box<[f32; FRAME_SAMPLES]>,
    /// Relógio monotônico do processo, em µs (ver `clock::now_us`).
    pub capture_us: u64,
    /// Pico do bloco, 0.0..=1.0.
    pub level: f32,
}
