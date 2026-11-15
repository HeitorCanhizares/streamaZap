//! Captura direta do microfone: WASAPI compartilhado, por evento, 48 kHz float mono.

use crate::com::ComGuard;
use crate::mic_devices::find_capture_device;
use crate::{AudioError, FRAME};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::HRESULT;
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE};
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

/// Acima disso (em blocos) o consumidor está lento: descarta o mais velho.
const MAX_BLOCOS: usize = 8;

pub struct DirectCapture {
    cliente: IAudioClient,
    captura: IAudioCaptureClient,
    evento: HANDLE,
    /// Amostras já capturadas e ainda não entregues (capacidade fixa, sem alocar no laço).
    buf: Vec<f32>,
    descontinuidades: u64,
    // Por último: COM só é desfeito depois de soltar as interfaces acima.
    _com: ComGuard,
}

/// Traduz erro do Windows para o erro do crate.
fn mapear(contexto: &str, e: &windows::core::Error) -> AudioError {
    let hr = e.code();
    if hr == E_ACCESSDENIED {
        AudioError::MicBlocked
    } else if hr == AUDCLNT_E_DEVICE_INVALIDATED || hr == HRESULT(0x8007_0490_u32 as i32) {
        AudioError::NoDevice
    } else {
        AudioError::Backend(format!("{contexto}: {e}"))
    }
}

/// Junta `novas` ao buffer e, havendo 960 amostras, tira exatamente um bloco (o resto fica).
fn juntar_blocos(buf: &mut Vec<f32>, novas: &[f32]) -> Option<Box<[f32; FRAME]>> {
    buf.extend_from_slice(novas);
    if buf.len() < FRAME {
        return None;
    }
    let mut bloco = Box::new([0.0f32; FRAME]);
    bloco.copy_from_slice(&buf[..FRAME]);
    buf.drain(..FRAME);
    Some(bloco)
}

impl DirectCapture {
    /// Abre o microfone (`None` = padrão de comunicação). `AUTOCONVERTPCM` para 48 kHz float mono.
    /// `E_ACCESSDENIED` vira `AudioError::MicBlocked`. Deve ser chamado na thread que vai ler.
    pub fn open(device_id: Option<&str>) -> Result<Self, AudioError> {
        let com = ComGuard::new();
        let device = find_capture_device(device_id)?;
        // SAFETY: COM inicializado na thread; dispositivo válido.
        let cliente: IAudioClient =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| mapear("Activate(IAudioClient)", &e))?;

        let fmt = WAVEFORMATEX {
            wFormatTag: 3, // WAVE_FORMAT_IEEE_FLOAT
            nChannels: 1,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 192_000,
            nBlockAlign: 4,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        // SAFETY: `fmt` vale durante a chamada; cliente recém-ativado.
        unsafe {
            cliente.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                2_000_000, // 200 ms em unidades de 100 ns
                0,
                &fmt,
                None,
            )
        }
        .map_err(|e| mapear("Initialize", &e))?;

        // SAFETY: cria evento anônimo de reset automático; fechado no Drop (ou abaixo, em erro).
        let evento = unsafe { CreateEventW(None, false, false, None) }.map_err(|e| mapear("CreateEventW", &e))?;
        let fechar = |e: AudioError| {
            // SAFETY: handle criado acima e ainda não fechado.
            unsafe {
                let _ = CloseHandle(evento);
            }
            e
        };
        // SAFETY: evento válido, fechado só depois de o cliente parar.
        unsafe { cliente.SetEventHandle(evento) }.map_err(|e| fechar(mapear("SetEventHandle", &e)))?;
        // SAFETY: cliente inicializado; GetService só consulta o objeto.
        let captura: IAudioCaptureClient =
            unsafe { cliente.GetService() }.map_err(|e| fechar(mapear("GetService(IAudioCaptureClient)", &e)))?;
        // SAFETY: cliente inicializado com evento configurado.
        unsafe { cliente.Start() }.map_err(|e| fechar(mapear("Start", &e)))?;

