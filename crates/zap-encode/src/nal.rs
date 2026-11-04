//! Parser de NAL H.264 em Annex B. Portável.
use std::ops::Range;

/// Tipos de NAL usados pelo projeto.
pub const NAL_SLICE: u8 = 1;
pub const NAL_IDR: u8 = 5;
pub const NAL_SPS: u8 = 7;
pub const NAL_PPS: u8 = 8;

/// Uma NAL localizada no buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NalUnit {
    /// Posição do início do start code (3 ou 4 bytes) no buffer.
    pub start_code_pos: usize,
    /// Bytes do start code (3 ou 4).
    pub start_code_len: usize,
    /// Faixa da NAL sem o start code (cabeçalho de 1 byte + payload), até o próximo start code.
    pub range: Range<usize>,
    /// `nal_unit_type` = primeiro byte da NAL & 0x1F.
    pub nal_type: u8,
}

/// Acha todas as NAL de um buffer Annex B (`00 00 01` ou `00 00 00 01`). Bytes antes do primeiro
/// start code são ignorados. Zeros finais de uma NAL (parte de um start code de 4 bytes do
/// próximo) não entram em `range`. NAL vazia (start code seguido de outro) é ignorada.
pub fn split_nals(data: &[u8]) -> Vec<NalUnit> {
    // Primeiro passo: achar cada `00 00 01`. Se o byte anterior também é 00, o start code
    // tem 4 bytes e começa um byte antes. Não há sobreposição: depois de um match pulamos 3 bytes,
    // e o byte anterior a um próximo match nunca é o `01` do anterior.
    let mut starts: Vec<(usize, usize)> = Vec::new(); // (start_code_pos, start_code_len)
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            if i > 0 && data[i - 1] == 0 {
                starts.push((i - 1, 4));
            } else {
                starts.push((i, 3));
            }
            i += 3;
        } else {
            i += 1;
        }
    }

    // Segundo passo: a NAL vai do fim do start code até o início do próximo (ou fim do buffer).
    // Zeros no fim da faixa (trailing_zero_8bits) são aparados.
    let mut nals = Vec::with_capacity(starts.len());
    for (k, &(pos, len)) in starts.iter().enumerate() {
        let start = pos + len;
        let mut end = starts.get(k + 1).map_or(data.len(), |&(next, _)| next);
        while end > start && data[end - 1] == 0 {
            end -= 1;
        }
        // NAL vazia não serve para nada: ignora.
        if end <= start {
            continue;
        }
        nals.push(NalUnit {
            start_code_pos: pos,
            start_code_len: len,
            range: start..end,
            nal_type: data[start] & 0x1F,
        });
    }
    nals
}

/// Tipos das NAL presentes, na ordem.
pub fn nal_types(data: &[u8]) -> Vec<u8> {
    split_nals(data).into_iter().map(|n| n.nal_type).collect()
}

/// Tem NAL 7 (SPS) e NAL 8 (PPS)?
pub fn has_sps_pps(data: &[u8]) -> bool {
    let types = nal_types(data);
    types.contains(&NAL_SPS) && types.contains(&NAL_PPS)
}

/// Tem NAL 5 (IDR)?
pub fn has_idr(data: &[u8]) -> bool {
    nal_types(data).contains(&NAL_IDR)
}

/// Garante SPS/PPS no começo de um keyframe. Se `data` já tem SPS e PPS devolve `(data, true)`;
/// senão, se `header` (Annex B com SPS+PPS) tem os dois, devolve `(header ++ data, true)`;
/// senão `(data, false)`.
pub fn ensure_params(data: &[u8], header: &[u8]) -> (Vec<u8>, bool) {
    if has_sps_pps(data) {
        return (data.to_vec(), true);
    }
    if has_sps_pps(header) {
        let mut out = Vec::with_capacity(header.len() + data.len());
        out.extend_from_slice(header);
        out.extend_from_slice(data);
        return (out, true);
    }
    (data.to_vec(), false)
}

