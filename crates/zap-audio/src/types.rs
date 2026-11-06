use crate::FRAME_SAMPLES;

/// Aplicativo que pode ter o som capturado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioApp {
    /// Raiz do app: o processo mais alto da árvore com o mesmo executável (o Chrome, o Discord e
    /// o Spotify tocam o som num processo filho). A captura pega a árvore inteira dele.
    pub pid: u32,
    /// Nome para mostrar, ex.: "Google Chrome" (descrição do executável) ou "cs2".
    pub name: String,
    /// Nome do executável, ex.: "cs2.exe".
    pub exe: String,
    pub sound: AppSound,
}

/// O que se sabe do som de um app na hora da listagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppSound {
    /// Tocando agora (sessão de áudio ativa).
    Playing,
    /// Já abriu o áudio, mas está quieto (pausado, entre faixas).
    Ready,
    /// Só tem janela aberta: ainda não abriu o áudio. A captura funciona quando ele tocar.
    Silent,
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
