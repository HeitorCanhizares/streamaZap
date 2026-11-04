use crate::cursor::draw_cursor;
use crate::dxgi_dup::{Acquired, Duplication};
use crate::{CaptureError, CaptureHandle, CapturedFrame, FrameSink, MonitorInfo, StaleTimer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};
use zap_gpu::{now_us, ComGuard, GpuDevice, PixelFormat, Texture, TextureRing};

/// Quantas texturas o anel de entrega tem (o consumidor tem folga de 2 quadros).
const TAMANHO_ANEL: usize = 3;
/// Espera de cada `AcquireNextFrame`.
const ESPERA_MS: u32 = 100;
/// Intervalo entre tentativas de recriar a duplicação e tempo total máximo.
const RETENTATIVA: Duration = Duration::from_millis(100);
const RETENTATIVAS_MAX: u32 = 10;

/// Leva o ponteiro do dispositivo para a thread de captura.
struct SendPtr<T>(*const T);

// SAFETY: o dispositivo (`GpuDevice`) é Send + Sync e protegido para multithread. O contrato de
// `capture_monitor` exige que o chamador mantenha o dispositivo vivo enquanto o handle existir.
unsafe impl<T> Send for SendPtr<T> {}

impl<T> SendPtr<T> {
    /// Método (e não acesso ao campo) para a closure capturar o wrapper inteiro, que é Send.
    fn ptr(&self) -> *const T {
        self.0
    }
}

/// Captura de monitor por DXGI Desktop Duplication, em thread própria.
///
/// O `dev` precisa continuar vivo até o `CaptureHandle` ser descartado: a thread o usa para
/// recriar a duplicação e as texturas.
pub fn capture_monitor(
    dev: &GpuDevice,
    monitor: &MonitorInfo,
    sink: impl FrameSink,
) -> Result<CaptureHandle, CaptureError> {
    // Cria tudo que pode falhar antes de spawnar, para o erro voltar ao chamador.
    let dup = Duplication::new(dev, monitor)?;
    let (w, h) = dup.size();
    let ring = TextureRing::new(dev, w, h, PixelFormat::Bgra8, TAMANHO_ANEL, true)?;
    let limpa = Texture::new(dev, w, h, PixelFormat::Bgra8, false)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let dev_ptr = SendPtr(dev as *const GpuDevice);
    let monitor = monitor.clone();

    let join = std::thread::Builder::new()
        .name("zap-capture-dxgi".to_string())
        .spawn(move || {
            // SAFETY: ver `SendPtr`; o chamador mantém o dispositivo vivo durante a captura.
            let dev = unsafe { &*dev_ptr.ptr() };
            let _com = match ComGuard::new() {
                Ok(g) => g,
                Err(e) => {
                    sink.on_error(e.into());
                    return;
                }
            };
            // SAFETY: altera só a prioridade da thread atual.
            if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) } {
                tracing::debug!("SetThreadPriority falhou: {e}");
            }
            laco(dev, &monitor, &sink, &stop_thread, dup, ring, limpa);
        })
        .map_err(|e| CaptureError::Os {
            context: "thread zap-capture-dxgi",
            message: e.to_string(),
        })?;
    Ok(CaptureHandle::new(stop, join))
}

/// Estado de entrega: anel com cursor, textura limpa com a última imagem e relógio de reentrega.
struct Entrega<'a, S: FrameSink> {
    dev: &'a GpuDevice,
    sink: &'a S,
    monitor: (i32, i32, u32, u32),
    ring: TextureRing,
    limpa: Texture,
    tem_imagem: bool,
    timer: StaleTimer,
}

impl<S: FrameSink> Entrega<'_, S> {
    /// Copia a imagem limpa para o próximo slot, desenha o cursor e entrega.
    fn entregar(&mut self, new_content: bool) {
        let (w, h) = (self.limpa.width(), self.limpa.height());
        let slot = self.ring.next();
        // SAFETY: as duas texturas são do mesmo dispositivo e têm o mesmo tamanho e formato.
        unsafe { self.dev.context().CopyResource(slot.raw(), self.limpa.raw()) };
        if let Err(e) = draw_cursor(slot, self.monitor) {
            tracing::debug!("cursor não desenhado: {e}");
        }
        // Mesma superfície do anel; o anel de 3 dá folga ao consumidor.
        let texture = Texture::from_raw(slot.raw().clone(), PixelFormat::Bgra8, w, h);
        let agora = now_us();
        self.timer.delivered(agora);
        self.sink.on_frame(CapturedFrame {
            texture,
            width: w,
            height: h,
            capture_us: agora,
            new_content,
        });
    }
}

