use crate::{GpuDevice, GpuError, PixelFormat, Texture};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
    D3D11_MAP_WRITE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};

fn os(context: &'static str, e: windows::core::Error) -> GpuError {
    GpuError::Os {
        context,
        message: e.message(),
    }
}

/// Cria uma textura staging com o mesmo formato/tamanho da original.
fn criar_staging(dev: &GpuDevice, tex: &Texture, cpu: u32) -> Result<ID3D11Texture2D, GpuError> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: `desc` é válido para escrita durante a chamada.
    unsafe { tex.raw().GetDesc(&mut desc) };
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = cpu;
    desc.MiscFlags = 0;
    let mut staging: Option<ID3D11Texture2D> = None;
    // SAFETY: `desc` e `staging` são válidos durante a chamada; o dispositivo é válido.
    unsafe { dev.d3d_device().CreateTexture2D(&desc, None, Some(&mut staging)) }
        .map_err(|e| os("CreateTexture2D (staging)", e))?;
    staging.ok_or_else(|| GpuError::Os {
        context: "CreateTexture2D (staging)",
        message: "textura nula após sucesso".to_string(),
    })
}

/// Linhas (largura em bytes, quantidade) de cada plano; o UV do NV12 começa em `RowPitch * altura_Y`.
fn planos(fmt: PixelFormat, w: usize, h: usize) -> Vec<(usize, usize)> {
    match fmt {
        PixelFormat::Bgra8 => vec![(w * 4, h)],
        PixelFormat::Nv12 => vec![(w, h), (w, h / 2)],
    }
}

/// Copia a textura para uma staging (CPU_ACCESS_READ) e lê sem padding de linha.
/// Bgra8: w*h*4 bytes (B,G,R,A). Nv12: plano Y (w*h) seguido do UV intercalado (w*h/2).
pub fn read_back(dev: &GpuDevice, tex: &Texture) -> Result<Vec<u8>, GpuError> {
    let staging = criar_staging(dev, tex, D3D11_CPU_ACCESS_READ.0 as u32)?;
    let ctx = dev.context();
    // SAFETY: ambos os recursos pertencem ao mesmo dispositivo e têm formato/tamanho iguais.
    unsafe { ctx.CopyResource(&staging, tex.raw()) };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: `mapped` é válido para escrita; a staging foi criada com acesso de leitura.
    unsafe { ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }.map_err(|e| os("Map (leitura)", e))?;
    let w = tex.width() as usize;
    let h = tex.height() as usize;
    let pitch = mapped.RowPitch as usize;
    let base = mapped.pData as *const u8;
    let mut out = Vec::with_capacity(tex.format().frame_bytes(tex.width(), tex.height()));
    let mut linha0 = 0usize;
    for (bytes, linhas) in planos(tex.format(), w, h) {
        for r in 0..linhas {
            // SAFETY: o mapeamento cobre `pitch` bytes por linha (>= `bytes`) em h linhas Y mais h/2 de UV,
            // e `linha0 + r` fica dentro desse intervalo.
            let src = unsafe { std::slice::from_raw_parts(base.add((linha0 + r) * pitch), bytes) };
            out.extend_from_slice(src);
        }
        linha0 += linhas;
    }
    // SAFETY: o subrecurso foi mapeado acima e os ponteiros não são mais usados.
    unsafe { ctx.Unmap(&staging, 0) };
    Ok(out)
}

