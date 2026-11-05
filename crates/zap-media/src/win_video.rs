//! Pipeline real de vídeo (Windows): captura -> Converter por camada -> H264Encoder por camada.
mod conversor;
mod fase_cpu;
mod processar;

use crate::logic::{LayerGate, Samples, SeqCounter};
use crate::video::Shared;
use crate::{
    EncodedFrame, EncoderChoiceSpec, LayerStats, MediaError, MonitorRef, VideoPipelineConfig, VideoPipelineOptions,
    VideoSourceSpec,
};
use conversor::{criar_conversor, Conversor};
use crossbeam_channel::{Receiver, SendTimeoutError, Sender};
use parking_lot::{Condvar, Mutex};
use processar::processar;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use zap_capture::{CaptureError, CaptureHandle, CapturedFrame, FrameSink};
use zap_encode::{EncodedPicture, EncoderChoice, EncoderConfig, H264Encoder};
use zap_gpu::{now_us, ComGuard, GpuDevice, PixelFormat, ReadbackStaging, Texture, TextureRing};
use zap_proto::LayerSpec;

/// Janela das estatísticas (1 s, em µs).
const JANELA_STATS_US: u64 = 1_000_000;
/// Sem quadro novo por este tempo, reenvia o último (1 s, em µs).
const SEM_QUADRO_US: u64 = 1_000_000;
/// Espera curta no slot, para recolher imagens codificadas mesmo sem quadro novo.
const FATIA_ESPERA: Duration = Duration::from_millis(10);
/// Depois de falhar ao criar uma camada, espera antes de tentar de novo (2 s, em µs).
const RETENTAR_US: u64 = 2_000_000;
/// Amostras mantidas por etapa.
const JANELA_AMOSTRAS: usize = 120;

/// Slot "o mais novo vence" entre a thread de captura e a thread de vídeo.
struct Slot {
    quadro: Mutex<Option<CapturedFrame>>,
    cond: Condvar,
    ultimo_erro: Mutex<Option<String>>,
}

struct SlotSink {
    slot: Arc<Slot>,
    shared: Arc<Shared>,
}

impl FrameSink for SlotSink {
    fn on_frame(&self, frame: CapturedFrame) {
        let mut g = self.slot.quadro.lock();
        // Quadro anterior ainda não consumido foi sobrescrito: conta como descarte.
        if g.replace(frame).is_some() {
            self.shared.dropped_capture.fetch_add(1, Ordering::Relaxed);
        }
        drop(g);
        self.slot.cond.notify_one();
    }
    fn on_error(&self, err: CaptureError) {
        tracing::warn!("erro de captura: {err}");
        *self.slot.ultimo_erro.lock() = Some(err.to_string());
    }
}

/// Parte da camada que existe só enquanto ela está ligada.
struct Pipe {
    conv: Conversor,
    ring: TextureRing,
    enc: H264Encoder,
    rx: Receiver<EncodedPicture>,
    in_w: u32,
    in_h: u32,
    in_fmt: PixelFormat,
}

struct Camada {
    spec: LayerSpec,
    gate: LayerGate,
    seq: SeqCounter,
    pipe: Option<Pipe>,
    kbps: u32,
    primeiro: bool,
    retentar_em_us: u64,
    /// Falhas seguidas de `convert` na GPU (zera num sucesso). 3 no modo `Auto` levam à CPU.
    falhas_gpu: u32,
    /// A camada caiu para a CPU por falhas da GPU; volta a `false` só quando a escada é refeita.
    forcar_cpu: bool,
    dropped_acum: u64,
    frames: u64,
    frames_janela: u64,
    bytes_janela: u64,
    convert: Samples,
    encode: Samples,
    convert_gpu: Samples,
    lock_wait: Samples,
}

impl Camada {
    fn nova(spec: &LayerSpec) -> Self {
        Self {
            spec: spec.clone(),
            gate: LayerGate::new(u32::from(spec.fps)),
            seq: SeqCounter::new(),
            pipe: None,
            kbps: 0,
            primeiro: true,
            retentar_em_us: 0,
            falhas_gpu: 0,
            forcar_cpu: false,
            dropped_acum: 0,
            frames: 0,
            frames_janela: 0,
            bytes_janela: 0,
            convert: Samples::new(JANELA_AMOSTRAS),
            encode: Samples::new(JANELA_AMOSTRAS),
            convert_gpu: Samples::new(JANELA_AMOSTRAS),
            lock_wait: Samples::new(JANELA_AMOSTRAS),
        }
    }
    /// Desliga a camada, guardando os descartes do codificador para as estatísticas.
    fn soltar(&mut self) {
        if let Some(p) = self.pipe.take() {
            self.dropped_acum += p.enc.dropped();
        }
        self.primeiro = true;
    }
}

