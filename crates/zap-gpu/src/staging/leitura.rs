//! Staging de leitura persistente.
use super::{esperar_evento, map_sem_bloquear, os, unmap};
use crate::cpu::SrcImage;
use crate::{GpuDevice, GpuError, PixelFormat, ReadbackTiming, Texture};
use std::time::Instant;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Query, ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_QUERY_EVENT, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC,
};

fn dxgi_de(f: PixelFormat) -> DXGI_FORMAT {
    match f {
        PixelFormat::Bgra8 => DXGI_FORMAT_B8G8R8A8_UNORM,
        PixelFormat::Nv12 => DXGI_FORMAT_NV12,
    }
}

/// Staging de leitura persistente (`D3D11_USAGE_STAGING`, `CPU_ACCESS_READ`) de um tamanho e formato.
pub struct ReadbackStaging {
    pub(crate) tex: ID3D11Texture2D,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: PixelFormat,
    /// Consulta de evento posta antes da cópia: separa a espera pelo quadro (trabalho anterior da
    /// GPU) da cópia em si. `None` se o driver não criar a consulta (aí `frame_wait_us` = 0).
    pub(crate) evento: Option<ID3D11Query>,
}

impl ReadbackStaging {
    pub fn new(dev: &GpuDevice, width: u32, height: u32, format: PixelFormat) -> Result<Self, GpuError> {
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidArgument(format!("tamanho inválido {width}x{height}")));
        }
        if format == PixelFormat::Nv12 && (width % 2 != 0 || height % 2 != 0) {
            return Err(GpuError::InvalidArgument(format!(
                "NV12 exige tamanho par, recebido {width}x{height}"
            )));
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: dxgi_de(format),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut t: Option<ID3D11Texture2D> = None;
        // SAFETY: `desc` e `t` são válidos durante a chamada; o dispositivo é válido.
        unsafe { dev.d3d_device().CreateTexture2D(&desc, None, Some(&mut t)) }
            .map_err(|e| os("CreateTexture2D (staging)", e))?;
        let tex = t.ok_or_else(|| GpuError::Os {
            context: "CreateTexture2D (staging)",
            message: "textura nula após sucesso".to_string(),
        })?;
        let evento = match crate::timing::criar_consulta(dev, D3D11_QUERY_EVENT) {
            Ok(q) => Some(q),
            Err(e) => {
                tracing::debug!("sem consulta de evento na leitura: {e}");
                None
            }
        };
        Ok(Self {
            tex,
            width,
            height,
            format,
            evento,
        })
    }

    pub fn matches(&self, width: u32, height: u32, format: PixelFormat) -> bool {
        self.width == width && self.height == height && self.format == format
    }

    /// Copia `src` (mesmo tamanho e formato; senão `InvalidArgument`) e mapeia para leitura.
    /// Dentro do lock: consulta de evento e `Flush` (fecha o lote do trabalho anterior), depois
    /// `CopyResource` e `Flush`. Fora dele: espera o evento (`frame_wait_us`) e então
    /// `map_sem_bloquear` (`D3D11_MAP_READ`), que volta a girar rápido e pega o fim da cópia sem atraso.
    pub fn map<'s>(
        &'s mut self,
        dev: &'s GpuDevice,
        src: &Texture,
    ) -> Result<(MappedFrame<'s>, ReadbackTiming), GpuError> {
        if !self.matches(src.width(), src.height(), src.format()) {
            return Err(GpuError::InvalidArgument(format!(
                "textura {}x{} {:?} diferente da staging {}x{} {:?}",
                src.width(),
                src.height(),
                src.format(),
                self.width,
                self.height,
                self.format
            )));
        }
        let mut lock_wait = 0.0f64;
        let t0;
        {
            let (_guard, espera) = dev.lock();
            lock_wait += espera.as_secs_f64() * 1e6;
            t0 = Instant::now();
            // SAFETY: ambos os recursos são do mesmo dispositivo, com formato/tamanho iguais;
            // o lock do dispositivo está tomado.
            unsafe {
                if let Some(q) = &self.evento {
                    dev.context().End(q);
                    dev.context().Flush();
                }
                dev.context().CopyResource(&self.tex, src.raw());
                dev.context().Flush();
            }
        }
        let frame_wait_us = match &self.evento {
            Some(q) => esperar_evento(dev, q, t0, &mut lock_wait).map_or(0.0, |d| d.as_secs_f64() * 1e6),
            None => 0.0,
        };
        let (mapped, tentativas) = map_sem_bloquear(dev, &self.tex, D3D11_MAP_READ, "Map (leitura)", &mut lock_wait)?;
        let copy_wait_us = t0.elapsed().as_secs_f64() * 1e6;
        let frame = MappedFrame {
            dev,
            staging: &self.tex,
            ptr: mapped.pData as *const u8,
            pitch: mapped.RowPitch as usize,
            width: self.width,
            height: self.height,
            format: self.format,
        };
        Ok((
            frame,
            ReadbackTiming {
                copy_wait_us,
                frame_wait_us: frame_wait_us.min(copy_wait_us),
                lock_wait_us: lock_wait,
                map_attempts: tentativas,
            },
        ))
    }
}

