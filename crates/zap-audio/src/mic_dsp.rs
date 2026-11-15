//! Voice Capture DSP (`CWMAudioAEC`) em modo fonte: eco, ruído e ganho (plano §4.2).

use crate::com::ComGuard;
use crate::mic_devices::capture_device_index;
use crate::mic_dsp_keys::{
    AEC_SYSTEM_MODE_SINGLE_CHANNEL_AEC, MFPKEY_WMAAECMA_DEVICE_INDEXES, MFPKEY_WMAAECMA_DMO_SOURCE_MODE,
    MFPKEY_WMAAECMA_FEATR_AGC, MFPKEY_WMAAECMA_FEATR_ECHO_LENGTH, MFPKEY_WMAAECMA_FEATR_NS,
    MFPKEY_WMAAECMA_FEATURE_MODE, MFPKEY_WMAAECMA_SYSTEM_MODE,
};
use crate::resample3::{Upsampler3, IN_SAMPLES};
use crate::{AudioError, FRAME};
use std::cell::{Cell, UnsafeCell};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows::core::{implement, ComObject, Interface, HRESULT};
use windows::Win32::Foundation::{E_ACCESSDENIED, E_INVALIDARG, E_POINTER, PROPERTYKEY, VARIANT_BOOL, VARIANT_TRUE};
use windows::Win32::Media::Audio::{WAVEFORMATEX, WAVE_FORMAT_PCM};
use windows::Win32::Media::DxMediaObjects::{
    IMediaBuffer, IMediaBuffer_Impl, IMediaObject, MoFreeMediaType, MoInitMediaType, DMO_MEDIA_TYPE,
    DMO_OUTPUT_DATA_BUFFER,
};
use windows::Win32::Media::MediaFoundation::{
    CWMAudioAEC, FORMAT_WaveFormatEx, MEDIATYPE_Audio, MEDIASUBTYPE_PCM, WMAAECMA_E_NO_ACTIVE_RENDER_STREAM,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Variant::{VT_BOOL, VT_I4};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

/// Taxa de saída do DMO.
const TAXA_DMO: u32 = 16_000;
/// 10 ms a 16 kHz, em amostras de 16 bits.
const AMOSTRAS_10MS: usize = 160;
/// 10 ms a 16 kHz, em bytes.
const BYTES_10MS: usize = AMOSTRAS_10MS * 2;

/// Empacota `MFPKEY_WMAAECMA_DEVICE_INDEXES`: índice da saída (word alto) e do microfone (word
/// baixo), 16 bits cada; -1 vira 0xFFFF = dispositivo padrão. Formato do exemplo da Microsoft
/// (AEC quick start): `(iSpkDevIdx << 16) + (0x0000ffff & iMicDevIdx)`.
fn pack_device_indexes(saida: i32, mic: i32) -> u32 {
    (((saida as u32) & 0xFFFF) << 16) | ((mic as u32) & 0xFFFF)
}

/// Converte 16 bits com sinal em f32 de -1.0 a 1.0.
fn i16_para_f32(src: &[i16; IN_SAMPLES]) -> [f32; IN_SAMPLES] {
    let mut dst = [0.0f32; IN_SAMPLES];
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d = f32::from(s) / 32768.0;
    }
    dst
}

/// Buffer de saída do DMO: 10 ms de PCM 16 bits, sem lock (DMO e chamador estão na mesma thread).
#[implement(IMediaBuffer)]
struct MemBuf {
    dados: UnsafeCell<[u8; BYTES_10MS]>,
    usado: Cell<u32>,
}

impl MemBuf {
    fn novo() -> Self {
        Self {
            dados: UnsafeCell::new([0; BYTES_10MS]),
            usado: Cell::new(0),
        }
    }
}

impl IMediaBuffer_Impl for MemBuf_Impl {
    fn SetLength(&self, cblength: u32) -> windows::core::Result<()> {
        if cblength as usize > BYTES_10MS {
            return Err(E_INVALIDARG.into());
        }
        self.usado.set(cblength);
        Ok(())
    }

    fn GetMaxLength(&self) -> windows::core::Result<u32> {
        Ok(BYTES_10MS as u32)
    }

    fn GetBufferAndLength(&self, ppbuffer: *mut *mut u8, pcblength: *mut u32) -> windows::core::Result<()> {
        if ppbuffer.is_null() && pcblength.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: ponteiros não nulos conferidos; o array vive tanto quanto o objeto COM.
        unsafe {
            if !ppbuffer.is_null() {
                *ppbuffer = self.dados.get().cast::<u8>();
            }
            if !pcblength.is_null() {
                *pcblength = self.usado.get();
            }
        }
        Ok(())
    }
}

fn backend(passo: &str, e: windows::core::Error) -> AudioError {
    if e.code() == E_ACCESSDENIED {
        AudioError::MicBlocked
    } else {
        AudioError::Backend(format!("DSP de voz: {passo}: {e}"))
    }
}

fn pv_i32(v: i32) -> PROPVARIANT {
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 { lVal: v },
            }),
        },
    }
}

