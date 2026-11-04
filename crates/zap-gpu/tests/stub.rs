#![cfg(not(windows))]
//! Confere que a API pública compila fora do Windows e devolve `Unsupported`.
use std::time::Duration;

use zap_gpu::{now_us, GpuDevice, GpuError, PixelFormat, Vendor};

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
