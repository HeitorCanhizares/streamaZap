#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Caminho de uma VM com adaptador básico (sem GPU de vídeo): conversão NV12 na CPU e
//! codificação H.264 por software, tudo menos a captura.

use std::time::{Duration, Instant};
use zap_encode::{EncoderChoice, EncoderConfig, H264Encoder};
use zap_gpu::{
    convert_batch, upload, BatchOptions, CpuScaler, GpuDevice, PixelFormat, ReadbackStaging, Texture, TextureRing,
    UploadStaging,
};

const W: u32 = 640;
const H: u32 = 360;

#[test]
fn cpu_basico_codifica_software() {
    let basico = match GpuDevice::create_basic() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("PULANDO: sem adaptador básico ({e})");
            return;
        }
    };
    let (tx, rx) = crossbeam_channel::bounded(8);
    let cfg = EncoderConfig {
        width: W,
        height: H,
        fps: 30,
        kbps: 1000,
        gop_frames: 300,
        choice: EncoderChoice::Software,
    };
    let enc = match H264Encoder::new(&basico, cfg, tx) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("PULANDO: sem codificador H.264 de software ({e})");
            return;
        }
    };

    let mut ring = TextureRing::new(&basico, W, H, PixelFormat::Nv12, 4, false).expect("anel");
    let mut leitura = ReadbackStaging::new(&basico, W, H, PixelFormat::Bgra8).expect("leitura");
    let scaler = CpuScaler::new(W, H, PixelFormat::Bgra8, W, H).expect("scaler");
    let mut up = UploadStaging::new(&basico, W, H).expect("upload");
    let origem = Texture::new(&basico, W, H, PixelFormat::Bgra8, false).expect("origem");

    let mut imagens = Vec::new();
    for i in 0..30u32 {
        // Cor derivada do índice, para cada quadro ser diferente do anterior.
        let (r, g, b) = ((i * 8) as u8, (255 - i * 8) as u8, (i * 3) as u8);
        let dados = [b, g, r, 255].repeat((W * H) as usize);
        upload(&basico, &origem, &dados, W * 4).expect("sobe BGRA");

        let (mapeado, _) = leitura.map(&basico, &origem).expect("lê a origem");
        let mut escrita = up.map(&basico).expect("mapeia a escrita");
        convert_batch(
            &mapeado.image(),
            &mut [(&scaler, escrita.nv12())],
            BatchOptions {
                threads: 2,
                on_thread_start: None,
            },
        )
        .expect("converte");
        drop(mapeado);
        let destino = ring.next();
        escrita.commit(destino).expect("commit");
        let clone = Texture::from_raw(
            destino.raw().clone(),
            destino.format(),
            destino.width(),
            destino.height(),
        );
        enc.submit(clone, zap_gpu::now_us(), i == 0);
        std::thread::sleep(Duration::from_millis(33));
        // Esvazia a saída a cada quadro: o canal é limitado (8) e o codificador descartaria o excesso.
        while let Ok(p) = rx.try_recv() {
            imagens.push(p);
        }
    }

    let limite = Instant::now() + Duration::from_secs(5);
    while imagens.len() < 30 {
        let resta = limite.saturating_duration_since(Instant::now());
        match rx.recv_timeout(resta) {
            Ok(p) => imagens.push(p),
            Err(_) => break,
        }
    }
    drop(enc);
    drop(basico);

    assert!(imagens.len() >= 20, "só {} imagens codificadas", imagens.len());
    assert!(
        imagens[0].keyframe && imagens[0].has_params,
        "a primeira imagem deve ser keyframe com SPS/PPS"
    );
}
