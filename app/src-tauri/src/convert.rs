//! Conversões puras entre o formato da interface e o do núcleo (testáveis sem janela).

use zap_core::{CoreError, Rgba, TileSize};

/// Tamanho do tile em pixels (`[largura, altura]` do `api.ts`) para o `TileSize` do núcleo:
/// largura < 640 → `Thumbnail`; < 1600 → `Main`; senão `Full`.
pub fn tile_size_from_px(size: [u32; 2]) -> TileSize {
    let largura = size[0];
    if largura < 640 {
        TileSize::Thumbnail
    } else if largura < 1600 {
        TileSize::Main
    } else {
        TileSize::Full
    }
}

/// Formato da miniatura (lido por `decodeThumbnail`): `u32 largura LE, u32 altura LE` + RGBA.
pub fn encode_thumbnail(img: &Rgba) -> Vec<u8> {
    // Cabeçalho de 8 bytes em little-endian, igual ao que o decodificador da interface espera.
    let mut out = Vec::with_capacity(8 + img.data.len());
    out.extend_from_slice(&img.width.to_le_bytes());
    out.extend_from_slice(&img.height.to_le_bytes());
    out.extend_from_slice(&img.data);
    out
}

/// Mensagem para a interface (`Display` do erro em português).
pub fn err_string(e: CoreError) -> String {
    e.to_string()
}

/// `chat_typing(typing)`: só `true` gera aviso (o núcleo só tem `send_typing`).
pub fn should_send_typing(typing: bool) -> bool {
    typing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_size_limites() {
        assert_eq!(tile_size_from_px([639, 360]), TileSize::Thumbnail);
        assert_eq!(tile_size_from_px([640, 360]), TileSize::Main);
        assert_eq!(tile_size_from_px([1599, 900]), TileSize::Main);
        assert_eq!(tile_size_from_px([1600, 900]), TileSize::Full);
    }

    #[test]
    fn miniatura_formato() {
        let img = Rgba {
            width: 2,
            height: 1,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let bytes = encode_thumbnail(&img);
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &2u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &1u32.to_le_bytes());
        assert_eq!(&bytes[8..], img.data.as_slice());
    }

    #[test]
    fn miniatura_confere_com_decodificador() {
        // Mesma conta de `decodeThumbnail`: len == 8 + w*h*4.
        let img = Rgba {
            width: 3,
            height: 2,
            data: vec![0; 3 * 2 * 4],
        };
        let bytes = encode_thumbnail(&img);
        assert_eq!(bytes.len(), 8 + 3 * 2 * 4);
    }

    #[test]
    fn erro_em_portugues() {
        assert_eq!(err_string(CoreError::NoRoom), "Você não está em uma sala");
    }

    #[test]
    fn typing_so_true() {
        assert!(should_send_typing(true));
        assert!(!should_send_typing(false));
    }
}
