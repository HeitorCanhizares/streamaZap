//! Tipos de mídia (`IMFMediaType`) de entrada e saída do codificador.
use crate::{EncodeError, EncoderConfig};
use windows::core::GUID;
use windows::Win32::Media::MediaFoundation::{
    eAVEncH264VProfile_High, IMFMediaType, MFCreateMediaType, MFMediaType_Video, MFVideoFormat_H264,
    MFVideoFormat_NV12, MFVideoInterlace_Progressive, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_MPEG_SEQUENCE_HEADER,
    MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
};

/// Empacota dois u32 num u64 (alto, baixo), como `MFSetAttributeSize/Ratio` do C++.
fn pack(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

/// Atributos comuns: tipo principal, subtipo, tamanho, taxa, progressivo e aspecto 1/1.
fn base_type(cfg: &EncoderConfig, subtype: &GUID) -> Result<IMFMediaType, EncodeError> {
    // SAFETY: todas as chamadas abaixo usam o objeto COM recém-criado e referências válidas
    // a GUIDs estáticos; não há ponteiros crus de nossa parte.
    unsafe {
        let t = MFCreateMediaType().map_err(|e| EncodeError::os("MFCreateMediaType", &e))?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| EncodeError::os("SetGUID(MF_MT_MAJOR_TYPE)", &e))?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)
            .map_err(|e| EncodeError::os("SetGUID(MF_MT_SUBTYPE)", &e))?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(|e| EncodeError::os("SetUINT32(MF_MT_INTERLACE_MODE)", &e))?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(cfg.width, cfg.height))
            .map_err(|e| EncodeError::os("SetUINT64(MF_MT_FRAME_SIZE)", &e))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(cfg.fps, 1))
            .map_err(|e| EncodeError::os("SetUINT64(MF_MT_FRAME_RATE)", &e))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))
            .map_err(|e| EncodeError::os("SetUINT64(MF_MT_PIXEL_ASPECT_RATIO)", &e))?;
        Ok(t)
    }
}

/// Tipo de saída H264: subtipo H264, `MF_MT_FRAME_SIZE`, `MF_MT_FRAME_RATE` (fps/1),
/// `MF_MT_AVG_BITRATE` (kbps*1000), `MF_MT_INTERLACE_MODE = Progressive`,
/// `MF_MT_MPEG2_PROFILE = eAVEncH264VProfile_High`, `MF_MT_PIXEL_ASPECT_RATIO` 1/1.
pub(crate) fn make_output_type(cfg: &EncoderConfig) -> Result<IMFMediaType, EncodeError> {
    let t = base_type(cfg, &MFVideoFormat_H264)?;
    // SAFETY: `t` é um objeto COM válido; só passamos GUIDs estáticos e inteiros.
    unsafe {
        t.SetUINT32(&MF_MT_AVG_BITRATE, cfg.kbps.saturating_mul(1000))
            .map_err(|e| EncodeError::os("SetUINT32(MF_MT_AVG_BITRATE)", &e))?;
        t.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)
            .map_err(|e| EncodeError::os("SetUINT32(MF_MT_MPEG2_PROFILE)", &e))?;
    }
    Ok(t)
}

/// Tipo de entrada NV12 com o mesmo tamanho/fps, progressivo, `MF_MT_DEFAULT_STRIDE` = largura.
pub(crate) fn make_input_type(cfg: &EncoderConfig) -> Result<IMFMediaType, EncodeError> {
    let t = base_type(cfg, &MFVideoFormat_NV12)?;
    // SAFETY: `t` é um objeto COM válido; só passamos um GUID estático e um inteiro.
    unsafe {
        t.SetUINT32(&MF_MT_DEFAULT_STRIDE, cfg.width)
            .map_err(|e| EncodeError::os("SetUINT32(MF_MT_DEFAULT_STRIDE)", &e))?;
    }
    Ok(t)
}

/// Lê `MF_MT_MPEG_SEQUENCE_HEADER` do tipo de saída atual (blob) e normaliza com
/// `nal::sequence_header_to_annexb`. Sem o atributo: vetor vazio.
pub(crate) fn read_sequence_header(out_type: &IMFMediaType) -> Vec<u8> {
    // SAFETY: o buffer tem exatamente `GetBlobSize` bytes e é passado como fatia mutável válida.
    unsafe {
        let Ok(size) = out_type.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) else {
            return Vec::new();
        };
        let mut raw = vec![0u8; size as usize];
        if out_type.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut raw, None).is_err() {
            return Vec::new();
        }
        crate::nal::sequence_header_to_annexb(&raw)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::mf::MfGuard;
    use crate::EncoderChoice;

    fn cfg() -> EncoderConfig {
        EncoderConfig {
            width: 1280,
            height: 720,
            fps: 60,
            kbps: 4000,
            gop_frames: 600,
            choice: EncoderChoice::Auto,
        }
    }

    #[test]
    fn output_type_tem_atributos() {
        let _g = MfGuard::acquire().unwrap();
        let t = make_output_type(&cfg()).unwrap();
        // SAFETY: objeto COM válido.
        unsafe {
            let v = t.GetUINT64(&MF_MT_FRAME_SIZE).unwrap();
            assert_eq!(((v >> 32) as u32, v as u32), (1280, 720));
            assert_eq!(t.GetUINT32(&MF_MT_AVG_BITRATE).unwrap(), 4_000_000);
            assert_eq!(t.GetGUID(&MF_MT_SUBTYPE).unwrap(), MFVideoFormat_H264);
        }
    }

    #[test]
    fn input_type_nv12() {
        let _g = MfGuard::acquire().unwrap();
        let t = make_input_type(&cfg()).unwrap();
        // SAFETY: objeto COM válido.
        unsafe {
            assert_eq!(t.GetGUID(&MF_MT_SUBTYPE).unwrap(), MFVideoFormat_NV12);
            assert_eq!(t.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap(), 1280);
        }
    }

    #[test]
    fn sequence_header_ausente_vazio() {
        let _g = MfGuard::acquire().unwrap();
        // SAFETY: criação simples de objeto COM.
        let t = unsafe { MFCreateMediaType() }.unwrap();
        assert!(read_sequence_header(&t).is_empty());
    }
}
