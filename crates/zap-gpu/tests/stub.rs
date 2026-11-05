#![cfg(not(windows))]
//! Confere que a API pública compila fora do Windows e devolve `Unsupported`.
use std::time::Duration;

use zap_gpu::{
    cpu_threads, now_us, BatchOptions, CpuScaler, FormatSupport, GpuDevice, GpuError, Nv12Mut, PixelFormat,
    ReadbackStaging, ReadbackTiming, SrcImage, UploadStaging, UploadTiming, Vendor,
};

/// Confere em tempo de compilação que os tipos de imagem da API portátil são públicos.
#[allow(dead_code)]
fn tipos_portateis(_src: Option<SrcImage<'_>>, _dst: Option<Nv12Mut<'_>>) {}

#[test]
fn api_publica_compila_e_devolve_unsupported() {
    assert!(matches!(GpuDevice::create_default(), Err(GpuError::Unsupported)));
    assert_eq!(Vendor::from_pci_id(0x10DE), Vendor::Nvidia);
    assert_eq!(PixelFormat::Bgra8.frame_bytes(2, 2), 16);

    let t0 = now_us();
    std::thread::sleep(Duration::from_millis(1));
    assert!(now_us() > t0);
    assert!(now_us() > 0);
}

#[test]
fn format_support_default_publico() {
    let s = FormatSupport::default();
    assert_eq!(
        s,
        FormatSupport {
            texture2d: false,
            render_target: false,
            cpu_lockable: false,
            video_processor_input: false,
            video_processor_output: false,
        }
    );
}

#[test]
fn create_basic_publico_unsupported() {
    assert!(matches!(GpuDevice::create_basic(), Err(GpuError::Unsupported)));
}

#[test]
fn readback_e_upload_staging_unsupported() {
    // Fora do Windows não existe `GpuDevice` público (todo construtor devolve `Unsupported`),
    // então não dá para chamar `new` de verdade. Fixamos as assinaturas com ponteiros de função
    // e conferimos que o construtor do dispositivo falha.
    let _new_rb: fn(&GpuDevice, u32, u32, PixelFormat) -> Result<ReadbackStaging, GpuError> = ReadbackStaging::new;
    let _new_up: fn(&GpuDevice, u32, u32) -> Result<UploadStaging, GpuError> = UploadStaging::new;
    assert!(GpuDevice::create_default().is_err());

    let rb = ReadbackTiming::default();
    let up = UploadTiming::default();
    assert_eq!(rb.map_attempts, 0);
    assert_eq!(up.map_attempts, 0);
}

#[test]
fn cpu_scaler_portavel_funciona() {
    // O conversor de CPU é portável: funciona fora do Windows também.
    let scaler = CpuScaler::new(64, 64, PixelFormat::Bgra8, 64, 64);
    assert!(scaler.is_ok());

    let n = cpu_threads();
    assert!((1..=4).contains(&n));
    assert_eq!(BatchOptions::default().threads, n);

    // Checa que a API de lote existe com o nome público esperado.
    let _f = zap_gpu::convert_batch;
}
