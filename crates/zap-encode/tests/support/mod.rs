//! Auxiliares compartilhados pelos testes do Windows.
#![allow(dead_code)]
use zap_gpu::{Converter, GpuDevice, PixelFormat, Texture, TextureRing};

pub const W: u32 = 1280;
pub const H: u32 = 720;

const BLOCO: u32 = 8;

fn xorshift32(mut s: u32) -> u32 {
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    s
}

/// BGRA com um bloco 8x8 de cor pseudoaleatória por posição. A semente muda com `i` e o campo de
/// blocos é deslocado por `i`, então cada quadro é incompressível de um jeito diferente.
fn preencher_ruido(i: u32) -> Vec<u8> {
    let mut dados = vec![255u8; (W * H * 4) as usize];
    let bw = W / BLOCO;
    let bh = H / BLOCO;
    let semente = i.wrapping_add(1).wrapping_mul(2_654_435_761u32);
    for by in 0..bh {
        for bx in 0..bw {
            let indice = (by * bw + bx).wrapping_add(i);
            let mut estado = semente ^ indice.wrapping_mul(0x9E37_79B9);
            if estado == 0 {
                // xorshift não sai do zero; estado 0 viraria cor fixa.
                estado = 1;
            }
            estado = xorshift32(xorshift32(estado));
            let b = (estado & 0xFF) as u8;
            let g = ((estado >> 8) & 0xFF) as u8;
            let r = ((estado >> 16) & 0xFF) as u8;
            for y in 0..BLOCO {
                for x in 0..BLOCO {
                    let px = ((by * BLOCO + y) * W + bx * BLOCO + x) as usize * 4;
                    dados[px..px + 4].copy_from_slice(&[b, g, r, 255]);
                }
            }
        }
    }
    dados
}

/// Sobe BGRA para a GPU e converte para NV12 no anel. O handle COM é clonado porque o chamador
/// consome a `Texture` devolvida.
fn converter_para_nv12(dev: &GpuDevice, ring: &mut TextureRing, conv: &mut Converter, dados: &[u8]) -> Texture {
    let src = Texture::new(dev, W, H, PixelFormat::Bgra8, false).expect("textura BGRA");
    zap_gpu::upload(dev, &src, dados, W * 4).expect("upload");
    let dst = ring.next();
    conv.convert(dev, &src, dst).expect("conversão");
    Texture::from_raw(dst.raw().clone(), PixelFormat::Nv12, W, H)
}

/// Quadro NV12 com ruído determinístico (ver `preencher_ruido`).
pub fn frame_ruido(dev: &GpuDevice, ring: &mut TextureRing, conv: &mut Converter, i: u32) -> Texture {
    converter_para_nv12(dev, ring, conv, &preencher_ruido(i))
}

/// Quadro NV12 de cor sólida fixa (não muda com o índice). `_i` existe só para bater com o tipo
/// de gerador usado pelos testes.
pub fn frame_estatico(dev: &GpuDevice, ring: &mut TextureRing, conv: &mut Converter, _i: u32) -> Texture {
    let dados: Vec<u8> = [40u8, 90, 160, 255]
        .iter()
        .copied()
        .cycle()
        .take((W * H * 4) as usize)
        .collect();
    converter_para_nv12(dev, ring, conv, &dados)
}

/// `PixelFormat` usado pelos quadros acima (evita import morto).
pub const FORMATO: PixelFormat = PixelFormat::Nv12;