/// Leitura mapeada; `Unmap` no `Drop`. Não é `Send` (guarda COM e ponteiro cru).
pub struct MappedFrame<'s> {
    pub(crate) dev: &'s GpuDevice,
    pub(crate) staging: &'s ID3D11Texture2D,
    pub(crate) ptr: *const u8,
    pub(crate) pitch: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: PixelFormat,
}

impl MappedFrame<'_> {
    /// Bgra: `stride = RowPitch`. Nv12: Y e UV com `RowPitch`, UV em `RowPitch * height`.
    /// Fatias: Bgra `pitch*(h-1) + w*4` bytes; Nv12 `pitch*(h + h/2 - 1) + w` bytes.
    pub fn image(&self) -> SrcImage<'_> {
        let w = self.width as usize;
        let h = self.height as usize;
        let pitch = self.pitch;
        match self.format {
            PixelFormat::Bgra8 => {
                // SAFETY: o D3D11 mapeia o subrecurso inteiro com `RowPitch` bytes por linha
                // (mesmo cálculo de `read_back`); a fatia vive enquanto `&self` vive.
                let data = unsafe { std::slice::from_raw_parts(self.ptr, pitch * (h - 1) + w * 4) };
                SrcImage::Bgra {
                    data,
                    stride: pitch,
                    width: self.width,
                    height: self.height,
                }
            }
            PixelFormat::Nv12 => {
                // SAFETY: o mapeamento cobre `h` linhas de Y mais `h/2` de UV, cada uma com `pitch`
                // bytes; a fatia vive enquanto `&self` vive.
                let all = unsafe { std::slice::from_raw_parts(self.ptr, pitch * (h + h / 2 - 1) + w) };
                SrcImage::Nv12 {
                    y: &all[..pitch * (h - 1) + w],
                    y_stride: pitch,
                    uv: &all[pitch * h..],
                    uv_stride: pitch,
                    width: self.width,
                    height: self.height,
                }
            }
        }
    }
}

