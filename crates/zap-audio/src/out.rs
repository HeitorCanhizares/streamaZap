//! Saída de áudio via cpal (só Windows). O callback do dispositivo roda `MixerCore::render`
//! e, se o dispositivo não for 48 kHz estéreo, reamostra/ajusta canais com `rubato`.
use crate::out_mixer::{self, MixerCore, MixerHandle};
use crate::{AudioError, TrackInput, CHANNELS, RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FastFixedOut, PolynomialDegree, Resampler};

/// Quadros de saída (na taxa do dispositivo) gerados por rodada do reamostrador.
const CHUNK: usize = 480;

/// Saída de áudio padrão via cpal; o callback roda `MixerCore::render`.
///
/// Não é `Send`: `cpal::Stream` não é `Send` no Windows, então `AudioOut` deve ficar na
/// thread que o criou. Use [`AudioOut::add_track`] etc. a partir dela (os `TrackInput`
/// devolvidos, esses sim, podem ir para outras threads).
pub struct AudioOut {
    pub(crate) mixer: MixerHandle,
    pub(crate) _stream: cpal::Stream,
}

/// Escreve um quadro estéreo (`l`, `r`) em `quadro`, que tem um item por canal do dispositivo.
/// Mono: média L+R; 2 canais: L,R; mais que isso: L,R e zero nos demais.
pub(crate) fn escrever_quadro(l: f32, r: f32, quadro: &mut [f32]) {
    match quadro.len() {
        0 => {}
        1 => quadro[0] = 0.5 * (l + r),
        _ => {
            quadro[0] = l;
            quadro[1] = r;
            quadro[2..].fill(0.0);
        }
    }
}

/// Caminho de conversão (reamostragem + canais) com tudo pré-alocado.
struct Conversor {
    core: MixerCore,
    resampler: FastFixedOut<f32>,
    canais: usize,
    /// Estéreo 48 kHz intercalado vindo do mixer.
    scratch: Vec<f32>,
    in_planar: Vec<Vec<f32>>,
    out_planar: Vec<Vec<f32>>,
    /// Sobra já convertida (intercalada, `canais` por quadro) entre callbacks.
    pend: Vec<f32>,
    pend_pos: usize,
    pend_len: usize,
}

impl Conversor {
    fn novo(core: MixerCore, taxa: u32, canais: usize) -> Result<Self, AudioError> {
        let ratio = f64::from(taxa) / f64::from(RATE);
        let resampler = FastFixedOut::<f32>::new(ratio, 1.0, PolynomialDegree::Cubic, CHUNK, CHANNELS)
            .map_err(|e| AudioError::Backend(format!("reamostrador: {e}")))?;
        let in_max = resampler.input_frames_max();
        let out_max = resampler.output_frames_max();
        Ok(Self {
            core,
            canais,
            scratch: vec![0.0; in_max * CHANNELS],
            in_planar: vec![vec![0.0; in_max]; CHANNELS],
            out_planar: vec![vec![0.0; out_max]; CHANNELS],
            pend: vec![0.0; out_max * canais],
            pend_pos: 0,
            pend_len: 0,
            resampler,
        })
    }

    /// Gera mais um bloco em `pend`. Em falha do reamostrador, entrega silêncio.
    fn produzir(&mut self) {
        let n_in = self.resampler.input_frames_next();
        self.core.render(&mut self.scratch[..n_in * CHANNELS]);
        for (i, q) in self.scratch[..n_in * CHANNELS].chunks_exact(CHANNELS).enumerate() {
            self.in_planar[0][i] = q[0];
            self.in_planar[1][i] = q[1];
        }
        let n_out = match self
            .resampler
            .process_into_buffer(&self.in_planar, &mut self.out_planar, None)
        {
            Ok((_, n_out)) => n_out,
            Err(_) => {
                self.pend[..CHUNK * self.canais].fill(0.0);
                self.pend_pos = 0;
                self.pend_len = CHUNK * self.canais;
                return;
            }
        };
        for i in 0..n_out {
            let (l, r) = (self.out_planar[0][i], self.out_planar[1][i]);
            escrever_quadro(l, r, &mut self.pend[i * self.canais..(i + 1) * self.canais]);
        }
        self.pend_pos = 0;
        self.pend_len = n_out * self.canais;
    }