fn construir(ladder: &[LayerSpec]) -> Vec<Camada> {
    ladder.iter().map(Camada::nova).collect()
}

/// Alguma camada tem pipe com conversão na CPU?
fn usa_cpu(camadas: &[Camada]) -> bool {
    camadas
        .iter()
        .any(|c| c.pipe.as_ref().is_some_and(|p| matches!(p.conv, Conversor::Cpu(_))))
}

fn escolha(c: EncoderChoiceSpec) -> EncoderChoice {
    match c {
        EncoderChoiceSpec::Auto => EncoderChoice::Auto,
        EncoderChoiceSpec::Hardware => EncoderChoice::Hardware,
        EncoderChoiceSpec::Software => EncoderChoice::Software,
    }
}

/// Clone barato (refcount COM) de uma textura.
fn clonar(t: &Texture) -> Texture {
    Texture::from_raw(t.raw().clone(), t.format(), t.width(), t.height())
}

/// Estado da thread de vídeo que `processar` e a fase da CPU compartilham.
struct Contexto {
    opts: VideoPipelineOptions,
    /// Codificador pedido (`VideoPipelineConfig::encoder`).
    encoder: EncoderChoiceSpec,
    /// Só nos testes: finge que não há Video Processor.
    simular_sem_vp: bool,
    /// Leitura persistente da origem para a CPU (uma por pipeline).
    leitura: Option<ReadbackStaging>,
    /// `copy_wait_us` das leituras, em µs.
    leitura_s: Samples,
    /// `frame_wait_us` das leituras (espera pelo quadro antes da cópia), em µs.
    espera_quadro_s: Samples,
    /// Threads da conversão na CPU (`zap_gpu::cpu_threads()`).
    threads: usize,
    /// Instante (µs) do último aviso de falha de leitura (no máximo 1 por segundo).
    ultimo_aviso_leitura_us: u64,
}

fn criar_pipe(
    dev: &GpuDevice,
    spec: &LayerSpec,
    kbps: u32,
    ctx: &Contexto,
    forcar_cpu: bool,
    entrada: (u32, u32, PixelFormat),
) -> Result<Pipe, MediaError> {
    let (in_w, in_h, in_fmt) = entrada;
    let (w, h) = (u32::from(spec.width), u32::from(spec.height));
    let conv = criar_conversor(dev, ctx, forcar_cpu, spec, entrada)?;
    let ring = TextureRing::new(dev, w, h, PixelFormat::Nv12, 4, false)?;
    let (tx, rx) = crossbeam_channel::bounded(8);
    let fps = u32::from(spec.fps);
    let cfg = EncoderConfig {
        width: w,
        height: h,
        fps,
        kbps,
        gop_frames: fps * 10,
        choice: escolha(ctx.encoder),
    };
    let enc = H264Encoder::new(dev, cfg, tx)?;
    Ok(Pipe {
        conv,
        ring,
        enc,
        rx,
        in_w,
        in_h,
        in_fmt,
    })
}

fn prioridade_acima_do_normal() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};
    // SAFETY: o pseudo-handle da thread atual é sempre válido.
    let r = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) };
    if let Err(e) = r {
        tracing::warn!("não foi possível subir a prioridade da thread de vídeo: {e}");
    }
}

