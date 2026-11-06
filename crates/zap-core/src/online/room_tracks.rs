//! Trilhas da sala Online: publicar, atualizar, remover (pelo WebSocket).

use std::time::Duration;

use tokio::sync::oneshot;
use zap_proto::{ControlMsg, LayerSpec, Track};

use super::room::OnlineRoom;
use crate::publish::PublishedTrack;
use crate::room::TrackSpec;
use crate::CoreError;

/// Tempo máximo esperando o `trilha.publicada` do servidor.
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

impl OnlineRoom {
    /// `TrackPublish{local_id, kind, title, layers, audio, address: None}` pelo WebSocket;
    /// espera `TrackPublished` com o mesmo `local_id` (entregue por `room_events` via `pending`)
    /// por 5 s (`Connect("servidor não respondeu")` senão; `Error` do servidor com `ref`/código
    /// → `CoreError::Server`). Devolve a `Track` com `id`/`number` do servidor (já no modelo).
    pub async fn publish_track(&self, spec: TrackSpec) -> Result<Track, CoreError> {
        let ws = self
            .session
            .ws()
            .ok_or_else(|| CoreError::Connect("sem conexão com o servidor".into()))?;
        // Registra antes de enviar: a resposta pode chegar antes de o `send` voltar.
        let (tx, rx) = oneshot::channel();
        let local_id = spec.local_id.clone();
        self.pending.lock().insert(local_id.clone(), tx);
        let msg = ControlMsg::TrackPublish {
            local_id: local_id.clone(),
            kind: spec.kind,
            title: spec.title,
            layers: spec.layers,
            audio: spec.audio,
            address: None,
            camera_layout: spec.camera_layout,
        };
        if let Err(e) = ws.send(msg).await {
            self.pending.lock().remove(&local_id);
            return Err(e.into());
        }
        match tokio::time::timeout(PUBLISH_TIMEOUT, rx).await {
            Ok(Ok(track)) => {
                // Garante que a trilha esteja no modelo mesmo se `room_events` ainda não aplicou.
                self.model.lock().upsert_track(track.clone());
                Ok(track)
            }
            Ok(Err(_)) => {
                self.pending.lock().remove(&local_id);
                Err(CoreError::Connect("conexão encerrada antes da resposta".into()))
            }
            Err(_) => {
                self.pending.lock().remove(&local_id);
                Err(CoreError::Connect("servidor não respondeu".into()))
            }
        }
    }

    /// `TrackUpdate{track_id, title, layers, camera_layout}`; o modelo muda quando chega `TrackUpdated`.
    pub async fn update_track(
        &self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<LayerSpec>>,
        camera_layout: Option<zap_proto::CameraLayout>,
    ) -> Result<(), CoreError> {
        let ws = self
            .session
            .ws()
            .ok_or_else(|| CoreError::Connect("sem conexão com o servidor".into()))?;
        ws.send(ControlMsg::TrackUpdate {
            track_id: track_id.into(),
            title,
            layers,
            camera_layout,
        })
        .await?;
        Ok(())
    }

    /// `TrackRemove{track_id}`, `publish.detach(track_id)` e tira do modelo na hora.
    /// Sem WebSocket não é erro: a remoção local acontece e o servidor avisa ao reconectar.
    pub async fn remove_track(&self, track_id: &str) -> Result<(), CoreError> {
        self.publish.detach(track_id);
        self.model.lock().remove_track(track_id);
        if let Some(ws) = self.session.ws() {
            ws.send(ControlMsg::TrackRemove {
                track_id: track_id.into(),
            })
            .await?;
        }
        Ok(())
    }

