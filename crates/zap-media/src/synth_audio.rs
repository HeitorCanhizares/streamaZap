//! Backend sintético de áudio: Opus de silêncio a cada 20 ms.
use crate::{EncodedAudio, MediaError};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Intervalo entre blocos: 960 amostras por canal a 48 kHz = 20 ms.
const BLOCO: Duration = Duration::from_millis(20);
/// Bitrate do encoder sintético, o mesmo do pipeline real.
const KBPS: u32 = 128;

/// Thread "zap-synth-audio": a cada 20 ms (relógio de parede, sem acumular atraso) codifica um
/// bloco de silêncio com `zap_audio::OpusEnc::new(128)` e envia `EncodedAudio`
/// (`seq` 0,1,2..., `capture_us = zap_gpu::now_us()`). Para quando `stop` for true ou o canal
/// fechar.
pub(crate) fn spawn(
    track: u16,
    stop: Arc<AtomicBool>,
    out: Sender<EncodedAudio>,
) -> Result<std::thread::JoinHandle<()>, MediaError> {
    // Encoder criado antes da thread: assim um erro de inicialização volta para quem chamou.
    let mut enc = zap_audio::OpusEnc::new(KBPS)?;
    let handle = std::thread::Builder::new()
        .name("zap-synth-audio".into())
        .spawn(move || {
            let silencio = [0.0f32; zap_audio::FRAME_SAMPLES];
            let mut inicio = Instant::now();
            let mut n: u32 = 0;
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                // O bloco n deve sair em inicio + n*20 ms. Se já passou desse instante, não dorme.
                let alvo = inicio + BLOCO * n;
                let agora = Instant::now();
                if alvo > agora {
                    std::thread::sleep(alvo - agora);
                } else if agora - alvo >= BLOCO {
                    // Atraso de um bloco ou mais: reancora o relógio no instante atual para não
                    // emitir rajadas de blocos atrasados tentando recuperar o tempo perdido.
                    inicio += agora - alvo;
                }
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let data = match enc.encode(&silencio) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("falha ao codificar bloco sintético de áudio: {e}");
                        return;
                    }
                };
                let pacote = EncodedAudio {
                    track,
                    seq: n,
                    capture_us: zap_gpu::now_us(),
                    data,
                };
                if out.send(pacote).is_err() {
                    // Receptor descartado: ninguém mais consome o áudio.
                    return;
                }
                n = n.wrapping_add(1);
            }
        })
        .map_err(|e| MediaError::Stopped(format!("falha ao criar thread de áudio: {e}")))?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    /// Roda o backend por `duracao`, depois para e devolve os pacotes recebidos.
    fn rodar(duracao: Duration) -> Vec<EncodedAudio> {
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn(7, stop.clone(), tx).expect("spawn");
        std::thread::sleep(duracao);
        stop.store(true, Ordering::Relaxed);
        handle.join().expect("thread");
        rx.try_iter().collect()
    }

    #[test]
    fn emite_cerca_de_50_blocos_por_segundo() {
        let pacotes = rodar(Duration::from_secs(1));
        assert!((40..=60).contains(&pacotes.len()), "{} blocos em 1 s", pacotes.len());
        for (i, p) in pacotes.iter().enumerate() {
            assert_eq!(p.seq as usize, i, "seq com buraco");
            assert_eq!(p.track, 7);
        }
    }

    #[test]
    fn payload_decodifica_com_opus_dec() {
        let pacotes = rodar(Duration::from_millis(100));
        let primeiro = pacotes.first().expect("ao menos um pacote");
        let mut dec = zap_audio::OpusDec::new().expect("decoder");
        let mut out = [0f32; zap_audio::FRAME_SAMPLES];
        dec.decode(Some(&primeiro.data[..]), None, &mut out).expect("decode");
        assert!(out.iter().all(|a| a.abs() < 1e-3), "amostras fora do silêncio");
    }

    #[test]
    fn para_com_stop() {
        let (tx, _rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn(0, stop.clone(), tx).expect("spawn");
        std::thread::sleep(Duration::from_millis(30));
        let inicio = Instant::now();
        stop.store(true, Ordering::Relaxed);
        handle.join().expect("thread");
        assert!(inicio.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn encerra_quando_canal_fecha() {
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn(0, stop, tx).expect("spawn");
        drop(rx);
        let limite = Instant::now() + Duration::from_secs(2);
        while !handle.is_finished() {
            assert!(Instant::now() < limite, "thread não encerrou após drop do receptor");
            std::thread::sleep(Duration::from_millis(5));
        }
        handle.join().expect("thread");
    }
}
