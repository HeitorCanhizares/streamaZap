#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Testes de integração das stagings no Windows: leitura, escrita alternada e lock do dispositivo.
//! Sem GPU D3D11 (ou sem suporte a NV12 na staging) os testes pulam com mensagem em vez de falhar.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use zap_gpu::{read_back, upload, GpuDevice, PixelFormat, ReadbackStaging, SrcImage, Texture, UploadStaging};

fn device() -> Option<GpuDevice> {
    match GpuDevice::create_default() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("PULANDO: sem GPU D3D11: {e}");
            None
        }
    }
}

/// Bytes pseudoaleatórios (LCG de semente fixa).
fn ruido(n: usize, semente: u64) -> Vec<u8> {
    let mut s = semente;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as u8
        })
        .collect()
}

/// Junta as linhas úteis de uma imagem mapeada, sem padding (mesmo layout do `read_back`).
fn compacta(img: &SrcImage<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    match *img {
        SrcImage::Bgra {
            data,
            stride,
            width,
            height,
        } => {
            for r in 0..height as usize {
                out.extend_from_slice(&data[r * stride..r * stride + width as usize * 4]);
            }
        }
        SrcImage::Nv12 {
            y,
            y_stride,
            uv,
            uv_stride,
            width,
            height,
        } => {
            for r in 0..height as usize {
                out.extend_from_slice(&y[r * y_stride..r * y_stride + width as usize]);
            }
            for r in 0..(height / 2) as usize {
                out.extend_from_slice(&uv[r * uv_stride..r * uv_stride + width as usize]);
            }
        }
    }
    out
}

/// Cria a textura e sobe os dados; `None` (pulo) se o formato não puder ser enviado.
fn textura_com(dev: &GpuDevice, w: u32, h: u32, fmt: PixelFormat, dados: &[u8]) -> Option<Texture> {
    let tex = match Texture::new(dev, w, h, fmt, false) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("PULANDO: textura {fmt:?} indisponível: {e}");
            return None;
        }
    };
    let stride = if fmt == PixelFormat::Bgra8 { w * 4 } else { w };
    match upload(dev, &tex, dados, stride) {
        Ok(()) => Some(tex),
        Err(e) => {
            eprintln!("PULANDO: upload {fmt:?} falhou: {e}");
            None
        }
    }
}

#[test]
fn readback_igual_read_back() {
    let Some(dev) = device() else { return };
    let (w, h) = (640u32, 360u32);
    for fmt in [PixelFormat::Bgra8, PixelFormat::Nv12] {
        let n = fmt.frame_bytes(w, h);
        let a = ruido(n, 1);
        let b = ruido(n, 2);
        let Some(tex) = textura_com(&dev, w, h, fmt, &a) else {
            continue;
        };
        let mut st = ReadbackStaging::new(&dev, w, h, fmt).expect("staging");
        {
            let (m, _) = st.map(&dev, &tex).expect("map");
            assert_eq!(
                compacta(&m.image()),
                read_back(&dev, &tex).expect("read_back"),
                "{fmt:?} a"
            );
        }
        // Segunda leitura da mesma staging, com outro conteúdo.
        let stride = if fmt == PixelFormat::Bgra8 { w * 4 } else { w };
        upload(&dev, &tex, &b, stride).expect("upload b");
        let (m, _) = st.map(&dev, &tex).expect("map 2");
        let lido = compacta(&m.image());
        assert_eq!(lido, read_back(&dev, &tex).expect("read_back 2"), "{fmt:?} b");
        assert_eq!(lido, b, "{fmt:?}: a segunda leitura deve ver o conteúdo novo");
    }
}

#[test]
fn upload_staging_alterna() {
    let Some(dev) = device() else { return };
    let (w, h) = (640u32, 360u32);
    let Some(dst) = textura_com(
        &dev,
        w,
        h,
        PixelFormat::Nv12,
        &ruido(PixelFormat::Nv12.frame_bytes(w, h), 9),
    ) else {
        return;
    };
    let mut st = match UploadStaging::new(&dev, w, h) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("PULANDO: staging NV12 de escrita indisponível: {e}");
            return;
        }
    };
    let (wu, hu) = (w as usize, h as usize);
    let mut de_primeira = 0;
    for i in 0..10usize {
        let mut m = st.map(&dev).expect("map");
        {
            let v = m.nv12();
            for r in 0..hu {
                for x in 0..wu {
                    v.y[r * v.y_stride + x] = (i * 31 + r * 7 + x) as u8;
                }
            }
            for r in 0..hu / 2 {
                for x in 0..wu {
                    v.uv[r * v.uv_stride + x] = (i * 17 + r * 5 + x * 3) as u8;
                }
            }
        }
        let t = m.commit(&dst).expect("commit");
        if t.map_attempts == 1 {
            de_primeira += 1;
        }
        let mut esperado = Vec::with_capacity(PixelFormat::Nv12.frame_bytes(w, h));
        for r in 0..hu {
            esperado.extend((0..wu).map(|x| (i * 31 + r * 7 + x) as u8));
        }
        for r in 0..hu / 2 {
            esperado.extend((0..wu).map(|x| (i * 17 + r * 5 + x * 3) as u8));
        }
        assert_eq!(read_back(&dev, &dst).expect("read_back"), esperado, "iteração {i}");
    }
    assert!(de_primeira >= 9, "map de primeira em só {de_primeira} de 10 iterações");
}

fn percentil(v: &mut [Duration], p: f64) -> Duration {
    v.sort();
    v[(((v.len() - 1) as f64) * p).round() as usize]
}

#[test]
fn readback_nao_segura_o_lock() {
    let Some(dev) = device() else { return };
    let (w, h) = (3840u32, 2160u32);
    let parar = AtomicBool::new(false);
    let (timings, mut esperas) = std::thread::scope(|s| {
        let a = s.spawn(|| {
            let dados = ruido(PixelFormat::Bgra8.frame_bytes(w, h), 3);
            let tex = textura_com(&dev, w, h, PixelFormat::Bgra8, &dados);
            let mut ts = Vec::new();
            if let Some(tex) = tex {
                let mut st = ReadbackStaging::new(&dev, w, h, PixelFormat::Bgra8).expect("staging");
                for _ in 0..20 {
                    let (m, t) = st.map(&dev, &tex).expect("map");
                    drop(m);
                    ts.push(t);
                }
            }
            parar.store(true, Ordering::Release);
            ts
        });
        let b = s.spawn(|| {
            let mut e = Vec::new();
            while !parar.load(Ordering::Acquire) {
                let (guarda, espera) = dev.lock();
                drop(guarda);
                e.push(espera);
                std::thread::sleep(Duration::from_micros(200));
            }
            e
        });
        (a.join().expect("A"), b.join().expect("B"))
    });
    if timings.is_empty() {
        return;
    }
    assert!(!esperas.is_empty(), "a thread B não conseguiu medir nada");
    let p50 = percentil(&mut esperas, 0.5);
    let p95 = percentil(&mut esperas, 0.95);
    eprintln!(
        "espera pelo lock (B): p50 = {p50:?}, p95 = {p95:?}, amostras = {}",
        esperas.len()
    );
    assert!(p95 <= Duration::from_micros(500), "p95 da espera pelo lock = {p95:?}");
    for (i, t) in timings.iter().enumerate() {
        assert!(t.copy_wait_us > 0.0, "leitura {i}: copy_wait_us = 0");
        assert!(t.map_attempts >= 1, "leitura {i}: map_attempts = 0");
    }
}