    /// Chamado por `share.rs` quando o publicador local da trilha já existe: liga-o à ponte
    /// (`publish.attach`) para o servidor poder pedir camadas. Sem ponte conectada, guarda e
    /// liga quando ela conectar (`reconnect`).
    pub fn on_track_started(&self, track: &Track, published: PublishedTrack) {
        // Função síncrona: `try_lock` evita `await`. Ponte ocupada agora = sem ponte; o
        // `reattach_all` da reconexão religa a trilha.
        let bridge = self.bridge.try_lock().ok().and_then(|guard| {
            let pc = guard.as_ref()?;
            let shared = self.shared().ok()?;
            Some((pc.conn.clone(), pc.ctrl.clone(), shared.clock.clone()))
        });
        self.publish.attach(track, published, bridge);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use zap_media::{EncoderChoiceSpec, VideoPipelineConfig, VideoSourceSpec};
    use zap_net::online::JoinMedia;
    use zap_proto::{Access, Participant, Role, Room, RoomCode, RoomMode, TrackKind};
    use zap_rate::ladder::{screen_ladder, Preset};

    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::publish::VideoPublisher;
    use crate::room::RoomModel;
    use crate::testutil::TestSink;

    fn core() -> Core {
        let dir = std::env::temp_dir().join(format!("zapcore-trilhas-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        Core::start_with(dir, TestSink::new(), opts).unwrap()
    }

    /// Sala Online sem servidor (`session.ws()` = `None`).
    fn sala(core: &Core) -> Arc<OnlineRoom> {
        let room = Room {
            id: "s_1".into(),
            code: None,
            name: "Corrida".into(),
            owner_id: "p_1".into(),
            access: Access::Open,
            mode: RoomMode::Online,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let owner = Participant {
            id: "p_1".into(),
            name: "Ana".into(),
            color: "#ff0000".into(),
            role: Role::Member,
            joined_at: "2026-01-01T00:00:00Z".into(),
            app_version: "1.0.0".into(),
            protocol: 1,
            latency_ms: None,
            watching: None,
        };
        OnlineRoom::new(
            &core.shared,
            RoomModel::new(room, owner),
            "p1".into(),
            RoomCode::from_entropy([1, 2, 3, 4]),
            None,
            JoinMedia {
                bridge: None,
                direct_allowed: false,
            },
            String::new(),
        )
    }

    fn trilha(id: &str) -> Track {
        serde_json::from_value(json!({
            "id": id, "participante_id": "p1", "numero": 1, "tipo": "tela", "titulo": "t"
        }))
        .unwrap()
    }

    fn spec() -> TrackSpec {
        TrackSpec {
            local_id: "local-1".into(),
            kind: TrackKind::Screen,
            title: "Tela".into(),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    fn publicador() -> Arc<VideoPublisher> {
        let cfg = VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Synthetic {
                width: 1280,
                height: 720,
            },
            ladder: screen_ladder(Preset::P720p30, 1280, 720),
            encoder: EncoderChoiceSpec::Auto,
        };
        VideoPublisher::start(cfg, tokio::runtime::Handle::current(), zap_net::Clock::new()).unwrap()
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn publish_track_sem_ws_erro_connect() {
        let core = core();
        let room = sala(&core);
        let r = rt().block_on(room.publish_track(spec()));
        assert!(matches!(r, Err(CoreError::Connect(ref m)) if m == "sem conexão com o servidor"));
        assert!(room.pending.lock().is_empty());
    }

    #[test]
    fn update_track_sem_ws_erro_connect() {
        let core = core();
        let room = sala(&core);
        let r = rt().block_on(room.update_track("t1", Some("novo".into()), None, None));
        assert!(matches!(r, Err(CoreError::Connect(_))));
    }

    #[test]
    fn remove_track_sem_ws_ok_e_tira_do_modelo() {
        let core = core();
        let room = sala(&core);
        room.model.lock().upsert_track(trilha("t1"));
        rt().block_on(room.remove_track("t1")).unwrap();
        assert!(room.model.lock().tracks.is_empty());
    }

    #[test]
    fn on_track_started_sem_ponte_guarda_feed() {
        let core = core();
        let room = sala(&core);
        let rt = rt();
        rt.block_on(async {
            let p = publicador();
            room.on_track_started(&trilha("v1"), PublishedTrack::Video(p.clone()));
            {
                let feeds = room.publish.feeds.lock();
                let feed = feeds.get("v1").expect("trilha guardada");
                assert!(feed.link.is_none());
            }
            p.stop();
        });
    }
}
