//! Testes de integração do codificador (só Windows; pulam sem GPU/MFT).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::time::{Duration, Instant};

use zap_encode::{logic, nal, EncodedPicture, EncoderChoice, EncoderConfig, H264Encoder};
use zap_gpu::{Converter, GpuDevice, PixelFormat, Texture, TextureRing};

const W: u32 = 1280;
const H: u32 = 720;
const PASSO_US: u64 = 33_333;

fn cfg(choice: EncoderChoice) -> EncoderConfig {
    EncoderConfig {
        width: W,
        height: H,
        fps: 30,
        kbps: 4000,
        gop_frames: 300,
        choice,
    }
}

/// Cria o dispositivo; `None` (com aviso) se não houver GPU ou codificador.
fn preparar() -> Option<(GpuDevice, Vec<zap_encode::EncoderInfo>)> {
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("pulando: sem GPU ({e})");
            return None;
        }
    };
    // Estes testes convertem BGRA → NV12 na GPU antes de codificar.
    if !dev.has_video() {
        eprintln!("pulando: GPU sem Video Processor ({})", dev.adapter().name);
        return None;
    }
    match zap_encode::list_encoders() {
        Ok(l) if !l.is_empty() => Some((dev, l)),
        Ok(_) => {
            eprintln!("pulando: nenhum codificador H.264 encontrado");
            None
        }
        Err(e) => {
            eprintln!("pulando: list_encoders falhou ({e})");
            None
        }
    }
}

/// Gera o quadro `i` já em NV12 (ver `support`).
type Gerador = fn(&GpuDevice, &mut TextureRing, &mut Converter, u32) -> Texture;

struct Sessao {
    saidas: Vec<EncodedPicture>,
    base_us: u64,
    hardware: bool,
    dropped: u64,
}

/// Envia `n` quadros a ~30 fps e coleta as saídas por até 5 s. `bitrate_em` e `forcar_em` são
/// índices de quadro.
fn executar(
    dev: &GpuDevice,
    choice: EncoderChoice,
    n: u32,
    bitrate_em: Option<u32>,
    forcar_em: Option<u32>,
    gerador: Gerador,
) -> Option<Sessao> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let enc = match H264Encoder::new(dev, cfg(choice), tx) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("pulando: não abriu o codificador ({e})");
            return None;
        }
    };
    let mut ring = TextureRing::new(dev, W, H, PixelFormat::Nv12, 4, false).expect("anel NV12");
    let mut conv = Converter::new(dev, W, H, PixelFormat::Bgra8, W, H).expect("conversor");
    let base_us = zap_gpu::now_us();
    let mut saidas = Vec::new();
    for i in 0..n {
        if bitrate_em == Some(i) {
            enc.set_bitrate(2000);
        }
        let tex = gerador(dev, &mut ring, &mut conv, i);
        enc.submit(tex, base_us + u64::from(i) * PASSO_US, forcar_em == Some(i));
        std::thread::sleep(Duration::from_millis(33));
        saidas.extend(rx.try_iter());
    }
    let limite = Instant::now() + Duration::from_secs(5);
    while Instant::now() < limite && (saidas.len() as u64) + enc.dropped() < u64::from(n) {
        if let Ok(p) = rx.recv_timeout(Duration::from_millis(100)) {
            saidas.push(p);
        }
    }
    let hardware = enc.info().hardware;
    let dropped = enc.dropped();
    Some(Sessao {
        saidas,
        base_us,
        hardware,
        dropped,
    })
}

#[test]
fn primeiro_quadro_e_keyframe_com_sps_pps() {
    let Some((dev, _)) = preparar() else { return };
    let Some(s) = executar(&dev, EncoderChoice::Auto, 60, None, None, support::frame_ruido) else {
        return;
    };
    assert!(
        s.saidas.len() >= 55,
        "saídas: {} (descartes {})",
        s.saidas.len(),
        s.dropped
    );
    let p = &s.saidas[0];
    assert!(p.keyframe);
    assert!(p.has_params);
    assert!(nal::has_sps_pps(&p.data));
    assert!(nal::has_idr(&p.data));
}

#[test]
fn set_bitrate_no_meio_nao_gera_keyframe() {
    let Some((dev, _)) = preparar() else { return };
    let Some(s) = executar(&dev, EncoderChoice::Auto, 60, Some(30), None, support::frame_ruido) else {
        return;
    };
    assert!(s.saidas.len() >= 55, "saídas: {}", s.saidas.len());
    let corte = s.base_us + 31 * PASSO_US;
    let depois: Vec<_> = s.saidas.iter().filter(|p| p.capture_us >= corte).collect();
    assert!(!depois.is_empty(), "sem vídeo após set_bitrate");
    if s.hardware {
        assert!(
            depois.iter().all(|p| !p.keyframe),
            "keyframe inesperado após set_bitrate"
        );
    }
}

