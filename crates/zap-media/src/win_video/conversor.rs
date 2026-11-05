//! Conversor de uma camada: Video Processor (GPU) ou `CpuScaler` + `UploadStaging` (CPU).
use super::Contexto;
use crate::logic::{escolher_conversor, Escolhido};
use crate::{ConversionSpec, ConverterKind, MediaError};
use zap_gpu::{Converter, CpuScaler, GpuDevice, GpuError, PixelFormat, UploadStaging};
use zap_proto::LayerSpec;

pub(super) enum Conversor {
    Gpu(Converter),
    Cpu(CamadaCpu),
}

/// Parte da camada que converte na CPU: pesos prontos e as 2 stagings NV12 de escrita.
pub(super) struct CamadaCpu {
    pub(super) scaler: CpuScaler,
    pub(super) upload: UploadStaging,
}

/// Cria o conversor da camada (`spec` dá a camada e o tamanho de saída; `entrada` = largura, altura
/// e formato da origem). Usado por `criar_pipe` e pelo ramo "entrada mudou" de `processar`.
pub(super) fn criar_conversor(
    dev: &GpuDevice,
    ctx: &Contexto,
    forcar_cpu: bool,
    spec: &LayerSpec,
    entrada: (u32, u32, PixelFormat),
) -> Result<Conversor, MediaError> {
    let (in_w, in_h, in_fmt) = entrada;
    let (out_w, out_h) = (u32::from(spec.width), u32::from(spec.height));
    let modo = if forcar_cpu {
        ConversionSpec::Cpu
    } else {
        ctx.opts.conversion
    };
    let escolhido = escolher_conversor(modo, || {
        if ctx.simular_sem_vp {
            Err(GpuError::Unsupported)
        } else {
            Converter::new(dev, in_w, in_h, in_fmt, out_w, out_h)
        }
    })?;
    match escolhido {
        Escolhido::Gpu(mut conv) => {
            // Medição de GPU é opcional: sem ela a camada funciona igual.
            if let Err(e) = conv.enable_gpu_timing(dev) {
                tracing::warn!("sem medição de GPU na conversão: {e}");
            }
            Ok(Conversor::Gpu(conv))
        }
        Escolhido::Cpu { motivo } => {
            let s = dev.format_support(PixelFormat::Nv12);
            if !(s.texture2d && s.cpu_lockable) {
                tracing::warn!(
                    "camada {}: o adaptador não tem textura NV12; sem conversão possível",
                    spec.layer
                );
                return Err(MediaError::Gpu(GpuError::Unsupported));
            }
            let scaler = CpuScaler::new(in_w, in_h, in_fmt, out_w, out_h)?;
            let upload = UploadStaging::new(dev, out_w, out_h)?;
            tracing::info!(
                "camada {}: conversão pela CPU ({}), {} threads",
                spec.layer,
                motivo.as_deref().unwrap_or("pedida"),
                ctx.threads
            );
            Ok(Conversor::Cpu(CamadaCpu { scaler, upload }))
        }
    }
}

impl Conversor {
    pub(super) fn kind(&self) -> ConverterKind {
        match self {
            Conversor::Gpu(_) => ConverterKind::Gpu,
            Conversor::Cpu(_) => ConverterKind::Cpu,
        }
    }
}
