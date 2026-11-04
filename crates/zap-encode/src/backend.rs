//! Tipos compartilhados entre `encoder.rs` (dono da thread) e os backends `hw.rs`/`sw.rs`.
use crate::{EncodedPicture, EncoderConfig};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::Arc;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};

/// Um quadro para codificar.
pub(crate) struct Job {
    pub nv12: zap_gpu::Texture,
    pub capture_us: u64,
    pub force_keyframe: bool,
}

/// Estado compartilhado entre `H264Encoder` e a thread do codificador.
#[derive(Default)]
pub(crate) struct Shared {
    /// Quadros descartados por fila cheia.
    pub dropped: AtomicU64,
    /// Novo bitrate pedido (kbps); 0 = nenhum pedido pendente. A thread consome com `swap(0)`.
    pub pending_kbps: AtomicU32,
    /// Pede parada da thread.
    pub stop: AtomicBool,
}

/// Dispositivo D3D11 emprestado do `GpuDevice` para a thread do codificador.
pub(crate) struct SendDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
}

// SAFETY: o GpuDevice foi criado com ID3D10Multithread::SetMultithreadProtected(TRUE); o D3D11
// serializa as chamadas ao dispositivo e ao contexto imediato entre threads.
unsafe impl Send for SendDevice {}

/// Entradas e saídas que o loop de um backend usa.
pub(crate) struct WorkerIo {
    pub cfg: EncoderConfig,
    pub rx: Receiver<Job>,
    pub out: Sender<EncodedPicture>,
    pub shared: Arc<Shared>,
}
