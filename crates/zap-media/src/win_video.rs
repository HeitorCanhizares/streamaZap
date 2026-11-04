//! Pipeline real de vídeo (Windows): captura -> Converter por camada -> H264Encoder por camada.
use crate::logic::{LayerGate, Samples, SeqCounter};
use crate::video::Shared;
use crate::{
    EncodedFrame, EncoderChoiceSpec, LayerStats, MediaError, MonitorRef, VideoPipelineConfig, VideoSourceSpec,
};
use crossbeam_channel::{Receiver, SendTimeoutError, Sender};
use parking_lot::{Condvar, Mutex};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zap_capture::{CaptureError, CaptureHandle, CapturedFrame, FrameSink};
use zap_encode::{EncodedPicture, EncoderChoice, EncoderConfig, H264Encoder};
use zap_gpu::{now_us, ComGuard, Converter, GpuDevice, PixelFormat, Texture, TextureRing};
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
    conv: Converter,
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
    dropped_acum: u64,
    frames: u64,
    frames_janela: u64,
    bytes_janela: u64,
    convert: Samples,
    encode: Samples,
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
            dropped_acum: 0,
            frames: 0,
            frames_janela: 0,
            bytes_janela: 0,
            convert: Samples::new(JANELA_AMOSTRAS),
            encode: Samples::new(JANELA_AMOSTRAS),
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

fn criar_pipe(
    dev: &GpuDevice,
    spec: &LayerSpec,
    kbps: u32,
    choice: EncoderChoiceSpec,
    in_w: u32,
    in_h: u32,
    in_fmt: PixelFormat,
) -> Result<Pipe, MediaError> {
    let (w, h) = (u32::from(spec.width), u32::from(spec.height));
    let conv = Converter::new(dev, in_w, in_h, in_fmt, w, h)?;
    let ring = TextureRing::new(dev, w, h, PixelFormat::Nv12, 4, false)?;
    let (tx, rx) = crossbeam_channel::bounded(8);
    let fps = u32::from(spec.fps);
    let cfg = EncoderConfig {
        width: w,
        height: h,
        fps,
        kbps,
        gop_frames: fps * 10,
        choice: escolha(choice),
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
pub(crate) fn spawn(
    cfg: VideoPipelineConfig,
    shared: Arc<Shared>,
    out: Sender<EncodedFrame>,
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
    std::thread::Builder::new()
        .name("zap-video".into())
        .spawn(move || executar(cfg, shared, out, slot, dev, handle))
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
        } else if agora.saturating_sub(ultimo_chegada_us) >= SEM_QUADRO_US {
            // Sem imagem nova há 1 s: reentrega o último quadro.
            ultimo_chegada_us = agora;
            origem_nova = ultimo.is_some();
        }

        if origem_nova {
            if let Some(frame) = &ultimo {
                processar(&mut camadas, &requests, &dev, &cfg, &shared, frame, agora);
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
                });
                c.frames_janela = 0;
                c.bytes_janela = 0;
            }
            let mut s = shared.stats.lock();
            s.layers = layers;
            s.capture = capture_s.stage();
            s.dropped_encode = dropped_encode;
            s.encoder_name = encoder_name.clone();
            ultima_stats = agora;
        }
    }

    // Ordem de encerramento: captura, codificadores, e só então o dispositivo.
    drop(handle);
    for cam in camadas.iter_mut() {
        cam.soltar();
    }
    drop(ultimo);
    drop(dev);
}