/// Extrai SPS e PPS de um `MF_MT_MPEG_SEQUENCE_HEADER`. Aceita Annex B; se vier em formato
/// AVCDecoderConfigurationRecord (primeiro byte 1) converte para Annex B (start code de 4 bytes).
/// Entrada que não é nenhum dos dois: vetor vazio.
pub fn sequence_header_to_annexb(raw: &[u8]) -> Vec<u8> {
    if raw.is_empty() {
        return Vec::new();
    }
    if !split_nals(raw).is_empty() {
        return raw.to_vec();
    }
    if raw[0] == 1 && raw.len() >= 7 {
        return avcc_to_annexb(raw).unwrap_or_default();
    }
    Vec::new()
}

/// Converte um AVCDecoderConfigurationRecord em Annex B. `None` se estiver truncado.
fn avcc_to_annexb(raw: &[u8]) -> Option<Vec<u8>> {
    // Byte 5 & 0x1F = número de SPS; depois, cada SPS é `u16 BE tamanho + bytes`.
    let mut out = Vec::new();
    let mut offset = 6;
    let num_sps = usize::from(raw.get(5)? & 0x1F);
    for _ in 0..num_sps {
        offset = push_avcc_nal(raw, offset, &mut out)?;
    }
    // Depois dos SPS: um byte com o número de PPS, e cada PPS no mesmo formato.
    let num_pps = usize::from(*raw.get(offset)?);
    offset += 1;
    for _ in 0..num_pps {
        offset = push_avcc_nal(raw, offset, &mut out)?;
    }
    Some(out)
}