        Ok(DirectCapture {
            cliente,
            captura,
            evento,
            buf: Vec::with_capacity((MAX_BLOCOS + 8) * FRAME),
            descontinuidades: 0,
            _com: com,
        })
    }

    /// Espera até juntar 20 ms (960 amostras) e devolve o bloco. `Ok(None)` se `stop` ligou.
    pub fn next_block(&mut self, stop: &AtomicBool) -> Result<Option<Box<[f32; FRAME]>>, AudioError> {
        loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(None);
            }
            // Sobra de uma acordada anterior.
            if let Some(b) = juntar_blocos(&mut self.buf, &[]) {
                return Ok(Some(b));
            }
            // SAFETY: handle de evento válido enquanto `self` existe.
            unsafe { WaitForSingleObject(self.evento, 20) };
            self.drenar()?;
        }
    }

    /// Lê todos os pacotes pendentes para o buffer interno.
    fn drenar(&mut self) -> Result<(), AudioError> {
        loop {
            // SAFETY: cliente de captura válido, usado só nesta thread.
            let tamanho = unsafe { self.captura.GetNextPacketSize() }.map_err(|e| mapear("GetNextPacketSize", &e))?;
            if tamanho == 0 {
                break;
            }
            let mut dados: *mut u8 = std::ptr::null_mut();
            let mut quadros = 0u32;
            let mut flags = 0u32;
            // SAFETY: ponteiros de saída válidos; ReleaseBuffer logo abaixo com o mesmo nº de quadros.
            unsafe { self.captura.GetBuffer(&mut dados, &mut quadros, &mut flags, None, None) }
                .map_err(|e| mapear("GetBuffer", &e))?;
            let n = quadros as usize;
            if flags & (AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32) != 0 {
                self.descontinuidades += 1;
                tracing::debug!(total = self.descontinuidades, "microfone: descontinuidade de dados");
            }
            if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || dados.is_null() {
                self.buf.resize(self.buf.len() + n, 0.0);
            } else {
                // SAFETY: o WASAPI garante `quadros * 4` bytes legíveis até o ReleaseBuffer.
                let bytes = unsafe { std::slice::from_raw_parts(dados, n * 4) };
                self.buf.extend(
                    bytes
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
                );
            }
            // SAFETY: devolve exatamente o que GetBuffer entregou.
            let _ = unsafe { self.captura.ReleaseBuffer(quadros) };
        }
        if self.buf.len() > MAX_BLOCOS * FRAME {
            let excesso = self.buf.len() - MAX_BLOCOS * FRAME;
            // Descarta em múltiplos de bloco para manter o alinhamento de 20 ms.
            let descartar = excesso.div_ceil(FRAME) * FRAME;
            self.buf.drain(..descartar.min(self.buf.len()));
            tracing::warn!(
                blocos = descartar / FRAME,
                "microfone: consumidor lento, blocos antigos descartados"
            );
        }
        Ok(())
    }
}

impl Drop for DirectCapture {
    fn drop(&mut self) {
        // SAFETY: cliente e evento válidos; Stop em cliente já parado é inofensivo.
        unsafe {
            let _ = self.cliente.Stop();
            let _ = CloseHandle(self.evento);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Abre o padrão; `None` (com aviso) se não houver microfone utilizável.
    fn abrir_ou_pular() -> Option<DirectCapture> {
        if crate::list_microphones().map(|l| l.is_empty()).unwrap_or(true) {
            eprintln!("aviso: sem microfone, teste pulado");
            return None;
        }
        match DirectCapture::open(None) {
            Ok(c) => Some(c),
            Err(AudioError::NoDevice | AudioError::MicBlocked) => {
                eprintln!("aviso: sem microfone, teste pulado");
                None
            }
            Err(e) => panic!("falha inesperada ao abrir: {e}"),
        }
    }

    #[test]
    fn captura_100_blocos_em_2s() {
        let Some(mut c) = abrir_ou_pular() else { return };
        let stop = AtomicBool::new(false);
        let ini = Instant::now();
        let mut n = 0;
        while ini.elapsed() < Duration::from_secs(2) {
            let b = c.next_block(&stop).expect("bloco").expect("sem stop");
            assert_eq!(b.len(), FRAME);
            assert!(b.iter().all(|v| v.is_finite() && (-1.5..=1.5).contains(v)));
            n += 1;
        }
        assert!(n >= 90, "só {n} blocos em 2 s");
    }

    #[test]
    fn periodo_medio_20ms() {
        let Some(mut c) = abrir_ou_pular() else { return };
        let stop = AtomicBool::new(false);
        let mut instantes = Vec::new();
        for _ in 0..60 {
            c.next_block(&stop).expect("bloco");
            instantes.push(Instant::now());
        }
        let regime = &instantes[5..];
        let total = regime[regime.len() - 1].duration_since(regime[0]);
        let medio = total.as_secs_f64() * 1000.0 / (regime.len() - 1) as f64;
        assert!((18.0..=22.0).contains(&medio), "período médio {medio:.2} ms");
    }

    #[test]
    fn stop_interrompe() {
        let Some(mut c) = abrir_ou_pular() else { return };
        let stop = AtomicBool::new(true);
        let ini = Instant::now();
        assert!(matches!(c.next_block(&stop), Ok(None)));
        assert!(ini.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn id_inexistente_da_erro() {
        assert!(DirectCapture::open(Some("zzz")).is_err());
    }

    #[test]
    fn juntar_blocos_acumula_e_corta() {
        let mut buf = Vec::new();
        assert!(juntar_blocos(&mut buf, &[0.1; 500]).is_none());
        let b = juntar_blocos(&mut buf, &[0.2; 500]).expect("bloco");
        assert_eq!(b.len(), FRAME);
        assert_eq!(buf.len(), 40);

        let mut buf = Vec::new();
        assert!(juntar_blocos(&mut buf, &[0.3; 1920]).is_some());
        assert_eq!(buf.len(), 960);
    }
}