#[test]
fn force_keyframe_no_30() {
    let Some((dev, _)) = preparar() else { return };
    let Some(s) = executar(&dev, EncoderChoice::Auto, 60, None, Some(30), support::frame_ruido) else {
        return;
    };
    assert!(s.saidas.len() >= 55, "saídas: {}", s.saidas.len());
    let alvo = s.base_us + 30 * PASSO_US;
    let p = s
        .saidas
        .iter()
        .find(|p| p.capture_us == alvo)
        .expect("saída do quadro 30");
    assert!(p.keyframe, "quadro 30 deveria ser keyframe");
}

/// Média de tamanho dos quadros não-chave a partir do quadro `a_partir`.
fn media_nao_chave(s: &Sessao, a_partir: u32) -> f64 {
    let corte = s.base_us + u64::from(a_partir) * PASSO_US;
    let tamanhos: Vec<f64> = s
        .saidas
        .iter()
        .filter(|p| !p.keyframe && p.capture_us >= corte)
        .map(|p| p.data.len() as f64)
        .collect();
    assert!(!tamanhos.is_empty(), "sem quadros não-chave");
    tamanhos.iter().sum::<f64>() / tamanhos.len() as f64
}

/// Compara a média (ruído) com a faixa ±50% de 4000 kbps a 30 fps. Pula se o codificador pedido
/// não existe ou não abre.
fn checar_media(choice: EncoderChoice) {
    let Some((dev, lista)) = preparar() else { return };
    if choice == EncoderChoice::Software && !lista.iter().any(|e| !e.hardware) {
        eprintln!("pulando: sem codificador de software");
        return;
    }
    let Some(s) = executar(&dev, choice, 60, None, None, support::frame_ruido) else {
        return;
    };
    assert!(s.saidas.len() >= 55, "saídas: {}", s.saidas.len());
    let media = media_nao_chave(&s, 10);
    let esperado = f64::from(logic::expected_frame_bytes(4000, 30));
    eprintln!("tamanho médio ({choice:?}, hw={}): {media:.0} bytes", s.hardware);
    assert!(
        media >= esperado * 0.5 && media <= esperado * 1.5,
        "média {media} fora de ±50% de {esperado}"
    );
}

#[test]
fn tamanho_medio_nao_chave() {
    checar_media(EncoderChoice::Auto);
}

#[test]
fn tamanho_medio_nao_chave_software() {
    checar_media(EncoderChoice::Software);
}

/// Conteúdo estático (cor sólida) não pode gastar mais que 1.5x do orçamento por quadro.
#[test]
fn conteudo_estatico_nao_passa_do_orcamento() {
    let Some((dev, _)) = preparar() else { return };
    let Some(s) = executar(&dev, EncoderChoice::Auto, 60, None, None, support::frame_estatico) else {
        return;
    };
    assert!(s.saidas.len() >= 55, "saídas: {}", s.saidas.len());
    let media = media_nao_chave(&s, 10);
    let teto = 1.5 * f64::from(logic::expected_frame_bytes(4000, 30));
    eprintln!("tamanho médio estático (hw={}): {media:.0} bytes", s.hardware);
    assert!(media <= teto, "média {media} acima de {teto}");
}

#[test]
fn software_funciona() {
    let Some((dev, lista)) = preparar() else { return };
    if !lista.iter().any(|e| !e.hardware) {
        eprintln!("pulando: sem codificador de software");
        return;
    }
    let Some(s) = executar(&dev, EncoderChoice::Software, 30, None, None, support::frame_ruido) else {
        return;
    };
    assert!(!s.hardware);
    let ok = s
        .saidas
        .iter()
        .any(|p| p.keyframe && p.has_params && nal::has_sps_pps(&p.data));
    assert!(ok, "nenhum keyframe com SPS/PPS");
}

#[test]
fn drop_encerra_thread() {
    let Some((dev, _)) = preparar() else { return };
    for _ in 0..5 {
        let (tx, _rx) = crossbeam_channel::unbounded();
        match H264Encoder::new(&dev, cfg(EncoderChoice::Auto), tx) {
            Ok(enc) => drop(enc),
            Err(e) => {
                eprintln!("pulando: não abriu o codificador ({e})");
                return;
            }
        }
    }
}