impl Drop for MappedFrame<'_> {
    fn drop(&mut self) {
        unmap(self.dev, self.staging, &mut 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_back, upload};

    fn dispositivo() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("sem GPU, pulando: {e}");
                None
            }
        }
    }

    fn padrao(n: usize, semente: usize) -> Vec<u8> {
        (0..n).map(|i| ((i * 7 + semente * 13) % 251) as u8).collect()
    }

    /// Junta `n` linhas úteis (largura `bytes`) de `data` com `stride`.
    fn linhas(data: &[u8], stride: usize, bytes: usize, n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for r in 0..n {
            v.extend_from_slice(&data[r * stride..r * stride + bytes]);
        }
        v
    }

    #[test]
    fn readback_bgra_igual_read_back() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        upload(&dev, &t, &padrao(64 * 32 * 4, 1), 64 * 4).unwrap();
        let esperado = read_back(&dev, &t).unwrap();
        let mut st = ReadbackStaging::new(&dev, 64, 32, PixelFormat::Bgra8).unwrap();
        let (m, timing) = st.map(&dev, &t).unwrap();
        match m.image() {
            SrcImage::Bgra { data, stride, .. } => assert_eq!(linhas(data, stride, 64 * 4, 32), esperado),
            _ => panic!("esperava Bgra"),
        }
        assert!(timing.map_attempts >= 1);
        assert!(timing.copy_wait_us > 0.0);
        assert!(
            timing.frame_wait_us >= 0.0 && timing.frame_wait_us <= timing.copy_wait_us,
            "frame_wait_us = {}, copy_wait_us = {}",
            timing.frame_wait_us,
            timing.copy_wait_us
        );
    }

    #[test]
    fn readback_nv12() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Nv12, false).unwrap();
        let dados = padrao(64 * 32 * 3 / 2, 2);
        if let Err(e) = upload(&dev, &t, &dados, 64) {
            eprintln!("driver não aceita upload NV12, pulando: {e}");
            return;
        }
        let Ok(esperado) = read_back(&dev, &t) else {
            eprintln!("driver não aceita read_back NV12, pulando");
            return;
        };
        let mut st = ReadbackStaging::new(&dev, 64, 32, PixelFormat::Nv12).unwrap();
        let (m, _) = st.map(&dev, &t).unwrap();
        match m.image() {
            SrcImage::Nv12 {
                y,
                y_stride,
                uv,
                uv_stride,
                ..
            } => {
                let mut got = linhas(y, y_stride, 64, 32);
                got.extend(linhas(uv, uv_stride, 64, 16));
                assert_eq!(got, esperado);
            }
            _ => panic!("esperava Nv12"),
        }
    }

    #[test]
    fn segunda_leitura_ve_conteudo_novo() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        let mut st = ReadbackStaging::new(&dev, 64, 32, PixelFormat::Bgra8).unwrap();
        let a = padrao(64 * 32 * 4, 3);
        let b = padrao(64 * 32 * 4, 4);
        upload(&dev, &t, &a, 64 * 4).unwrap();
        {
            let (m, _) = st.map(&dev, &t).unwrap();
            let SrcImage::Bgra { data, stride, .. } = m.image() else {
                panic!("esperava Bgra")
            };
            assert_eq!(linhas(data, stride, 256, 32), a);
        }
        upload(&dev, &t, &b, 64 * 4).unwrap();
        let (m, _) = st.map(&dev, &t).unwrap();
        let SrcImage::Bgra { data, stride, .. } = m.image() else {
            panic!("esperava Bgra")
        };
        assert_eq!(linhas(data, stride, 256, 32), b);
    }

    #[test]
    fn matches_e_tamanho_errado() {
        let Some(dev) = dispositivo() else { return };
        let mut st = ReadbackStaging::new(&dev, 64, 32, PixelFormat::Bgra8).unwrap();
        assert!(st.matches(64, 32, PixelFormat::Bgra8));
        assert!(!st.matches(32, 32, PixelFormat::Bgra8));
        assert!(!st.matches(64, 32, PixelFormat::Nv12));
        let outra = Texture::new(&dev, 32, 32, PixelFormat::Bgra8, false).unwrap();
        assert!(matches!(st.map(&dev, &outra), Err(GpuError::InvalidArgument(_))));
        assert!(matches!(
            ReadbackStaging::new(&dev, 0, 32, PixelFormat::Bgra8),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            ReadbackStaging::new(&dev, 32, 0, PixelFormat::Bgra8),
            Err(GpuError::InvalidArgument(_))
        ));
        assert!(matches!(
            ReadbackStaging::new(&dev, 63, 32, PixelFormat::Nv12),
            Err(GpuError::InvalidArgument(_))
        ));
    }
}
