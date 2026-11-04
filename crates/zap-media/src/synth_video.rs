//! Backend sintético de vídeo: gera `EncodedFrame` sem GPU.
use crate::logic::{synthetic_frame_size, LayerGate, SeqCounter};
use crate::video::Shared;
use crate::{EncodedFrame, LayerStats, MediaError, VideoPipelineConfig};
use crossbeam_channel::{SendTimeoutError, Sender};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use zap_proto::LayerSpec;

/// Intervalo entre keyframes periódicos (10 s, em µs).
const KEYFRAME_PERIODICO_US: u64 = 10_000_000;
/// Janela de atualização das estatísticas (1 s, em µs).
const JANELA_STATS_US: u64 = 1_000_000;

/// Monta os bytes de um quadro: `[0,0,0,1, 0x65 (keyframe) | 0x41, ...]` preenchido até `size`.
/// Keyframe com `has_params`: antes do IDR vai `[0,0,0,1,0x67,...]` (SPS) e `[0,0,0,1,0x68,...]`
/// (PPS) curtos, e o total ainda é `size` (mínimo 32 bytes quando keyframe).
pub(crate) fn build_frame_bytes(size: usize, keyframe: bool) -> bytes::Bytes {
    // 0xAB não forma start code (00 00 00/01) nem confunde o parser de NALs.
    const SPS: [u8; 8] = [0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f];
    const PPS: [u8; 8] = [0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80];
    let mut v = Vec::with_capacity(size.max(if keyframe { 32 } else { 6 }));
    if keyframe {
        let size = size.max(32);
        v.extend_from_slice(&SPS);
        v.extend_from_slice(&PPS);
        v.extend_from_slice(&[0, 0, 0, 1, 0x65]);
        v.resize(size, 0xAB);
    } else {
        let size = size.max(6);
        v.extend_from_slice(&[0, 0, 0, 1, 0x41]);
        v.resize(size, 0xAB);
    }
    bytes::Bytes::from(v)
}

/// Estado local de uma camada da escada.
struct Camada {
    spec: LayerSpec,
    gate: LayerGate,
    seq: SeqCounter,
    ligada: bool,
    ultimo_keyframe_us: u64,
    frames: u64,
    frames_janela: u64,
    bytes_janela: u64,
}

fn construir(ladder: &[LayerSpec]) -> Vec<Camada> {
    ladder
        .iter()
        .map(|s| Camada {
            spec: s.clone(),
            gate: LayerGate::new(u32::from(s.fps)),
            seq: SeqCounter::new(),
            ligada: false,
            ultimo_keyframe_us: 0,
            frames: 0,
            frames_janela: 0,
            bytes_janela: 0,
        })
        .collect()
}

