//! Fase da CPU de `processar`: uma leitura da origem por quadro, conversão em 2 fases
//! (a camada da CPU de menor índice sozinha, depois as outras num lote) e entrega ao codificador.
use super::conversor::{CamadaCpu, Conversor};
use super::{clonar, Camada, Contexto, Pipe};
use crate::logic::Samples;
use crate::video::Shared;
use std::time::Instant;
use zap_capture::CapturedFrame;
use zap_encode::H264Encoder;
use zap_gpu::{convert_batch, BatchOptions, CpuScaler, GpuDevice, MappedUpload, ReadbackStaging, TextureRing};

/// Intervalo mínimo entre avisos de falha de leitura (1 s, em µs).
const AVISO_MIN_US: u64 = 1_000_000;

/// O aviso de leitura sai no máximo 1 vez por segundo (o primeiro sempre sai).
fn deve_avisar(agora_us: u64, ultimo_us: u64) -> bool {
    agora_us.saturating_sub(ultimo_us) >= AVISO_MIN_US
}

/// Loga `msg` como aviso se o limite de 1 por segundo deixar.
fn avisar_leitura(ultimo: &mut u64, msg: &str) {
    let agora = zap_gpu::now_us();
    if deve_avisar(agora, *ultimo) {
        *ultimo = agora;
        tracing::warn!("{msg}");
    }
}

/// Uma camada no meio da conversão: empréstimos disjuntos da `Camada`.
struct Item<'a> {
    layer: u8,
    scaler: &'a CpuScaler,
    mapeado: Option<MappedUpload<'a>>,
    ring: &'a mut TextureRing,
    enc: &'a mut H264Encoder,
    primeiro: &'a mut bool,
    convert: &'a mut Samples,
    lock_wait: &'a mut Samples,
}

/// Converte e entrega `frame` às camadas `cpu` (devidas neste quadro, com `Conversor::Cpu`, em
/// ordem de camada). `cap_us` é o instante de captura que vai ao codificador.
pub(super) fn rodar(
    cpu: &mut [&mut Camada],
    dev: &GpuDevice,
    shared: &Shared,
    frame: &CapturedFrame,
    cap_us: u64,
    ctx: &mut Contexto,
) {
    if cpu.is_empty() {
        return;
    }
    let t0 = Instant::now();
    let (fmt, w, h) = (frame.texture.format(), frame.texture.width(), frame.texture.height());
    if ctx.leitura.as_ref().is_none_or(|l| !l.matches(w, h, fmt)) {
        match ReadbackStaging::new(dev, w, h, fmt) {
            Ok(l) => ctx.leitura = Some(l),
            Err(e) => {
                ctx.leitura = None;
                avisar_leitura(
                    &mut ctx.ultimo_aviso_leitura_us,
                    &format!("falha ao criar a staging de leitura: {e}"),
                );
                return;
            }
        }
    }
    let Some(leitura) = ctx.leitura.as_mut() else {
        return;
    };
    let (mapeado, rt) = match leitura.map(dev, &frame.texture) {
        Ok(r) => r,
        Err(e) => {
            avisar_leitura(
                &mut ctx.ultimo_aviso_leitura_us,
                &format!("falha ao ler o quadro de origem: {e}"),
            );
            return;
        }
    };
    ctx.leitura_s.push(rt.copy_wait_us);
    let opts = BatchOptions {
        threads: ctx.threads,
        on_thread_start: Some(super::prioridade_acima_do_normal),
    };
    let src = mapeado.image();

    // Fase 1: a primeira camada sozinha (entrega cedo); fase 2: as outras num lote.
    let (primeira, resto) = cpu.split_at_mut(1);
    converter_lote(primeira, dev, shared, &src, cap_us, t0, rt.lock_wait_us, opts);
    if !resto.is_empty() {
        converter_lote(resto, dev, shared, &src, cap_us, t0, rt.lock_wait_us, opts);
    }
    // `mapeado` faz o Unmap ao sair.
    drop(mapeado);
}

/// Mapeia a escrita de cada camada, converte todas com UM `convert_batch`, e então faz
/// `commit` + `submit` de cada uma. Erro numa camada só a faz pular o quadro.
#[allow(clippy::too_many_arguments)]
fn converter_lote(
    camadas: &mut [&mut Camada],
    dev: &GpuDevice,
    shared: &Shared,
    src: &zap_gpu::SrcImage<'_>,
    cap_us: u64,
    t0: Instant,
    lock_leitura_us: f64,
    opts: BatchOptions,
) {
    let mut itens: Vec<Item<'_>> = Vec::with_capacity(camadas.len());
    for cam in camadas.iter_mut() {
        let Camada {
            spec,
            pipe,
            primeiro,
            convert,
            lock_wait,
            ..
        } = &mut **cam;
        let layer = spec.layer;
        let Some(Pipe {
            conv: Conversor::Cpu(CamadaCpu { scaler, upload }),
            ring,
            enc,
            ..
        }) = pipe.as_mut()
        else {
            continue;
        };
        match upload.map(dev) {
            Ok(m) => itens.push(Item {
                layer,
                scaler: &*scaler,
                mapeado: Some(m),
                ring,
                enc,
                primeiro,
                convert,
                lock_wait,
            }),
            Err(e) => tracing::warn!("camada {layer}: falha ao mapear a escrita: {e}"),
        }
    }
    if itens.is_empty() {
        return;
    }

    let resultado = {
        let mut jobs = Vec::with_capacity(itens.len());
        for it in itens.iter_mut() {
            if let Some(m) = it.mapeado.as_mut() {
                jobs.push((it.scaler, m.nv12()));
            }
        }
        convert_batch(src, &mut jobs, opts)
    };
    if let Err(e) = resultado {
        // Os `MappedUpload` fazem Unmap no Drop.
        tracing::warn!("falha na conversão pela CPU: {e}");
        return;
    }

    for it in itens.iter_mut() {
        let Some(m) = it.mapeado.take() else { continue };
        let layer = it.layer;
        let dst = it.ring.next();
        let t = match m.commit(dst) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("camada {layer}: falha ao enviar o NV12 à GPU: {e}");
                continue;
            }
        };
        let tex = clonar(dst);
        it.convert.push(t0.elapsed().as_micros() as f64);
        it.lock_wait.push(lock_leitura_us + t.lock_wait_us);
        let force = shared.take_keyframe(layer) || *it.primeiro;
        if it.enc.submit(tex, cap_us, force) {
            *it.primeiro = false;
        } else if force {
            // Fila cheia: o pedido não pode se perder.
            let mut c = shared.control.lock();
            if !c.keyframes.contains(&layer) {
                c.keyframes.push(layer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avisa_o_primeiro() {
        assert!(deve_avisar(5_000_000, 0));
    }

    #[test]
    fn nao_avisa_antes_de_1s() {
        assert!(!deve_avisar(1_500_000, 1_000_001));
        assert!(deve_avisar(2_000_001, 1_000_001));
    }
}
