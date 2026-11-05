//! Latência de codificação do MFT de hardware (só Windows; pula sem hardware).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::time::{Duration, Instant};

use zap_encode::{EncoderChoice, EncoderConfig, H264Encoder};
use zap_gpu::{Converter, GpuDevice, PixelFormat, TextureRing};

const W: u32 = 1280;
const H: u32 = 720;
const PASSO_US: u64 = 33_333;
const QUADROS: u32 = 60;
/// Os primeiros quadros pagam o aquecimento do codificador; não entram na estatística.
const AQUECIMENTO: u64 = 5;
/// Limite de p95 de `encode_us` (submit até saída) para hardware.
const LIMITE_P95_MS: f64 = 12.0;

fn cfg() -> EncoderConfig {
    EncoderConfig {
        width: W,
        height: H,
        fps: 30,
        kbps: 4000,
        gop_frames: 300,
        choice: EncoderChoice::Hardware,
    }
}

/// Cria o dispositivo e confere se há codificador de hardware; `None` (com aviso) caso contrário.
fn preparar() -> Option<GpuDevice> {
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("pulando: sem GPU ({e})");
            return None;
        }
    };
    // O teste converte BGRA → NV12 na GPU antes de codificar.
    if !dev.has_video() {
        eprintln!("pulando: GPU sem Video Processor ({})", dev.adapter().name);
        return None;
    }
    match zap_encode::list_encoders() {
        Ok(l) if l.iter().any(|e| e.hardware) => Some(dev),
        Ok(_) => {
            eprintln!("pulando: nenhum codificador de hardware");
            None
        }
        Err(e) => {
            eprintln!("pulando: list_encoders falhou ({e})");
            None
        }
    }
}

#[test]
fn latencia_hardware() {
    let Some(dev) = preparar() else { return };
    let (tx, rx) = crossbeam_channel::unbounded();
    let enc = match H264Encoder::new(&dev, cfg(), tx) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("pulando: não abriu o codificador ({e})");
            return;
        }
    };
    let mut ring = TextureRing::new(&dev, W, H, PixelFormat::Nv12, 4, false).expect("anel NV12");
    let mut conv = Converter::new(&dev, W, H, PixelFormat::Bgra8, W, H).expect("conversor");

    // Envio a ~30 fps: uma saída por quadro de captura.
    let base_us = zap_gpu::now_us();
    let mut saidas = Vec::new();
    for i in 0..QUADROS {
        let tex = support::frame_ruido(&dev, &mut ring, &mut conv, i);
        enc.submit(tex, base_us + u64::from(i) * PASSO_US, false);
        std::thread::sleep(Duration::from_millis(33));
        saidas.extend(rx.try_iter());
    }

    // Coleta o restante por até 5 s depois do último envio.
    let limite = Instant::now() + Duration::from_secs(5);
    while Instant::now() < limite && (saidas.len() as u64) + enc.dropped() < u64::from(QUADROS) {
        if let Ok(p) = rx.recv_timeout(Duration::from_millis(100)) {
            saidas.push(p);
        }
    }

    assert!(
        saidas.len() >= 55,
        "saídas: {} (descartes {})",
        saidas.len(),
        enc.dropped()
    );

    // Tempo dentro do codificador, só dos quadros após o aquecimento (índice >= 5).
    let corte = base_us + AQUECIMENTO * PASSO_US;
    let mut ms: Vec<f64> = saidas
        .iter()
        .filter(|p| p.capture_us >= corte)
        .map(|p| f64::from(p.encode_us) / 1000.0)
        .collect();
    assert!(!ms.is_empty(), "sem quadros após o aquecimento");
    ms.sort_by(|a, b| a.total_cmp(b));

    let n = ms.len();
    let media_ms = ms.iter().sum::<f64>() / n as f64;
    let idx_p95 = ((0.95 * n as f64).ceil() as usize).saturating_sub(1);
    let p95_ms = ms[idx_p95.min(n - 1)];

    eprintln!("latencia_hardware: n={n} media={media_ms:.2} ms p95={p95_ms:.2} ms");
    assert!(
        p95_ms <= LIMITE_P95_MS,
        "p95 de encode_us {p95_ms:.2} ms acima de {LIMITE_P95_MS} ms"
    );
}

/// Intervalo entre envios no teste de saída; também é o tempo máximo de espera por cada saída.
const CICLO_SAIDA: Duration = Duration::from_millis(100);
/// Quanto a saída pode demorar após o submit para não segurar o quadro até o seguinte.
const LIMITE_SAIDA: Duration = Duration::from_millis(40);
const QUADROS_SAIDA: u32 = 20;
/// Os primeiros quadros pagam o aquecimento; são enviados e drenados, mas não entram no assert.
const AQUECIMENTO_SAIDA: u32 = 3;

#[test]
fn saida_sai_antes_do_proximo_quadro() {
    let Some(dev) = preparar() else { return };
    let (tx, rx) = crossbeam_channel::unbounded();
    let enc = match H264Encoder::new(&dev, cfg(), tx) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("pulando: não abriu o codificador ({e})");
            return;
        }
    };
    let mut ring = TextureRing::new(&dev, W, H, PixelFormat::Nv12, 4, false).expect("anel NV12");
    let mut conv = Converter::new(&dev, W, H, PixelFormat::Bgra8, W, H).expect("conversor");

    let mut maior = Duration::ZERO;
    for i in 0..QUADROS_SAIDA {
        let tex = support::frame_ruido(&dev, &mut ring, &mut conv, i);
        let captura = zap_gpu::now_us();
        let t0 = Instant::now();
        enc.submit(tex, captura, false);

        // Espera a saída deste quadro (casada pelo carimbo de captura); saídas de quadros
        // anteriores que chegarem antes são descartadas.
        let limite = t0 + CICLO_SAIDA;
        let mut saida = None;
        while saida.is_none() {
            let restante = limite.saturating_duration_since(Instant::now());
            match rx.recv_timeout(restante) {
                Ok(p) if p.capture_us == captura => saida = Some(t0.elapsed()),
                Ok(_) => {}
                Err(_) => break,
            }
        }

        match saida {
            None => assert!(i < AQUECIMENTO_SAIDA, "quadro {i} não saiu em 100 ms"),
            Some(e) if i >= AQUECIMENTO_SAIDA => {
                assert!(
                    e <= LIMITE_SAIDA,
                    "quadro {i} saiu em {e:?}, acima de 40 ms após o submit"
                );
                maior = maior.max(e);
            }
            Some(_) => {}
        }

        // Completa o ciclo de 100 ms antes do próximo envio.
        std::thread::sleep(CICLO_SAIDA.saturating_sub(t0.elapsed()));
    }

    eprintln!(
        "saida_sai_antes_do_proximo_quadro: maior tempo até a saída = {:.2} ms",
        maior.as_secs_f64() * 1000.0
    );
}
