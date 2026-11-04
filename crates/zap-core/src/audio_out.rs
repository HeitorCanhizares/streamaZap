//! Saída de áudio. `zap_audio::AudioOut` não é `Send` (cpal): no Windows vive numa thread
//! dedicada que recebe comandos; fora do Windows não há saída (stub que descarta o som).

use zap_audio::TrackInput;

#[derive(Clone)]
pub struct AudioOutHandle {
    tx: Option<crossbeam_channel::Sender<AudioCmd>>,
}

pub(crate) enum AudioCmd {
    Add(u64, std::sync::mpsc::Sender<TrackInput>),
    Remove(u64),
    Master(f32),
}

impl AudioOutHandle {
    /// Windows: sobe a thread "zap-audio-out" que cria `AudioOut::start()`; se falhar (sem
    /// dispositivo), loga e age como stub. Fora do Windows: stub.
    pub fn start() -> Self {
        #[cfg(windows)]
        {
            Self::start_windows()
        }
        #[cfg(not(windows))]
        {
            Self { tx: None }
        }
    }

    #[cfg(windows)]
    fn start_windows() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<AudioCmd>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<bool>();
        let spawned = std::thread::Builder::new().name("zap-audio-out".into()).spawn(move || {
            // O AudioOut não é Send: nasce e morre nesta thread.
            let out = match zap_audio::AudioOut::start() {
                Ok(o) => {
                    let _ = ready_tx.send(true);
                    o
                }
                Err(e) => {
                    tracing::warn!("saida de audio indisponivel, modo stub: {e}");
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            // Termina quando todos os Sender caem; o AudioOut vive até lá.
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCmd::Add(id, reply) => {
                        let _ = reply.send(out.add_track(id));
                    }
                    AudioCmd::Remove(id) => out.remove_track(id),
                    AudioCmd::Master(v) => out.set_master_volume(v),
                }
            }
        });
        match spawned {
            Ok(_) if ready_rx.recv().unwrap_or(false) => Self { tx: Some(tx) },
            Ok(_) => Self { tx: None },
            Err(e) => {
                tracing::warn!("falha ao criar thread de audio, modo stub: {e}");
                Self { tx: None }
            }
        }
    }

    /// Entrada de uma trilha de reprodução. Stub: `zap_audio::out_track::new_track().0`.
    pub fn add_track(&self, id: u64) -> TrackInput {
        if let Some(tx) = &self.tx {
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            if tx.send(AudioCmd::Add(id, reply_tx)).is_ok() {
                if let Ok(input) = reply_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    return input;
                }
            }
        }
        zap_audio::out_track::new_track().0
    }

    pub fn remove_track(&self, id: u64) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(AudioCmd::Remove(id));
        }
    }

    pub fn set_master_volume(&self, v: f32) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(AudioCmd::Master(v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub() -> AudioOutHandle {
        AudioOutHandle { tx: None }
    }

    #[test]
    fn stub_add_track_devolve_entrada_que_aceita_push() {
        let h = stub();
        let mut t = h.add_track(1);
        let _ = t.push(&[0.0; 1920]);
    }

    #[test]
    fn stub_remove_e_master_nao_panicam() {
        let h = stub();
        h.remove_track(1);
        h.set_master_volume(0.5);
    }

    #[test]
    fn handle_e_send_sync() {
        fn ok<T: Send + Sync + Clone>() {}
        ok::<AudioOutHandle>();
    }

    #[test]
    fn clone_compartilha_o_mesmo_canal() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let a = AudioOutHandle { tx: Some(tx) };
        let b = a.clone();
        a.remove_track(1);
        b.set_master_volume(0.2);
        assert!(matches!(rx.try_recv(), Ok(AudioCmd::Remove(1))));
        assert!(matches!(rx.try_recv(), Ok(AudioCmd::Master(_))));
    }
}
