//! `H264Encoder`: fila, thread e escolha do backend.
use crate::backend::{Job, SendDevice, Shared, WorkerIo};
use crate::enumerate::{self, Candidate};
use crate::mf::MfGuard;
use crate::{hw, logic, sw};
use crate::{EncodeError, EncodedPicture, EncoderChoice, EncoderConfig, EncoderInfo};
use crossbeam_channel::{bounded, Sender, TrySendError};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct H264Encoder {
    info: EncoderInfo,
    tx: Option<Sender<Job>>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

/// Backend já aberto, pronto para rodar o loop.
enum Aberto {
    Hw(hw::HwEncoder),
    Sw(sw::SwEncoder),
}

impl H264Encoder {
    /// Valida a config (`logic::validate_config`), cria a thread do codificador (que faz
    /// `ComGuard`, `MfGuard`, enumera, ordena com `logic::order_candidates`, tenta abrir cada
    /// candidato) e espera o resultado da abertura por um canal. Erro se nenhum abrir:
    /// `EncodeError::NoEncoder`.
    pub fn new(
        dev: &zap_gpu::GpuDevice,
        cfg: EncoderConfig,
        out: crossbeam_channel::Sender<EncodedPicture>,
    ) -> Result<Self, EncodeError> {
        logic::validate_config(&cfg)?;
        let sdev = SendDevice {
            device: dev.d3d_device().clone(),
            context: dev.context().clone(),
        };
        let gpu_vendor = dev.adapter().vendor;
        let shared = Arc::new(Shared::default());
        let (tx, rx) = bounded::<Job>(2);
        let (ready_tx, ready_rx) = bounded::<Result<EncoderInfo, EncodeError>>(1);

        let io = WorkerIo {
            cfg: cfg.clone(),
            rx,
            out,
            shared: shared.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("zap-encode".to_string())
            .spawn(move || worker(sdev, cfg, gpu_vendor, io, ready_tx))
            .map_err(|e| EncodeError::Stopped(format!("falha ao criar a thread: {e}")))?;

        // Cria o Self antes de esperar, para o Drop encerrar a thread se der timeout.
        let mut enc = Self {
            info: EncoderInfo {
                name: String::new(),
                vendor: gpu_vendor,
                hardware: false,
            },
            tx: Some(tx),
            shared,
            thread: Some(thread),
        };
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(info)) => {
                enc.info = info;
                Ok(enc)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(EncodeError::Stopped("tempo esgotado abrindo o codificador".to_string())),
        }
    }

    /// Não bloqueia. Fila `bounded(2)`: cheia = descarta, conta em `dropped()` e devolve false.
    pub fn submit(&self, nv12: zap_gpu::Texture, capture_us: u64, force_keyframe: bool) -> bool {
        let Some(tx) = &self.tx else { return false };
        match tx.try_send(Job {
            nv12,
            capture_us,
            force_keyframe,
        }) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.shared.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Guarda em `Shared::pending_kbps` (a thread aplica).
    pub fn set_bitrate(&self, kbps: u32) {
        self.shared.pending_kbps.store(kbps.max(1), Ordering::Relaxed);
    }

    pub fn info(&self) -> &EncoderInfo {
        &self.info
    }

    pub fn dropped(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for H264Encoder {
    /// Marca `stop`, fecha a fila e espera a thread.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(t) = self.thread.take() {
            // Pânico da thread é ignorado: não há o que fazer no Drop.
            let _ = t.join();
        }
    }
}

/// Corpo da thread: prepara COM/MF, escolhe e abre o backend, avisa o resultado e roda o loop.
fn worker(
    sdev: SendDevice,
    cfg: EncoderConfig,
    gpu_vendor: zap_gpu::Vendor,
    io: WorkerIo,
    ready: Sender<Result<EncoderInfo, EncodeError>>,
) {
    // Prioridade acima do normal para não atrasar a codificação.
    // SAFETY: só altera a prioridade da thread atual.
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
    let _com = match zap_gpu::ComGuard::new() {
        Ok(g) => g,
        Err(e) => {
            let _ = ready.send(Err(e.into()));
            return;
        }
    };
    let _mf = match MfGuard::acquire() {
        Ok(g) => g,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };

    let mut cands: Vec<Option<Candidate>> = Vec::new();
    let mut tentativas = Vec::new();
    if cfg.choice != EncoderChoice::Software {
        tentativas.push(true);
    }
    if cfg.choice != EncoderChoice::Hardware {
        tentativas.push(false);
    }
    for hardware in tentativas {
        match enumerate::enumerate(hardware) {
            Ok(v) => cands.extend(v.into_iter().map(Some)),
            Err(e) => tracing::warn!("falha ao enumerar codificadores (hardware={hardware}): {e}"),
        }
    }
    let infos: Vec<EncoderInfo> = cands
        .iter()
        .filter_map(|c| c.as_ref().map(|c| c.info.clone()))
        .collect();
    let ordem = logic::order_candidates(&infos, cfg.choice, gpu_vendor);

    let mut aberto = None;
    for i in ordem {
        let Some(cand) = cands.get_mut(i).and_then(Option::take) else {
            continue;
        };
        let nome = cand.info.name.clone();
        let r = if cand.info.hardware {
            hw::HwEncoder::open(&sdev, &cfg, cand).map(Aberto::Hw)
        } else {
            sw::SwEncoder::open(&cfg, cand).map(Aberto::Sw)
        };
        match r {
            Ok(a) => {
                aberto = Some(a);
                break;
            }
            Err(e) => tracing::warn!("não abriu o codificador {nome}: {e}"),
        }
    }

    match aberto {
        None => {
            let _ = ready.send(Err(EncodeError::NoEncoder));
        }
        Some(Aberto::Hw(h)) => {
            let _ = ready.send(Ok(h.info().clone()));
            h.run(io);
        }
        Some(Aberto::Sw(s)) => {
            let _ = ready.send(Ok(s.info().clone()));
            s.run(sdev, io);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::Instant;
    use zap_gpu::{GpuDevice, PixelFormat, TextureRing};

    fn cfg(w: u32, h: u32) -> EncoderConfig {
        EncoderConfig {
            width: w,
            height: h,
            fps: 30,
            kbps: 2000,
            gop_frames: 60,
            choice: EncoderChoice::Auto,
        }
    }

    #[test]
    fn config_invalida_antes_de_tudo() {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("sem GPU, pulando: {e}");
                return;
            }
        };
        let (tx, _rx) = crossbeam_channel::unbounded();
        let r = H264Encoder::new(&dev, cfg(0, 360), tx);
        assert!(matches!(r, Err(EncodeError::InvalidConfig(_))));
    }

    #[test]
    fn submit_com_fila_cheia_conta_descartes() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("sem GPU, pulando");
            return;
        };
        let (tx, _rx) = crossbeam_channel::unbounded();
        let Ok(enc) = H264Encoder::new(&dev, cfg(640, 360), tx) else {
            eprintln!("sem codificador, pulando");
            return;
        };
        let Ok(mut ring) = TextureRing::new(&dev, 640, 360, PixelFormat::Nv12, 1, false) else {
            eprintln!("sem textura NV12, pulando");
            return;
        };
        // Texture não é Clone: cada envio consome uma textura nova.
        let mut texs = Vec::new();
        for _ in 0..50 {
            match zap_gpu::Texture::new(&dev, 640, 360, PixelFormat::Nv12, false) {
                Ok(t) => texs.push(t),
                Err(_) => return,
            }
        }
        let _ = ring.next();
        let ini = Instant::now();
        let mut aceitos = 0;
        for (i, t) in texs.into_iter().enumerate() {
            if enc.submit(t, i as u64 * 33_000, false) {
                aceitos += 1;
            }
        }
        assert!(ini.elapsed() < Duration::from_secs(1), "submit bloqueou");
        assert!(enc.dropped() > 0 || aceitos == 50);
    }
}
