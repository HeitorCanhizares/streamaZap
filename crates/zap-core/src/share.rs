//! O que estou publicando: tela/janela + áudio + câmera.

use std::sync::Arc;

use zap_media::VideoPipelineConfig;
use zap_proto::{AudioSpec, LayerSpec, Track, TrackKind};

use crate::engine::Shared;
use crate::publish::{AudioPublisher, AudioSource, PublishedTrack, VideoPublisher};
use crate::room::TrackSpec;
use crate::snapshot::ShareView;
use crate::sources::{parse_audio, parse_source, to_video_spec};
use crate::types::{AudioChoice, EncoderPref, Preset, ShareRequest, SourceRef};
use crate::util::new_id;
use crate::CoreError;

/// Resolução usada quando o monitor/janela não é encontrado na listagem.
const RESOLUCAO_PADRAO: (u32, u32) = (1920, 1080);

fn sem_audio(c: &AudioChoice) -> bool {
    matches!(c, AudioChoice::Keyword(k) if k == "none")
}

/// Título amigável da fonte; sem achar, o próprio id.
fn titulo_da_fonte(src: &SourceRef, id: &str) -> String {
    let achado = match src {
        SourceRef::Monitor(n) => zap_capture::list_monitors()
            .ok()
            .and_then(|l| l.into_iter().find(|m| m.index == *n).map(|m| m.name)),
        SourceRef::Window(h) => zap_capture::list_windows()
            .ok()
            .and_then(|l| l.into_iter().find(|w| w.hwnd == *h).map(|w| w.title)),
        _ => None,
    };
    achado.filter(|t| !t.is_empty()).unwrap_or_else(|| id.to_string())
}

pub struct PublishedVideo {
    pub track: Track,
    pub publisher: Arc<VideoPublisher>,
}

#[derive(Default)]
pub struct ShareSession {
    pub request: Option<ShareRequest>,
    pub screen: Option<PublishedVideo>,
    pub audio: Option<(Track, Arc<AudioPublisher>)>,
    pub camera: Option<PublishedVideo>,
}

impl ShareSession {
    /// Escada: `zap_rate::ladder::screen_ladder(preset.to_rate(), src_w, src_h)` com a resolução
    /// da fonte (monitor/janela: do `zap_capture`; sintética: a pedida; padrão 1920×1080) e
    /// `camera_ladder` para câmera.
    pub fn ladder_for(src: &SourceRef, preset: Preset) -> Vec<LayerSpec> {
        let (w, h) = match src {
            SourceRef::Synthetic { width, height } => (*width, *height),
            SourceRef::Monitor(n) => zap_capture::list_monitors()
                .ok()
                .and_then(|l| l.into_iter().find(|m| m.index == *n))
                .map_or(RESOLUCAO_PADRAO, |m| (m.width, m.height)),
            SourceRef::Window(hw) => zap_capture::list_windows()
                .ok()
                .and_then(|l| l.into_iter().find(|w| w.hwnd == *hw))
                .map_or(RESOLUCAO_PADRAO, |w| (w.width, w.height)),
            SourceRef::Camera(_) => return zap_rate::ladder::camera_ladder(1280, 720),
        };
        zap_rate::ladder::screen_ladder(preset.to_rate(), w, h)
    }

    /// Fluxo de `start_share`: exige sala; publica vídeo (e áudio, se pedido) e registra no
    /// `MediaServer`. Já compartilhando → `Invalid("já compartilhando")`.
    pub async fn start(shared: &Arc<Shared>, req: ShareRequest) -> Result<(), CoreError> {
        let mut sessao = shared.share.lock().await;
        Self::start_locked(shared, &mut sessao, req).await
    }

