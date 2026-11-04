//! Roteia os `MediaEvent` de uma conexão para o tratador da trilha.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc::Receiver;
use zap_net::media_rx::MediaEvent;

pub trait TrackHandler: Send + Sync {
    fn on_event(&self, ev: MediaEvent);
}

#[derive(Default)]
pub struct Router {
    handlers: Mutex<HashMap<u16, Arc<dyn TrackHandler>>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }
    /// Registra o tratador da trilha; se já havia um, ele é substituído.
    pub fn register(&self, track: u16, h: Arc<dyn TrackHandler>) {
        self.handlers.lock().insert(track, h);
    }
    pub fn unregister(&self, track: u16) {
        self.handlers.lock().remove(&track);
    }
    /// Entrega ao tratador de `header.track` (Video/Audio) ou `track` (Lost). Sem tratador: descarta.
    pub fn dispatch(&self, ev: MediaEvent) {
        let track = match &ev {
            MediaEvent::Video { header, .. } => header.track,
            MediaEvent::Audio { header, .. } => header.track,
            MediaEvent::Lost { track, .. } => *track,
        };
        // Clona o Arc e solta o lock antes de chamar o tratador: assim ele pode
        // se (des)registrar sem travar o roteador.
        let h = self.handlers.lock().get(&track).cloned();
        if let Some(h) = h {
            h.on_event(ev);
        }
    }
    /// Consome o canal até fechar chamando `dispatch`.
    pub async fn run(self: Arc<Self>, mut rx: Receiver<MediaEvent>) {
        while let Some(ev) = rx.recv().await {
            self.dispatch(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use zap_proto::media::{AudioHeader, VideoFlags, VideoHeader};

    /// Tratador falso: guarda os eventos recebidos.
    #[derive(Default)]
    struct Guarda(Mutex<Vec<MediaEvent>>);

    impl TrackHandler for Guarda {
        fn on_event(&self, ev: MediaEvent) {
            self.0.lock().push(ev);
        }
    }

    fn video(track: u16) -> MediaEvent {
        MediaEvent::Video {
            header: VideoHeader {
                flags: VideoFlags(0),
                track,
                layer: 0,
                width: 320,
                height: 240,
                seq: 1,
                capture_us: 0,
                send_us: 0,
                size: 0,
            },
            data: Bytes::new(),
            arrival_us: 0,
        }
    }

    fn audio(track: u16) -> MediaEvent {
        MediaEvent::Audio {
            header: AudioHeader {
                track,
                seq: 1,
                capture_us: 0,
                send_us: 0,
            },
            opus: Bytes::new(),
            arrival_us: 0,
        }
    }

    fn lost(track: u16) -> MediaEvent {
        MediaEvent::Lost {
            track,
            layer: 0,
            seq: 1,
        }
    }

    #[test]
    fn despacha_video_audio_lost_para_a_trilha_certa() {
        let r = Router::new();
        let a = Arc::new(Guarda::default());
        let b = Arc::new(Guarda::default());
        r.register(1, a.clone());
        r.register(2, b.clone());

        r.dispatch(video(1));
        r.dispatch(audio(1));
        r.dispatch(lost(1));
        r.dispatch(video(2));

        let va = a.0.lock();
        assert_eq!(va.len(), 3);
        assert!(matches!(va[0], MediaEvent::Video { .. }));
        assert!(matches!(va[1], MediaEvent::Audio { .. }));
        assert!(matches!(va[2], MediaEvent::Lost { track: 1, .. }));
        drop(va);

        let vb = b.0.lock();
        assert_eq!(vb.len(), 1);
        assert!(matches!(vb[0], MediaEvent::Video { .. }));
    }

    #[test]
    fn trilha_sem_tratador_descarta() {
        let r = Router::new();
        let a = Arc::new(Guarda::default());
        r.register(1, a.clone());

        r.dispatch(video(9));
        r.dispatch(audio(9));
        r.dispatch(lost(9));

        assert!(a.0.lock().is_empty());
    }

    #[test]
    fn unregister_para_de_entregar() {
        let r = Router::new();
        let a = Arc::new(Guarda::default());
        r.register(1, a.clone());

        r.dispatch(video(1));
        r.unregister(1);
        r.dispatch(video(1));

        assert_eq!(a.0.lock().len(), 1);
    }

    #[test]
    fn register_substitui() {
        let r = Router::new();
        let antigo = Arc::new(Guarda::default());
        let novo = Arc::new(Guarda::default());
        r.register(1, antigo.clone());
        r.register(1, novo.clone());

        r.dispatch(video(1));

        assert!(antigo.0.lock().is_empty());
        assert_eq!(novo.0.lock().len(), 1);
    }

    #[tokio::test]
    async fn run_consome_ate_fechar() {
        let r = Arc::new(Router::new());
        let a = Arc::new(Guarda::default());
        r.register(1, a.clone());

        let (tx, rx) = tokio::sync::mpsc::channel(8);
        tx.send(video(1)).await.unwrap();
        tx.send(audio(1)).await.unwrap();
        tx.send(lost(1)).await.unwrap();
        drop(tx);

        r.clone().run(rx).await;

        assert_eq!(a.0.lock().len(), 3);
    }
}
