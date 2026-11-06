//! Saída de áudio via cpal (só Windows). O callback do dispositivo roda `MixerCore::render`
//! e, se o dispositivo não for 48 kHz estéreo, reamostra/ajusta canais com `rubato`.
//!
//! A saída se conserta sozinha: se o dispositivo cair (fone desconectado, driver reiniciado),
//! parar de pedir som ou o Windows trocar o dispositivo padrão, [`AudioOut::maintain`] fecha o
//! stream e abre outro no dispositivo padrão de agora. As trilhas continuam as mesmas: o
//! `MixerCore` é emprestado ao callback e volta para casa quando o stream acaba.
use crate::com::ComGuard;
use crate::out_mixer::{self, MixerCore, MixerHandle};
use crate::{AudioError, TrackInput, CHANNELS, RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender};
use rubato::{FastFixedOut, PolynomialDegree, Resampler};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};

/// Quadros de saída (na taxa do dispositivo) gerados por rodada do reamostrador.
const CHUNK: usize = 480;
/// Sem nenhum pedido de som do dispositivo por esse tempo: o stream travou.
const TRAVADO: Duration = Duration::from_secs(3);
/// De quanto em quanto tempo confere se o dispositivo padrão do Windows mudou.
const CONFERIR_PADRAO: Duration = Duration::from_secs(2);
/// Espera entre tentativas de abrir quando não há dispositivo ou ele acabou de falhar.
const ESPERA_REABRIR: Duration = Duration::from_secs(2);
/// Um stream que caiu antes disso espera `ESPERA_REABRIR` para reabrir (evita laço de falhas).
const VIDA_MINIMA: Duration = Duration::from_secs(5);

/// Saída de áudio padrão via cpal; o callback roda `MixerCore::render`.
///
/// Não é `Send`: `cpal::Stream` não é `Send` no Windows, então `AudioOut` deve ficar na
/// thread que o criou. Use [`AudioOut::add_track`] etc. a partir dela (os `TrackInput`
/// devolvidos, esses sim, podem ir para outras threads). Chame [`AudioOut::maintain`] de
/// tempos em tempos (a cada ~0,5 s) para a saída se recuperar sozinha.
pub struct AudioOut {
    pub(crate) mixer: MixerHandle,
    /// Onde o `MixerCore` fica entre um stream e outro (canal de 1 lugar).
    casa_tx: Sender<MixerCore>,
    casa_rx: Receiver<MixerCore>,
    stream: Option<Aberto>,
    proxima_tentativa: Instant,
    proxima_conferencia: Instant,
    /// Já avisou no log que não há saída (não repete a cada tentativa).
    avisou_sem_saida: bool,
}

/// Stream aberto e os sinais de saúde dele.
struct Aberto {
    _stream: cpal::Stream,
    nome: String,
    /// Id do dispositivo padrão do Windows quando o stream abriu.
    id_padrao: Option<String>,
    falhou: Arc<AtomicBool>,
    /// Quantas vezes o callback rodou; parado por `TRAVADO` = stream travado.
    batidas: Arc<AtomicU64>,
    ultima_batida: (u64, Instant),
    aberto_em: Instant,
}

/// O `MixerCore` emprestado ao callback. O callback pega o mixer de casa na primeira vez que roda
/// (sem travar: `try_recv`); quando o callback é destruído (stream fechado ou morto), o mixer
/// volta para casa e o próximo stream o encontra lá.
struct Emprestimo {
    core: Option<MixerCore>,
    casa_tx: Sender<MixerCore>,
    casa_rx: Receiver<MixerCore>,
    batidas: Arc<AtomicU64>,
}

impl Emprestimo {
    fn render(&mut self, out: &mut [f32]) {
        self.batidas.fetch_add(1, Ordering::Relaxed);
        if self.core.is_none() {
            self.core = self.casa_rx.try_recv().ok();
        }
        match self.core.as_mut() {
            Some(core) => core.render(out),
            None => out.fill(0.0),
        }
    }
}

