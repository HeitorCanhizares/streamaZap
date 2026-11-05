//! `convert_batch`: validação, divisão em faixas de 64 linhas e threads (`std::thread::scope`).
use super::bgra::faixa_bgra;
use super::eixo::{Eixos, FaixaDst, Scratch};
use super::nv12::faixa_nv12;
use super::{BatchOptions, CpuScaler, Nv12Mut, SrcImage};
use crate::GpuError;

/// Linhas de luma por faixa (e metade disso em UV).
const LINHAS_FAIXA: usize = 64;

/// Uma faixa de um destino, pronta para uma thread.
struct Trabalho<'a> {
    eixos: &'a Eixos,
    in_w: u32,
    in_h: u32,
    y: &'a mut [u8],
    y_stride: usize,
    uv: &'a mut [u8],
    uv_stride: usize,
    y0: usize,
    linhas: usize,
    out_w: u32,
}

fn invalido(msg: impl Into<String>) -> GpuError {
    GpuError::InvalidArgument(msg.into())
}

/// Bytes mínimos de um plano: `stride * (linhas - 1) + largura` (saturando, para não estourar).
fn minimo(stride: usize, linhas: usize, largura: usize) -> usize {
    stride.saturating_mul(linhas.saturating_sub(1)).saturating_add(largura)
}

fn valida_origem(src: &SrcImage<'_>) -> Result<(), GpuError> {
    let w = src.width() as usize;
    let h = src.height() as usize;
    match *src {
        SrcImage::Bgra { data, stride, .. } => {
            if stride < w.saturating_mul(4) {
                return Err(invalido(format!(
                    "stride da origem BGRA ({stride}) menor que a linha ({} bytes)",
                    w * 4
                )));
            }
            if data.len() < minimo(stride, h, w * 4) {
                return Err(invalido("buffer da origem BGRA curto demais"));
            }
        }
        SrcImage::Nv12 {
            y,
            y_stride,
            uv,
            uv_stride,
            ..
        } => {
            if y_stride < w || uv_stride < w {
                return Err(invalido("stride da origem NV12 menor que a largura"));
            }
            if y.len() < minimo(y_stride, h, w) || uv.len() < minimo(uv_stride, h / 2, w) {
                return Err(invalido("buffer da origem NV12 curto demais"));
            }
        }
    }
    Ok(())
}

fn valida_job(src: &SrcImage<'_>, scaler: &CpuScaler, dst: &Nv12Mut<'_>) -> Result<(), GpuError> {
    let entrada = (src.width(), src.height(), src.format());
    if scaler.input() != entrada {
        return Err(invalido(format!(
            "origem {}x{} ({:?}) diferente da entrada do conversor {:?}",
            entrada.0,
            entrada.1,
            entrada.2,
            scaler.input()
        )));
    }
    if (dst.width, dst.height) != scaler.output() {
        return Err(invalido(format!(
            "destino {}x{} diferente da saída do conversor {:?}",
            dst.width,
            dst.height,
            scaler.output()
        )));
    }
    let w = dst.width as usize;
    let h = dst.height as usize;
    if dst.y_stride < w || dst.uv_stride < w {
        return Err(invalido("stride do destino menor que a largura"));
    }
    if dst.y.len() < minimo(dst.y_stride, h, w) || dst.uv.len() < minimo(dst.uv_stride, h / 2, w) {
        return Err(invalido("buffer do destino curto demais"));
    }
    Ok(())
}

/// Roda as faixas de uma thread com um rascunho só.
fn processa(src: &SrcImage<'_>, lista: Vec<Trabalho<'_>>, max_in_w: u32, max_out_w: u32) {
    let mut scratch = Scratch::new(max_in_w, max_out_w);
    for t in lista {
        let dst = FaixaDst {
            y: t.y,
            y_stride: t.y_stride,
            uv: t.uv,
            uv_stride: t.uv_stride,
            y0: t.y0,
            linhas: t.linhas,
            out_w: t.out_w,
        };
        match *src {
            SrcImage::Bgra { data, stride, .. } => faixa_bgra(data, stride, t.in_w, t.in_h, t.eixos, dst, &mut scratch),
            SrcImage::Nv12 {
                y,
                y_stride,
                uv,
                uv_stride,
                ..
            } => faixa_nv12(y, y_stride, uv, uv_stride, t.in_w, t.in_h, t.eixos, dst, &mut scratch),
        }
    }
}