/// Inicia a thread sintética (nome "zap-synth-video"). Para quando `shared.stop` for true.
/// Por tick de 1/120 s (relógio `zap_gpu::now_us`), para cada camada ligada cujo `LayerGate`
/// deixar, emite um `EncodedFrame` (`width/height` da camada, `capture_us = now_us`,
/// tamanho por `synthetic_frame_size`). Keyframe: primeiro quadro após ligar a camada, quando
/// `shared.take_keyframe(layer)`, ou a cada 10 s. Atualiza `shared` stats.
pub(crate) fn spawn(
    cfg: VideoPipelineConfig,
    shared: Arc<Shared>,
    out: Sender<EncodedFrame>,
) -> Result<std::thread::JoinHandle<()>, MediaError> {
    let handle = std::thread::Builder::new()
        .name("zap-synth-video".into())
        .spawn(move || {
            let mut camadas = construir(&cfg.ladder);
            let mut ultima_stats = zap_gpu::now_us();
            while !shared.stop.load(Ordering::Relaxed) {
                let now = zap_gpu::now_us();
                // Lock rápido: só copia o que precisa.
                let requests = {
                    let mut c = shared.control.lock();
                    if c.ladder_dirty {
                        camadas = construir(&c.ladder);
                        c.ladder_dirty = false;
                    }
                    c.requests_dirty = false;
                    c.requests.clone()
                };
                for cam in camadas.iter_mut() {
                    let layer = cam.spec.layer;
                    let Some(&(_, kbps)) = requests.iter().find(|(l, _)| *l == layer) else {
                        cam.gate.reset();
                        cam.ligada = false;
                        continue;
                    };
                    let acabou_de_ligar = !cam.ligada;
                    if acabou_de_ligar {
                        cam.gate.reset();
                    }
                    if !cam.gate.ready(now) {
                        continue;
                    }
                    cam.ligada = true;
                    // Consome o pedido sempre, para não ficar pendente.
                    let pedido = shared.take_keyframe(layer);
                    let keyframe = acabou_de_ligar
                        || pedido
                        || now.saturating_sub(cam.ultimo_keyframe_us) >= KEYFRAME_PERIODICO_US;
                    if keyframe {
                        cam.ultimo_keyframe_us = now;
                    }
                    let size = synthetic_frame_size(kbps, u32::from(cam.spec.fps), keyframe);
                    let data = build_frame_bytes(size, keyframe);
                    let tam = data.len() as u64;
                    let mut frame = EncodedFrame {
                        track: cfg.track,
                        layer,
                        seq: cam.seq.take(),
                        keyframe,
                        has_params: keyframe,
                        width: cam.spec.width,
                        height: cam.spec.height,
                        capture_us: now,
                        data,
                    };
                    // Envio com timeout para continuar atento ao stop se o canal encher.
                    loop {
                        match out.send_timeout(frame, Duration::from_millis(50)) {
                            Ok(()) => {
                                cam.frames += 1;
                                cam.frames_janela += 1;
                                cam.bytes_janela += tam;
                                break;
                            }
                            Err(SendTimeoutError::Disconnected(_)) => return,
                            Err(SendTimeoutError::Timeout(f)) => {
                                if shared.stop.load(Ordering::Relaxed) {
                                    return;
                                }
                                frame = f;
                            }
                        }
                    }
                }
                let agora = zap_gpu::now_us();
                let dt = agora.saturating_sub(ultima_stats);
                if dt >= JANELA_STATS_US {
                    let secs = dt as f32 / 1_000_000.0;
                    let layers = camadas
                        .iter_mut()
                        .map(|c| {
                            let st = LayerStats {
                                layer: c.spec.layer,
                                enabled: c.ligada,
                                fps: c.frames_janela as f32 / secs,
                                kbps: c.bytes_janela as f32 * 8.0 / 1000.0 / secs,
                                frames: c.frames,
                                ..Default::default()
                            };
                            c.frames_janela = 0;
                            c.bytes_janela = 0;
                            st
                        })
                        .collect();
                    let mut s = shared.stats.lock();
                    s.layers = layers;
                    s.encoder_name = "synthetic".into();
                    ultima_stats = agora;
                }
                std::thread::sleep(Duration::from_micros(1_000_000 / 120));
            }
        })
        .map_err(|e| MediaError::Stopped(format!("falha ao criar thread de vídeo: {e}")))?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn camada(fps: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer: 0,
            width: 640,
            height: 360,
            fps,
            kbps,
        }
    }

    fn cfg() -> VideoPipelineConfig {
        VideoPipelineConfig {
            track: 1,
            source: crate::VideoSourceSpec::Synthetic {
                width: 640,
                height: 360,
            },
            ladder: vec![camada(30, 2000)],
            encoder: crate::EncoderChoiceSpec::Auto,
        }
    }

    fn nals(b: &[u8]) -> Vec<u8> {
        let mut r = vec![];
        let mut i = 0;
        while i + 4 < b.len() {
            if b[i..i + 4] == [0, 0, 0, 1] {
                r.push(b[i + 4] & 0x1f);
                i += 5;
            } else {
                i += 1;
            }
        }
        r
    }

    #[test]
    fn build_nao_chave_tem_start_code_e_nal_41() {
        let b = build_frame_bytes(100, false);
        assert_eq!(b.len(), 100);
        assert_eq!(&b[..4], &[0, 0, 0, 1]);
        assert_eq!(b[4], 0x41);
    }

    #[test]
    fn build_chave_tem_sps_pps_idr() {
        let b = build_frame_bytes(500, true);
        assert_eq!(b.len(), 500);
        assert_eq!(nals(&b), vec![7, 8, 5]);
        assert_eq!(build_frame_bytes(1, true).len(), 32);
    }

    fn iniciar() -> (
        Arc<Shared>,
        crossbeam_channel::Receiver<EncodedFrame>,
        std::thread::JoinHandle<()>,
    ) {
        let c = cfg();
        let shared = Arc::new(Shared::new(c.ladder.clone()));
        let (tx, rx) = crossbeam_channel::bounded(256);
        let h = spawn(c, shared.clone(), tx).expect("spawn");
        (shared, rx, h)
    }

    fn parar(shared: &Shared, h: std::thread::JoinHandle<()>) {
        shared.stop.store(true, Ordering::Relaxed);
        h.join().expect("join");
    }

    #[test]
    fn spawn_camada_ligada_emite_a_30fps() {
        let (shared, rx, h) = iniciar();
        shared.control.lock().requests = vec![(0, 2000)];
        std::thread::sleep(Duration::from_millis(1000));
        parar(&shared, h);
        let q: Vec<_> = rx.try_iter().collect();
        assert!((20..=40).contains(&q.len()), "quadros: {}", q.len());
        assert!(q[0].keyframe && q[0].has_params);
        for (i, f) in q.iter().enumerate() {
            assert_eq!(f.seq, i as u32);
        }
    }

    #[test]
    fn spawn_camada_desligada_nao_emite() {
        let (shared, rx, h) = iniciar();
        std::thread::sleep(Duration::from_millis(300));
        parar(&shared, h);
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[test]
    fn spawn_keyframe_pedido() {
        let (shared, rx, h) = iniciar();
        shared.control.lock().requests = vec![(0, 2000)];
        std::thread::sleep(Duration::from_millis(300));
        shared.control.lock().keyframes.push(0);
        std::thread::sleep(Duration::from_millis(300));
        parar(&shared, h);
        let q: Vec<_> = rx.try_iter().collect();
        assert!(q[0].keyframe);
        assert!(q[1..].iter().any(|f| f.keyframe), "sem keyframe pedido");
        assert!(q[1..].iter().any(|f| !f.keyframe));
    }

    #[test]
    fn spawn_para_com_stop() {
        let (shared, _rx, h) = iniciar();
        std::thread::sleep(Duration::from_millis(50));
        let t = Instant::now();
        parar(&shared, h);
        assert!(t.elapsed() < Duration::from_millis(500));
    }
}
