//! Sonda das placas de vídeo para o Diagnóstico: o nome de cada placa que importa (a padrão e as
//! que dirigem monitores) e se o Video Processor converte BGRA para NV12 nela. Sem ele, o pipeline
//! converte pela CPU (`ConversionSpec::Auto`, plano `docs/planos/m2b-conversao-cpu.md`).

use crate::VideoAdapter;

/// Placas de vídeo relevantes, sem repetição, com o Video Processor testado de verdade: cria o
/// mesmo conversor que a camada 1 de uma tela 1080p usaria (1920x1080 BGRA para 1280x720 NV12).
///
/// **Bloqueante** (cria dispositivos D3D11, dezenas de ms por placa): chame fora da thread da
/// interface. Fora do Windows devolve uma lista vazia.
pub fn probe_video_adapters() -> Vec<VideoAdapter> {
    #[cfg(windows)]
    {
        win::sondar()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
mod win {
    use crate::VideoAdapter;
    use zap_gpu::{Converter, GpuDevice, PixelFormat};

    pub(super) fn sondar() -> Vec<VideoAdapter> {
        // A padrão primeiro (janelas, câmera e a transmissão sintética usam ela), depois as que
        // dirigem monitores (a captura de tela usa a placa do monitor).
        let mut dispositivos: Vec<GpuDevice> = Vec::new();
        match GpuDevice::create_default() {
            Ok(d) => dispositivos.push(d),
            Err(e) => tracing::debug!("sonda de vídeo: sem dispositivo padrão ({e})"),
        }
        let monitores = zap_capture::list_monitors().unwrap_or_default();
        for m in &monitores {
            if dispositivos.iter().any(|d| d.adapter().luid == m.adapter_luid) {
                continue;
            }
            match GpuDevice::create_for_adapter(m.adapter_luid) {
                Ok(d) => dispositivos.push(d),
                Err(e) => tracing::debug!("sonda de vídeo: placa do monitor {} ({e})", m.index),
            }
        }
        let mut out: Vec<VideoAdapter> = Vec::new();
        let mut vistos: Vec<i64> = Vec::new();
        for dev in &dispositivos {
            let info = dev.adapter();
            if vistos.contains(&info.luid) {
                continue;
            }
            vistos.push(info.luid);
            let video_processor =
                dev.has_video() && Converter::new(dev, 1920, 1080, PixelFormat::Bgra8, 1280, 720).is_ok();
            out.push(VideoAdapter {
                name: info.name.clone(),
                video_processor,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonda_ve_a_placa_quando_ha_dispositivo() {
        let v = probe_video_adapters();
        eprintln!("placas: {v:?}");
        assert!(v.iter().all(|a| !a.name.is_empty()), "placa sem nome: {v:?}");
        #[cfg(windows)]
        if zap_gpu::GpuDevice::create_default().is_ok() {
            assert!(!v.is_empty(), "há dispositivo D3D11, mas a sonda não viu placa nenhuma");
        }
        #[cfg(not(windows))]
        assert!(v.is_empty());
    }
}
