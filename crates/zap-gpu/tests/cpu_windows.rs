#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Testes do Windows: conversão de CPU (`CpuScaler`) contra a de GPU (`Converter`).
//! Sem GPU D3D11 (ou sem Video Processor) os testes pulam com mensagem em vez de falhar.

use zap_gpu::{
    read_back, upload, BatchOptions, Converter, CpuScaler, GpuDevice, GpuError, Nv12Mut, PixelFormat, ReadbackStaging,
    SrcImage, Texture, Vendor,
};

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

/// A saída da CPU de uma cor sólida fica a ±1 da fórmula em todo pixel (garante que CPU e GPU não
/// estão iguais por estarem erradas juntas).
fn conferir_formula(cpu: &[u8], w: u32, h: u32, (r, g, b): (u8, u8, u8), ctx: &str) {
    let (ey, eu, ev) = esperado(r, g, b);
    let perto = |v: u8, e: f64| (v as f64 - e.round()).abs() <= 1.0;
    let (y, uv) = planos(cpu, w, h);
    assert!(y.iter().all(|&v| perto(v, ey)), "{ctx}: Y da CPU longe de {ey:.1}");
    assert!(
        uv.chunks_exact(2).all(|p| perto(p[0], eu) && perto(p[1], ev)),
        "{ctx}: UV da CPU longe de ({eu:.1}, {ev:.1})"
    );
}

const CORES: [(u8, u8, u8); 5] = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255), (0, 0, 0)];

/// Gradiente BGRA: R = x*255/(w-1), G = y*255/(h-1), B = (x+y)*255/(w+h-2).
fn bgra_gradiente(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = x * 255 / (w - 1);
            let g = y * 255 / (h - 1);
            let b = (x + y) * 255 / (w + h - 2);
            v.extend_from_slice(&[b as u8, g as u8, r as u8, 255]);
        }
    }
    v
}

/// Gerador congruencial linear (semente 12345 no padrão "tela").
struct Lcg(u32);

impl Lcg {
    fn proximo(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 16) as u8
    }
}

/// Padrão "tela" BGRA: blocos 37x23 coloridos, linhas brancas nas bordas dos blocos e tabuleiro
/// de 1 px nos blocos com `(bx + by) % 3 == 0` (o mesmo de `examples/cpu_nv12.rs`).
fn padrao_tela(w: u32, h: u32) -> Vec<u8> {
    const BW: usize = 37;
    const BH: usize = 23;
    let (w, h) = (w as usize, h as usize);
    let mut px = vec![0u8; w * h * 4];
    let mut lcg = Lcg(12345);
    for by in 0..h.div_ceil(BH) {
        for bx in 0..w.div_ceil(BW) {
            let (x0, y0) = (bx * BW, by * BH);
            let (x1, y1) = ((x0 + BW).min(w), (y0 + BH).min(h));
            let cor = [lcg.proximo(), lcg.proximo(), lcg.proximo()];
            let tabuleiro = (bx + by) % 3 == 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (b, g, r) = if x == x0 || y == y0 {
                        (255, 255, 255)
                    } else if tabuleiro {
                        if (x + y) % 2 == 0 {
                            (0, 0, 0)
                        } else {
                            (255, 255, 255)
                        }
                    } else {
                        (cor[0], cor[1], cor[2])
                    };
                    let i = (y * w + x) * 4;
                    px[i..i + 4].copy_from_slice(&[b, g, r, 255]);
                }
            }
        }
    }
    px
}

/// PSNR em dB; 99 quando idênticos.
fn psnr(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let soma: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = x as f64 - y as f64;
            d * d
        })
        .sum();
    if soma == 0.0 {
        return 99.0;
    }
    10.0 * (255.0f64 * 255.0 / (soma / a.len() as f64)).log10()
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y)).max().unwrap_or(0)
}

/// Separa NV12 compacto em (Y, UV).
fn planos(nv12: &[u8], w: u32, h: u32) -> (&[u8], &[u8]) {
    nv12.split_at((w * h) as usize)
}

