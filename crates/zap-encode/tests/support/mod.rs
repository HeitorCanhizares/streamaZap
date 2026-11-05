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

const LINHAS_TEXTO: u32 = 40;
const Y_PRIMEIRA_LINHA: u32 = 20;
const PASSO_LINHA: u32 = 16;
const ALTURA_LINHA: u32 = 12;
const LARGURA_RETANGULO: u32 = 2;
const PASSO_RETANGULO: u32 = 6;

/// Padrão fixo: diz se o retângulo `n` da linha `k` existe. Não depende do quadro, então o "texto"
/// rola sem mudar de forma.
fn retangulo_existe(k: u32, n: u32) -> bool {
    let semente = k
        .wrapping_mul(7919)
        .wrapping_add(n.wrapping_mul(104_729))
        .wrapping_add(1);
    xorshift32(xorshift32(semente)) & 1 == 1
}

/// BGRA realista: gradiente horizontal (anda 4 px por quadro) com 40 linhas de retângulos pretos
/// de 2x12 px que rolam 2 px por quadro. Determinístico: mesmo `i`, mesmos bytes.
fn preencher_realista(i: u32) -> Vec<u8> {
    let mut dados = vec![255u8; (W * H * 4) as usize];

    // Fundo: o valor depende só da coluna, então cada linha tem o mesmo gradiente.
    for y in 0..H {
        for x in 0..W {
            let v = (x.wrapping_add(i.wrapping_mul(4)) % W) * 255 / W;
            let px = (y * W + x) as usize * 4;
            dados[px..px + 4].copy_from_slice(&[v as u8, (255 - v) as u8, 128, 255]);
        }
    }

    // Texto: retângulos pretos. A coluna inicial rola com o quadro; quem passa da borda é cortado.
    for k in 0..LINHAS_TEXTO {
        let y0 = Y_PRIMEIRA_LINHA + k * PASSO_LINHA;
        for n in 0..W / PASSO_RETANGULO {
            if !retangulo_existe(k, n) {
                continue;
            }
            let x0 = (PASSO_RETANGULO * n).wrapping_add(i.wrapping_mul(2)) % W;
            for y in y0..y0 + ALTURA_LINHA {
                for x in x0..(x0 + LARGURA_RETANGULO).min(W) {
                    let px = (y * W + x) as usize * 4;
                    dados[px..px + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
    }
    dados
}

/// Quadro NV12 realista (ver `preencher_realista`), tarefa encode-05.
pub fn frame_realista(dev: &GpuDevice, ring: &mut TextureRing, conv: &mut Converter, i: u32) -> Texture {
    converter_para_nv12(dev, ring, conv, &preencher_realista(i))
}

#[cfg(test)]
mod testes_realista {
    use super::*;

    fn pixel(dados: &[u8], x: u32, y: u32) -> [u8; 4] {
        let p = (y * W + x) as usize * 4;
        [dados[p], dados[p + 1], dados[p + 2], dados[p + 3]]
    }

    #[test]
    fn realista_e_deterministico() {
        assert_eq!(preencher_realista(3), preencher_realista(3));
    }

    #[test]
    fn realista_muda_com_o_quadro() {
        assert_ne!(preencher_realista(0), preencher_realista(1));
    }

    #[test]
    fn realista_tem_tamanho_certo() {
        let dados = preencher_realista(0);
        assert_eq!(dados.len(), (W * H * 4) as usize);
        for y in (0..H).step_by(37) {
            for x in (0..W).step_by(29) {
                assert_eq!(pixel(&dados, x, y)[3], 255, "alfa em ({x},{y})");
            }
        }
    }

    #[test]
    fn realista_tem_texto_e_gradiente() {
        let dados = preencher_realista(0);
        let tem_preto = (20..32).any(|y| (0..W).any(|x| pixel(&dados, x, y)[..3] == [0, 0, 0]));
        assert!(tem_preto, "faltou texto preto na faixa y 20..32");
        // Os cantos ficam fora da faixa de texto (y 20..=655), então são fundo com R=128.
        // (0,100) não serve: com i=0 cai no retângulo n=0 da linha 5, que é preto.
        assert_eq!(pixel(&dados, 0, 0)[2], 128);
        assert_eq!(pixel(&dados, W - 1, H - 1)[2], 128);
    }
}
