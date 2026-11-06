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

    /// Troca a qualidade da câmera (à parte da tela): grava `settings.video.camera_preset` e, com a
    /// câmera ligada, aplica ao vivo (`ShareSession::set_camera_quality`). Sem câmera ligada só grava.
    pub async fn set_camera_quality(&self, preset: Preset) -> Result<(), CoreError> {
        self.update_settings(|s| s.video.camera_preset = preset);
        let shared = self.shared.clone();
        let r = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::set_camera_quality(&shared, preset).await }
            })
            .await
            .unwrap_or(Err(CoreError::Stopped));
        fecha_share(&shared, &r).await;
        r
    }

    /// Troca o jeito da câmera (canto ou solta): grava `settings.video.camera_layout` e, com a
    /// câmera ligada, manda `trilha.atualizar` só com o campo `camera`.
    pub async fn set_camera_layout(&self, layout: zap_proto::CameraLayout) -> Result<(), CoreError> {
        self.update_settings(|s| s.video.camera_layout = layout);
        let shared = self.shared.clone();
        let r = shared
            .rt
            .clone()
            .spawn({
                let shared = shared.clone();
                async move { ShareSession::set_camera_layout(&shared, layout).await }
            })
            .await
            .unwrap_or(Err(CoreError::Stopped));
        fecha_share(&shared, &r).await;
        r
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
    async fn so_audio_sem_som_e_invalido() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        let r = core
            .start_share(ShareRequest {
                source: crate::share::SO_AUDIO.into(),
                ..pedido_sintetico()
            })
            .await;
        assert!(matches!(r, Err(CoreError::Invalid(_))));
        assert!(core.snapshot().share.is_none());
        encerrar(core, d).await;
    }

    /// Trocar para "só o som" sem escolher o som é recusado e mantém a tela no ar.
    #[tokio::test(flavor = "multi_thread")]
    async fn update_para_so_audio_sem_som_mantem_a_tela() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_sintetico()).await.expect("iniciar share");
        let r = core
            .update_share(ShareRequest {
                source: crate::share::SO_AUDIO.into(),
                ..pedido_sintetico()
            })
            .await;
        assert!(matches!(r, Err(CoreError::Invalid(_))));
        let share = core.snapshot().share.clone().expect("continua transmitindo");
        assert_eq!(share.source, "synthetic:1920x1080");
        assert_eq!(share.request.map(|q| q.source).as_deref(), Some("synthetic:1920x1080"));
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

    // ---- Testes de aceite B1-B4 (câmera: qualidade, webcam como fonte, jeito) ----

    use crate::snapshot::TrackViewKind;
    use zap_proto::{CameraLayout, Corner, Track, TrackKind};

    fn pedido_webcam(audio: AudioChoice) -> ShareRequest {
        ShareRequest {
            source: "camera:synthetic:1280x720".into(),
            audio,
            preset: Preset::P720p30,
            encoder: "auto".into(),
        }
    }

    fn sem_som() -> AudioChoice {
        AudioChoice::Keyword("none".into())
    }

    /// Trilhas do modelo do dono (fonte da verdade da sala Local).
    fn trilhas(core: &Core) -> Vec<Track> {
        match core.shared.current_room().as_deref() {
            Some(crate::room::ActiveRoom::Owner(o)) => o.model.lock().tracks.clone(),
            _ => Vec::new(),
        }
    }

    fn de_tipo(core: &Core, k: TrackKind) -> Vec<Track> {
        trilhas(core).into_iter().filter(|t| t.kind == k).collect()
    }

    fn camera_sintetica(core: &Core, preset: Preset) {
        core.update_settings(|s| {
            s.video.camera_id = Some("synthetic:1920x1080".into());
            s.video.camera_preset = preset;
        });
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn criar_sala_sem_transmitir_depois_start_share_publica() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        assert!(core.snapshot().share.is_none());
        assert!(trilhas(&core).is_empty());
        core.start_share(pedido_sintetico()).await.expect("iniciar share");
        assert!(core.snapshot().share.is_some());
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Screen).len(), 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn camera_1080p30_com_tela_720p30_publica_3_camadas() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P1080p30);
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(ShareRequest {
            preset: Preset::P720p30,
            ..pedido_sintetico()
        })
        .await
        .expect("iniciar share");
        let escada_tela = de_tipo(&core, TrackKind::Screen)[0].layers.clone();
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 2));
        let cam = de_tipo(&core, TrackKind::Camera);
        assert_eq!(cam.len(), 1);
        assert_eq!(cam[0].layers.len(), 3);
        assert_eq!((cam[0].layers[0].width, cam[0].layers[0].height), (1920, 1080));
        assert_eq!(de_tipo(&core, TrackKind::Screen)[0].layers, escada_tela);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn trocar_qualidade_da_camera_nao_mexe_na_tela() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P1080p30);
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(ShareRequest {
            preset: Preset::P720p30,
            ..pedido_sintetico()
        })
        .await
        .expect("iniciar share");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 2));
        let cam0 = de_tipo(&core, TrackKind::Camera)[0].clone();
        let tela0 = de_tipo(&core, TrackKind::Screen)[0].clone();

        core.set_camera_quality(Preset::P720p30).await.expect("qualidade");

        let cam1 = de_tipo(&core, TrackKind::Camera);
        let tela1 = de_tipo(&core, TrackKind::Screen);
        assert_eq!(cam1.len(), 1);
        assert_eq!(cam1[0].id, cam0.id);
        assert_eq!(tela1[0].id, tela0.id);
        assert_eq!((cam1[0].layers[0].width, cam1[0].layers[0].height), (1280, 720));
        assert_eq!(tela1[0].layers, tela0.layers);
        assert_eq!(core.settings().video.camera_preset, Preset::P720p30);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn qualidade_sem_camera_so_grava() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.set_camera_quality(Preset::P1080p60).await.expect("qualidade");
        assert_eq!(core.settings().video.camera_preset, Preset::P1080p60);
        assert!(trilhas(&core).is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn webcam_como_fonte_publica_camera_e_nenhuma_tela() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_webcam(sem_som()))
            .await
            .expect("iniciar webcam");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);
        assert!(de_tipo(&core, TrackKind::Screen).is_empty());
        let snap = core.snapshot();
        let room = snap.room.as_ref().expect("sala");
        assert_eq!(room.tracks[0].kind, TrackViewKind::Camera);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn webcam_mais_som() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_webcam(AudioChoice::Keyword("all".into())))
            .await
            .expect("iniciar webcam com som");
        assert!(espera_trilhas(&core, 2));
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);
        assert_eq!(de_tipo(&core, TrackKind::Audio).len(), 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn tecla_c_desliga_a_camera_da_webcam() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_webcam(sem_som()))
            .await
            .expect("iniciar webcam");
        assert!(espera_trilhas(&core, 1));
        core.set_camera(false).await.expect("desligar");
        assert!(espera_trilhas(&core, 0));
        assert!(de_tipo(&core, TrackKind::Camera).is_empty());
        core.set_camera(true).await.expect("religar");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn webcam_e_minha_camera_juntas_nao_duplicam() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P720p30);
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_webcam(sem_som()))
            .await
            .expect("iniciar webcam");
        assert!(espera_trilhas(&core, 1));
        core.set_camera(true).await.expect("ligar camera");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn trocar_tela_por_webcam_remove_a_tela() {
        let (core, d) = novo();
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_sintetico()).await.expect("iniciar tela");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Screen).len(), 1);

        core.update_share(pedido_webcam(sem_som()))
            .await
            .expect("trocar para webcam");
        assert!(espera_trilhas(&core, 1));
        assert!(de_tipo(&core, TrackKind::Screen).is_empty());
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);

        // O inverso: da webcam de volta para a tela.
        core.update_share(pedido_sintetico()).await.expect("trocar para tela");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Screen).len(), 1);
        assert!(de_tipo(&core, TrackKind::Camera).is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn minha_camera_ligada_e_depois_webcam_como_fonte() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P720p30);
        core.create_room(sala()).await.expect("criar sala");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 1));
        core.start_share(pedido_webcam(sem_som()))
            .await
            .expect("iniciar webcam");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(de_tipo(&core, TrackKind::Camera).len(), 1);
        assert!(de_tipo(&core, TrackKind::Screen).is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn camera_publica_com_o_jeito_das_configuracoes() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P720p30);
        core.create_room(sala()).await.expect("criar sala");
        // Sem configurar nada: o padrão.
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(
            de_tipo(&core, TrackKind::Camera)[0].camera_layout,
            Some(CameraLayout::default())
        );
        core.set_camera(false).await.expect("desligar");
        assert!(espera_trilhas(&core, 0));

        let canto = CameraLayout::Corner {
            corner: Corner::TopLeft,
        };
        core.set_camera_layout(canto).await.expect("jeito");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 1));
        assert_eq!(de_tipo(&core, TrackKind::Camera)[0].camera_layout, Some(canto));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn trocar_jeito_ao_vivo_nao_republica() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P720p30);
        core.create_room(sala()).await.expect("criar sala");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 1));
        let id0 = de_tipo(&core, TrackKind::Camera)[0].id.clone();

        core.set_camera_layout(CameraLayout::Free).await.expect("jeito");

        let cam = de_tipo(&core, TrackKind::Camera);
        assert_eq!(cam.len(), 1);
        assert_eq!(cam[0].id, id0);
        assert_eq!(cam[0].camera_layout, Some(CameraLayout::Free));
        // O snapshot é atualizado pelo tick da sala: espera (até 1 s) refletir o novo jeito.
        let fim = Instant::now() + Duration::from_secs(1);
        let na_view = loop {
            let atual = core
                .snapshot()
                .room
                .as_ref()
                .and_then(|r| r.tracks.iter().find(|t| t.id == id0).map(|t| t.camera_layout));
            if atual == Some(Some(CameraLayout::Free)) || Instant::now() >= fim {
                break atual;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(na_view, Some(Some(CameraLayout::Free)));
        assert_eq!(core.settings().video.camera_layout, CameraLayout::Free);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn trilha_de_tela_nunca_leva_camera_layout() {
        let (core, d) = novo();
        camera_sintetica(&core, Preset::P720p30);
        core.create_room(sala()).await.expect("criar sala");
        core.start_share(pedido_sintetico()).await.expect("iniciar tela");
        core.set_camera(true).await.expect("ligar camera");
        assert!(espera_trilhas(&core, 2));
        core.set_camera_layout(CameraLayout::Free).await.expect("jeito");
        for t in trilhas(&core) {
            if t.kind != TrackKind::Camera {
                assert_eq!(t.camera_layout, None, "trilha {:?}", t.kind);
            }
        }
        let snap = core.snapshot();
        let room = snap.room.as_ref().expect("sala");
        for v in room.tracks.iter().filter(|v| v.kind != TrackViewKind::Camera) {
            assert_eq!(v.camera_layout, None);
        }
        encerrar(core, d).await;
    }
}
