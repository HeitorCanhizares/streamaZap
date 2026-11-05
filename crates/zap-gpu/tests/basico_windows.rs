#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Testes do Windows para o adaptador básico (sem Video Processor) e para `format_support`.
//! Sem GPU D3D11 os testes pulam com mensagem em vez de falhar.

use zap_gpu::{
    read_back, upload, BatchOptions, Converter, CpuScaler, GpuDevice, GpuError, Nv12Mut, PixelFormat, ReadbackStaging,
    SrcImage, Texture, TextureRing, UploadStaging,
};

/// Gradiente BGRA determinístico (alfa 255).
fn gradiente(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            v.push((x * 255 / w) as u8);
            v.push((y * 255 / h) as u8);
            v.push(((x + y) * 255 / (w + h)) as u8);
            v.push(255);
        }
    }
    v
}

#[test]
fn adaptador_basico_sem_vp() {
    let basico = match GpuDevice::create_basic() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("PULANDO: sem adaptador básico: {e}");
            return;
        }
    };
    assert!(!basico.has_video(), "o adaptador básico não deve ter Video Processor");
    assert!(
        matches!(
            Converter::new(&basico, 640, 360, PixelFormat::Bgra8, 640, 360),
            Err(GpuError::Unsupported)
        ),
        "Converter::new no básico deve devolver Unsupported"
    );
    let fs = basico.format_support(PixelFormat::Nv12);
    assert!(fs.texture2d, "NV12 sem texture2d no básico");
    assert!(fs.cpu_lockable, "NV12 sem cpu_lockable no básico");

    let (w, h) = (640u32, 360u32);
    let bgra = gradiente(w, h);
    let tex_bgra = Texture::new(&basico, w, h, PixelFormat::Bgra8, false).expect("textura BGRA");
    upload(&basico, &tex_bgra, &bgra, w * 4).expect("upload BGRA");
    let mut rb = ReadbackStaging::new(&basico, w, h, PixelFormat::Bgra8).expect("readback staging");
    let (mapeado, _) = rb.map(&basico, &tex_bgra).expect("map");
    let SrcImage::Bgra {
        data,
        stride,
        width,
        height,
    } = mapeado.image()
    else {
        panic!("imagem mapeada deveria ser BGRA");
    };
    // Cópia compacta para comparar com a conversão direta e usar nos dois destinos.
    let mut compacto = Vec::with_capacity(bgra.len());
    for r in 0..height as usize {
        compacto.extend_from_slice(&data[r * stride..r * stride + width as usize * 4]);
    }
    assert_eq!(compacto, bgra, "o readback BGRA deve devolver o gradiente enviado");
    let opts = BatchOptions::default();

    for (ow, oh) in [(640u32, 360u32), (320, 180)] {
        let scaler = CpuScaler::new(w, h, PixelFormat::Bgra8, ow, oh).expect("scaler");

        // Caminho pela staging mapeada até a textura NV12.
        let mut up = UploadStaging::new(&basico, ow, oh).expect("staging de escrita");
        let tex_nv12 = {
            let mut anel = TextureRing::new(&basico, ow, oh, PixelFormat::Nv12, 3, false).expect("anel NV12");
            let dst = anel.next();
            let mut m = up.map(&basico).expect("map de escrita");
            scaler.convert(&mapeado.image(), &mut m.nv12(), opts).expect("convert");
            m.commit(dst).expect("commit");
            read_back(&basico, dst).expect("read_back")
        };

        // Referência: conversão direta na memória, com stride = largura.
        let mut ref_buf = vec![0u8; PixelFormat::Nv12.frame_bytes(ow, oh)];
        let mut destino = Nv12Mut::from_contiguous(&mut ref_buf, ow as usize, ow, oh).expect("destino");
        let src = SrcImage::Bgra {
            data: &bgra,
            stride: w as usize * 4,
            width: w,
            height: h,
        };
        scaler.convert(&src, &mut destino, opts).expect("convert direto");
        assert_eq!(tex_nv12.len(), ref_buf.len(), "{ow}x{oh}: tamanho diferente");
        assert!(tex_nv12 == ref_buf, "{ow}x{oh}: o NV12 lido difere da conversão direta");
    }
}

#[test]
fn format_support_nv12() {
    let dev = match GpuDevice::create_default() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("PULANDO: sem GPU D3D11: {e}");
            return;
        }
    };
    let nv12 = dev.format_support(PixelFormat::Nv12);
    assert!(nv12.texture2d, "NV12 sem texture2d");
    if dev.has_video() {
        assert!(nv12.video_processor_output, "NV12 sem video_processor_output com VP");
    }
    let bgra = dev.format_support(PixelFormat::Bgra8);
    assert!(bgra.texture2d, "BGRA sem texture2d");
    assert!(bgra.render_target, "BGRA sem render_target");
}
