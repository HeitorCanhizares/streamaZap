//! Foto de uma fonte de vídeo: a miniatura da tela "Criar sala" e do seletor de fonte da sala.
//!
//! Abre a mesma captura do pipeline (DXGI no monitor, WGC na janela, Media Foundation na câmera),
//! espera um quadro, lê na CPU (`zap_gpu::read_back`) e reduz para RGBA `w`×`h` mantendo a
//! proporção (tarjas pretas). Fora do Windows: `Unsupported`.

use std::time::Duration;

use crate::{MediaError, VideoSourceSpec};

/// Imagem RGBA 8 bits, linhas de cima para baixo (`rgba.len() == width * height * 4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Formato do quadro lido da GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawFormat {
    /// B, G, R, A por pixel.
    Bgra,
    /// Plano Y (`stride`×`altura`) seguido do UV intercalado (`stride`×`altura/2`).
    Nv12,
}

/// Quadro lido na CPU, sem padding de linha (`stride` = largura da textura).
#[derive(Clone, Debug)]
pub struct RawFrame<'a> {
    pub data: &'a [u8],
    pub format: RawFormat,
    /// Largura da textura em pixels (linhas têm `stride` pixels).
    pub stride: u32,
    /// Altura da textura em linhas.
    pub rows: u32,
    /// Área com imagem (canto superior esquerdo da textura).
    pub width: u32,
    pub height: u32,
}

/// Retângulo `(x, y, largura, altura)` que cabe em `dw`×`dh` com a proporção de `sw`×`sh`,
/// centralizado. Nunca zero.
pub fn fit_rect(sw: u32, sh: u32, dw: u32, dh: u32) -> (u32, u32, u32, u32) {
    let (sw, sh) = (u64::from(sw.max(1)), u64::from(sh.max(1)));
    let (dw64, dh64) = (u64::from(dw.max(1)), u64::from(dh.max(1)));
    // Compara as proporções em inteiros: sw/sh > dw/dh  <=>  sw*dh > dw*sh.
    let (w, h) = if sw * dh64 > dw64 * sh {
        (dw64, (dw64 * sh / sw).max(1))
    } else {
        ((dh64 * sw / sh).max(1), dh64)
    };
    let (w, h) = (w as u32, h as u32);
    ((dw.max(1) - w) / 2, (dh.max(1) - h) / 2, w, h)
}

/// YUV (BT.601, faixa limitada, a do codificador e da câmera) para RGB.
fn yuv_para_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let c = i32::from(y) - 16;
    let d = i32::from(u) - 128;
    let e = i32::from(v) - 128;
    let canal = |x: i32| ((x + 128) >> 8).clamp(0, 255) as u8;
    [
        canal(298 * c + 409 * e),
        canal(298 * c - 100 * d - 208 * e),
        canal(298 * c + 516 * d),
    ]
}

/// RGB do pixel `(x, y)` do quadro (já conferido que está dentro).
fn pixel(f: &RawFrame<'_>, x: u32, y: u32) -> [u8; 3] {
    let (x, y, s) = (x as usize, y as usize, f.stride as usize);
    match f.format {
        RawFormat::Bgra => {
            let i = (y * s + x) * 4;
            [f.data[i + 2], f.data[i + 1], f.data[i]]
        }
        RawFormat::Nv12 => {
            let luma = f.data[y * s + x];
            let base_uv = s * f.rows as usize;
            let i = base_uv + (y / 2) * s + (x / 2) * 2;
            yuv_para_rgb(luma, f.data[i], f.data[i + 1])
        }
    }
}

/// Bytes que o quadro precisa ter para o formato e o tamanho declarados.
fn bytes_esperados(f: &RawFrame<'_>) -> usize {
    let (s, r) = (f.stride as usize, f.rows as usize);
    match f.format {
        RawFormat::Bgra => s * r * 4,
        RawFormat::Nv12 => s * r + s * (r / 2),
    }
}

/// Amostras por eixo dentro da área de cada pixel de destino (média simples).
const AMOSTRAS: u32 = 3;