/// Cria `GpuDevice` (para monitor, `create_for_adapter(luid do monitor)`), inicia a captura da
/// fonte e a thread de vídeo "zap-video" (prioridade ABOVE_NORMAL).
/// Cada captura recebe o próprio clone do `GpuDevice` (AddRef dos objetos COM), então o `dev`
/// movido para a thread `zap-video` não deixa ponteiro pendurado.
pub(crate) fn spawn(
    cfg: VideoPipelineConfig,
    shared: Arc<Shared>,
    out: Sender<EncodedFrame>,
    opts: VideoPipelineOptions,
    simular_sem_vp: bool,
) -> Result<std::thread::JoinHandle<()>, MediaError> {
    let slot = Arc::new(Slot {
        quadro: Mutex::new(None),
        cond: Condvar::new(),
        ultimo_erro: Mutex::new(None),
    });
    let sink = SlotSink {
        slot: slot.clone(),
        shared: shared.clone(),
    };
    let (dev, handle) = match &cfg.source {
        VideoSourceSpec::Monitor(MonitorRef(i)) => {
            let monitores = zap_capture::list_monitors()?;
            let m = monitores
                .iter()
                .find(|m| m.index == *i)
                .ok_or(CaptureError::MonitorNotFound)?;
            let dev = GpuDevice::create_for_adapter(m.adapter_luid)?;
            let h = zap_capture::capture_monitor(&dev, m, sink)?;
            (dev, h)
        }
        VideoSourceSpec::Window(hwnd) => {
            let dev = GpuDevice::create_default()?;
            let h = zap_capture::capture_window(&dev, *hwnd, sink)?;
            (dev, h)
        }
        VideoSourceSpec::Camera(id) => {
            let dev = GpuDevice::create_default()?;
            // Pede a maior resolução e fps da escada; o modo mais próximo é escolhido pela captura.
            let w = cfg.ladder.iter().map(|l| u32::from(l.width)).max().unwrap_or(1280);
            let h = cfg.ladder.iter().map(|l| u32::from(l.height)).max().unwrap_or(720);
            let fps = cfg.ladder.iter().map(|l| u32::from(l.fps)).max().unwrap_or(30);
            let ch = zap_capture::capture_camera(&dev, id, w, h, fps, sink)?;
            (dev, ch)
        }
        VideoSourceSpec::Synthetic { .. } => {
            return Err(MediaError::InvalidConfig("fonte sintética no backend real".into()));
        }
    };
    let ctx = Contexto {
        opts,
        encoder: cfg.encoder,
        simular_sem_vp,
        leitura: None,
        leitura_s: Samples::new(JANELA_AMOSTRAS),
        espera_quadro_s: Samples::new(JANELA_AMOSTRAS),
        threads: zap_gpu::cpu_threads(),
        ultimo_aviso_leitura_us: 0,
    };
    std::thread::Builder::new()
        .name("zap-video".into())
        .spawn(move || executar(cfg, shared, out, slot, dev, handle, ctx))
        .map_err(|e| MediaError::Stopped(format!("falha ao criar thread de vídeo: {e}")))
}

/// Envia ao consumidor sem perder a atenção ao `stop`. `false` = encerrar a thread.
fn enviar(out: &Sender<EncodedFrame>, shared: &Shared, mut frame: EncodedFrame) -> bool {
    loop {
        match out.send_timeout(frame, Duration::from_millis(50)) {
            Ok(()) => return true,
            Err(SendTimeoutError::Disconnected(_)) => return false,
            Err(SendTimeoutError::Timeout(f)) => {
                if shared.stop.load(Ordering::Relaxed) {
                    return false;
                }
                frame = f;
            }
        }
    }
}

