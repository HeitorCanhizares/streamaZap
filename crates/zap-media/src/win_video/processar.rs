//! `processar`: garante os pipes das camadas pedidas e entrega o quadro a cada uma: primeiro as
//! camadas na GPU, depois (em `fase_cpu`) as que convertem na CPU.
use super::conversor::{criar_conversor, Conversor};
use super::{clonar, criar_pipe, Camada, Contexto, RETENTAR_US};
use crate::video::Shared;
use crate::ConversionSpec;
use std::time::Instant;
use zap_capture::CapturedFrame;
use zap_gpu::GpuDevice;

/// Converte e entrega o quadro a cada camada ligada cujo `LayerGate` deixar.
pub(super) fn processar(
    camadas: &mut [Camada],
    requests: &[(u8, u32)],
    dev: &GpuDevice,
    shared: &Shared,
    frame: &CapturedFrame,
    agora: u64,
    ctx: &mut Contexto,
) {
    let in_fmt = frame.texture.format();
    let (in_w, in_h) = (frame.width, frame.height);
    // Reentrega usa o relógio atual; imagem nova mantém o instante real da captura.
    let cap_us = if frame.new_content { frame.capture_us } else { agora };
    let entrada = (in_w, in_h, in_fmt);
    let mut devidas: Vec<usize> = Vec::new();
    for (idx, cam) in camadas.iter_mut().enumerate() {
        let layer = cam.spec.layer;
        let Some(&(_, kbps)) = requests.iter().find(|(l, _)| *l == layer) else {
            continue;
        };
        // Tamanho/formato da entrada mudou: recria só o conversor.
        if let Some(p) = &mut cam.pipe {
            if p.in_w != in_w || p.in_h != in_h || p.in_fmt != in_fmt {
                match criar_conversor(dev, ctx, cam.forcar_cpu, &cam.spec, (in_w, in_h, in_fmt)) {
                    Ok(c) => {
                        p.conv = c;
                        p.in_w = in_w;
                        p.in_h = in_h;
                        p.in_fmt = in_fmt;
                        // Imagem nova de outro tamanho: o decodificador precisa de um ponto de entrada.
                        cam.primeiro = true;
                    }
                    Err(e) => {
                        tracing::warn!("camada {layer}: falha ao recriar o conversor: {e}");
                        cam.soltar();
                        cam.retentar_em_us = agora + RETENTAR_US;
                        continue;
                    }
                }
            }
        }
        if cam.pipe.is_none() {
            if agora < cam.retentar_em_us {
                continue;
            }
            match criar_pipe(dev, &cam.spec, kbps, ctx, cam.forcar_cpu, (in_w, in_h, in_fmt)) {
                Ok(p) => {
                    cam.pipe = Some(p);
                    cam.kbps = kbps;
                    cam.gate.reset();
                    cam.primeiro = true;
                    let mut c = shared.control.lock();
                    if !c.keyframes.contains(&layer) {
                        c.keyframes.push(layer);
                    }
                }
                Err(e) => {
                    tracing::warn!("camada {layer}: falha ao ligar: {e}");
                    cam.retentar_em_us = agora + RETENTAR_US;
                    continue;
                }
            }
        }
        if cam.pipe.is_some() && cam.gate.ready(agora) {
            devidas.push(idx);
        }
    }

    // Fase da GPU. Camadas que trocam para a CPU só entram na fase da CPU no próximo quadro.
    let mut cpu: Vec<usize> = Vec::new();
    for idx in devidas {
        let cam = &mut camadas[idx];
        let layer = cam.spec.layer;
        let Some(p) = &mut cam.pipe else { continue };
        let Conversor::Gpu(conv) = &mut p.conv else {
            cpu.push(idx);
            continue;
        };
        let t0 = Instant::now();
        let dst = p.ring.next();
        if let Err(e) = conv.convert(dev, &frame.texture, dst) {
            tracing::warn!("camada {layer}: falha na conversão: {e}");
            cam.falhas_gpu += 1;
            if cam.falhas_gpu >= 3 && ctx.opts.conversion == ConversionSpec::Auto {
                tracing::warn!("camada {layer}: GPU falhou 3 vezes seguidas; passando para a CPU");
                cam.forcar_cpu = true;
                cam.falhas_gpu = 0;
                match criar_conversor(dev, ctx, true, &cam.spec, entrada) {
                    Ok(c) => {
                        p.conv = c;
                        cam.primeiro = true;
                    }
                    Err(e) => {
                        tracing::warn!("camada {layer}: falha ao passar para a CPU: {e}");
                        cam.soltar();
                        cam.retentar_em_us = agora + RETENTAR_US;
                    }
                }
            }
            continue;
        }
        cam.falhas_gpu = 0;
        let tex = clonar(dst);
        cam.convert.push(t0.elapsed().as_micros() as f64);
        cam.lock_wait.push(conv.last_timing().lock_wait_us);
        for us in conv.take_gpu_us(dev) {
            cam.convert_gpu.push(us);
        }
        let force = shared.take_keyframe(layer) || cam.primeiro;
        if p.enc.submit(tex, cap_us, force) {
            cam.primeiro = false;
        } else if force {
            // Fila cheia: o pedido não pode se perder.
            let mut c = shared.control.lock();
            if !c.keyframes.contains(&layer) {
                c.keyframes.push(layer);
            }
        }
    }

    // Fase da CPU: empréstimos mutáveis só agora, em ordem de índice.
    if !cpu.is_empty() {
        let mut v: Vec<&mut Camada> = camadas
            .iter_mut()
            .enumerate()
            .filter(|(i, _)| cpu.contains(i))
            .map(|(_, c)| c)
            .collect();
        super::fase_cpu::rodar(&mut v, dev, shared, frame, cap_us, ctx);
    }
}