/// Reduz o quadro para RGBA `dw`×`dh` com tarjas pretas, pela média de até 3×3 amostras da área
/// de cada pixel de destino.
pub fn downscale_to_rgba(f: &RawFrame<'_>, dw: u32, dh: u32) -> Result<SnapshotImage, MediaError> {
    if dw == 0 || dh == 0 {
        return Err(MediaError::InvalidConfig(format!("miniatura {dw}x{dh}")));
    }
    if f.width == 0 || f.height == 0 || f.width > f.stride || f.height > f.rows {
        return Err(MediaError::InvalidConfig(format!(
            "quadro {}x{} numa textura {}x{}",
            f.width, f.height, f.stride, f.rows
        )));
    }
    if f.data.len() < bytes_esperados(f) {
        return Err(MediaError::InvalidConfig(format!(
            "quadro com {} bytes, esperado {}",
            f.data.len(),
            bytes_esperados(f)
        )));
    }
    let (rx, ry, rw, rh) = fit_rect(f.width, f.height, dw, dh);
    // Fundo preto opaco (as tarjas).
    let mut rgba = vec![0u8; dw as usize * dh as usize * 4];
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let (sw, sh) = (u64::from(f.width), u64::from(f.height));
    for dy in 0..rh {
        let y0 = u64::from(dy) * sh / u64::from(rh);
        let y1 = (u64::from(dy + 1) * sh / u64::from(rh)).max(y0 + 1);
        for dx in 0..rw {
            let x0 = u64::from(dx) * sw / u64::from(rw);
            let x1 = (u64::from(dx + 1) * sw / u64::from(rw)).max(x0 + 1);
            let mut soma = [0u32; 3];
            let mut n = 0u32;
            for j in 0..AMOSTRAS {
                let sy = y0 + (y1 - y0) * u64::from(j) / u64::from(AMOSTRAS);
                for i in 0..AMOSTRAS {
                    let sx = x0 + (x1 - x0) * u64::from(i) / u64::from(AMOSTRAS);
                    let c = pixel(f, sx as u32, sy as u32);
                    soma[0] += u32::from(c[0]);
                    soma[1] += u32::from(c[1]);
                    soma[2] += u32::from(c[2]);
                    n += 1;
                }
            }
            let o = (((ry + dy) * dw + rx + dx) * 4) as usize;
            rgba[o] = (soma[0] / n) as u8;
            rgba[o + 1] = (soma[1] / n) as u8;
            rgba[o + 2] = (soma[2] / n) as u8;
        }
    }
    Ok(SnapshotImage {
        width: dw,
        height: dh,
        rgba,
    })
}

/// Quanto esperar pelo quadro: a câmera demora para abrir e acertar a exposição.
pub fn snapshot_timeout(source: &VideoSourceSpec) -> Duration {
    match source {
        VideoSourceSpec::Camera(_) => Duration::from_secs(4),
        _ => Duration::from_millis(1500),
    }
}