/// Converte várias camadas do mesmo quadro numa passada só. Valida tudo antes (com qualquer
/// trabalho inválido, `InvalidArgument` e nada escrito): a entrada do `CpuScaler` igual a `src`
/// (tamanho e formato), o destino do tamanho da saída, as fatias longas o bastante para os strides.
pub fn convert_batch(
    src: &SrcImage<'_>,
    jobs: &mut [(&CpuScaler, Nv12Mut<'_>)],
    opts: BatchOptions,
) -> Result<(), GpuError> {
    if jobs.is_empty() {
        return Ok(());
    }
    // Validação completa antes de escrever qualquer byte.
    valida_origem(src)?;
    for (scaler, dst) in jobs.iter() {
        valida_job(src, scaler, dst)?;
    }

    let n_threads = opts.threads.max(1);
    let max_in_w = jobs.iter().map(|(s, _)| s.in_w).max().unwrap_or(0);
    let max_out_w = jobs.iter().map(|(s, _)| s.out_w).max().unwrap_or(0);

    // Lista ordenada de faixas, distribuída por índice global (i % n) antes do escopo.
    let mut todas: Vec<Trabalho<'_>> = Vec::new();
    for (scaler, dst) in jobs.iter_mut() {
        let altura = dst.height as usize;
        let n_faixas = altura.div_ceil(LINHAS_FAIXA);
        let (ys, uvs) = (dst.y_stride, dst.uv_stride);
        let y_chunks = dst.y.chunks_mut(LINHAS_FAIXA * ys);
        let uv_chunks = dst.uv.chunks_mut(LINHAS_FAIXA / 2 * uvs);
        for (i, (y, uv)) in y_chunks.zip(uv_chunks).take(n_faixas).enumerate() {
            let y0 = i * LINHAS_FAIXA;
            todas.push(Trabalho {
                eixos: &scaler.eixos,
                in_w: scaler.in_w,
                in_h: scaler.in_h,
                y,
                y_stride: ys,
                uv,
                uv_stride: uvs,
                y0,
                linhas: LINHAS_FAIXA.min(altura - y0),
                out_w: scaler.out_w,
            });
        }
    }

    let n = n_threads.min(todas.len()).max(1);
    let mut listas: Vec<Vec<Trabalho<'_>>> = (0..n).map(|_| Vec::new()).collect();
    for (i, t) in todas.into_iter().enumerate() {
        listas[i % n].push(t);
    }
    if n == 1 {
        let Some(lista) = listas.pop() else {
            return Ok(());
        };
        processa(src, lista, max_in_w, max_out_w);
        return Ok(());
    }

    let mut listas = listas.into_iter();
    let propria = listas.next().unwrap_or_default();
    let on_start = opts.on_thread_start;
    std::thread::scope(|escopo| {
        let mut handles = Vec::with_capacity(n - 1);
        let mut erro = false;
        for lista in listas {
            let r = std::thread::Builder::new().spawn_scoped(escopo, move || {
                if let Some(f) = on_start {
                    f();
                }
                processa(src, lista, max_in_w, max_out_w);
            });
            match r {
                Ok(h) => handles.push(h),
                Err(_) => erro = true,
            }
        }
        processa(src, propria, max_in_w, max_out_w);
        for h in handles {
            if h.join().is_err() {
                erro = true;
            }
        }
        if erro {
            Err(invalido("falha nas threads de conversao"))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn lcg(n: usize, mut s: u32) -> Vec<u8> {
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                (s >> 24) as u8
            })
            .collect()
    }

    fn opts(threads: usize) -> BatchOptions {
        BatchOptions {
            threads,
            on_thread_start: None,
        }
    }

    fn tam(w: usize, h: usize) -> usize {
        w * (h + h / 2)
    }

    fn converte(scaler: &CpuScaler, src: &SrcImage<'_>, threads: usize) -> Vec<u8> {
        let (w, h) = scaler.output();
        let mut buf = vec![0u8; tam(w as usize, h as usize)];
        let mut d = Nv12Mut::from_contiguous(&mut buf, w as usize, w, h).unwrap();
        scaler.convert(src, &mut d, opts(threads)).unwrap();
        buf
    }

    #[test]
    fn threads_nao_mudam_nada() {
        let bgra = lcg(1920 * 1080 * 4, 7);
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: 1920 * 4,
            width: 1920,
            height: 1080,
        };
        for (w, h) in [(1920, 1080), (1280, 720), (640, 360)] {
            let s = CpuScaler::new(1920, 1080, PixelFormat::Bgra8, w, h).unwrap();
            let base = converte(&s, &src, 1);
            for t in [3, 4, 8] {
                assert_eq!(base, converte(&s, &src, t), "{w}x{h} threads {t}");
            }
        }
        let y = lcg(1280 * 720, 11);
        let uv = lcg(1280 * 360, 13);
        let src = SrcImage::Nv12 {
            y: &y,
            y_stride: 1280,
            uv: &uv,
            uv_stride: 1280,
            width: 1280,
            height: 720,
        };
        let s = CpuScaler::new(1280, 720, PixelFormat::Nv12, 640, 360).unwrap();
        let base = converte(&s, &src, 1);
        for t in [3, 4, 8] {
            assert_eq!(base, converte(&s, &src, t), "nv12 threads {t}");
        }
    }

    #[test]
    fn lote_igual_separado() {
        let bgra = lcg(1920 * 1080 * 4, 21);
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: 1920 * 4,
            width: 1920,
            height: 1080,
        };
        let tamanhos = [(1920u32, 1080u32), (1280, 720), (640, 360)];
        let scalers: Vec<CpuScaler> = tamanhos
            .iter()
            .map(|&(w, h)| CpuScaler::new(1920, 1080, PixelFormat::Bgra8, w, h).unwrap())
            .collect();
        let mut bufs: Vec<Vec<u8>> = tamanhos
            .iter()
            .map(|&(w, h)| vec![0u8; tam(w as usize, h as usize)])
            .collect();
        {
            let mut jobs: Vec<(&CpuScaler, Nv12Mut<'_>)> = scalers
                .iter()
                .zip(bufs.iter_mut())
                .zip(tamanhos.iter())
                .map(|((s, b), &(w, h))| (s, Nv12Mut::from_contiguous(b, w as usize, w, h).unwrap()))
                .collect();
            convert_batch(&src, &mut jobs, opts(4)).unwrap();
        }
        for (i, s) in scalers.iter().enumerate() {
            assert_eq!(bufs[i], converte(s, &src, 1), "job {i}");
        }
    }

    #[test]
    fn stride_e_pitch() {
        let (iw, ih) = (200usize, 130usize);
        let compacto = lcg(iw * ih * 4, 5);
        let st_in = iw * 4 + 64;
        let mut largo = vec![0x11u8; st_in * ih];
        for r in 0..ih {
            largo[r * st_in..r * st_in + iw * 4].copy_from_slice(&compacto[r * iw * 4..(r + 1) * iw * 4]);
        }
        let s = CpuScaler::new(iw as u32, ih as u32, PixelFormat::Bgra8, 100, 66).unwrap();
        let src_c = SrcImage::Bgra {
            data: &compacto,
            stride: iw * 4,
            width: iw as u32,
            height: ih as u32,
        };
        let src_l = SrcImage::Bgra {
            data: &largo,
            stride: st_in,
            width: iw as u32,
            height: ih as u32,
        };
        let ref_buf = converte(&s, &src_c, 1);
        let (w, h, st) = (100usize, 66usize, 132usize);
        let mut buf = vec![0xEEu8; st * (h + h / 2)];
        {
            let mut d = Nv12Mut::from_contiguous(&mut buf, st, w as u32, h as u32).unwrap();
            s.convert(&src_l, &mut d, opts(3)).unwrap();
        }
        for r in 0..h + h / 2 {
            assert_eq!(&buf[r * st..r * st + w], &ref_buf[r * w..(r + 1) * w], "linha {r}");
            assert!(buf[r * st + w..(r + 1) * st].iter().all(|&b| b == 0xEE), "sobra {r}");
        }
    }

    #[test]
    fn argumentos_invalidos() {
        let bgra = lcg(64 * 64 * 4, 3);
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: 256,
            width: 64,
            height: 64,
        };
        let bom = CpuScaler::new(64, 64, PixelFormat::Bgra8, 32, 32).unwrap();
        let outro_tam = CpuScaler::new(32, 32, PixelFormat::Bgra8, 32, 32).unwrap();
        let outro_fmt = CpuScaler::new(64, 64, PixelFormat::Nv12, 32, 32).unwrap();
        let saida_maior = CpuScaler::new(64, 64, PixelFormat::Bgra8, 64, 64).unwrap();

        let confere = |scaler: &CpuScaler, src: &SrcImage<'_>, dst_w: u32, dst_h: u32| {
            let mut buf = vec![0xABu8; tam(dst_w as usize, dst_h as usize)];
            let r = {
                let d = Nv12Mut::from_contiguous(&mut buf, dst_w as usize, dst_w, dst_h).unwrap();
                convert_batch(src, &mut [(scaler, d)], opts(2))
            };
            assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
            assert!(buf.iter().all(|&b| b == 0xAB));
        };
        confere(&outro_tam, &src, 32, 32); // origem de outro tamanho
        confere(&outro_fmt, &src, 32, 32); // origem de outro formato
        confere(&bom, &src, 64, 64); // destino de outro tamanho
        let curta = SrcImage::Bgra {
            data: &bgra[..bgra.len() - 100],
            stride: 256,
            width: 64,
            height: 64,
        };
        confere(&bom, &curta, 32, 32); // origem curta
        let stride_menor = SrcImage::Bgra {
            data: &bgra,
            stride: 200,
            width: 64,
            height: 64,
        };
        confere(&bom, &stride_menor, 32, 32);

        // Destino curto: fatias montadas à mão.
        let mut y = vec![0xABu8; 32 * 32 - 1];
        let mut uv = vec![0xABu8; 32 * 16];
        let d = Nv12Mut {
            y: &mut y,
            y_stride: 32,
            uv: &mut uv,
            uv_stride: 32,
            width: 32,
            height: 32,
        };
        let r = convert_batch(&src, &mut [(&bom, d)], opts(2));
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
        assert!(y.iter().all(|&b| b == 0xAB) && uv.iter().all(|&b| b == 0xAB));

        // Lote com dois bons e um ruim (conversor de outra entrada): os bons também ficam intactos.
        let mut b1 = vec![0xABu8; tam(32, 32)];
        let mut b2 = vec![0xABu8; tam(64, 64)];
        let mut b3 = vec![0xABu8; tam(32, 32)];
        let r = {
            let d1 = Nv12Mut::from_contiguous(&mut b1, 32, 32, 32).unwrap();
            let d2 = Nv12Mut::from_contiguous(&mut b2, 64, 64, 64).unwrap();
            let d3 = Nv12Mut::from_contiguous(&mut b3, 32, 32, 32).unwrap();
            convert_batch(&src, &mut [(&bom, d1), (&saida_maior, d2), (&outro_tam, d3)], opts(2))
        };
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
        assert!(b1.iter().all(|&b| b == 0xAB) && b2.iter().all(|&b| b == 0xAB));
    }

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);
    fn conta() {
        CONTADOR.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn on_thread_start_roda_n_menos_1_vezes() {
        let bgra = lcg(256 * 256 * 4, 9);
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: 1024,
            width: 256,
            height: 256,
        };
        let s = CpuScaler::new(256, 256, PixelFormat::Bgra8, 256, 256).unwrap();
        let mut buf = vec![0u8; tam(256, 256)];
        CONTADOR.store(0, Ordering::SeqCst);
        let mut d = Nv12Mut::from_contiguous(&mut buf, 256, 256, 256).unwrap();
        let o = BatchOptions {
            threads: 4,
            on_thread_start: Some(conta),
        };
        s.convert(&src, &mut d, o).unwrap();
        assert_eq!(CONTADOR.load(Ordering::SeqCst), 3);
        CONTADOR.store(0, Ordering::SeqCst);
        let o = BatchOptions {
            threads: 1,
            on_thread_start: Some(conta),
        };
        s.convert(&src, &mut d, o).unwrap();
        assert_eq!(CONTADOR.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn lote_vazio_ok() {
        let bgra = vec![0u8; 16];
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: 8,
            width: 2,
            height: 2,
        };
        assert!(convert_batch(&src, &mut [], opts(4)).is_ok());
    }
}
