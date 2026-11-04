//! Núcleo do mixer de saída (portável): lista de trilhas, volume mestre, comandos sem lock.

use crate::mix;
use crate::out_track::TrackReader;
use crossbeam_channel::TrySendError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Máximo de trilhas simultâneas; o Vec é pré-alocado para não alocar no callback.
const MAX_TRILHAS: usize = 64;
/// Capacidade do canal de comandos.
const CAP_COMANDOS: usize = 256;
/// Tentativas de envio quando o canal está cheio.
const TENTATIVAS: usize = 100;

pub(crate) enum MixerCmd {
    Add(u64, TrackReader),
    Remove(u64),
}

/// Lado de controle (qualquer thread). Envia comandos por canal limitado.
#[derive(Clone)]
pub struct MixerHandle {
    pub(crate) tx: crossbeam_channel::Sender<MixerCmd>,
    pub(crate) master_bits: Arc<AtomicU32>,
}

/// Lado do callback de áudio. `render` não aloca nem trava.
pub struct MixerCore {
    pub(crate) rx: crossbeam_channel::Receiver<MixerCmd>,
    pub(crate) master_bits: Arc<AtomicU32>,
    pub(crate) tracks: Vec<(u64, TrackReader)>,
    /// Último volume mestre aplicado (início da rampa do próximo bloco).
    last_master: f32,
}

pub fn new_mixer() -> (MixerHandle, MixerCore) {
    let (tx, rx) = crossbeam_channel::bounded::<MixerCmd>(CAP_COMANDOS);
    let master_bits = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    (
        MixerHandle {
            tx,
            master_bits: master_bits.clone(),
        },
        MixerCore {
            rx,
            master_bits,
            tracks: Vec::with_capacity(MAX_TRILHAS),
            last_master: 1.0,
        },
    )
}

impl MixerHandle {
    /// Envia um comando; se o canal estiver cheio, tenta de novo algumas vezes antes de descartar.
    fn enviar(&self, mut cmd: MixerCmd) {
        for _ in 0..TENTATIVAS {
            match self.tx.try_send(cmd) {
                Ok(()) => return,
                Err(TrySendError::Full(c)) => {
                    cmd = c;
                    std::thread::yield_now();
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        tracing::warn!("canal do mixer cheio: comando descartado");
    }

    /// Cria a trilha e a registra no mixer. `id` repetido substitui a anterior.
    pub fn add_track(&self, id: u64) -> crate::TrackInput {
        let (input, reader) = crate::out_track::new_track();
        self.enviar(MixerCmd::Add(id, reader));
        input
    }
    pub fn remove_track(&self, id: u64) {
        self.enviar(MixerCmd::Remove(id));
    }
    /// Volume mestre linear 0.0..=2.0.
    pub fn set_master_volume(&self, v: f32) {
        let v = if v.is_nan() { 1.0 } else { v.clamp(0.0, 2.0) };
        self.master_bits.store(v.to_bits(), Ordering::Relaxed);
    }
}

impl MixerCore {
    /// Preenche `out` (estéreo intercalado @48 kHz): aplica comandos pendentes, soma as trilhas,
    /// aplica o volume mestre e limita a ±1.0.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);

        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                MixerCmd::Add(id, reader) => {
                    if let Some(slot) = self.tracks.iter_mut().find(|(i, _)| *i == id) {
                        slot.1 = reader;
                    } else if self.tracks.len() < MAX_TRILHAS {
                        self.tracks.push((id, reader));
                    } else {
                        tracing::warn!(id, "mixer cheio: trilha ignorada");
                    }
                }
                MixerCmd::Remove(id) => {
                    if let Some(pos) = self.tracks.iter().position(|(i, _)| *i == id) {
                        self.tracks.swap_remove(pos);
                    }
                }
            }
        }

        for (_, reader) in self.tracks.iter_mut() {
            reader.render_add(out);
        }

        let alvo = f32::from_bits(self.master_bits.load(Ordering::Relaxed));
        let m0 = self.last_master;
        if (alvo - m0).abs() < 1e-6 && (alvo - 1.0).abs() < 1e-6 {
            // ganho unitário estável: nada a fazer
        } else {
            let n = (out.len() / 2).max(1);
            for (i, frame) in out.chunks_mut(2).enumerate() {
                let g = m0 + (alvo - m0) * ((i + 1) as f32 / n as f32);
                for s in frame {
                    *s *= g;
                }
            }
        }
        self.last_master = alvo;

        mix::limit(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 960;

    #[test]
    fn duas_trilhas_somam() {
        let (h, mut core) = new_mixer();
        let mut a = h.add_track(1);
        let mut b = h.add_track(2);
        a.push(&vec![0.2; N]);
        b.push(&vec![0.2; N]);
        let mut out = vec![1.0; N];
        core.render(&mut out);
        assert!(out.iter().all(|&x| (x - 0.4).abs() < 1e-5));
    }

    #[test]
    fn remove_trilha_silencia() {
        let (h, mut core) = new_mixer();
        let mut a = h.add_track(1);
        a.push(&vec![0.2; N]);
        let mut out = vec![0.0; N];
        core.render(&mut out);
        assert!(out[N - 1] > 0.1);
        a.push(&vec![0.2; N]);
        h.remove_track(1);
        core.render(&mut out);
        assert!(out.iter().all(|&x| x == 0.0));
        assert!(core.tracks.is_empty());
    }

    #[test]
    fn master_volume_escala() {
        let (h, mut core) = new_mixer();
        let mut a = h.add_track(1);
        h.set_master_volume(0.5);
        a.push(&vec![0.4; N]);
        let mut out = vec![0.0; N];
        core.render(&mut out);
        assert!((out[N - 1] - 0.2).abs() < 1e-5);
        assert!(out[0] > 0.2);
        // próximo bloco já estável em 0.5
        a.push(&vec![0.4; N]);
        core.render(&mut out);
        assert!(out.iter().all(|&x| (x - 0.2).abs() < 1e-5));
    }

    #[test]
    fn master_volume_clamp() {
        let (h, _core) = new_mixer();
        h.set_master_volume(9.0);
        assert_eq!(f32::from_bits(h.master_bits.load(Ordering::Relaxed)), 2.0);
        h.set_master_volume(f32::NAN);
        assert_eq!(f32::from_bits(h.master_bits.load(Ordering::Relaxed)), 1.0);
    }

    #[test]
    fn limita_em_1() {
        let (h, mut core) = new_mixer();
        let mut ins: Vec<_> = (0..3).map(|i| h.add_track(i)).collect();
        for t in ins.iter_mut() {
            t.push(&vec![1.0; N]);
        }
        let mut out = vec![0.0; N];
        core.render(&mut out);
        assert!(out.iter().all(|&x| x == 1.0));
    }

    #[test]
    fn id_repetido_substitui() {
        let (h, mut core) = new_mixer();
        let mut a = h.add_track(7);
        let mut b = h.add_track(7);
        a.push(&vec![0.3; N]);
        b.push(&vec![0.1; N]);
        let mut out = vec![0.0; N];
        core.render(&mut out);
        assert_eq!(core.tracks.len(), 1);
        assert!(out.iter().all(|&x| (x - 0.1).abs() < 1e-5));
    }

    #[test]
    fn sem_trilhas_zero() {
        let (_h, mut core) = new_mixer();
        let mut out = vec![0.7; N];
        core.render(&mut out);
        assert!(out.iter().all(|&x| x == 0.0));
    }
}
