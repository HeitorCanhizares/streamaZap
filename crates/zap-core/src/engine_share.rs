//! `impl Core`: compartilhamento (delegam a `ShareSession`, saltando para o runtime do núcleo,
//! e depois atualizam `UiState.share` + `notifier.touch()`).

use std::sync::Arc;

use crate::engine::{Core, Shared};
use crate::share::ShareSession;
use crate::types::*;
use crate::CoreError;

/// Depois de cada operação (inclusive as que falharam): registra o erro, atualiza
/// `UiState.share` e avisa a interface. Os dois locks nunca são segurados juntos: primeiro o
/// `async` do `ShareSession` (vira `view()` owned), depois o do estado.
async fn fecha_share(shared: &Arc<Shared>, r: &Result<(), CoreError>) {
    if let Err(e) = r {
        shared.state.lock().push_error(e.to_string());
    }
    let view = shared.share.lock().await.view();
    shared.state.lock().set_share(view);
    shared.notifier.touch();
}

impl Core {
    pub async fn start_share(&self, req: ShareRequest) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        let r = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::start(&shared, req).await }
            })
            .await
            .unwrap_or(Err(CoreError::Stopped));
        fecha_share(&shared, &r).await;
        r
    }

    pub async fn update_share(&self, req: ShareRequest) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        let r = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::update(&shared, req).await }
            })
            .await
            .unwrap_or(Err(CoreError::Stopped));
        fecha_share(&shared, &r).await;
        r
    }

    pub async fn stop_share(&self) {
        let shared = self.shared.clone();
        let _ = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::stop(&shared).await }
            })
            .await;
        fecha_share(&shared, &Ok(())).await;
    }

    pub async fn set_camera(&self, on: bool) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        let r = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::set_camera(&shared, on).await }
            })
            .await
            .unwrap_or(Err(CoreError::Stopped));
        fecha_share(&shared, &r).await;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;
    use crate::types::{AudioChoice, CreateRoom, Preset};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use zap_proto::RoomMode;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("zapcore-share-{}", uuid::Uuid::new_v4()))
    }
    fn opts() -> CoreOptions {
        CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        }
    }
    fn sala() -> CreateRoom {
        CreateRoom {
            name: "Sala Share".into(),
            mode: RoomMode::Local,
            password: None,
        }
    }
    fn pedido_sintetico() -> ShareRequest {
        ShareRequest {
            source: "synthetic:1920x1080".into(),
            audio: AudioChoice::Keyword("none".into()),
            preset: Preset::P1080p60,
            encoder: "auto".into(),
        }
    }
    fn novo() -> (Core, PathBuf) {
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).expect("core");
        (core, d)
    }

    /// `Core::drop` faz `block_on`: fora do contexto assíncrono do teste.
    async fn encerrar(core: Core, d: PathBuf) {
        let _ = tokio::task::spawn_blocking(move || drop(core)).await;
        let _ = std::fs::remove_dir_all(d);
    }

    /// Espera (até 1 s) o número de trilhas da sala no snapshot chegar a `n`.
    fn espera_trilhas(core: &Core, n: usize) -> bool {
        let fim = Instant::now() + Duration::from_secs(1);
        loop {
            let atual = core.snapshot().room.as_ref().map_or(0, |r| r.tracks.len());
            if atual == n {
                return true;
            }
            if Instant::now() >= fim {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_share_sintetico_aparece_no_snapshot() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_sintetico()).await.expect("iniciar share");
        assert!(core.snapshot().share.is_some());
        assert!(espera_trilhas(&core, 1));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_share_limpa() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_sintetico()).await.expect("iniciar share");
        core.stop_share().await;
        assert!(core.snapshot().share.is_none());
        assert!(espera_trilhas(&core, 0));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_share_sem_sala_erro_no_room() {
        let (core, d) = novo();
        let r = core.start_share(pedido_sintetico()).await;
        assert!(matches!(r, Err(CoreError::NoRoom)));
        let snap = core.snapshot();
        assert!(snap.share.is_none());
        assert!(!snap.errors.is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_camera_sintetica() {
        let (core, d) = novo();
        core.update_settings(|s| s.video.camera_id = Some("synthetic:1280x720".into()));
        core.create_room(sala()).await.expect("criar sala");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 1));
        let _ = core.set_camera(false).await;
        assert!(espera_trilhas(&core, 0));
        encerrar(core, d).await;
    }
}