/// Converte na CPU o conteúdo de uma textura (via staging de leitura) e devolve NV12 compacto.
fn cpu_de_textura(dev: &GpuDevice, tex: &Texture, in_w: u32, in_h: u32, out_w: u32, out_h: u32) -> Vec<u8> {
    let mut stg = ReadbackStaging::new(dev, in_w, in_h, tex.format()).expect("staging");
    let (mapeado, _t) = stg.map(dev, tex).expect("map");
    let scaler = CpuScaler::new(in_w, in_h, tex.format(), out_w, out_h).expect("scaler");
    let mut saida = vec![0u8; (out_w * out_h * 3 / 2) as usize];
    let mut dst = Nv12Mut::from_contiguous(&mut saida, out_w as usize, out_w, out_h).expect("destino");
    scaler
        .convert(&mapeado.image(), &mut dst, BatchOptions::default())
        .expect("convert CPU");
    drop(mapeado);
    saida
}

/// GPU: converte `tex` (BGRA ou NV12) para `ow x oh` e lê o NV12 compacto. `None` se sem VP.
fn gpu_de_textura(dev: &GpuDevice, tex: &Texture, ow: u32, oh: u32) -> Option<Vec<u8>> {
    let mut conv = conversor(dev, tex.width(), tex.height(), tex.format(), ow, oh)?;
    let dst = Texture::new(dev, ow, oh, PixelFormat::Nv12, false).expect("dst");
    conv.convert(dev, tex, &dst).expect("convert GPU");
    Some(read_back(dev, &dst).expect("read_back"))
}

fn textura_bgra(dev: &GpuDevice, w: u32, h: u32, dados: &[u8]) -> Texture {
    let t = Texture::new(dev, w, h, PixelFormat::Bgra8, false).expect("src");
    upload(dev, &t, dados, w * 4).expect("upload");
    t
}

/// Números da comparação GPU x CPU de uma saída.
struct Medida {
    max_y: u8,
    max_uv: u8,
    psnr_y: f64,
    psnr_u: f64,
    psnr_v: f64,
}

fn comparar(rotulo: &str, gpu: &[u8], cpu: &[u8], w: u32, h: u32) -> Medida {
    let (gy, guv) = planos(gpu, w, h);
    let (cy, cuv) = planos(cpu, w, h);
    let separa = |uv: &[u8], off: usize| -> Vec<u8> { uv.iter().skip(off).step_by(2).copied().collect() };
    let (gu, gv) = (separa(guv, 0), separa(guv, 1));
    let (cu, cv) = (separa(cuv, 0), separa(cuv, 1));
    let m = Medida {
        max_y: max_diff(gy, cy),
        max_uv: max_diff(guv, cuv),
        psnr_y: psnr(gy, cy),
        psnr_u: psnr(&gu, &cu),
        psnr_v: psnr(&gv, &cv),
    };
    eprintln!(
        "{rotulo} {w}x{h}: max Y/U/V = {}/{}/{}; PSNR Y/U/V = {:.1}/{:.1}/{:.1} dB",
        m.max_y,
        max_diff(&gu, &cu),
        max_diff(&gv, &cv),
        m.psnr_y,
        m.psnr_u,
        m.psnr_v
    );
    m
}

fn exigir(m: &Medida, max: u8, psnr_min: f64, ctx: &str) {
    assert!(m.max_y <= max, "{ctx}: máxima de Y {} > {max}", m.max_y);
    assert!(m.max_uv <= max, "{ctx}: máxima de UV {} > {max}", m.max_uv);
    assert!(m.psnr_y >= psnr_min, "{ctx}: PSNR Y {:.1} < {psnr_min}", m.psnr_y);
    assert!(m.psnr_u >= psnr_min, "{ctx}: PSNR U {:.1} < {psnr_min}", m.psnr_u);
    assert!(m.psnr_v >= psnr_min, "{ctx}: PSNR V {:.1} < {psnr_min}", m.psnr_v);
}

const SAIDAS: [(u32, u32); 4] = [(1920, 1080), (1280, 720), (960, 540), (640, 360)];

#[test]
fn cpu_igual_gpu_cores_e_gradiente() {
    let Some(dev) = device() else { return };
    for (r, g, b) in CORES {
        let src = textura_bgra(&dev, 1920, 1080, &bgra_solido(1920, 1080, r, g, b));
        for (ow, oh) in SAIDAS {
            let Some(gpu) = gpu_de_textura(&dev, &src, ow, oh) else {
                return;
            };
            let cpu = cpu_de_textura(&dev, &src, 1920, 1080, ow, oh);
            let ctx = format!("sólido ({r},{g},{b})");
            conferir_formula(&cpu, ow, oh, (r, g, b), &ctx);
            let m = comparar(&ctx, &gpu, &cpu, ow, oh);
            assert!(
                m.max_y <= 1 && m.max_uv <= 1,
                "sólido ({r},{g},{b}) {ow}x{oh} acima de 1"
            );
        }
    }
    let src = textura_bgra(&dev, 1920, 1080, &bgra_gradiente(1920, 1080));
    for (ow, oh) in SAIDAS {
        let Some(gpu) = gpu_de_textura(&dev, &src, ow, oh) else {
            return;
        };
        let cpu = cpu_de_textura(&dev, &src, 1920, 1080, ow, oh);
        let m = comparar("gradiente", &gpu, &cpu, ow, oh);
        exigir(&m, 2, 50.0, &format!("gradiente {ow}x{oh}"));
    }
}

