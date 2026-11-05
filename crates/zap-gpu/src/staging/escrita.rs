//! Duas stagings NV12 de escrita usadas em rodízio.
use super::{map_sem_bloquear, os, unmap};
use crate::cpu::Nv12Mut;
use crate::{GpuDevice, GpuError, PixelFormat, Texture, UploadTiming};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_CPU_ACCESS_WRITE, D3D11_MAP_WRITE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};

/// Duas stagings NV12 de escrita (`CPU_ACCESS_WRITE`) usadas em rodízio.
pub struct UploadStaging {
    pub(crate) tex: [ID3D11Texture2D; 2],
    pub(crate) next: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl UploadStaging {
    /// Tamanho par (senão `InvalidArgument`).
    pub fn new(dev: &GpuDevice, width: u32, height: u32) -> Result<Self, GpuError> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(GpuError::InvalidArgument(format!(
                "NV12 exige largura e altura pares e não nulas, veio {width}x{height}"
            )));
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
        };
        let criar = || -> Result<ID3D11Texture2D, GpuError> {
            let mut t: Option<ID3D11Texture2D> = None;
            // SAFETY: `desc` e `t` são válidos durante a chamada; o dispositivo é válido.
            unsafe { dev.d3d_device().CreateTexture2D(&desc, None, Some(&mut t)) }
                .map_err(|e| os("CreateTexture2D (staging)", e))?;
            t.ok_or_else(|| GpuError::Os {
                context: "CreateTexture2D (staging)",
                message: "textura nula após sucesso".to_string(),
            })
        };
        let tex = [criar()?, criar()?];
        Ok(Self {
            tex,
            next: 0,
            width,
            height,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Mapeia para escrita a próxima staging do rodízio (`map_sem_bloquear` com `D3D11_MAP_WRITE`).
    pub fn map<'s>(&'s mut self, dev: &'s GpuDevice) -> Result<MappedUpload<'s>, GpuError> {
        let idx = self.next;
        self.next ^= 1;
        let mut lock_wait = 0.0f64;
        let (mapped, tentativas) =
            map_sem_bloquear(dev, &self.tex[idx], D3D11_MAP_WRITE, "Map (escrita)", &mut lock_wait)?;
        Ok(MappedUpload {
            dev,
            staging: &self.tex[idx],
            ptr: mapped.pData as *mut u8,
            pitch: mapped.RowPitch as usize,
            width: self.width,
            height: self.height,
            mapeado: true,
            lock_wait_us: lock_wait,
            map_attempts: tentativas,
        })
    }
}

/// Escrita mapeada; `Unmap` no `Drop` se não houve `commit`. Não é `Send`.
pub struct MappedUpload<'s> {
    pub(crate) dev: &'s GpuDevice,
    pub(crate) staging: &'s ID3D11Texture2D,
    pub(crate) ptr: *mut u8,
    pub(crate) pitch: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// `true` enquanto o subrecurso está mapeado.
    pub(crate) mapeado: bool,
    pub(crate) lock_wait_us: f64,
    pub(crate) map_attempts: u32,
}

impl MappedUpload<'_> {
    /// Destino para o conversor: Y em `[0, RowPitch*h)`, UV logo depois (`split_at_mut`).
    pub fn nv12(&mut self) -> Nv12Mut<'_> {
        let w = self.width as usize;
        let h = self.height as usize;
        let pitch = self.pitch;
        // SAFETY: o D3D11 mapeia o subrecurso com `RowPitch` bytes por linha em h linhas de Y e
        // h/2 de UV; a fatia vive no empréstimo de `&mut self`, e o mapeamento só termina no
        // `commit`/`Drop`, que consomem o valor.
        let all = unsafe { std::slice::from_raw_parts_mut(self.ptr, pitch * (h + h / 2 - 1) + w) };
        let (a, uv) = all.split_at_mut(pitch * h);
        Nv12Mut {
            y: &mut a[..pitch * (h - 1) + w],
            y_stride: pitch,
            uv,
            uv_stride: pitch,
            width: self.width,
            height: self.height,
        }
    }

    /// `Unmap` + `CopyResource(dst, staging)`. `dst` deve ser NV12 do mesmo tamanho; se não for,
    /// faz o `Unmap` e devolve `InvalidArgument`.
    pub fn commit(mut self, dst: &Texture) -> Result<UploadTiming, GpuError> {
        // Desmapeia uma única vez em qualquer caminho; o `Drop` que roda no fim vê `mapeado=false`.
        unmap(self.dev, self.staging, &mut self.lock_wait_us);
        self.mapeado = false;
        if dst.format() != PixelFormat::Nv12 || dst.width() != self.width || dst.height() != self.height {
            return Err(GpuError::InvalidArgument(format!(
                "destino {}x{} {:?} diferente da staging NV12 {}x{}",
                dst.width(),
                dst.height(),
                dst.format(),
                self.width,
                self.height
            )));
        }
        {
            let (_guard, espera) = self.dev.lock();
            self.lock_wait_us += espera.as_secs_f64() * 1e6;
            // SAFETY: ambos os recursos são do mesmo dispositivo, NV12 do mesmo tamanho; a
            // staging já foi desmapeada e o lock do dispositivo está tomado.
            unsafe { self.dev.context().CopyResource(dst.raw(), self.staging) };
        }
        Ok(UploadTiming {
            lock_wait_us: self.lock_wait_us,
            map_attempts: self.map_attempts,
        })
    }
}