fn pv_bool(v: bool) -> PROPVARIANT {
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    boolVal: if v { VARIANT_TRUE } else { VARIANT_BOOL(0) },
                },
            }),
        },
    }
}

fn grava(props: &IPropertyStore, nome: &str, chave: &PROPERTYKEY, valor: PROPVARIANT) -> Result<(), AudioError> {
    // SAFETY: chave e valor vivem durante a chamada; o PROPVARIANT é escalar (nada a liberar).
    unsafe { props.SetValue(chave, &valor) }.map_err(|e| backend(nome, e))
}

pub struct DspCapture {
    dmo: IMediaObject,
    buf: ComObject<MemBuf>,
    iface: IMediaBuffer,
    up: Upsampler3,
    /// Amostras de 16 kHz já lidas e ainda não entregues.
    pendente: [i16; IN_SAMPLES],
    n_pendente: usize,
    // Por último: o COM só é desfeito depois de liberar o DMO.
    _com: ComGuard,
}

impl DspCapture {
    /// Abre o DMO com 16 kHz mono PCM 16 bits na saída (`None` = microfone padrão). Qualquer
    /// falha vira erro para a captura cair na direta. Deve ser chamado na thread que vai ler.
    pub fn open(device_id: Option<&str>) -> Result<Self, AudioError> {
        let com = ComGuard::new();
        // SAFETY: COM inicializado acima; CLSID do Voice Capture DSP.
        let dmo: IMediaObject = unsafe { CoCreateInstance(&CWMAudioAEC, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| backend("CoCreateInstance(CWMAudioAEC)", e))?;
        let props: IPropertyStore = dmo.cast().map_err(|e| backend("IPropertyStore", e))?;

        let mic = capture_device_index(device_id)?;
        grava(
            &props,
            "DMO_SOURCE_MODE",
            &MFPKEY_WMAAECMA_DMO_SOURCE_MODE,
            pv_bool(true),
        )?;
        grava(
            &props,
            "SYSTEM_MODE",
            &MFPKEY_WMAAECMA_SYSTEM_MODE,
            pv_i32(AEC_SYSTEM_MODE_SINGLE_CHANNEL_AEC),
        )?;
        grava(&props, "FEATURE_MODE", &MFPKEY_WMAAECMA_FEATURE_MODE, pv_bool(true))?;
        grava(&props, "FEATR_NS", &MFPKEY_WMAAECMA_FEATR_NS, pv_i32(1))?;
        grava(&props, "FEATR_AGC", &MFPKEY_WMAAECMA_FEATR_AGC, pv_bool(true))?;
        grava(
            &props,
            "FEATR_ECHO_LENGTH",
            &MFPKEY_WMAAECMA_FEATR_ECHO_LENGTH,
            pv_i32(256),
        )?;
        grava(
            &props,
            "DEVICE_INDEXES",
            &MFPKEY_WMAAECMA_DEVICE_INDEXES,
            pv_i32(pack_device_indexes(-1, mic) as i32),
        )?;

        definir_saida(&dmo)?;
        // SAFETY: tipo de saída já definido; chamada COM simples.
        unsafe { dmo.AllocateStreamingResources() }.map_err(|e| backend("AllocateStreamingResources", e))?;

        let buf = ComObject::new(MemBuf::novo());
        let iface: IMediaBuffer = buf.to_interface();
        Ok(Self {
            dmo,
            buf,
            iface,
            up: Upsampler3::new(),
            pendente: [0; IN_SAMPLES],
            n_pendente: 0,
            _com: com,
        })
    }

    /// Lê 10 ms por `ProcessOutput`, junta 20 ms (320 amostras a 16 kHz) e reamostra para 48 kHz
    /// (`Upsampler3`). `Ok(None)` se `stop` ligou.
    pub fn next_block(&mut self, stop: &AtomicBool) -> Result<Option<Box<[f32; FRAME]>>, AudioError> {
        while self.n_pendente < IN_SAMPLES {
            if stop.load(Ordering::Relaxed) {
                return Ok(None);
            }
            match self.le_10ms() {
                Ok(true) => {}
                Ok(false) => std::thread::sleep(Duration::from_millis(5)),
                Err(e) if e.code() == HRESULT(WMAAECMA_E_NO_ACTIVE_RENDER_STREAM as i32) => {
                    // Sem stream de saída ativo o DMO não produz: entrega silêncio no ritmo normal.
                    std::thread::sleep(Duration::from_millis(20));
                    self.n_pendente = 0;
                    return Ok(Some(Box::new([0.0; FRAME])));
                }
                Err(e) => return Err(backend("ProcessOutput", e)),
            }
        }
        self.n_pendente = 0;
        let entrada = i16_para_f32(&self.pendente);
        let mut saida = Box::new([0.0f32; FRAME]);
        self.up.process(&entrada, &mut saida);
        Ok(Some(saida))
    }

    /// Um `ProcessOutput`; `Ok(true)` se entrou dado em `pendente`.
    fn le_10ms(&mut self) -> windows::core::Result<bool> {
        self.buf.usado.set(0);
        let mut saida = [DMO_OUTPUT_DATA_BUFFER {
            pBuffer: ManuallyDrop::new(Some(self.iface.clone())),
            dwStatus: 0,
            rtTimestamp: 0,
            rtTimelength: 0,
        }];
        let mut status = 0u32;
        // SAFETY: `saida` tem um buffer válido (MemBuf com 320 bytes) e `status` é um u32 local.
        let r = unsafe { self.dmo.ProcessOutput(0, &mut saida, &mut status) };
        // SAFETY: devolve a referência clonada acima; o DMO não fica com ela.
        unsafe { ManuallyDrop::drop(&mut saida[0].pBuffer) };
        r?;
        // Sem dados (S_FALSE ou nada novo) = tamanho 0, com ou sem INCOMPLETE.
        let n = (self.buf.usado.get() as usize).min(BYTES_10MS) / 2;
        if n == 0 {
            return Ok(false);
        }
        // SAFETY: o DMO terminou de escrever; leitura de `n * 2` bytes dentro do array.
        let bytes = unsafe { std::slice::from_raw_parts(self.buf.dados.get().cast::<u8>(), n * 2) };
        for par in bytes.chunks_exact(2) {
            if self.n_pendente < IN_SAMPLES {
                self.pendente[self.n_pendente] = i16::from_le_bytes([par[0], par[1]]);
                self.n_pendente += 1;
            }
        }
        Ok(true)
    }
}

/// PCM 16 bits, mono, 16 kHz como tipo de saída do fluxo 0.
fn definir_saida(dmo: &IMediaObject) -> Result<(), AudioError> {
    let mut mt = DMO_MEDIA_TYPE::default();
    // SAFETY: `mt` zerado; MoInitMediaType aloca `cbFormat` bytes em `pbFormat`.
    unsafe { MoInitMediaType(&mut mt, std::mem::size_of::<WAVEFORMATEX>() as u32) }
        .map_err(|e| backend("MoInitMediaType", e))?;
    mt.majortype = MEDIATYPE_Audio;
    mt.subtype = MEDIASUBTYPE_PCM;
    mt.formattype = FORMAT_WaveFormatEx;
    mt.bFixedSizeSamples = true.into();
    mt.lSampleSize = 2;
    let wf = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 1,
        nSamplesPerSec: TAXA_DMO,
        nAvgBytesPerSec: TAXA_DMO * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    // SAFETY: `pbFormat` tem `size_of::<WAVEFORMATEX>()` bytes (struct packed: escrita sem alinhamento).
    unsafe { std::ptr::write_unaligned(mt.pbFormat.cast::<WAVEFORMATEX>(), wf) };
    // SAFETY: `mt` completo e válido durante a chamada.
    let r = unsafe { dmo.SetOutputType(0, Some(&mt), 0) };
    // SAFETY: libera o `pbFormat` alocado por MoInitMediaType.
    unsafe {
        let _ = MoFreeMediaType(&mut mt);
    }
    r.map_err(|e| backend("SetOutputType 16 kHz mono PCM16", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn empacota_indices_padrao() {
        assert_eq!(pack_device_indexes(-1, -1), 0xFFFF_FFFF);
        assert_eq!(pack_device_indexes(-1, 2), 0xFFFF_0002);
        assert_eq!(pack_device_indexes(1, 0), 0x0001_0000);
    }

    #[test]
    fn i16_para_f32_converte() {
        let mut v = [0i16; IN_SAMPLES];
        v[1] = 32767;
        v[2] = -32768;
        let f = i16_para_f32(&v);
        assert_eq!(f.len(), 320);
        assert_eq!(f[0], 0.0);
        assert!((f[1] - 1.0).abs() < 1e-4);
        assert_eq!(f[2], -1.0);
    }

    #[test]
    fn captura_com_dsp_entrega_blocos() {
        let mut cap = match DspCapture::open(None) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("sem DSP de voz neste PC, teste pulado: {e}");
                return;
            }
        };
        let stop = AtomicBool::new(false);
        let ini = Instant::now();
        let mut n = 0;
        while ini.elapsed() < Duration::from_secs(1) {
            match cap.next_block(&stop) {
                Ok(Some(b)) => {
                    assert_eq!(b.len(), FRAME);
                    assert!(b.iter().all(|x| x.is_finite()));
                    n += 1;
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("erro lendo o DSP, teste pulado: {e}");
                    return;
                }
            }
        }
        assert!(n >= 40, "só {n} blocos em 1 s");
    }

    #[test]
    fn stop_interrompe() {
        let mut cap = match DspCapture::open(None) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("sem DSP de voz neste PC, teste pulado: {e}");
                return;
            }
        };
        let stop = AtomicBool::new(true);
        let ini = Instant::now();
        assert!(matches!(cap.next_block(&stop), Ok(None)));
        assert!(ini.elapsed() < Duration::from_millis(200));
    }
}
