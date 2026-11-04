//! Amostras (`IMFSample`) de entrada e saída.
use crate::{logic, EncodeError};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaBuffer, IMFSample, MFCreateDXGISurfaceBuffer, MFCreateMemoryBuffer, MFCreateSample,
    MFSampleExtension_CleanPoint,
};
use zap_gpu::PixelFormat;

/// Resultado de ler uma amostra de saída.
pub(crate) struct OutputData {
    /// Bytes H.264 concatenados dos buffers da amostra.
    pub data: Vec<u8>,
    /// `MFSampleExtension_CleanPoint` = 1.
    pub clean_point: bool,
    /// `GetSampleTime` em 100 ns (a entrada e a saída casam por esse valor).
    pub time_hns: i64,
}

/// Monta a amostra com o buffer e os tempos de captura/duração.
fn build_sample(buf: &IMFMediaBuffer, capture_us: u64, fps: u32) -> Result<IMFSample, EncodeError> {
    // SAFETY: objetos COM válidos; chamadas sem ponteiros crus.
    unsafe {
        let sample = MFCreateSample().map_err(|e| EncodeError::os("MFCreateSample", &e))?;
        sample
            .AddBuffer(buf)
            .map_err(|e| EncodeError::os("IMFSample::AddBuffer", &e))?;
        sample
            .SetSampleTime(logic::us_to_hns(capture_us))
            .map_err(|e| EncodeError::os("IMFSample::SetSampleTime", &e))?;
        sample
            .SetSampleDuration(logic::frame_duration_hns(fps))
            .map_err(|e| EncodeError::os("IMFSample::SetSampleDuration", &e))?;
        Ok(sample)
    }
}

/// Amostra de entrada na GPU (textura NV12): buffer DXGI + `MFCreateSample` + `AddBuffer`, com
/// tempo e duração em 100 ns. Define `SetCurrentLength` (`w*h*3/2`), obrigatório em alguns drivers.
pub(crate) fn input_from_texture(tex: &zap_gpu::Texture, capture_us: u64, fps: u32) -> Result<IMFSample, EncodeError> {
    if tex.format() != PixelFormat::Nv12 {
        return Err(EncodeError::InvalidConfig(
            "a textura de entrada do codificador deve ser NV12".into(),
        ));
    }
    // SAFETY: a textura é válida durante a chamada; o buffer criado guarda sua própria referência.
    let buf = unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, tex.raw(), 0, false) }
        .map_err(|e| EncodeError::os("MFCreateDXGISurfaceBuffer", &e))?;
    let len = tex.width() * tex.height() * 3 / 2;
    // SAFETY: objeto COM válido. Alguns drivers exigem o tamanho; falha não é fatal.
    if let Err(e) = unsafe { buf.SetCurrentLength(len) } {
        tracing::debug!("SetCurrentLength no buffer DXGI falhou: {e}");
    }
    build_sample(&buf, capture_us, fps)
}

/// Amostra de entrada em memória (NV12 sem padding, `w*h*3/2` bytes): `MFCreateMemoryBuffer`,
/// copia, `SetCurrentLength`, mesmos tempos da função acima.
pub(crate) fn input_from_memory(nv12: &[u8], capture_us: u64, fps: u32) -> Result<IMFSample, EncodeError> {
    if nv12.is_empty() {
        return Err(EncodeError::InvalidConfig("quadro NV12 vazio".into()));
    }
    let len = u32::try_from(nv12.len()).map_err(|_| EncodeError::InvalidConfig("quadro NV12 grande demais".into()))?;
    // SAFETY: criação simples de objeto COM.
    let buf = unsafe { MFCreateMemoryBuffer(len) }.map_err(|e| EncodeError::os("MFCreateMemoryBuffer", &e))?;
    let mut ptr: *mut u8 = std::ptr::null_mut();
    let mut max_len: u32 = 0;
    // SAFETY: ponteiros de saída apontam para locais válidos da pilha.
    unsafe { buf.Lock(&mut ptr, Some(&mut max_len), None) }.map_err(|e| EncodeError::os("IMFMediaBuffer::Lock", &e))?;
    // Copia antes de propagar qualquer erro, para sempre chamar Unlock.
    let copiou = if !ptr.is_null() && max_len >= len {
        // SAFETY: o buffer travado tem ao menos `len` bytes e não se sobrepõe a `nv12`.
        unsafe { std::ptr::copy_nonoverlapping(nv12.as_ptr(), ptr, nv12.len()) };
        true
    } else {
        false
    };
    // SAFETY: o Lock acima teve sucesso.
    let unlock = unsafe { buf.Unlock() };
    if !copiou {
        return Err(EncodeError::InvalidConfig(
            "buffer de memória menor que o quadro".into(),
        ));
    }
    unlock.map_err(|e| EncodeError::os("IMFMediaBuffer::Unlock", &e))?;
    // SAFETY: objeto COM válido.
    unsafe { buf.SetCurrentLength(len) }.map_err(|e| EncodeError::os("IMFMediaBuffer::SetCurrentLength", &e))?;
    build_sample(&buf, capture_us, fps)
}