impl Drop for Emprestimo {
    fn drop(&mut self) {
        if let Some(core) = self.core.take() {
            let _ = self.casa_tx.try_send(core);
        }
    }
}

/// Id do dispositivo de saída padrão do Windows (o mesmo papel que o cpal usa).
fn default_output_id() -> Option<String> {
    let _com = ComGuard::new();
    // SAFETY: COM inicializado nesta thread; a string devolvida por GetId é liberada com
    // CoTaskMemFree depois de copiada.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let id = device.GetId().ok()?;
        let texto = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const _));
        texto
    }
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
    core: Emprestimo,
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
    fn novo(core: Emprestimo, taxa: u32, canais: usize) -> Result<Self, AudioError> {
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
    /// Cria o mixer e tenta abrir o dispositivo padrão em f32 (se a taxa/canais ≠ 48 kHz estéreo,
    /// reamostra/ajusta canais com `rubato`). Sem dispositivo agora não é erro: as trilhas já
    /// funcionam e [`AudioOut::maintain`] abre a saída quando aparecer uma.
    pub fn start() -> Result<Self, AudioError> {
        let (mixer, core) = out_mixer::new_mixer();
        let (casa_tx, casa_rx) = crossbeam_channel::bounded::<MixerCore>(1);
        casa_tx
            .try_send(core)
            .map_err(|_| AudioError::Backend("mixer de saída".into()))?;
        let agora = Instant::now();
        let mut out = Self {
            mixer,
            casa_tx,
            casa_rx,
            stream: None,
            proxima_tentativa: agora,
            proxima_conferencia: agora + CONFERIR_PADRAO,
            avisou_sem_saida: false,
        };
        out.maintain();
        Ok(out)
    }

    /// Há um stream aberto tocando agora.
    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    /// Nome do dispositivo aberto agora.
    pub fn device_name(&self) -> Option<&str> {
        self.stream.as_ref().map(|a| a.nome.as_str())
    }

    /// Confere a saúde da saída e conserta: fecha o stream que falhou, travou ou ficou no
    /// dispositivo que deixou de ser o padrão, e abre outro (ou tenta de novo mais tarde).
    pub fn maintain(&mut self) {
        let agora = Instant::now();
        if let Some(a) = self.stream.as_mut() {
            let mut motivo: Option<&str> = None;
            if a.falhou.load(Ordering::Relaxed) {
                motivo = Some("o dispositivo deu erro");
            } else {
                let b = a.batidas.load(Ordering::Relaxed);
                if b != a.ultima_batida.0 {
                    a.ultima_batida = (b, agora);
                } else if agora.duration_since(a.ultima_batida.1) > TRAVADO {
                    motivo = Some("o dispositivo parou de pedir som");
                }
            }
            if motivo.is_none() && agora >= self.proxima_conferencia {
                self.proxima_conferencia = agora + CONFERIR_PADRAO;
                let padrao = default_output_id();
                if padrao.is_some() && padrao != a.id_padrao {
                    motivo = Some("o dispositivo padrão do Windows mudou");
                }
            }
            if let Some(m) = motivo {
                let curto = agora.duration_since(a.aberto_em) < VIDA_MINIMA;
                tracing::warn!(dispositivo = %a.nome, "saída de áudio reiniciada: {m}");
                // Fechar primeiro: o callback antigo devolve o mixer antes do novo stream nascer.
                self.stream = None;
                self.proxima_tentativa = if curto { agora + ESPERA_REABRIR } else { agora };
            }
        }
        if self.stream.is_none() && agora >= self.proxima_tentativa {
            match self.abrir() {
                Ok(a) => {
                    tracing::info!(dispositivo = %a.nome, "saída de áudio aberta");
                    self.avisou_sem_saida = false;
                    self.proxima_conferencia = agora + CONFERIR_PADRAO;
                    self.stream = Some(a);
                }
                Err(e) => {
                    if !self.avisou_sem_saida {
                        tracing::warn!("saída de áudio indisponível, tentando de novo: {e}");
                        self.avisou_sem_saida = true;
                    }
                    self.proxima_tentativa = agora + ESPERA_REABRIR;
                }
            }
        }
    }

    /// Abre um stream no dispositivo padrão de agora, com o mixer emprestado.
    fn abrir(&self) -> Result<Aberto, AudioError> {
        let id_padrao = default_output_id();
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let nome = device.name().unwrap_or_else(|_| "saída padrão".into());
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

        let falhou = Arc::new(AtomicBool::new(false));
        let batidas = Arc::new(AtomicU64::new(0));
        let emprestimo = Emprestimo {
            core: None,
            casa_tx: self.casa_tx.clone(),
            casa_rx: self.casa_rx.clone(),
            batidas: batidas.clone(),
        };
        let err_fn = {
            let falhou = falhou.clone();
            move |e| {
                tracing::error!("erro no stream de saída: {e}");
                falhou.store(true, Ordering::Relaxed);
            }
        };

        let stream = if taxa == RATE && canais == CHANNELS {
            let mut e = emprestimo;
            device.build_output_stream::<f32, _, _>(&config, move |data, _| e.render(data), err_fn, None)
        } else {
            let mut conv = Conversor::novo(emprestimo, taxa, canais)?;
            device.build_output_stream::<f32, _, _>(&config, move |data, _| conv.preencher(data), err_fn, None)
        }
        .map_err(|e| AudioError::Backend(format!("abrir stream: {e}")))?;

        stream
            .play()
            .map_err(|e| AudioError::Backend(format!("iniciar stream: {e}")))?;
        let agora = Instant::now();
        Ok(Aberto {
            _stream: stream,
            nome,
            id_padrao,
            falhou,
            batidas,
            ultima_batida: (0, agora),
            aberto_em: agora,
        })
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

    #[test]
    fn emprestimo_devolve_o_mixer_ao_cair() {
        let (h, core) = out_mixer::new_mixer();
        let (casa_tx, casa_rx) = crossbeam_channel::bounded::<MixerCore>(1);
        casa_tx.try_send(core).expect("casa vazia");
        let novo = |batidas: &Arc<AtomicU64>| Emprestimo {
            core: None,
            casa_tx: casa_tx.clone(),
            casa_rx: casa_rx.clone(),
            batidas: batidas.clone(),
        };
        let batidas = Arc::new(AtomicU64::new(0));
        let mut t = h.add_track(1);
        let mut a = novo(&batidas);
        t.push(&[0.25; 960]);
        let mut buf = [0.0f32; 960];
        a.render(&mut buf);
        assert!(
            buf.iter().all(|&x| (x - 0.25).abs() < 1e-6),
            "o primeiro stream toca a trilha"
        );
        assert!(casa_rx.is_empty(), "o mixer está emprestado");

        // Um segundo stream ao mesmo tempo fica em silêncio (o mixer não se divide).
        let mut b = novo(&batidas);
        t.push(&[0.25; 960]);
        b.render(&mut buf);
        assert!(buf.iter().all(|&x| x == 0.0));

        // O primeiro cai: o mixer volta para casa e o segundo continua a mesma trilha.
        drop(a);
        b.render(&mut buf);
        assert!(
            buf.iter().all(|&x| (x - 0.25).abs() < 1e-6),
            "a trilha sobreviveu à troca"
        );
        assert_eq!(batidas.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn reabre_quando_o_dispositivo_falha() {
        let mut out = match AudioOut::start() {
            Ok(o) if o.is_open() => o,
            _ => {
                println!("sem saida de audio, pulando");
                return;
            }
        };
        if let Some(a) = out.stream.as_ref() {
            a.falhou.store(true, Ordering::Relaxed);
        }
        out.maintain();
        assert!(!out.is_open(), "fechou o stream que falhou");
        // Acabou de abrir: espera um pouco antes de reabrir; aqui adianta a hora.
        out.proxima_tentativa = Instant::now();
        out.maintain();
        assert!(out.is_open(), "reabriu");
        assert!(out.device_name().is_some());
    }
}
