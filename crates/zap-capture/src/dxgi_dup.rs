//! Envoltório fino da Desktop Duplication.
#![allow(dead_code)] // usado só quando dxgi.rs (capture-08) for ligado
use crate::{CaptureError, MonitorInfo};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutput5, IDXGIOutputDuplication, IDXGIResource,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use zap_gpu::GpuDevice;

/// Resultado de uma espera por quadro.
pub(crate) enum Acquired {
    /// `WAIT_TIMEOUT`.
    Timeout,
    /// `DXGI_ERROR_ACCESS_LOST`: é preciso recriar a duplicação.
    Lost,
    Frame {
        /// `LastPresentTime != 0`: imagem nova em `texture`.
        new_image: bool,
        /// `LastMouseUpdateTime != 0`.
        mouse_updated: bool,
        /// Recurso adquirido (válido até `release`). Só existe quando `new_image`.
        texture: Option<ID3D11Texture2D>,
    },
}

fn os_err(context: &'static str, e: windows::core::Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.to_string(),
    }
}

pub(crate) struct Duplication {
    dupl: IDXGIOutputDuplication,
    width: u32,
    height: u32,
}

/// Acha a saída DXGI do monitor (HMONITOR) no adaptador do LUID dado (0 = qualquer adaptador).
fn achar_saida(monitor: &MonitorInfo) -> Result<IDXGIOutput, CaptureError> {
    // SAFETY: chamadas COM com ponteiros locais válidos; os objetos são liberados ao sair do escopo.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| os_err("CreateDXGIFactory1", e))?;
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            a += 1;
            if monitor.adapter_luid != 0 {
                let Ok(desc) = adapter.GetDesc1() else { continue };
                let luid = ((desc.AdapterLuid.HighPart as i64) << 32) | desc.AdapterLuid.LowPart as i64;
                if luid != monitor.adapter_luid {
                    continue;
                }
            }
            let mut o = 0;
            while let Ok(output) = adapter.EnumOutputs(o) {
                o += 1;
                if let Ok(od) = output.GetDesc() {
                    if od.Monitor.0 as isize == monitor.id {
                        return Ok(output);
                    }
                }
            }
        }
    }
    Err(CaptureError::MonitorNotFound)
}

impl Duplication {
    /// `IDXGIOutput5::DuplicateOutput1` (BGRA) com fallback `IDXGIOutput1::DuplicateOutput`.
    /// Acha a saída pelo `monitor.id` (HMONITOR) no adaptador de `monitor.adapter_luid`.
    pub(crate) fn new(dev: &GpuDevice, monitor: &MonitorInfo) -> Result<Self, CaptureError> {
        let output = achar_saida(monitor)?;
        // Tenta a versão 1 (aceita formatos); se faltar a interface ou falhar, cai para a clássica.
        // SAFETY: o dispositivo D3D11 é válido durante a chamada e a lista de formatos é local.
        let dupl = unsafe {
            let nova = output
                .cast::<IDXGIOutput5>()
                .and_then(|o5| o5.DuplicateOutput1(dev.d3d_device(), 0, &[DXGI_FORMAT_B8G8R8A8_UNORM]));
            match nova {
                Ok(d) => d,
                Err(e) => {
                    tracing::debug!("DuplicateOutput1 indisponível, usando DuplicateOutput: {e}");
                    let o1 = output.cast::<IDXGIOutput1>().map_err(|e| os_err("IDXGIOutput1", e))?;
                    o1.DuplicateOutput(dev.d3d_device())
                        .map_err(|e| os_err("DuplicateOutput", e))?
                }
            }
        };
        // SAFETY: `dupl` é válido; `GetDesc` só escreve no retorno.
        let desc = unsafe { dupl.GetDesc() };
        Ok(Self {
            dupl,
            width: desc.ModeDesc.Width,
            height: desc.ModeDesc.Height,
        })
    }

    pub(crate) fn acquire(&mut self, timeout_ms: u32) -> Result<Acquired, CaptureError> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        // SAFETY: `info` e `resource` são locais e válidos; o quadro é liberado em `release`.
        let r = unsafe { self.dupl.AcquireNextFrame(timeout_ms, &mut info, &mut resource) };
        match r {
            Ok(()) => {
                let new_image = info.LastPresentTime != 0;
                let mouse_updated = info.LastMouseUpdateTime != 0;
                let texture = if new_image {
                    resource.and_then(|r| r.cast::<ID3D11Texture2D>().ok())
                } else {
                    None
                };
                Ok(Acquired::Frame {
                    new_image,
                    mouse_updated,
                    texture,
                })
            }
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => Ok(Acquired::Timeout),
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => Ok(Acquired::Lost),
            Err(e) => Err(os_err("AcquireNextFrame", e)),
        }
    }

    /// `ReleaseFrame`. Chamar depois de cada `Acquired::Frame`.
    pub(crate) fn release(&mut self) -> Result<(), CaptureError> {
        // SAFETY: chamada COM sobre a duplicação válida.
        match unsafe { self.dupl.ReleaseFrame() } {
            Ok(()) => Ok(()),
            // Duplicação perdida: o chamador recria no próximo acquire.
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => Ok(()),
            Err(e) => Err(os_err("ReleaseFrame", e)),
        }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preparar() -> Option<(GpuDevice, MonitorInfo)> {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pulando: sem GPU ({e})");
                return None;
            }
        };
        match crate::list_monitors() {
            Ok(m) if !m.is_empty() => Some((dev, m.into_iter().next()?)),
            _ => {
                eprintln!("pulando: sem monitores");
                None
            }
        }
    }

    #[test]
    fn duplication_cria_e_tem_tamanho_do_monitor() {
        let Some((dev, mon)) = preparar() else { return };
        let dup = match Duplication::new(&dev, &mon) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pulando: duplicação indisponível ({e})");
                return;
            }
        };
        let (w, h) = dup.size();
        let normal = (w, h) == (mon.width, mon.height);
        let girado = (w, h) == (mon.height, mon.width);
        assert!(
            normal || girado,
            "tamanho {w}x{h} difere do monitor {}x{}",
            mon.width,
            mon.height
        );
    }

    #[test]
    fn acquire_retorna_frame_ou_timeout_em_500ms() {
        let Some((dev, mon)) = preparar() else { return };
        let Ok(mut dup) = Duplication::new(&dev, &mon) else {
            eprintln!("pulando: duplicação indisponível");
            return;
        };
        for _ in 0..5 {
            match dup.acquire(100) {
                Ok(Acquired::Timeout) => {}
                Ok(Acquired::Frame { .. }) => {
                    assert!(dup.release().is_ok());
                    return;
                }
                Ok(Acquired::Lost) => return,
                Err(e) => panic!("acquire falhou: {e}"),
            }
        }
        // Tela parada: só timeouts é aceitável.
    }
}