fn executar(
    cfg: VideoPipelineConfig,
    shared: Arc<Shared>,
    out: Sender<EncodedFrame>,
    slot: Arc<Slot>,
    dev: GpuDevice,
    handle: CaptureHandle,
    mut ctx: Contexto,
) {
    prioridade_acima_do_normal();
    let _com = match ComGuard::new() {
        Ok(g) => Some(g),
        Err(e) => {
            tracing::warn!("ComGuard falhou: {e}");
            None
        }
    };
    let mut camadas = construir(&cfg.ladder);
    let mut requests: Vec<(u8, u32)> = Vec::new();
    let mut ultimo: Option<CapturedFrame> = None;
    let mut ultimo_chegada_us = now_us();
    let mut ultima_stats = now_us();
    let mut capture_s = Samples::new(JANELA_AMOSTRAS);
    let mut novos_janela: u64 = 0;
    let mut encoder_name = String::new();

    'principal: while !shared.stop.load(Ordering::Relaxed) {
        // Controle: lock curto, só copia o necessário.
        {
            let mut c = shared.control.lock();
            let mut reaplicar = false;
            if c.ladder_dirty {
                // Descarta tudo; as camadas voltam sob demanda.
                for cam in camadas.iter_mut() {
                    cam.soltar();
                }
                camadas = construir(&c.ladder);
                c.ladder_dirty = false;
                reaplicar = true;
            }
            if c.requests_dirty || reaplicar {
                requests = c.requests.clone();
                c.requests_dirty = false;
            }
        }
        for cam in camadas.iter_mut() {
            match requests.iter().find(|(l, _)| *l == cam.spec.layer) {
                None => cam.soltar(),
                Some(&(_, kbps)) => {
                    if let Some(p) = &cam.pipe {
                        if kbps != cam.kbps {
                            p.enc.set_bitrate(kbps);
                            cam.kbps = kbps;
                        }
                    }
                }
            }
        }

        // Espera um quadro novo (em fatias curtas, para recolher codificados no intervalo).
        let novo = {
            let mut g = slot.quadro.lock();
            if g.is_none() {
                slot.cond.wait_for(&mut g, FATIA_ESPERA);
            }
            g.take()
        };
        let agora = now_us();
        let mut origem_nova = false;
        if let Some(f) = novo {
            capture_s.push(agora.saturating_sub(f.capture_us) as f64);
            ultimo = Some(f);
            ultimo_chegada_us = agora;
            origem_nova = true;
            if ultimo.as_ref().is_some_and(|f| f.new_content) {
                novos_janela += 1;
            }
        } else if agora.saturating_sub(ultimo_chegada_us) >= SEM_QUADRO_US {
            // Sem imagem nova há 1 s: reentrega o último quadro.
            ultimo_chegada_us = agora;
            origem_nova = ultimo.is_some();
        }

        if origem_nova {
            if let Some(frame) = &ultimo {
                processar(&mut camadas, &requests, &dev, &shared, frame, agora, &mut ctx);
            }
            // Sem camada na CPU, libera a leitura persistente (~8 MB em 1080p).
            if ctx.leitura.is_some() && !usa_cpu(&camadas) {
                ctx.leitura = None;
            }
        }

        // Recolhe os codificados.
        for cam in camadas.iter_mut() {
            let Some(p) = &cam.pipe else { continue };
            while let Ok(pic) = p.rx.try_recv() {
                cam.encode.push(f64::from(pic.encode_us));
                cam.frames += 1;
                cam.frames_janela += 1;
                cam.bytes_janela += pic.data.len() as u64;
                let ef = EncodedFrame {
                    track: cfg.track,
                    layer: cam.spec.layer,
                    seq: cam.seq.take(),
                    keyframe: pic.keyframe,
                    has_params: pic.has_params,
                    width: cam.spec.width,
                    height: cam.spec.height,
                    capture_us: pic.capture_us,
                    data: pic.data,
                };
                if !enviar(&out, &shared, ef) {
                    break 'principal;
                }
            }
        }

        // Estatísticas a cada ~1 s.
        let agora = now_us();
        let dt = agora.saturating_sub(ultima_stats);
        if dt >= JANELA_STATS_US {
            let secs = dt as f32 / 1_000_000.0;
            let mut dropped_encode = 0;
            let mut layers = Vec::with_capacity(camadas.len());
            for c in camadas.iter_mut() {
                dropped_encode += c.dropped_acum;
                if let Some(p) = &c.pipe {
                    dropped_encode += p.enc.dropped();
                    encoder_name = p.enc.info().name.clone();
                }
                layers.push(LayerStats {
                    layer: c.spec.layer,
                    enabled: requests.iter().any(|(l, _)| *l == c.spec.layer),
                    fps: c.frames_janela as f32 / secs,
                    kbps: c.bytes_janela as f32 * 8.0 / 1000.0 / secs,
                    frames: c.frames,
                    convert: c.convert.stage(),
                    encode: c.encode.stage(),
                    convert_gpu: c.convert_gpu.stage(),
                    lock_wait: c.lock_wait.stage(),
                    converter: c.pipe.as_ref().map(|p| p.conv.kind()),
                });
                c.frames_janela = 0;
                c.bytes_janela = 0;
            }
            let mut s = shared.stats.lock();
            s.layers = layers;
            s.capture = capture_s.stage();
            s.capture_new_fps = novos_janela as f32 / secs;
            s.dropped_encode = dropped_encode;
            s.cpu_readback = ctx.leitura_s.stage();
            s.cpu_frame_wait = ctx.espera_quadro_s.stage();
            s.cpu_threads = if usa_cpu(&camadas) { ctx.threads as u32 } else { 0 };
            s.encoder_name = encoder_name.clone();
            ultima_stats = agora;
            novos_janela = 0;
        }
    }

    // Ordem de encerramento: captura, codificadores, e só então o dispositivo. O `dev` desta
    // thread é só mais uma referência: a captura tem a sua.
    drop(handle);
    for cam in camadas.iter_mut() {
        cam.soltar();
    }
    drop(ultimo);
    drop(dev);
}

#[cfg(all(test, windows))]
mod tests;