/// Lê uma NAL `u16 BE tamanho + bytes` em `offset`, grava com start code de 4 bytes e devolve o
/// novo offset. `None` se faltarem bytes.
fn push_avcc_nal(raw: &[u8], offset: usize, out: &mut Vec<u8>) -> Option<usize> {
    let hi = *raw.get(offset)?;
    let lo = *raw.get(offset + 1)?;
    let size = usize::from(u16::from_be_bytes([hi, lo]));
    let start = offset + 2;
    let body = raw.get(start..start.checked_add(size)?)?;
    out.extend_from_slice(&[0, 0, 0, 1]);
    out.extend_from_slice(body);
    Some(start + size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_start_codes_3_e_4_bytes() {
        // SPS com start code de 4 bytes, PPS e IDR com 3 e 4 bytes.
        let data = [
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, // SPS (4 bytes de start code)
            0x00, 0x00, 0x01, 0x68, 0xCE, // PPS (3 bytes)
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, // IDR (4 bytes)
        ];
        let nals = split_nals(&data);
        assert_eq!(nals.len(), 3);
        assert_eq!(nals[0].nal_type, NAL_SPS);
        assert_eq!(nals[0].start_code_pos, 0);
        assert_eq!(nals[0].start_code_len, 4);
        assert_eq!(nals[0].range, 4..6);
        assert_eq!(nals[1].nal_type, NAL_PPS);
        assert_eq!(nals[1].start_code_pos, 6);
        assert_eq!(nals[1].start_code_len, 3);
        assert_eq!(nals[1].range, 9..11);
        assert_eq!(nals[2].nal_type, NAL_IDR);
        assert_eq!(nals[2].start_code_pos, 11);
        assert_eq!(nals[2].start_code_len, 4);
        assert_eq!(nals[2].range, 15..18);
        assert_eq!(nal_types(&data), vec![NAL_SPS, NAL_PPS, NAL_IDR]);
    }

    #[test]
    fn ignora_lixo_antes_do_primeiro_start_code() {
        let data = [0xAA, 0xBB, 0x00, 0x00, 0x01, 0x65, 0x01];
        let nals = split_nals(&data);
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].start_code_pos, 2);
        assert_eq!(nals[0].range, 5..7);
        assert_eq!(nals[0].nal_type, NAL_IDR);
    }

    #[test]
    fn apara_zeros_finais() {
        // Payload seguido de zeros antes de um start code de 3 bytes.
        let data = [
            0x00, 0x00, 0x01, 0x41, 0x9A, 0x00, 0x00, // zeros finais
            0x00, 0x00, 0x01, 0x41, 0x00, 0x00, // zeros finais no fim do buffer
        ];
        let nals = split_nals(&data);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0].range, 3..5);
        assert_eq!(nals[1].range, 10..11);
        assert_eq!(nals[0].nal_type, NAL_SLICE);
        assert_eq!(nals[1].nal_type, NAL_SLICE);
    }

    #[test]
    fn nal_vazia_ignorada() {
        // Dois start codes seguidos: a primeira NAL é vazia e some.
        let data = [0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x67, 0x42];
        let nals = split_nals(&data);
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].nal_type, NAL_SPS);
        assert_eq!(nals[0].start_code_pos, 3);
        assert_eq!(nals[0].range, 6..8);
    }

    #[test]
    fn has_sps_pps_e_has_idr() {
        let com = [0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x01, 0x68, 0x00, 0x00, 0x01, 0x65];
        assert!(has_sps_pps(&com));
        assert!(has_idr(&com));

        let so_sps = [0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x01, 0x41];
        assert!(!has_sps_pps(&so_sps));
        assert!(!has_idr(&so_sps));
    }

    #[test]
    fn ensure_params_ja_tem() {
        let data = [0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x01, 0x68, 0x00, 0x00, 0x01, 0x65];
        let header = [0x00, 0x00, 0x01, 0x41];
        let (out, ok) = ensure_params(&data, &header);
        assert!(ok);
        assert_eq!(out, data.to_vec());
    }

    #[test]
    fn ensure_params_usa_header() {
        let data = [0x00, 0x00, 0x01, 0x65, 0x01];
        let header = [0x00, 0x00, 0x01, 0x67, 0x00, 0x00, 0x01, 0x68];
        let (out, ok) = ensure_params(&data, &header);
        assert!(ok);
        assert_eq!(out, [header.as_slice(), data.as_slice()].concat());
    }

    #[test]
    fn ensure_params_sem_header() {
        let data = [0x00, 0x00, 0x01, 0x65, 0x01];
        let header = [0x00, 0x00, 0x01, 0x67];
        let (out, ok) = ensure_params(&data, &header);
        assert!(!ok);
        assert_eq!(out, data.to_vec());
    }

    #[test]
    fn sequence_header_annexb_passa_direto() {
        let raw = [0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x00, 0x01, 0x68, 0xCE];
        assert_eq!(sequence_header_to_annexb(&raw), raw.to_vec());
    }

    #[test]
    fn sequence_header_avcc_converte() {
        // AVCDecoderConfigurationRecord com 1 SPS (67 42) e 1 PPS (68 CE).
        let avcc = [
            0x01, 0x42, 0x00, 0x1E, 0xFF, // versão, perfil, compat, nível, lengthSizeMinusOne
            0xE1, // 1 SPS
            0x00, 0x02, 0x67, 0x42, // SPS
            0x01, // 1 PPS
            0x00, 0x02, 0x68, 0xCE, // PPS
        ];
        let esperado = [0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x00, 0x00, 0x01, 0x68, 0xCE];
        assert_eq!(sequence_header_to_annexb(&avcc), esperado.to_vec());
    }

    #[test]
    fn sequence_header_avcc_truncado_vazio() {
        let avcc = [
            0x01, 0x42, 0x00, 0x1E, 0xFF, 0xE1, 0x00, 0x02, 0x67, 0x42, 0x01, 0x00, 0x02, 0x68, 0xCE,
        ];
        // Corta no meio do PPS.
        assert!(sequence_header_to_annexb(&avcc[..12]).is_empty());
        // Corta logo após o cabeçalho.
        assert!(sequence_header_to_annexb(&avcc[..7]).is_empty());
    }

    #[test]
    fn entrada_vazia_nao_panica() {
        let vazio: &[u8] = &[];
        assert!(split_nals(vazio).is_empty());
        assert!(nal_types(vazio).is_empty());
        assert!(!has_sps_pps(vazio));
        assert!(!has_idr(vazio));
        let (out, ok) = ensure_params(vazio, vazio);
        assert!(out.is_empty());
        assert!(!ok);
        assert!(sequence_header_to_annexb(vazio).is_empty());
    }
}
