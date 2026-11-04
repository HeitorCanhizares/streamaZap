#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Testes de integração no Windows: cores BT.709 limitada, escala e anel de texturas.
//! Sem GPU D3D11 (ou sem Video Processor) os testes pulam com mensagem em vez de falhar.

use zap_gpu::{now_us, read_back, upload, ComGuard, Converter, GpuDevice, GpuError, PixelFormat, Texture, TextureRing};

fn device() -> Option<GpuDevice> {
    match GpuDevice::create_default() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("PULANDO: sem GPU D3D11: {e}");
            None
        }
    }
}

/// Cria o conversor; `Unsupported`/`Os` (WARP, sem Video Processor) viram pulo.
fn conversor(dev: &GpuDevice, iw: u32, ih: u32, fmt: PixelFormat, ow: u32, oh: u32) -> Option<Converter> {
    match Converter::new(dev, iw, ih, fmt, ow, oh) {
        Ok(c) => Some(c),
        Err(e @ (GpuError::Unsupported | GpuError::Os { .. })) => {
            eprintln!("PULANDO: conversor indisponível: {e}");
            None
        }
        Err(e) => panic!("Converter::new falhou: {e}"),
    }
}

/// Quadro BGRA sólido com stride justo.
fn bgra_solido(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
    [b, g, r, 255].repeat((w * h) as usize)
}

/// (Y, U, V) esperados em BT.709 faixa limitada.
fn esperado(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    (
        16.0 + 219.0 * y,
        128.0 + 224.0 * (b - y) / 1.8556,
        128.0 + 224.0 * (r - y) / 1.5748,
    )
}

/// Converte um quadro sólido e devolve (Y, U, V) do pixel central.
fn converter_solido(dev: &GpuDevice, iw: u32, ih: u32, ow: u32, oh: u32, rgb: (u8, u8, u8)) -> Option<(u8, u8, u8)> {
    let mut conv = conversor(dev, iw, ih, PixelFormat::Bgra8, ow, oh)?;
    let src = Texture::new(dev, iw, ih, PixelFormat::Bgra8, false).expect("src");
    upload(dev, &src, &bgra_solido(iw, ih, rgb.0, rgb.1, rgb.2), iw * 4).expect("upload");
    let dst = Texture::new(dev, ow, oh, PixelFormat::Nv12, false).expect("dst");
    conv.convert(dev, &src, &dst).expect("convert");
    let buf = read_back(dev, &dst).expect("read_back");
    let (w, h) = (ow as usize, oh as usize);
    let (cx, cy) = (w / 2, h / 2);
    let y = buf[cy * w + cx];
    let uv = w * h + (cy / 2) * w + (cx / 2) * 2;
    Some((y, buf[uv], buf[uv + 1]))
}

fn perto(obtido: u8, esperado: f64, nome: &str, ctx: &str) {
    let e = esperado.round();
    assert!(
        (obtido as f64 - e).abs() <= 3.0,
        "{nome} fora da tolerância em {ctx}: obtido {obtido}, esperado {e}"
    );
}

#[test]
fn cores_bt709() {
    let Some(dev) = device() else { return };
    for (r, g, b) in [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255), (0, 0, 0)] {
        let Some((y, u, v)) = converter_solido(&dev, 64, 64, 64, 64, (r, g, b)) else {
            return;
        };
        let (ey, eu, ev) = esperado(r, g, b);
        let ctx = format!("({r},{g},{b})");
        perto(y, ey, "Y", &ctx);
        perto(u, eu, "U", &ctx);
        perto(v, ev, "V", &ctx);
    }
}

#[test]
fn escala_1080_para_720() {
    let Some(dev) = device() else { return };
    let Some((y, _, _)) = converter_solido(&dev, 1920, 1080, 1280, 720, (0, 128, 255)) else {
        return;
    };
    perto(y, esperado(0, 128, 255).0, "Y", "escala 1080->720");
}

#[test]
fn nv12_para_nv12_escala() {
    let Some(dev) = device() else { return };
    let Some(mut conv) = conversor(&dev, 128, 128, PixelFormat::Nv12, 64, 64) else {
        return;
    };
    let src = Texture::new(&dev, 128, 128, PixelFormat::Nv12, false).expect("src");
    // Y constante 100, UV neutro 128.
    let mut dados = vec![100u8; 128 * 128];
    dados.extend(vec![128u8; 128 * 64]);
    upload(&dev, &src, &dados, 128).expect("upload");
    let dst = Texture::new(&dev, 64, 64, PixelFormat::Nv12, false).expect("dst");
    conv.convert(&dev, &src, &dst).expect("convert");
    let buf = read_back(&dev, &dst).expect("read_back");
    perto(buf[32 * 64 + 32], 100.0, "Y", "nv12->nv12");
}

#[test]
fn ring_e_converter_em_loop() {
    let Some(dev) = device() else { return };
    let Some(mut conv) = conversor(&dev, 64, 64, PixelFormat::Bgra8, 64, 64) else {
        return;
    };
    let src = Texture::new(&dev, 64, 64, PixelFormat::Bgra8, false).expect("src");
    upload(&dev, &src, &bgra_solido(64, 64, 10, 20, 30), 256).expect("upload");
    let mut ring = TextureRing::new(&dev, 64, 64, PixelFormat::Nv12, 4, false).expect("ring");
    for i in 0..30 {
        let dst = ring.next();
        conv.convert(&dev, &src, dst)
            .unwrap_or_else(|e| panic!("conversão {i} falhou: {e}"));
    }
}

#[test]
fn now_us_monotonico_windows() {
    let a = now_us();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = now_us();
    assert!(b > a, "relógio não avançou: {a} -> {b}");
    assert!(b - a < 5_000_000, "salto absurdo: {}", b - a);
}

#[test]
fn com_guard_ok() {
    let g = ComGuard::new();
    assert!(g.is_ok(), "ComGuard falhou: {:?}", g.err());
}