#[test]
fn cpu_igual_gpu_conteudo_nitido() {
    let Some(dev) = device() else { return };
    let nvidia = dev.adapter().vendor == Vendor::Nvidia;
    let src = textura_bgra(&dev, 1920, 1080, &padrao_tela(1920, 1080));
    for (ow, oh) in SAIDAS {
        let Some(gpu) = gpu_de_textura(&dev, &src, ow, oh) else {
            return;
        };
        let cpu = cpu_de_textura(&dev, &src, 1920, 1080, ow, oh);
        let m = comparar("tela", &gpu, &cpu, ow, oh);
        if nvidia {
            exigir(&m, 2, 50.0, &format!("tela {ow}x{oh}"));
        } else {
            eprintln!("fabricante não NVIDIA: só imprimindo");
        }
    }
}

/// Converte BGRA -> NV12 compacto na CPU (1:1) para servir de origem NV12.
fn nv12_por_cpu(bgra: &[u8], w: u32, h: u32) -> Vec<u8> {
    let sc = CpuScaler::new(w, h, PixelFormat::Bgra8, w, h).expect("scaler");
    let src = SrcImage::Bgra {
        data: bgra,
        stride: (w * 4) as usize,
        width: w,
        height: h,
    };
    let mut saida = vec![0u8; (w * h * 3 / 2) as usize];
    let mut dst = Nv12Mut::from_contiguous(&mut saida, w as usize, w, h).expect("destino");
    sc.convert(&src, &mut dst, BatchOptions::default()).expect("convert");
    saida
}

#[test]
fn nv12_cpu_igual_gpu() {
    let Some(dev) = device() else { return };
    let (iw, ih) = (1280u32, 720u32);
    let origens = [("gradiente", bgra_gradiente(iw, ih)), ("tela", padrao_tela(iw, ih))];
    for (nome, bgra) in origens {
        let nv12 = nv12_por_cpu(&bgra, iw, ih);
        let src = Texture::new(&dev, iw, ih, PixelFormat::Nv12, false).expect("src");
        if let Err(e) = upload(&dev, &src, &nv12, iw) {
            eprintln!("PULANDO: upload NV12 falhou: {e}");
            return;
        }
        for (ow, oh) in [(1280u32, 720u32), (960, 540), (640, 360)] {
            let Some(gpu) = gpu_de_textura(&dev, &src, ow, oh) else {
                return;
            };
            let cpu = cpu_de_textura(&dev, &src, iw, ih, ow, oh);
            let m = comparar(&format!("nv12 {nome}"), &gpu, &cpu, ow, oh);
            if (ow, oh) == (iw, ih) {
                assert_eq!(gpu, cpu, "nv12 {nome} 1:1 deveria ser idêntico");
            } else {
                exigir(&m, 2, 50.0, &format!("nv12 {nome} {ow}x{oh}"));
            }
        }
    }
}

#[test]
fn cpu_igual_gpu_entrada_impar() {
    let Some(dev) = device() else { return };
    for ((iw, ih), (ow, oh)) in [((1001u32, 777u32), (1002u32, 778u32)), ((641, 361), (642, 360))] {
        for (r, g, b) in CORES {
            let src = textura_bgra(&dev, iw, ih, &bgra_solido(iw, ih, r, g, b));
            let Some(gpu) = gpu_de_textura(&dev, &src, ow, oh) else {
                return;
            };
            let cpu = cpu_de_textura(&dev, &src, iw, ih, ow, oh);
            let ctx = format!("ímpar {iw}x{ih} ({r},{g},{b})");
            conferir_formula(&cpu, ow, oh, (r, g, b), &ctx);
            let m = comparar(&ctx, &gpu, &cpu, ow, oh);
            assert!(
                m.max_y <= 1 && m.max_uv <= 1,
                "ímpar {iw}x{ih}->{ow}x{oh} ({r},{g},{b}) acima de 1"
            );
        }
    }
}
