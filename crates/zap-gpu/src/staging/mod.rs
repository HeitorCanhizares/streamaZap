//! Stagings persistentes de leitura e escrita (Windows), sem segurar o lock do dispositivo
//! enquanto a GPU copia. Ver `docs/planos/m2b-conversao-cpu.md` §5.1 b.
use crate::{GpuDevice, GpuError};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_MAP, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_FLAG_DO_NOT_WAIT,
};
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING;

mod escrita;
mod leitura;

pub use escrita::{MappedUpload, UploadStaging};
pub use leitura::{MappedFrame, ReadbackStaging};

/// Converte um erro do Windows no erro do crate.
pub(crate) fn os(context: &'static str, e: windows::core::Error) -> GpuError {
    GpuError::Os {
        context,
        message: e.message(),
    }
}

/// `Map` com `D3D11_MAP_FLAG_DO_NOT_WAIT` sem segurar o lock entre as tentativas.
///
/// Cada tentativa: `dev.lock()` (a espera é somada em `lock_wait_us`), `Map`, solta o lock. Com
/// `DXGI_ERROR_WAS_STILL_DRAWING`, espera sem o lock: `std::thread::yield_now()` enquanto a espera
/// total for menor que 2 ms, `std::thread::sleep(1 ms)` depois. Passou de 100 ms:
/// `GpuError::Os { context, message: "tempo esgotado (100 ms)" }`. Outro erro: `Os`.
/// Devolve o mapeamento e o número de tentativas. `context` é "Map (leitura)" ou "Map (escrita)".
pub(crate) fn map_sem_bloquear(
    dev: &GpuDevice,
    tex: &ID3D11Texture2D,
    modo: D3D11_MAP,
    context: &'static str,
    lock_wait_us: &mut f64,
) -> Result<(D3D11_MAPPED_SUBRESOURCE, u32), GpuError> {
    let inicio = Instant::now();
    let mut tentativas = 0u32;
    loop {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let (guard, espera) = dev.lock();
        *lock_wait_us += espera.as_secs_f64() * 1e6;
        tentativas += 1;
        // SAFETY: `mapped` é válido para escrita durante a chamada; o recurso é uma staging
        // com o acesso pedido em `modo`, e o lock do dispositivo está tomado.
        let r = unsafe {
            dev.context()
                .Map(tex, 0, modo, D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32, Some(&mut mapped))
        };
        drop(guard);
        match r {
            Ok(()) => return Ok((mapped, tentativas)),
            Err(e) if e.code() == DXGI_ERROR_WAS_STILL_DRAWING => {
                let decorrido = inicio.elapsed();
                if decorrido > Duration::from_millis(100) {
                    return Err(GpuError::Os {
                        context,
                        message: "tempo esgotado (100 ms)".to_string(),
                    });
                }
                // Espera sem o lock: gira rápido no começo, depois dorme para não queimar CPU.
                if decorrido < Duration::from_millis(2) {
                    std::thread::yield_now();
                } else {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Err(e) => return Err(os(context, e)),
        }
    }
}

/// `Unmap` do subrecurso 0 dentro do lock (a espera é somada em `lock_wait_us`).
pub(crate) fn unmap(dev: &GpuDevice, tex: &ID3D11Texture2D, lock_wait_us: &mut f64) {
    let (_guard, espera) = dev.lock();
    *lock_wait_us += espera.as_secs_f64() * 1e6;
    // SAFETY: só é chamado para um recurso mapeado antes por `map_sem_bloquear`; lock tomado.
    unsafe { dev.context().Unmap(tex, 0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_WRITE, D3D11_MAP_READ, D3D11_MAP_WRITE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    fn staging_escrita(dev: &GpuDevice) -> Result<ID3D11Texture2D, GpuError> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: 64,
            Height: 32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
        };
        let mut t: Option<ID3D11Texture2D> = None;
        // SAFETY: `desc` e `t` são válidos durante a chamada; o dispositivo é válido.
        unsafe { dev.d3d_device().CreateTexture2D(&desc, None, Some(&mut t)) }
            .map_err(|e| os("CreateTexture2D (staging)", e))?;
        t.ok_or_else(|| GpuError::Os {
            context: "CreateTexture2D (staging)",
            message: "textura nula".to_string(),
        })
    }

    fn dispositivo() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("PULANDO: sem dispositivo D3D11 ({e})");
                None
            }
        }
    }

    #[test]
    fn map_unmap_basico() {
        let Some(dev) = dispositivo() else { return };
        let tex = staging_escrita(&dev).expect("staging");
        let mut espera = 0.0;
        let (m, tentativas) = map_sem_bloquear(&dev, &tex, D3D11_MAP_WRITE, "Map (escrita)", &mut espera).expect("map");
        assert!(!m.pData.is_null());
        assert!(m.RowPitch >= 256);
        assert!(tentativas >= 1);
        unmap(&dev, &tex, &mut espera);
    }

    #[test]
    fn lock_wait_acumula() {
        let Some(dev) = dispositivo() else { return };
        let tex = staging_escrita(&dev).expect("staging");
        let mut espera = 0.0;
        map_sem_bloquear(&dev, &tex, D3D11_MAP_WRITE, "Map (escrita)", &mut espera).expect("map");
        unmap(&dev, &tex, &mut espera);
        assert!(espera.is_finite() && espera >= 0.0);
        let antes = espera;
        map_sem_bloquear(&dev, &tex, D3D11_MAP_WRITE, "Map (escrita)", &mut espera).expect("map");
        unmap(&dev, &tex, &mut espera);
        assert!(espera.is_finite() && espera >= antes);
    }

    #[test]
    fn erro_de_map_vira_os() {
        let Some(dev) = dispositivo() else { return };
        let tex = staging_escrita(&dev).expect("staging");
        let mut espera = 0.0;
        // Leitura numa staging só de escrita: o D3D11 deve recusar.
        match map_sem_bloquear(&dev, &tex, D3D11_MAP_READ, "Map (leitura)", &mut espera) {
            Err(GpuError::Os { context, .. }) => assert_eq!(context, "Map (leitura)"),
            Err(e) => panic!("erro inesperado: {e}"),
            Ok(_) => {
                eprintln!("PULANDO: este driver aceitou Map de leitura");
                unmap(&dev, &tex, &mut espera);
            }
        }
    }
}
