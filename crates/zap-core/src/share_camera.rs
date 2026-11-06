//! Câmera do compartilhamento: liga/desliga, qualidade ao vivo e troca de dispositivo.
//! `impl ShareSession` (a struct vive em `share.rs`; o jeito da câmera em `share_camera_layout.rs`).

use std::sync::Arc;

use zap_media::VideoPipelineConfig;
use zap_proto::TrackKind;

use crate::camera_plan::{plan_from_modes, plan_synthetic, same_capture, CameraPlan};
use crate::engine::Shared;
use crate::publish::{PublishedTrack, VideoPublisher};
use crate::room::TrackSpec;
use crate::share::{camera_source, PublishedVideo, ShareSession};
use crate::sources::to_video_spec;
use crate::types::{Preset, ShareRequest, SourceRef};
use crate::util::new_id;
use crate::CoreError;

/// O que a câmera ligada está usando.
#[derive(Clone, Debug)]
pub struct CameraRuntime {
    /// Fonte resolvida (`SourceRef::Camera(id)` ou `SourceRef::Synthetic`).
    pub src: SourceRef,
    /// Modo de captura previsto e escada publicada.
    pub plan: CameraPlan,
    /// Modos nativos lidos antes de abrir a captura. Com a câmera aberta, ler de novo pode dar
    /// `CameraBusy`, e a troca de qualidade ao vivo cairia na escada de reserva (720p30).
    pub modes: Vec<zap_capture::CameraMode>,
}

impl ShareSession {
    /// Liga/desliga a câmera (botão "Minha câmera", tecla C). Desligar para só a trilha da câmera.
    /// Ligar com a câmera já ligada é no-op. Com a webcam como fonte do pedido
    /// (`request.source == "camera:<id>"`), liga esse mesmo dispositivo.
    pub async fn set_camera(shared: &Arc<Shared>, on: bool) -> Result<(), CoreError> {
        let mut sessao = shared.share.lock().await;
        if !on {
            Self::stop_camera_locked(shared, &mut sessao).await;
            return Ok(());
        }
        if sessao.camera.is_some() {
            return Ok(());
        }
        let device = webcam_device(sessao.request.as_ref());
        Self::start_camera_locked(shared, &mut sessao, device).await
    }

    /// Para a câmera (publicador, registro no `MediaServer`, trilha na sala). No-op sem câmera.
    pub(crate) async fn stop_camera_locked(shared: &Arc<Shared>, sessao: &mut ShareSession) {
        sessao.camera_info = None;
        if let Some(c) = sessao.camera.take() {
            let ms = shared.media_server.lock().await.clone();
            c.publisher.stop();
            Self::remover_trilhas(ms, shared.current_room(), vec![c.track.id]).await;
        }
    }

    /// Publica a trilha `camera` e abre a captura. Exige sala (`NoRoom`). `device`: id do
    /// dispositivo (sem o prefixo `camera:`; `synthetic:LxA` vale) ou `None` para usar
    /// `settings.video.camera_id`. Guarda o resultado em `sessao.camera` e `sessao.camera_info`.
    pub(crate) async fn start_camera_locked(
        shared: &Arc<Shared>,
        sessao: &mut ShareSession,
        device: Option<String>,
    ) -> Result<(), CoreError> {
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let (camera_id, encoder_pref, interface, preset, layout) = {
            let s = shared.settings.read();
            (
                device.or_else(|| s.video.camera_id.clone()),
                s.video.encoder,
                s.local.interface.clone(),
                s.video.camera_preset,
                s.video.camera_layout,
            )
        };
        let src = camera_source(camera_id)?;
        let modes = modes_for(&src);
        let plan = plan_with(&src, &modes, preset);
        let ms = shared.ensure_media_server().await?;
        let track = room
            .publish_track(TrackSpec {
                local_id: new_id("l_"),
                kind: TrackKind::Camera,
                title: "Câmera".into(),
                layers: plan.ladder.clone(),
                audio: None,
                address: Some(ms.address(interface.as_deref(), shared.options.include_loopback)),
                camera_layout: Some(layout),
            })
            .await?;
        let publisher = match VideoPublisher::start(
            VideoPipelineConfig {
                track: track.number,
                source: to_video_spec(&src),
                ladder: plan.ladder.clone(),
                encoder: encoder_pref.to_spec(),
            },
            shared.rt.clone(),
            shared.clock.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                let _ = room.remove_track(&track.id).await;
                return Err(e);
            }
        };
        ms.register(track.id.clone(), PublishedTrack::Video(publisher.clone()));
        // Modo Online: liga o publicador da câmera à ponte (no-op nas outras salas).
        room.on_track_started(&track, PublishedTrack::Video(publisher.clone()));
        sessao.camera = Some(PublishedVideo { track, publisher });
        sessao.camera_info = Some(CameraRuntime { src, plan, modes });
        Ok(())
    }

    /// Troca ao vivo a qualidade da câmera. Sem câmera ligada: no-op. Mesma captura: só troca a
    /// escada; captura diferente: reinicia só a câmera, mantendo a mesma trilha. Nunca toca na tela.
    pub async fn set_camera_quality(shared: &Arc<Shared>, preset: Preset) -> Result<(), CoreError> {
        let mut sessao = shared.share.lock().await;
        let Some(info) = sessao.camera_info.clone() else {
            return Ok(());
        };
        let Some(atual) = sessao.camera.as_ref() else {
            return Ok(());
        };
        let track_id = atual.track.id.clone();
        let publisher = atual.publisher.clone();
        // Os modos guardados ao ligar: a câmera está aberta pela captura agora.
        let novo = plan_with(&info.src, &info.modes, preset);
        let ladder = novo.ladder.clone();

        if same_capture(&info.plan, &novo) {
            publisher.set_ladder(ladder.clone());
        } else {
            // Captura diferente: reinicia só a captura, no mesmo publicador e na mesma trilha
            // (espectadores, ponte Online e prévia continuam). A tela não é tocada.
            let encoder = shared.settings.read().video.encoder;
            if let Err(e) = publisher.restart_capture(to_video_spec(&info.src), ladder.clone(), encoder.to_spec()) {
                // Sem pipeline o publicador não serve mais: a câmera fica desligada.
                Self::stop_camera_locked(shared, &mut sessao).await;
                return Err(e);
            }
        }
        // A sessão já reflete o que está no ar antes de avisar a sala.
        sessao.camera_info = Some(CameraRuntime {
            src: info.src,
            plan: novo,
            modes: info.modes,
        });
        if let Some(room) = shared.current_room() {
            if let Err(e) = room.update_track(&track_id, None, Some(ladder), None).await {
                tracing::warn!(erro = %e, "falha ao avisar a sala da escada nova da câmera");
            }
        }
        Ok(())
    }
}

