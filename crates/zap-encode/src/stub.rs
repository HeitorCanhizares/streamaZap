//! Stub para plataformas sem Media Foundation: tudo devolve `EncodeError::Unsupported`.
use crate::{EncodeError, EncodedPicture, EncoderConfig, EncoderInfo};

pub fn list_encoders() -> Result<Vec<EncoderInfo>, EncodeError> {
    Err(EncodeError::Unsupported)
}

// Sem Media Foundation nunca há instância: `new` falha antes de construir, então `info`
// não é preenchido de verdade aqui.
pub struct H264Encoder {
    info: EncoderInfo,
}

impl H264Encoder {
    pub fn new(
        _dev: &zap_gpu::GpuDevice,
        _cfg: EncoderConfig,
        _out: crossbeam_channel::Sender<EncodedPicture>,
    ) -> Result<Self, EncodeError> {
        Err(EncodeError::Unsupported)
    }
    pub fn submit(&self, _nv12: zap_gpu::Texture, _capture_us: u64, _force_keyframe: bool) -> bool {
        false
    }
    pub fn set_bitrate(&self, _kbps: u32) {}
    pub fn info(&self) -> &EncoderInfo {
        &self.info
    }
    pub fn dropped(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_encoders_nao_suportado() {
        assert!(matches!(list_encoders(), Err(EncodeError::Unsupported)));
    }

    #[test]
    fn new_nao_suportado() {
        // No stub do zap-gpu o dispositivo nunca existe, então `H264Encoder::new` não é
        // alcançável no Linux. Confere só que a origem do erro já é `Unsupported`.
        assert!(matches!(
            zap_gpu::GpuDevice::create_default(),
            Err(zap_gpu::GpuError::Unsupported)
        ));
    }
}