/// Tenta recriar a duplicação a cada 100 ms por até 1 s. `Ok(None)` = pediram parada.
fn recriar(dev: &GpuDevice, monitor: &MonitorInfo, stop: &AtomicBool) -> Result<Option<Duplication>, CaptureError> {
    let mut ultimo = None;
    for tentativa in 0..=RETENTATIVAS_MAX {
        if stop.load(Ordering::Acquire) {
            return Ok(None);
        }
        match Duplication::new(dev, monitor) {
            Ok(d) => return Ok(Some(d)),
            Err(e) => ultimo = Some(e),
        }
        if tentativa < RETENTATIVAS_MAX {
            std::thread::sleep(RETENTATIVA);
        }
    }
    Err(CaptureError::Os {
        context: "recriar duplicação",
        message: ultimo.map(|e| e.to_string()).unwrap_or_default(),
    })
}

fn laco<S: FrameSink>(
    dev: &GpuDevice,
    monitor: &MonitorInfo,
    sink: &S,
    stop: &AtomicBool,
    dup: Duplication,
    ring: TextureRing,
    limpa: Texture,
) {
    let mut ent = Entrega {
        dev,
        sink,
        monitor: (monitor.x, monitor.y, monitor.width, monitor.height),
        ring,
        limpa,
        tem_imagem: false,
        timer: StaleTimer::new(Duration::from_secs(1)),
    };
    let mut dup = Some(dup);
    while !stop.load(Ordering::Acquire) {
        let Some(d) = dup.as_mut() else { return };
        match d.acquire(ESPERA_MS) {
            Err(e) => {
                sink.on_error(e);
                return;
            }
            Ok(Acquired::Timeout) => {
                if ent.tem_imagem && ent.timer.due(now_us()) {
                    ent.entregar(false);
                }
            }
            Ok(Acquired::Lost) => {
                let antigo = d.size();
                drop(dup.take()); // libera a duplicação perdida antes de recriar
                let nova = match recriar(dev, monitor, stop) {
                    Ok(Some(n)) => n,
                    Ok(None) => return,
                    Err(e) => {
                        sink.on_error(e);
                        return;
                    }
                };
                let (w, h) = nova.size();
                if (w, h) != antigo {
                    let refeito = TextureRing::new(dev, w, h, PixelFormat::Bgra8, TAMANHO_ANEL, true)
                        .and_then(|r| Texture::new(dev, w, h, PixelFormat::Bgra8, false).map(|t| (r, t)));
                    match refeito {
                        Ok((r, t)) => {
                            ent.ring = r;
                            ent.limpa = t;
                            ent.tem_imagem = false;
                        }
                        Err(e) => {
                            sink.on_error(e.into());
                            return;
                        }
                    }
                }
                dup = Some(nova);
            }
            Ok(Acquired::Frame {
                new_image,
                mouse_updated,
                texture,
            }) => {
                let mut novo = false;
                if let Some(t) = texture {
                    // SAFETY: `t` é do dispositivo da duplicação e `limpa` do mesmo dispositivo.
                    unsafe { dev.context().CopyResource(ent.limpa.raw(), &t) };
                    ent.tem_imagem = true;
                    novo = new_image;
                }
                // Libera logo para a duplicação poder seguir.
                if let Err(e) = d.release() {
                    sink.on_error(e);
                    return;
                }
                if ent.tem_imagem && (novo || mouse_updated) {
                    ent.entregar(novo);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

    struct SinkCanal(mpsc::Sender<(u32, u32)>);

    impl FrameSink for SinkCanal {
        fn on_frame(&self, frame: CapturedFrame) {
            let _ = self.0.send((frame.width, frame.height));
        }
        fn on_error(&self, err: CaptureError) {
            eprintln!("erro de captura: {err}");
        }
    }

    #[test]
    fn capture_monitor_entrega_quadro_em_1s() {
        let Ok(monitores) = crate::list_monitors() else {
            eprintln!("pulando: sem monitores");
            return;
        };
        let Some(mon) = monitores.into_iter().next() else {
            eprintln!("pulando: sem monitor 1");
            return;
        };
        let Ok(dev) = GpuDevice::create_for_adapter(mon.adapter_luid) else {
            eprintln!("pulando: sem GPU");
            return;
        };
        let (tx, rx) = mpsc::channel();
        let handle = match capture_monitor(&dev, &mon, SinkCanal(tx)) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("pulando: duplicação indisponível ({e})");
                return;
            }
        };
        // Mexe o mouse para garantir atividade na tela durante 1 s.
        let inicio = Instant::now();
        let mut i = 0;
        while inicio.elapsed() < Duration::from_secs(1) {
            // SAFETY: chamada simples sem ponteiros.
            let _ = unsafe { SetCursorPos(mon.x + 10 + (i % 50), mon.y + 10 + (i % 50)) };
            i += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
        let quadros: Vec<_> = rx.try_iter().collect();
        assert!(!quadros.is_empty(), "nenhum quadro em 1 s");
        let (w, h) = quadros[0];
        assert!((w, h) == (mon.width, mon.height) || (w, h) == (mon.height, mon.width));
        drop(handle); // o drop faz join: termina sem travar
    }
}
