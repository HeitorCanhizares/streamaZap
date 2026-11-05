use super::*;
use crate::{ConversionSpec, ConverterKind, VideoPipeline, VideoPipelineOptions};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;
use zap_capture::movimento::JanelaAnimada;
use zap_rate::allocator::LayerRequest;

fn camada(layer: u8, w: u16, h: u16) -> LayerSpec {
    camada_fps(layer, w, h, 30)
}

fn camada_fps(layer: u8, w: u16, h: u16, fps: u8) -> LayerSpec {
    LayerSpec {
        layer,
        width: w,
        height: h,
        fps,
        kbps: 3000,
    }
}

/// O Windows só deixa uma duplicação do mesmo monitor por processo: em paralelo, o segundo
/// `DuplicateOutput` dá `E_INVALIDARG` e o teste pulava sem rodar. Os testes do pipeline entram
/// nesta fila e a vez só é solta depois do pipeline (que para a captura no `Drop`).
static MONITOR_1: Mutex<()> = Mutex::new(());

/// Janela animada e a vez no monitor 1. Fica na primeira posição da tupla de `iniciar_com`, então
/// é solta depois do pipeline e do canal (variáveis caem na ordem inversa da declaração).
struct Ambiente {
    _janela: JanelaAnimada,
    _vez: MutexGuard<'static, ()>,
}

/// Para os testes de GPU antigos: exige também o Video Processor.
fn iniciar(ladder: Vec<LayerSpec>) -> Option<(Ambiente, VideoPipeline, Receiver<EncodedFrame>)> {
    if !GpuDevice::create_default().is_ok_and(|d| d.has_video()) {
        eprintln!("PULANDO: sem GPU com Video Processor");
        return None;
    }
    iniciar_com(ladder, VideoPipelineOptions::default(), false)
}

/// `None` (com aviso) quando o ambiente não tem GPU ou monitor. Não exige Video Processor.
fn iniciar_com(
    ladder: Vec<LayerSpec>,
    opts: VideoPipelineOptions,
    sem_vp: bool,
) -> Option<(Ambiente, VideoPipeline, Receiver<EncodedFrame>)> {
    if GpuDevice::create_default().is_err() {
        eprintln!("PULANDO: sem GPU");
        return None;
    }
    // Um teste que entrou em pânico envenena a trava; a vez continua valendo.
    let vez = MONITOR_1.lock().unwrap_or_else(|e| e.into_inner());
    let monitor1 = match zap_capture::list_monitors() {
        Ok(m) => m.into_iter().find(|m| m.index == 1),
        Err(_) => None,
    };
    let Some(monitor1) = monitor1 else {
        eprintln!("PULANDO: sem monitor 1");
        return None;
    };
    let janela = match JanelaAnimada::abrir(&monitor1) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("PULANDO: sem janela animada ({e})");
            return None;
        }
    };
    let (tx, rx) = crossbeam_channel::bounded(256);
    let cfg = VideoPipelineConfig {
        track: 1,
        source: VideoSourceSpec::Monitor(MonitorRef(1)),
        ladder,
        encoder: EncoderChoiceSpec::Auto,
    };
    let r = if sem_vp {
        VideoPipeline::start_sem_vp(cfg, tx, opts)
    } else {
        VideoPipeline::start_with(cfg, tx, opts)
    };
    match r {
        Ok(p) => Some((
            Ambiente {
                _janela: janela,
                _vez: vez,
            },
            p,
            rx,
        )),
        Err(e) => {
            eprintln!("PULANDO: não iniciou ({e})");
            None
        }
    }
}

