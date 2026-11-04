//! Cabeçalhos binários de mídia (`docs/api/midia.md`), big-endian.
use crate::error::ProtoError;

pub const VIDEO_HEADER_LEN: usize = 30;
pub const AUDIO_HEADER_LEN: usize = 20;
pub const PROBE_HEADER_LEN: usize = 12;
pub const TYPE_VIDEO: u8 = 0x01;
pub const TYPE_AUDIO: u8 = 0x02;
pub const TYPE_PROBE: u8 = 0x03;

/// Converte um trecho em array de tamanho fixo; trecho curto vira `Truncated`.
fn arr<const N: usize>(s: &[u8]) -> Result<[u8; N], ProtoError> {
    s.try_into().map_err(|_| ProtoError::Truncated)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoFlags(pub u8);

impl VideoFlags {
    pub const KEYFRAME: u8 = 0x01;
    pub const PARAMS: u8 = 0x02;
    pub fn is_keyframe(self) -> bool {
        self.0 & Self::KEYFRAME != 0
    }
    pub fn has_params(self) -> bool {
        self.0 & Self::PARAMS != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoHeader {
    pub flags: VideoFlags,
    pub track: u16,
    pub layer: u8,
    pub width: u16,
    pub height: u16,
    pub seq: u32,
    pub capture_us: u64,
    pub send_us: u32,
    pub size: u32,
}

impl VideoHeader {
    pub fn encode(&self, out: &mut [u8; VIDEO_HEADER_LEN]) {
        out[0] = TYPE_VIDEO;
        out[1] = self.flags.0;
        out[2..4].copy_from_slice(&self.track.to_be_bytes());
        out[4] = self.layer;
        // Byte 5 é reservado: sempre zero na codificação.
        out[5] = 0;
        out[6..8].copy_from_slice(&self.width.to_be_bytes());
        out[8..10].copy_from_slice(&self.height.to_be_bytes());
        out[10..14].copy_from_slice(&self.seq.to_be_bytes());
        out[14..22].copy_from_slice(&self.capture_us.to_be_bytes());
        out[22..26].copy_from_slice(&self.send_us.to_be_bytes());
        out[26..30].copy_from_slice(&self.size.to_be_bytes());
    }

    /// Lê só os 30 primeiros bytes; o payload H.264 vem depois, fora do cabeçalho.
    /// Byte reservado e bits desconhecidos de flags são aceitos e ignorados.
    pub fn decode(buf: &[u8]) -> Result<Self, ProtoError> {
        let h: [u8; VIDEO_HEADER_LEN] = arr(buf.get(..VIDEO_HEADER_LEN).ok_or(ProtoError::Truncated)?)?;
        if h[0] != TYPE_VIDEO {
            return Err(ProtoError::BadType(h[0]));
        }
        Ok(VideoHeader {
            flags: VideoFlags(h[1]),
            track: u16::from_be_bytes(arr(&h[2..4])?),
            layer: h[4],
            width: u16::from_be_bytes(arr(&h[6..8])?),
            height: u16::from_be_bytes(arr(&h[8..10])?),
            seq: u32::from_be_bytes(arr(&h[10..14])?),
            capture_us: u64::from_be_bytes(arr(&h[14..22])?),
            send_us: u32::from_be_bytes(arr(&h[22..26])?),
            size: u32::from_be_bytes(arr(&h[26..30])?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHeader {
    pub track: u16,
    pub seq: u32,
    pub capture_us: u64,
    pub send_us: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeHeader {
    pub cluster: u16,
    pub seq: u32,
    pub send_us: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Datagram<'a> {
    Audio(AudioHeader, &'a [u8]),
    /// Segundo campo: bytes totais do datagrama.
    Probe(ProbeHeader, usize),
}

pub fn encode_audio(h: &AudioHeader, opus: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(AUDIO_HEADER_LEN + opus.len());
    out.push(TYPE_AUDIO);
    out.push(0);
    out.extend_from_slice(&h.track.to_be_bytes());
    out.extend_from_slice(&h.seq.to_be_bytes());
    out.extend_from_slice(&h.capture_us.to_be_bytes());
    out.extend_from_slice(&h.send_us.to_be_bytes());
    out.extend_from_slice(opus);
    out
}

/// Completa com zeros até `total_len` (>= 12; menor vira 12).
pub fn encode_probe(h: &ProbeHeader, total_len: usize) -> Vec<u8> {
    let len = total_len.max(PROBE_HEADER_LEN);
    let mut out = vec![0u8; len];
    out[0] = TYPE_PROBE;
    out[1] = 0;
    out[2..4].copy_from_slice(&h.cluster.to_be_bytes());
    out[4..8].copy_from_slice(&h.seq.to_be_bytes());
    out[8..12].copy_from_slice(&h.send_us.to_be_bytes());
    out
}

pub fn decode_datagram(buf: &[u8]) -> Result<Datagram<'_>, ProtoError> {
    let tipo = *buf.first().ok_or(ProtoError::Truncated)?;
    match tipo {
        TYPE_AUDIO => {
            let h: [u8; AUDIO_HEADER_LEN] = arr(buf.get(..AUDIO_HEADER_LEN).ok_or(ProtoError::Truncated)?)?;
            let header = AudioHeader {
                track: u16::from_be_bytes(arr(&h[2..4])?),
                seq: u32::from_be_bytes(arr(&h[4..8])?),
                capture_us: u64::from_be_bytes(arr(&h[8..16])?),
                send_us: u32::from_be_bytes(arr(&h[16..20])?),
            };
            Ok(Datagram::Audio(header, &buf[AUDIO_HEADER_LEN..]))
        }
        TYPE_PROBE => {
            let h: [u8; PROBE_HEADER_LEN] = arr(buf.get(..PROBE_HEADER_LEN).ok_or(ProtoError::Truncated)?)?;
            let header = ProbeHeader {
                cluster: u16::from_be_bytes(arr(&h[2..4])?),
                seq: u32::from_be_bytes(arr(&h[4..8])?),
                send_us: u32::from_be_bytes(arr(&h[8..12])?),
            };
            Ok(Datagram::Probe(header, buf.len()))
        }
        outro => Err(ProtoError::BadType(outro)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video_exemplo() -> VideoHeader {
        VideoHeader {
            flags: VideoFlags(VideoFlags::KEYFRAME | VideoFlags::PARAMS),
            track: 0xBEEF,
            layer: 3,
            width: 1920,
            height: 1080,
            seq: 0x0102_0304,
            capture_us: 0x1122_3344_5566_7788,
            send_us: 0x99AA_BBCC,
            size: 0xDDEE_FF00,
        }
    }

    #[test]
    fn video_roundtrip() {
        let h = VideoHeader {
            flags: VideoFlags(0xA5),
            track: 0x1234,
            layer: 7,
            width: 0xFEDC,
            height: 0x0BA9,
            seq: u32::MAX,
            capture_us: u64::MAX,
            send_us: u32::MAX,
            size: 0x0F0E_0D0C,
        };
        let mut out = [0u8; VIDEO_HEADER_LEN];
        h.encode(&mut out);
        assert_eq!(VideoHeader::decode(&out).unwrap(), h);
    }

    #[test]
    fn video_bytes_exatos() {
        let h = VideoHeader {
            flags: VideoFlags(0x03),
            track: 0x0102,
            layer: 0x04,
            width: 0x0506,
            height: 0x0708,
            seq: 0x090A_0B0C,
            capture_us: 0x0D0E_0F10_1112_1314,
            send_us: 0x1516_1718,
            size: 0x191A_1B1C,
        };
        let esperado: [u8; 30] = [
            0x01, 0x03, 0x01, 0x02, 0x04, 0x00, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
        ];
        let mut out = [0u8; VIDEO_HEADER_LEN];
        h.encode(&mut out);
        assert_eq!(out, esperado);
    }

    #[test]
    fn video_curto() {
        assert!(matches!(VideoHeader::decode(&[0x01; 29]), Err(ProtoError::Truncated)));
    }

    #[test]
    fn video_tipo_errado() {
        let mut buf = [0u8; 30];
        buf[0] = 0x02;
        assert!(matches!(VideoHeader::decode(&buf), Err(ProtoError::BadType(2))));
    }

    #[test]
    fn video_reservado_ignorado() {
        let mut buf = [0u8; 30];
        buf[0] = TYPE_VIDEO;
        buf[1] = 0xFF;
        buf[5] = 0xFF;
        let h = VideoHeader::decode(&buf).unwrap();
        assert_eq!(h.flags, VideoFlags(0xFF));
        assert!(h.flags.is_keyframe());
        assert!(h.flags.has_params());
    }

    #[test]
    fn video_com_payload() {
        let h = video_exemplo();
        let mut buf = vec![0u8; VIDEO_HEADER_LEN];
        h.encode(buf.as_mut_slice().try_into().unwrap());
        buf.extend_from_slice(&[0xAA; 100]);
        assert_eq!(VideoHeader::decode(&buf).unwrap(), h);
    }

    #[test]
    fn audio_roundtrip() {
        let h = AudioHeader {
            track: 0xCAFE,
            seq: u32::MAX,
            capture_us: u64::MAX,
            send_us: 42,
        };
        let buf = encode_audio(&h, &[1, 2, 3]);
        assert_eq!(buf.len(), AUDIO_HEADER_LEN + 3);
        assert_eq!(decode_datagram(&buf).unwrap(), Datagram::Audio(h, &[1, 2, 3][..]));

        let buf = encode_audio(&h, &[]);
        assert_eq!(buf.len(), AUDIO_HEADER_LEN);
        assert_eq!(decode_datagram(&buf).unwrap(), Datagram::Audio(h, &[][..]));
    }

    #[test]
    fn audio_curto() {
        assert!(matches!(decode_datagram(&[TYPE_AUDIO; 19]), Err(ProtoError::Truncated)));
    }

    #[test]
    fn probe_tamanho() {
        let h = ProbeHeader {
            cluster: 0x0A0B,
            seq: 0x0C0D_0E0F,
            send_us: 0x1011_1213,
        };
        let buf = encode_probe(&h, 1200);
        assert_eq!(buf.len(), 1200);
        assert!(buf[12..].iter().all(|&b| b == 0));
        assert_eq!(buf[0], TYPE_PROBE);
        assert_eq!(decode_datagram(&buf).unwrap(), Datagram::Probe(h, 1200));
    }

    #[test]
    fn probe_minimo() {
        let h = ProbeHeader {
            cluster: 1,
            seq: 2,
            send_us: 3,
        };
        let buf = encode_probe(&h, 5);
        assert_eq!(buf.len(), PROBE_HEADER_LEN);
        assert_eq!(decode_datagram(&buf).unwrap(), Datagram::Probe(h, 12));
    }

    #[test]
    fn datagrama_desconhecido() {
        assert!(matches!(decode_datagram(&[0x09, 0, 0, 0]), Err(ProtoError::BadType(9))));
    }

    #[test]
    fn datagrama_vazio() {
        assert!(matches!(decode_datagram(&[]), Err(ProtoError::Truncated)));
    }
}