    fn preencher(&mut self, data: &mut [f32]) {
        let mut feito = 0;
        while feito < data.len() {
            if self.pend_pos >= self.pend_len {
                self.produzir();
                if self.pend_len == 0 {
                    data[feito..].fill(0.0);
                    return;
                }
            }
            let n = (data.len() - feito).min(self.pend_len - self.pend_pos);
            data[feito..feito + n].copy_from_slice(&self.pend[self.pend_pos..self.pend_pos + n]);
            self.pend_pos += n;
            feito += n;
        }
    }
}

impl AudioOut {
    /// Abre o dispositivo padrão em f32. Se a taxa/canais do dispositivo ≠ 48 kHz estéreo,
    /// reamostra/ajusta canais com `rubato` entre o `MixerCore` e o buffer do cpal.
    /// `Err(AudioError::NoDevice)` se não houver saída.
    pub fn start() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let suportada = device
            .default_output_config()
            .map_err(|e| AudioError::Backend(format!("config de saída: {e}")))?;
        // Pedimos f32 mesmo se o formato nativo for outro; o backend converte ou falha.
        let config: cpal::StreamConfig = suportada.config();
        let taxa = config.sample_rate.0;
        let canais = usize::from(config.channels);
        if canais == 0 {
            return Err(AudioError::Backend("dispositivo sem canais".into()));
        }

        let (mixer, mut core) = out_mixer::new_mixer();
        let err_fn = |e| tracing::error!("erro no stream de saída: {e}");

        let stream = if taxa == RATE && canais == CHANNELS {
            device.build_output_stream::<f32, _, _>(&config, move |data, _| core.render(data), err_fn, None)
        } else {
            let mut conv = Conversor::novo(core, taxa, canais)?;
            device.build_output_stream::<f32, _, _>(&config, move |data, _| conv.preencher(data), err_fn, None)
        }
        .map_err(|e| AudioError::Backend(format!("abrir stream: {e}")))?;

        stream
            .play()
            .map_err(|e| AudioError::Backend(format!("iniciar stream: {e}")))?;
        Ok(Self { mixer, _stream: stream })
    }
    pub fn add_track(&self, id: u64) -> TrackInput {
        self.mixer.add_track(id)
    }
    pub fn remove_track(&self, id: u64) {
        self.mixer.remove_track(id)
    }
    pub fn set_master_volume(&self, v: f32) {
        self.mixer.set_master_volume(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canais_mono_estereo_multi() {
        let mut m = [9.0];
        escrever_quadro(0.2, 0.6, &mut m);
        assert!((m[0] - 0.4).abs() < 1e-6);
        let mut e = [0.0; 2];
        escrever_quadro(0.2, 0.6, &mut e);
        assert_eq!(e, [0.2, 0.6]);
        let mut q = [9.0; 6];
        escrever_quadro(0.2, 0.6, &mut q);
        assert_eq!(q, [0.2, 0.6, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn audio_out_abre_dispositivo() {
        let out = match AudioOut::start() {
            Err(AudioError::NoDevice) => {
                println!("sem saida de audio, pulando");
                return;
            }
            Err(_) => return,
            Ok(o) => o,
        };
        let mut t = out.add_track(1);
        let quadro: Vec<f32> = (0..crate::FRAME)
            .flat_map(|i| {
                let s = (i as f32 * 0.05).sin() * 0.2;
                [s, s]
            })
            .collect();
        for _ in 0..50 {
            t.push(&quadro);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        out.remove_track(1);
        out.set_master_volume(0.5);
    }
}