/// Foto da fonte em RGBA `w`×`h`. Fonte sintética: `InvalidConfig` (quem chama desenha a dela).
/// Monitor que já está sendo duplicado por este processo (o que estou transmitindo) falha: o
/// Windows deixa só uma duplicação do mesmo monitor por processo.
pub fn snapshot(source: &VideoSourceSpec, w: u32, h: u32) -> Result<SnapshotImage, MediaError> {
    if w == 0 || h == 0 {
        return Err(MediaError::InvalidConfig(format!("miniatura {w}x{h}")));
    }
    if matches!(source, VideoSourceSpec::Synthetic { .. }) {
        return Err(MediaError::InvalidConfig("fonte sintética não tem foto".into()));
    }
    imp::snapshot(source, w, h)
}

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;

    use parking_lot::Mutex;
    use zap_capture::{CaptureError, CapturedFrame, FrameSink};
    use zap_gpu::{GpuDevice, PixelFormat};

    use super::{downscale_to_rgba, snapshot_timeout, RawFormat, RawFrame, SnapshotImage};
    use crate::{MediaError, MonitorRef, VideoSourceSpec};

    /// Quadro já lido na CPU.
    struct Lido {
        dados: Vec<u8>,
        formato: RawFormat,
        stride: u32,
        linhas: u32,
        largura: u32,
        altura: u32,
    }

    type Resposta = Result<Lido, String>;

    /// Lê um quadro (depois de pular os `pular` primeiros) e manda pelo canal, uma vez só.
    struct FotoSink {
        dev: GpuDevice,
        pular: u32,
        vistos: AtomicU32,
        tx: Mutex<Option<mpsc::SyncSender<Resposta>>>,
    }

    impl FrameSink for FotoSink {
        fn on_frame(&self, frame: CapturedFrame) {
            if self.vistos.fetch_add(1, Ordering::Relaxed) < self.pular {
                return;
            }
            let Some(tx) = self.tx.lock().take() else {
                return;
            };
            let formato = match frame.texture.format() {
                PixelFormat::Bgra8 => RawFormat::Bgra,
                PixelFormat::Nv12 => RawFormat::Nv12,
            };
            let lido = {
                let (_trava, _) = self.dev.lock();
                zap_gpu::read_back(&self.dev, &frame.texture)
            };
            let r = lido
                .map(|dados| Lido {
                    dados,
                    formato,
                    stride: frame.texture.width(),
                    linhas: frame.texture.height(),
                    largura: frame.width.min(frame.texture.width()),
                    altura: frame.height.min(frame.texture.height()),
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(r);
        }
        fn on_error(&self, err: CaptureError) {
            if let Some(tx) = self.tx.lock().take() {
                let _ = tx.send(Err(err.to_string()));
            }
        }
    }

    pub(super) fn snapshot(source: &VideoSourceSpec, w: u32, h: u32) -> Result<SnapshotImage, MediaError> {
        let (tx, rx) = mpsc::sync_channel::<Resposta>(1);
        let sink = |dev: &GpuDevice, pular: u32| FotoSink {
            dev: dev.clone(),
            pular,
            vistos: AtomicU32::new(0),
            tx: Mutex::new(Some(tx.clone())),
        };
        // A captura vive até o quadro chegar; o `Drop` do handle a encerra.
        let _captura = match source {
            VideoSourceSpec::Monitor(MonitorRef(i)) => {
                let monitores = zap_capture::list_monitors()?;
                let m = monitores
                    .iter()
                    .find(|m| m.index == *i)
                    .ok_or(CaptureError::MonitorNotFound)?;
                let dev = GpuDevice::create_for_adapter(m.adapter_luid)?;
                zap_capture::capture_monitor(&dev, m, sink(&dev, 0))?
            }
            VideoSourceSpec::Window(hwnd) => {
                let dev = GpuDevice::create_default()?;
                zap_capture::capture_window(&dev, *hwnd, sink(&dev, 0))?
            }
            VideoSourceSpec::Camera(id) => {
                let dev = GpuDevice::create_default()?;
                // Os primeiros quadros da câmera costumam vir escuros (exposição acertando).
                zap_capture::capture_camera(&dev, id, 1280, 720, 30, sink(&dev, 8))?
            }
            VideoSourceSpec::Synthetic { .. } => {
                return Err(MediaError::InvalidConfig("fonte sintética não tem foto".into()));
            }
        };
        drop(tx);
        let espera = snapshot_timeout(source);
        let lido = match rx.recv_timeout(espera) {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => return Err(MediaError::Stopped(format!("foto da fonte: {e}"))),
            Err(_) => {
                return Err(MediaError::Stopped(format!(
                    "a fonte não mandou imagem em {} ms",
                    espera.as_millis()
                )))
            }
        };
        downscale_to_rgba(
            &RawFrame {
                data: &lido.dados,
                format: lido.formato,
                stride: lido.stride,
                rows: lido.linhas,
                width: lido.largura,
                height: lido.altura,
            },
            w,
            h,
        )
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SnapshotImage;
    use crate::{MediaError, VideoSourceSpec};

    pub(super) fn snapshot(_source: &VideoSourceSpec, _w: u32, _h: u32) -> Result<SnapshotImage, MediaError> {
        Err(MediaError::Unsupported("foto de fonte só no Windows".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rect_mantem_proporcao() {
        // 16:9 em 16:9 ocupa tudo.
        assert_eq!(fit_rect(1920, 1080, 320, 180), (0, 0, 320, 180));
        // 4:3 em 16:9: tarjas dos lados.
        assert_eq!(fit_rect(640, 480, 320, 180), (40, 0, 240, 180));
        // Retrato: tarjas largas dos lados.
        assert_eq!(fit_rect(1080, 1920, 320, 180), (109, 0, 101, 180));
        // Panorâmica: tarjas em cima e embaixo.
        assert_eq!(fit_rect(3840, 1080, 320, 180), (0, 45, 320, 90));
    }

    #[test]
    fn bgra_vira_rgba_com_tarjas() {
        // Quadro 4x2 vermelho (BGRA) dentro de uma textura 6x2: só a área de imagem conta.
        let mut dados = vec![0u8; 6 * 2 * 4];
        for y in 0..2 {
            for x in 0..4 {
                let i = (y * 6 + x) * 4;
                dados[i..i + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
        let f = RawFrame {
            data: &dados,
            format: RawFormat::Bgra,
            stride: 6,
            rows: 2,
            width: 4,
            height: 2,
        };
        // 2:1 em 4:4 → imagem 4x2 no meio, tarjas pretas em cima e embaixo.
        let img = downscale_to_rgba(&f, 4, 4).unwrap();
        assert_eq!(img.rgba.len(), 4 * 4 * 4);
        assert_eq!(&img.rgba[0..4], &[0, 0, 0, 255], "tarja");
        let meio = (4 + 1) * 4;
        assert_eq!(&img.rgba[meio..meio + 4], &[255, 0, 0, 255], "imagem");
    }

    #[test]
    fn nv12_cinza_vira_cinza() {
        // Y=126 e UV neutro (128) dão cinza médio (~128).
        let (w, h) = (4u32, 4u32);
        let mut dados = vec![126u8; (w * h) as usize];
        dados.extend(std::iter::repeat_n(128u8, (w * h / 2) as usize));
        let f = RawFrame {
            data: &dados,
            format: RawFormat::Nv12,
            stride: w,
            rows: h,
            width: w,
            height: h,
        };
        let img = downscale_to_rgba(&f, 2, 2).unwrap();
        for px in img.rgba.chunks_exact(4) {
            for c in &px[0..3] {
                assert!((120..=136).contains(c), "canal {c}");
            }
            assert_eq!(px[3], 255);
        }
    }

    #[test]
    fn quadro_curto_ou_vazio_e_erro() {
        let f = RawFrame {
            data: &[0u8; 8],
            format: RawFormat::Bgra,
            stride: 4,
            rows: 4,
            width: 4,
            height: 4,
        };
        assert!(downscale_to_rgba(&f, 2, 2).is_err());
        assert!(downscale_to_rgba(&f, 0, 2).is_err());
    }

    /// Foto de verdade do monitor 1 (Windows com monitor): 320x180 e não toda preta.
    #[cfg(windows)]
    #[test]
    fn foto_do_monitor_1() {
        // O Windows deixa uma duplicação do monitor por processo: espera a vez dos outros testes.
        let _vez = crate::win_video::tests::MONITOR_1
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if zap_capture::list_monitors().map_or(true, |m| m.is_empty()) {
            eprintln!("sem monitor: pulando");
            return;
        }
        let img = snapshot(&VideoSourceSpec::Monitor(crate::MonitorRef(1)), 320, 180).expect("foto do monitor 1");
        assert_eq!((img.width, img.height), (320, 180));
        assert_eq!(img.rgba.len(), 320 * 180 * 4);
        let acesos = img
            .rgba
            .chunks_exact(4)
            .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
            .count();
        assert!(
            acesos > 320 * 180 / 10,
            "a foto saiu quase toda preta ({acesos} pixels acesos)"
        );
    }

    #[test]
    fn sintetica_nao_tem_foto() {
        let s = VideoSourceSpec::Synthetic { width: 8, height: 8 };
        assert!(matches!(snapshot(&s, 4, 4), Err(MediaError::InvalidConfig(_))));
    }
}