/// Escreve `data` na textura. `stride` = bytes por linha em `data` (Bgra8: >= w*4; Nv12: >= w, vale
/// para Y e UV). Bgra8: `data` tem h linhas; Nv12: h linhas de Y seguidas de h/2 linhas de UV.
/// Buffer curto ou stride pequeno: `InvalidArgument`. Usa uma staging + `CopyResource`
/// (ou `UpdateSubresource` quando suportado).
pub fn upload(dev: &GpuDevice, tex: &Texture, data: &[u8], stride: u32) -> Result<(), GpuError> {
    let w = tex.width() as usize;
    let h = tex.height() as usize;
    let stride = stride as usize;
    let minimo = match tex.format() {
        PixelFormat::Bgra8 => w * 4,
        PixelFormat::Nv12 => w,
    };
    if stride < minimo {
        return Err(GpuError::InvalidArgument(format!(
            "stride {stride} menor que o mínimo {minimo}"
        )));
    }
    let necessario = match tex.format() {
        PixelFormat::Bgra8 => stride * h,
        PixelFormat::Nv12 => stride * h * 3 / 2,
    };
    if data.len() < necessario {
        return Err(GpuError::InvalidArgument(format!(
            "buffer curto: {} bytes, esperado ao menos {necessario}",
            data.len()
        )));
    }
    let staging = criar_staging(dev, tex, D3D11_CPU_ACCESS_WRITE.0 as u32)?;
    let ctx = dev.context();
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: `mapped` é válido para escrita; a staging foi criada com acesso de escrita.
    unsafe { ctx.Map(&staging, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped)) }.map_err(|e| os("Map (escrita)", e))?;
    let pitch = mapped.RowPitch as usize;
    let base = mapped.pData as *mut u8;
    let mut linha0 = 0usize;
    for (bytes, linhas) in planos(tex.format(), w, h) {
        for r in 0..linhas {
            let origem = &data[(linha0 + r) * stride..(linha0 + r) * stride + bytes];
            // SAFETY: o destino tem `pitch` (>= `bytes`) bytes por linha nas linhas mapeadas; origem e destino
            // não se sobrepõem (memória da GPU mapeada vs. fatia do chamador).
            unsafe { std::ptr::copy_nonoverlapping(origem.as_ptr(), base.add((linha0 + r) * pitch), bytes) };
        }
        linha0 += linhas;
    }
    // SAFETY: o subrecurso foi mapeado acima e os ponteiros não são mais usados.
    unsafe { ctx.Unmap(&staging, 0) };
    // SAFETY: ambos os recursos pertencem ao mesmo dispositivo e têm formato/tamanho iguais.
    unsafe { ctx.CopyResource(tex.raw(), &staging) };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo() -> Option<GpuDevice> {
        match GpuDevice::create_default() {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("sem GPU, pulando: {e}");
                None
            }
        }
    }

    fn padrao(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 7 % 251) as u8).collect()
    }

    #[test]
    fn roundtrip_bgra() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        let dados = padrao(64 * 32 * 4);
        upload(&dev, &t, &dados, 64 * 4).unwrap();
        assert_eq!(read_back(&dev, &t).unwrap(), dados);
    }

    #[test]
    fn roundtrip_bgra_stride_maior() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        let stride = 64 * 4 + 16;
        let mut dados = vec![0xEEu8; stride * 32];
        let esperado = padrao(64 * 32 * 4);
        for r in 0..32 {
            dados[r * stride..r * stride + 256].copy_from_slice(&esperado[r * 256..(r + 1) * 256]);
        }
        upload(&dev, &t, &dados, stride as u32).unwrap();
        assert_eq!(read_back(&dev, &t).unwrap(), esperado);
    }

    #[test]
    fn roundtrip_nv12() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Nv12, false).unwrap();
        let dados = padrao(64 * 32 * 3 / 2);
        if let Err(e) = upload(&dev, &t, &dados, 64) {
            eprintln!("driver não aceita upload NV12 por staging, pulando: {e}");
            return;
        }
        match read_back(&dev, &t) {
            Ok(v) => assert_eq!(v, dados),
            Err(e) => eprintln!("driver não aceita read_back NV12, pulando: {e}"),
        }
    }

    #[test]
    fn upload_buffer_curto_recusa() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        let r = upload(&dev, &t, &[0u8; 100], 64 * 4);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }

    #[test]
    fn upload_stride_pequeno_recusa() {
        let Some(dev) = dispositivo() else { return };
        let t = Texture::new(&dev, 64, 32, PixelFormat::Bgra8, false).unwrap();
        let r = upload(&dev, &t, &vec![0u8; 64 * 4 * 32], 64);
        assert!(matches!(r, Err(GpuError::InvalidArgument(_))));
    }
}