fn opts(conversion: ConversionSpec) -> VideoPipelineOptions {
    VideoPipelineOptions { conversion }
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
    let Some((_janela, p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let v = coletar(&rx, Duration::from_secs(2));
    assert!(v.len() >= 15, "só {} quadros", v.len());
    assert!(v[0].keyframe && v[0].has_params);
    assert!(v.iter().all(|f| f.layer == 0 && f.width == 1280 && f.height == 720));
    assert_eq!(p.stats().layers[0].converter, Some(ConverterKind::Gpu));
}

#[test]
fn desligar_camada_para_de_emitir() {
    let Some((_janela, p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
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
    let Some((_janela, p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
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
    let Some((_janela, p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
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

#[test]
fn stats_trazem_gpu_e_captura_nova() {
    let Some((_janela, p, rx)) = iniciar(vec![camada(0, 1280, 720)]) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let _ = coletar(&rx, Duration::from_secs(2));
    let s = p.stats();
    assert!(s.capture_new_fps > 5.0, "capture_new_fps = {}", s.capture_new_fps);
    let l = &s.layers[0];
    assert!(l.lock_wait.p95_us.is_finite() && l.lock_wait.p95_us >= 0.0);
    assert!(l.convert_gpu.p95_us.is_finite() && l.convert_gpu.p95_us >= 0.0);
}

#[test]
fn cpu_gera_quadros() {
    let Some((_janela, p, rx)) = iniciar_com(vec![camada(0, 1280, 720)], opts(ConversionSpec::Cpu), false) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let v = coletar(&rx, Duration::from_secs(2));
    assert!(v.len() >= 15, "só {} quadros", v.len());
    assert!(v[0].keyframe && v[0].has_params);
    assert!(v.iter().all(|f| f.width == 1280 && f.height == 720));
    let s = p.stats();
    assert_eq!(s.layers[0].converter, Some(ConverterKind::Cpu));
    assert!(s.cpu_readback.p95_us > 0.0, "cpu_readback = {:?}", s.cpu_readback);
    assert!(s.cpu_threads >= 1);
}

#[test]
fn cpu_duas_camadas() {
    let ladder = vec![camada(0, 1280, 720), camada_fps(1, 640, 360, 15)];
    let Some((_janela, p, rx)) = iniciar_com(ladder, opts(ConversionSpec::Cpu), false) else {
        return;
    };
    p.set_requests(&[req(0), req(1)]);
    let v = coletar(&rx, Duration::from_secs(3));
    let c0 = v.iter().filter(|f| f.layer == 0).count();
    let c1 = v.iter().filter(|f| f.layer == 1).count();
    assert!(c0 >= 30, "camada 0: só {c0} quadros");
    assert!(c1 >= 15, "camada 1: só {c1} quadros");
    let s = p.stats();
    assert_eq!(s.layers[0].converter, Some(ConverterKind::Cpu));
    assert_eq!(s.layers[1].converter, Some(ConverterKind::Cpu));
}

#[test]
fn cpu_keyframe_pedido_chega() {
    let Some((_janela, p, rx)) = iniciar_com(vec![camada(0, 1280, 720)], opts(ConversionSpec::Cpu), false) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let _ = coletar(&rx, Duration::from_secs(1));
    p.request_keyframe(0);
    let v = coletar(&rx, Duration::from_secs(2));
    assert!(v.iter().any(|f| f.keyframe), "nenhum keyframe após o pedido");
}

#[test]
fn auto_cai_para_cpu_sem_vp() {
    let Some((_janela, p, rx)) = iniciar_com(vec![camada(0, 1280, 720)], opts(ConversionSpec::Auto), true) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let v = coletar(&rx, Duration::from_secs(2));
    assert!(v.len() >= 15, "só {} quadros", v.len());
    assert_eq!(p.stats().layers[0].converter, Some(ConverterKind::Cpu));
}

#[test]
fn gpu_sem_vp_nao_cai() {
    let Some((_janela, p, rx)) = iniciar_com(vec![camada(0, 1280, 720)], opts(ConversionSpec::Gpu), true) else {
        return;
    };
    p.set_requests(&[req(0)]);
    let v = coletar(&rx, Duration::from_secs(2));
    assert!(v.is_empty(), "{} quadros sem Video Processor", v.len());
    assert_eq!(p.stats().layers[0].converter, None);
}