/// Lê uma amostra de saída: `ConvertToContiguousBuffer`, `Lock`/`Unlock`, copia os bytes.
pub(crate) fn read_output(sample: &IMFSample) -> Result<OutputData, EncodeError> {
    // SAFETY: objeto COM válido.
    let buf = unsafe { sample.ConvertToContiguousBuffer() }
        .map_err(|e| EncodeError::os("IMFSample::ConvertToContiguousBuffer", &e))?;
    let mut ptr: *mut u8 = std::ptr::null_mut();
    let mut cur_len: u32 = 0;
    // SAFETY: ponteiros de saída apontam para locais válidos da pilha.
    unsafe { buf.Lock(&mut ptr, None, Some(&mut cur_len)) }.map_err(|e| EncodeError::os("IMFMediaBuffer::Lock", &e))?;
    let data = if ptr.is_null() || cur_len == 0 {
        Vec::new()
    } else {
        // SAFETY: entre Lock e Unlock o ponteiro é válido para `cur_len` bytes.
        unsafe { std::slice::from_raw_parts(ptr, cur_len as usize) }.to_vec()
    };
    // SAFETY: o Lock acima teve sucesso.
    unsafe { buf.Unlock() }.map_err(|e| EncodeError::os("IMFMediaBuffer::Unlock", &e))?;
    // SAFETY: objeto COM válido; atributo ausente vale 0.
    let clean_point = unsafe { sample.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0;
    // SAFETY: objeto COM válido.
    let time_hns = unsafe { sample.GetSampleTime() }.unwrap_or(0);
    Ok(OutputData {
        data,
        clean_point,
        time_hns,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::mf::MfGuard;
    use zap_gpu::GpuDevice;

    #[test]
    fn memoria_roundtrip() {
        let _g = MfGuard::acquire().unwrap();
        let dados: Vec<u8> = (0..64 * 64 * 3 / 2).map(|i| (i % 251) as u8).collect();
        assert_eq!(dados.len(), 6144);
        let us = 123_456u64;
        let s = input_from_memory(&dados, us, 30).unwrap();
        // SAFETY: objeto COM válido.
        unsafe {
            assert_eq!(s.GetSampleTime().unwrap(), (us * 10) as i64);
            assert_eq!(s.GetSampleDuration().unwrap(), 10_000_000 / 30);
        }
        let out = read_output(&s).unwrap();
        assert_eq!(out.data, dados);
        assert!(!out.clean_point);
        assert_eq!(out.time_hns, (us * 10) as i64);
    }

    #[test]
    fn memoria_vazia_erro() {
        let _g = MfGuard::acquire().unwrap();
        assert!(matches!(
            input_from_memory(&[], 0, 30),
            Err(EncodeError::InvalidConfig(_))
        ));
    }

    #[test]
    fn textura_bgra_rejeitada() {
        let _g = MfGuard::acquire().unwrap();
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("sem GPU: teste pulado");
            return;
        };
        let tex = zap_gpu::Texture::new(&dev, 64, 64, PixelFormat::Bgra8, false).unwrap();
        assert!(input_from_texture(&tex, 0, 30).is_err());
    }
}