/// Converte e entrega o quadro a cada camada ligada cujo `LayerGate` deixar.
fn processar(
    camadas: &mut [Camada],
    requests: &[(u8, u32)],
    dev: &GpuDevice,
    cfg: &VideoPipelineConfig,
    shared: &Shared,
    frame: &CapturedFrame,
    agora: u64,
) {
    let in_fmt = frame.texture.format();
    let (in_w, in_h) = (frame.width, frame.height);
    // Reentrega usa o relógio atual; imagem nova mantém o instante real da captura.
    let cap_us = if frame.new_content { frame.capture_us } else { agora };
    for cam in camadas.iter_mut() {
        let layer = cam.spec.layer;
        let Some(&(_, kbps)) = requests.iter().find(|(l, _)| *l == layer) else {
            continue;
        };
        // Tamanho/formato da entrada mudou: recria só o conversor.
        if let Some(p) = &mut cam.pipe {
            if p.in_w != in_w || p.in_h != in_h || p.in_fmt != in_fmt {
                match Converter::new(
                    dev,
                    in_w,
                    in_h,
                    in_fmt,
                    u32::from(cam.spec.width),
                    u32::from(cam.spec.height),
                ) {
                    Ok(c) => {
                        p.conv = c;
                        p.in_w = in_w;
                        p.in_h = in_h;
                        p.in_fmt = in_fmt;
                        // Imagem nova de outro tamanho: o decodificador precisa de um ponto de entrada.
                        cam.primeiro = true;
                    }
                    Err(e) => {
                        tracing::warn!("camada {layer}: falha ao recriar o conversor: {e}");
                        cam.soltar();
                        cam.retentar_em_us = agora + RETENTAR_US;
                        continue;
                    }
                }
            }
        }
        if cam.pipe.is_none() {
            if agora < cam.retentar_em_us {
                continue;
            }
            match criar_pipe(dev, &cam.spec, kbps, cfg.encoder, in_w, in_h, in_fmt) {
                Ok(p) => {
                    cam.pipe = Some(p);
                    cam.kbps = kbps;
                    cam.gate.reset();
                    cam.primeiro = true;
                    let mut c = shared.control.lock();
                    if !c.keyframes.contains(&layer) {
                        c.keyframes.push(layer);
                    }
                }
                Err(e) => {
                    tracing::warn!("camada {layer}: falha ao ligar: {e}");
                    cam.retentar_em_us = agora + RETENTAR_US;
                    continue;
                }
            }
        }
        if !cam.gate.ready(agora) {
            continue;
        }
        let Some(p) = &mut cam.pipe else { continue };
        let t0 = Instant::now();
        let dst = p.ring.next();
        if let Err(e) = p.conv.convert(dev, &frame.texture, dst) {
            tracing::warn!("camada {layer}: falha na conversão: {e}");
            continue;
        }
        let tex = clonar(dst);
        cam.convert.push(t0.elapsed().as_micros() as f64);
        let force = shared.take_keyframe(layer) || cam.primeiro;
        if p.enc.submit(tex, cap_us, force) {
            cam.primeiro = false;
        } else if force {
            // Fila cheia: o pedido não pode se perder.
            let mut c = shared.control.lock();
            if !c.keyframes.contains(&layer) {
                c.keyframes.push(layer);
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::VideoPipeline;
    use zap_rate::allocator::LayerRequest;

    fn camada(layer: u8, w: u16, h: u16) -> LayerSpec {
        LayerSpec {
            layer,
            width: w,
            height: h,
            fps: 30,
            kbps: 3000,
        }
    }

    /// `None` (com aviso) quando o ambiente não tem GPU ou monitor.
    fn iniciar(ladder: Vec<LayerSpec>) -> Option<(VideoPipeline, Receiver<EncodedFrame>)> {
        if !GpuDevice::create_default().is_ok_and(|d| d.has_video()) {
            eprintln!("sem GPU com Video Processor: teste ignorado");
            return None;
        }
        match zap_capture::list_monitors() {
            Ok(m) if !m.is_empty() => {}
            _ => {
                eprintln!("sem monitor: teste ignorado");
                return None;
            }
        }
        let (tx, rx) = crossbeam_channel::bounded(256);
        let cfg = VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Monitor(MonitorRef(1)),
            ladder,
            encoder: EncoderChoiceSpec::Auto,
        };
        match VideoPipeline::start(cfg, tx) {
            Ok(p) => Some((p, rx)),
            Err(e) => {
                eprintln!("não iniciou ({e}): teste ignorado");
                None
            }
        }
    }

    fn req(layer: u8) -> LayerRequest {
        LayerRequest { layer, kbps: 3000 }
    }

    fn coletar(rx: &Receiver<EncodedFrame>, dur: Duration) -> Vec<EncodedFrame> {
        let fim = Instant::now() + dur;
        let mut v = Vec::new();
        while let Some(resto) = fim.checked_duration_since(Instant::now()) {
            if let Ok(f) = rx.recv_timeout(resto) {
                v.push(f);
            }
        }
        v
    }

    #[test]
    fn monitor_1_gera_quadros_na_camada_0() {
        let Some((p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
            return;
        };
        p.set_requests(&[req(0)]);
        let v = coletar(&rx, Duration::from_secs(2));
        assert!(v.len() >= 15, "só {} quadros", v.len());
        assert!(v[0].keyframe && v[0].has_params);
        assert!(v.iter().all(|f| f.layer == 0 && f.width == 1280 && f.height == 720));
    }

    #[test]
    fn desligar_camada_para_de_emitir() {
        let Some((p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
            return;
        };
        p.set_requests(&[req(0)]);
        assert!(!coletar(&rx, Duration::from_secs(1)).is_empty());
        p.set_requests(&[]);
        // Deixa o que já estava em voo sair.
        let _ = coletar(&rx, Duration::from_millis(800));
        assert!(coletar(&rx, Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn keyframe_pedido_chega() {
        let Some((p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
            return;
        };
        p.set_requests(&[req(0)]);
        let _ = coletar(&rx, Duration::from_secs(1));
        p.request_keyframe(0);
        let v = coletar(&rx, Duration::from_secs(2));
        assert!(v.iter().any(|f| f.keyframe), "nenhum keyframe após o pedido");
    }

    #[test]
    fn set_ladder_troca_resolucao() {
        let Some((p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
            return;
        };
        p.set_requests(&[req(0)]);
        assert!(!coletar(&rx, Duration::from_secs(1)).is_empty());
        p.set_ladder(vec![camada(0, 640, 360)]);
        p.set_requests(&[req(0)]);
        let v = coletar(&rx, Duration::from_secs(3));
        assert!(
            v.iter().any(|f| f.width == 640 && f.height == 360),
            "nenhum quadro na nova resolução"
        );
    }
}