/// Modos nativos da fonte (`zap_capture::camera_modes`). Lê só com a câmera ainda fechada; fonte
/// sintética, outra fonte ou erro de leitura: lista vazia.
pub(crate) fn modes_for(src: &SourceRef) -> Vec<zap_capture::CameraMode> {
    match src {
        SourceRef::Camera(id) => zap_capture::camera_modes(id).unwrap_or_else(|e| {
            tracing::warn!("não consegui listar os modos da câmera {id}: {e}");
            Vec::new()
        }),
        _ => Vec::new(),
    }
}

/// Plano da câmera para a fonte, os modos já lidos (`modes_for`) e a qualidade.
pub(crate) fn plan_with(src: &SourceRef, modes: &[zap_capture::CameraMode], preset: Preset) -> CameraPlan {
    match src {
        SourceRef::Synthetic { width, height } => plan_synthetic(*width, *height, preset),
        _ => plan_from_modes(modes, preset),
    }
}

/// Dispositivo da webcam do pedido atual (`source = "camera:<id>"`), sem o prefixo.
fn webcam_device(req: Option<&ShareRequest>) -> Option<String> {
    req?.source.strip_prefix("camera:").map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(source: &str) -> ShareRequest {
        ShareRequest {
            source: source.into(),
            audio: crate::types::AudioChoice::Keyword("none".into()),
            preset: Preset::P720p30,
            encoder: "auto".into(),
        }
    }

    #[test]
    fn webcam_device_extrai_id() {
        assert_eq!(webcam_device(Some(&req("camera:abc"))).as_deref(), Some("abc"));
        assert_eq!(
            webcam_device(Some(&req("camera:synthetic:1280x720"))).as_deref(),
            Some("synthetic:1280x720")
        );
        assert_eq!(webcam_device(Some(&req("screen:0"))), None);
        assert_eq!(webcam_device(None), None);
    }

    #[test]
    fn plan_with_sintetica_1080p30() {
        let p = plan_with(
            &SourceRef::Synthetic {
                width: 1920,
                height: 1080,
            },
            &[],
            Preset::P1080p30,
        );
        assert_eq!(p.ladder.len(), 3);
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1920, 1080));
    }

    #[test]
    fn plan_with_camera_sem_modos_cai_na_reserva() {
        let p = plan_with(&SourceRef::Camera("x".into()), &[], Preset::P1080p60);
        assert_eq!(p.mode, None);
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1280, 720));
    }

    #[test]
    fn plan_with_camera_usa_os_modos_guardados() {
        let modos = [
            zap_capture::CameraMode {
                width: 1280,
                height: 720,
                fps: 30,
                type_index: 0,
            },
            zap_capture::CameraMode {
                width: 1920,
                height: 1080,
                fps: 30,
                type_index: 1,
            },
        ];
        let p = plan_with(&SourceRef::Camera("x".into()), &modos, Preset::P1080p30);
        assert_eq!(p.mode.map(|m| (m.width, m.height)), Some((1920, 1080)));
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1920, 1080));
    }

    #[test]
    fn modes_for_fonte_sintetica_nao_le_dispositivo() {
        assert!(modes_for(&SourceRef::Synthetic {
            width: 1280,
            height: 720
        })
        .is_empty());
    }
}