    async fn start_locked(shared: &Arc<Shared>, sessao: &mut ShareSession, req: ShareRequest) -> Result<(), CoreError> {
        if sessao.screen.is_some() {
            return Err(CoreError::Invalid("já compartilhando".into()));
        }
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let src = parse_source(&req.source)?;
        // Interpreta o áudio antes de publicar qualquer coisa: pedido ruim não deixa trilha órfã.
        let selecao = if sem_audio(&req.audio) {
            None
        } else {
            Some(parse_audio(&req.audio)?)
        };
        let ms = shared.ensure_media_server().await?;
        let ladder = Self::ladder_for(&src, req.preset);
        let kind = if matches!(src, SourceRef::Window(_)) {
            TrackKind::Window
        } else {
            TrackKind::Screen
        };
        let (interface, loopback) = (
            shared.settings.read().local.interface.clone(),
            shared.options.include_loopback,
        );
        let track = room
            .publish_track(TrackSpec {
                local_id: new_id("l_"),
                kind,
                title: titulo_da_fonte(&src, &req.source),
                layers: ladder.clone(),
                audio: None,
                address: Some(ms.address(interface.as_deref(), loopback)),
            })
            .await?;
        let encoder = EncoderPref::parse(&req.encoder).unwrap_or_default().to_spec();
        let publisher = match VideoPublisher::start(
            VideoPipelineConfig {
                track: track.number,
                source: to_video_spec(&src),
                ladder,
                encoder,
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
        // Modo Online: liga o publicador à ponte (no-op nas outras salas).
        room.on_track_started(&track, PublishedTrack::Video(publisher.clone()));
        sessao.screen = Some(PublishedVideo { track, publisher });

        if let Some(sel) = selecao {
            let sintetica = matches!(src, SourceRef::Synthetic { .. });
            if let Err(e) = Self::start_audio(shared, sessao, sintetica, sel).await {
                // Segue só com vídeo; a interface recebe o aviso.
                tracing::warn!(erro = %e, "falha ao iniciar o áudio do compartilhamento");
                shared.state.lock().push_error(format!("Áudio não iniciou: {e}"));
                shared.notifier.touch();
            }
        }
        sessao.request = Some(req);
        Ok(())
    }

    /// Publica a trilha de áudio e o `AudioPublisher`; em falha remove a trilha.
    async fn start_audio(
        shared: &Arc<Shared>,
        sessao: &mut ShareSession,
        sintetica: bool,
        sel: zap_audio::AudioSelection,
    ) -> Result<(), CoreError> {
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let ms = shared.ensure_media_server().await?;
        let (interface, loopback) = (
            shared.settings.read().local.interface.clone(),
            shared.options.include_loopback,
        );
        let atrack = room
            .publish_track(TrackSpec {
                local_id: new_id("l_"),
                kind: TrackKind::Audio,
                title: "Áudio".into(),
                layers: vec![],
                audio: Some(AudioSpec {
                    rate: 48000,
                    channels: 2,
                    kbps: 96,
                }),
                address: Some(ms.address(interface.as_deref(), loopback)),
            })
            .await?;
        let fonte = if sintetica {
            AudioSource::Synthetic
        } else {
            AudioSource::Capture(sel)
        };
        match AudioPublisher::start(atrack.number, fonte) {
            Ok(pub_audio) => {
                ms.register(atrack.id.clone(), PublishedTrack::Audio(pub_audio.clone()));
                // Modo Online: liga o publicador de áudio à ponte (no-op nas outras salas).
                room.on_track_started(&atrack, PublishedTrack::Audio(pub_audio.clone()));
                sessao.audio = Some((atrack, pub_audio));
                Ok(())
            }
            Err(e) => {
                let _ = room.remove_track(&atrack.id).await;
                Err(e)
            }
        }
    }

    /// Troca ao vivo: mesma fonte → `set_ladder` + `room.update_track(layers)`; fonte nova →
    /// para e reinicia (stop+start). Áudio `set_selection`.
    pub async fn update(shared: &Arc<Shared>, req: ShareRequest) -> Result<(), CoreError> {
        let mut sessao = shared.share.lock().await;
        let Some(screen) = sessao.screen.as_ref() else {
            return Err(CoreError::Invalid("não está compartilhando".into()));
        };
        let mesma_fonte = sessao.request.as_ref().is_some_and(|r| r.source == req.source);
        if !mesma_fonte {
            Self::stop_video_audio(shared, &mut sessao).await;
            return Self::start_locked(shared, &mut sessao, req).await;
        }
        let src = parse_source(&req.source)?;
        let selecao = if sem_audio(&req.audio) {
            zap_audio::AudioSelection::None
        } else {
            parse_audio(&req.audio)?
        };
        let ladder = Self::ladder_for(&src, req.preset);
        screen.publisher.set_ladder(ladder.clone());
        let track_id = screen.track.id.clone();
        if let Some(room) = shared.current_room() {
            room.update_track(&track_id, None, Some(ladder)).await?;
        }
        if let Some((_, a)) = sessao.audio.as_ref() {
            a.set_selection(selecao);
        } else if !sem_audio(&req.audio) {
            let sintetica = matches!(src, SourceRef::Synthetic { .. });
            if let Err(e) = Self::start_audio(shared, &mut sessao, sintetica, selecao).await {
                tracing::warn!(erro = %e, "falha ao iniciar o áudio do compartilhamento");
                shared.state.lock().push_error(format!("Áudio não iniciou: {e}"));
                shared.notifier.touch();
            }
        }
        sessao.request = Some(req);
        Ok(())
    }

    /// Para tela e áudio (a câmera fica), removendo as trilhas do modelo.
    async fn stop_video_audio(shared: &Arc<Shared>, sessao: &mut ShareSession) {
        let ms = shared.media_server.lock().await.clone();
        let room = shared.current_room();
        let mut trilhas: Vec<String> = Vec::new();
        if let Some(v) = sessao.screen.take() {
            v.publisher.stop();
            trilhas.push(v.track.id);
        }
        if let Some((t, a)) = sessao.audio.take() {
            a.stop();
            trilhas.push(t.id);
        }
        sessao.request = None;
        Self::remover_trilhas(ms, room, trilhas).await;
    }

    async fn remover_trilhas(
        ms: Option<Arc<crate::publish::MediaServer>>,
        room: Option<Arc<crate::room::ActiveRoom>>,
        trilhas: Vec<String>,
    ) {
        for id in trilhas {
            if let Some(ms) = &ms {
                ms.unregister(&id);
            }
            if let Some(room) = &room {
                // Sala já pode ter caído: o erro não muda o que fazer.
                let _ = room.remove_track(&id).await;
            }
        }
    }

    /// Para publicadores, `unregister`, `room.remove_track` de cada trilha, limpa a sessão.
    pub async fn stop(shared: &Arc<Shared>) {
        let mut sessao = shared.share.lock().await;
        Self::stop_camera_locked(shared, &mut sessao).await;
        Self::stop_video_audio(shared, &mut sessao).await;
        *sessao = ShareSession::default();
    }

    async fn stop_camera_locked(shared: &Arc<Shared>, sessao: &mut ShareSession) {
        if let Some(c) = sessao.camera.take() {
            let ms = shared.media_server.lock().await.clone();
            c.publisher.stop();
            Self::remover_trilhas(ms, shared.current_room(), vec![c.track.id]).await;
        }
    }

    /// Liga/desliga a câmera: trilha `camera` com `camera_ladder`, fonte
    /// `Camera(settings.video.camera_id)` (ou sintética 1280×720 se `camera_id` for
    /// `synthetic:..`); sem câmera configurada em plataforma sem câmera → `Unsupported`.
    pub async fn set_camera(shared: &Arc<Shared>, on: bool) -> Result<(), CoreError> {
        let mut sessao = shared.share.lock().await;
        if !on {
            Self::stop_camera_locked(shared, &mut sessao).await;
            return Ok(());
        }
        if sessao.camera.is_some() {
            return Ok(());
        }
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let (camera_id, encoder_pref, interface) = {
            let s = shared.settings.read();
            (s.video.camera_id.clone(), s.video.encoder, s.local.interface.clone())
        };
        let src = match camera_id {
            Some(id) if id.starts_with("synthetic:") => parse_source(&id)?,
            Some(id) => SourceRef::Camera(id),
            None if cfg!(windows) => return Err(CoreError::Unsupported("nenhuma câmera configurada".into())),
            None => SourceRef::Synthetic {
                width: 1280,
                height: 720,
            },
        };
        let ms = shared.ensure_media_server().await?;
        let ladder = zap_rate::ladder::camera_ladder(1280, 720);
        let track = room
            .publish_track(TrackSpec {
                local_id: new_id("l_"),
                kind: TrackKind::Camera,
                title: "Câmera".into(),
                layers: ladder.clone(),
                audio: None,
                address: Some(ms.address(interface.as_deref(), shared.options.include_loopback)),
            })
            .await?;
        let publisher = match VideoPublisher::start(
            VideoPipelineConfig {
                track: track.number,
                source: to_video_spec(&src),
                ladder,
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
        Ok(())
    }

    /// Reinicia o que eu compartilhava depois de `RoomEvent::Reconnected`: não dá para manter os
    /// pipelines com ids novos, então faz stop + start do último pedido (e da câmera, se ligada).
    pub async fn republish(shared: &Arc<Shared>) -> Result<(), CoreError> {
        let (pedido, tinha_camera) = {
            let s = shared.share.lock().await;
            (s.request.clone(), s.camera.is_some())
        };
        if pedido.is_none() && !tinha_camera {
            return Ok(());
        }
        Self::stop(shared).await;
        if let Some(req) = pedido {
            Self::start(shared, req).await?;
        }
        if tinha_camera {
            Self::set_camera(shared, true).await?;
        }
        Ok(())
    }

    /// `ShareView` do snapshot: `source` (texto da fonte), `audio` ("Sem som"/"Todo o PC"/
    /// "N apps"), `preset`, `encoder` (nome real do pipeline), `layers` ativas, espectadores por
    /// camada, `encode_ms` da camada 0.
    pub fn view(&self) -> Option<ShareView> {
        let screen = self.screen.as_ref()?;
        let req = self.request.as_ref()?;
        let audio = match &req.audio {
            AudioChoice::Keyword(k) if k == "none" => "Sem som".to_string(),
            AudioChoice::Keyword(_) => "Todo o PC".to_string(),
            AudioChoice::Apps(a) => format!("{} apps", a.len()),
        };
        let st = screen.publisher.stats();
        Some(ShareView {
            source: req.source.clone(),
            audio,
            preset: req.preset,
            encoder: st.encoder_name,
            layers: st.layers,
            viewers_per_layer: st.viewers_per_layer,
            encode_ms: st.encode_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_for_sintetica_1080p60_tem_3_camadas_e_camada0_1080() {
        let l = ShareSession::ladder_for(
            &SourceRef::Synthetic {
                width: 1920,
                height: 1080,
            },
            Preset::P1080p60,
        );
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].height, 1080);
    }

    #[test]
    fn ladder_for_camera() {
        let l = ShareSession::ladder_for(&SourceRef::Camera("x".into()), Preset::P720p30);
        assert_eq!(l, zap_rate::ladder::camera_ladder(1280, 720));
    }

    #[test]
    fn view_sem_sessao_e_none() {
        assert!(ShareSession::default().view().is_none());
    }
}