impl Drop for MappedUpload<'_> {
    fn drop(&mut self) {
        if self.mapeado {
            unmap(self.dev, self.staging, &mut 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_back;

    fn dispositivo() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("PULANDO: sem GPU ({e})");
                None
            }
        }
    }

    /// Cria a staging; `None` se o driver não aceita NV12 com `CPU_ACCESS_WRITE`.
    fn staging(dev: &GpuDevice, w: u32, h: u32) -> Option<UploadStaging> {
        match UploadStaging::new(dev, w, h) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("PULANDO: driver não aceita staging NV12 de escrita ({e})");
                None
            }
        }
    }

    /// Escreve um padrão determinístico e devolve o quadro esperado sem padding (Y seguido de UV).
    fn escrever(m: &mut MappedUpload<'_>, semente: usize) -> Vec<u8> {
        let n = m.nv12();
        let (w, h) = (n.width as usize, n.height as usize);
        let mut esperado = Vec::new();
        for r in 0..h {
            for c in 0..w {
                let v = ((r * 31 + c * 7 + semente * 13) % 251) as u8;
                n.y[r * n.y_stride + c] = v;
                esperado.push(v);
            }
        }
        for r in 0..h / 2 {
            for c in 0..w {
                let v = ((r * 17 + c * 5 + semente * 29 + 3) % 253) as u8;
                n.uv[r * n.uv_stride + c] = v;
                esperado.push(v);
            }
        }
        esperado
    }

    #[test]
    fn upload_staging_roundtrip() {
        let Some(dev) = dispositivo() else { return };
        let Some(mut st) = staging(&dev, 64, 32) else { return };
        let dst = Texture::new(&dev, 64, 32, PixelFormat::Nv12, false).expect("textura");
        let mut m = st.map(&dev).expect("map");
        let esperado = escrever(&mut m, 1);
        let t = m.commit(&dst).expect("commit");
        assert!(t.map_attempts >= 1);
        assert!(t.lock_wait_us.is_finite() && t.lock_wait_us >= 0.0);
        match read_back(&dev, &dst) {
            Ok(got) => assert_eq!(got, esperado),
            Err(e) => eprintln!("PULANDO: read_back NV12 não suportado ({e})"),
        }
    }

    #[test]
    fn alterna_duas_stagings() {
        let Some(dev) = dispositivo() else { return };
        let Some(mut st) = staging(&dev, 64, 32) else { return };
        let dst = Texture::new(&dev, 64, 32, PixelFormat::Nv12, false).expect("textura");
        for i in 0..10 {
            let mut m = st.map(&dev).expect("map");
            let esperado = escrever(&mut m, i);
            m.commit(&dst).expect("commit");
            match read_back(&dev, &dst) {
                Ok(got) => assert_eq!(got, esperado, "iteração {i}"),
                Err(e) => {
                    eprintln!("PULANDO: read_back NV12 não suportado ({e})");
                    return;
                }
            }
        }
        assert_eq!(st.next, 0);
    }

    #[test]
    fn commit_destino_errado() {
        let Some(dev) = dispositivo() else { return };
        let Some(mut st) = staging(&dev, 64, 32) else { return };
        let bgra = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).expect("bgra");
        let outro = Texture::new(&dev, 32, 32, PixelFormat::Nv12, false).expect("nv12");
        for dst in [&bgra, &outro] {
            let m = st.map(&dev).expect("map");
            assert!(matches!(m.commit(dst), Err(GpuError::InvalidArgument(_))));
        }
        // Nenhuma ficou mapeada: as duas podem ser mapeadas de novo.
        let a = st.map(&dev).expect("map a");
        drop(a);
        let b = st.map(&dev).expect("map b");
        drop(b);
    }

    #[test]
    fn drop_sem_commit_desmapeia() {
        let Some(dev) = dispositivo() else { return };
        let Some(mut st) = staging(&dev, 64, 32) else { return };
        for _ in 0..4 {
            let m = st.map(&dev).expect("map");
            drop(m);
        }
        let a = st.map(&dev).expect("map 1");
        drop(a);
        // Mapeia as duas do rodízio em sequência.
        let b = st.map(&dev).expect("map 2");
        drop(b);
    }

    #[test]
    fn new_invalido() {
        let Some(dev) = dispositivo() else { return };
        for (w, h) in [(0, 32), (64, 0), (63, 32), (64, 31)] {
            assert!(matches!(
                UploadStaging::new(&dev, w, h),
                Err(GpuError::InvalidArgument(_))
            ));
        }
        if let Some(st) = staging(&dev, 64, 32) {
            assert_eq!(st.size(), (64, 32));
        }
    }
}
