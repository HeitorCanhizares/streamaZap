//! Repasse de mídia da ponte (`docs/api/midia.md#ponte-servidor--o-que-repassar`).

use bytes::Bytes;
use zap_net::media_tx::MediaSender;
use zap_proto::media::{AudioHeader, VideoHeader};
use zap_proto::ControlMsg;

use crate::state::{Hub, Inner};

/// Resultado do planejamento de um quadro: a quem enviar e quais trocas de camada avisar.
pub struct VideoPlan {
    /// Conexões QUIC dos espectadores que devem receber.
    pub targets: Vec<quinn::Connection>,
    /// `(participante, camada nova)` para anunciar `camada.atual`.
    pub switched: Vec<(String, u8)>,
}

/// Conexões QUIC dos participantes com esses números (quem não tem QUIC fica de fora).
fn resolve_targets(inner: &Inner, nums: &[u64]) -> Vec<quinn::Connection> {
    nums.iter()
        .filter_map(|n| inner.participants.values().find(|p| p.num == *n))
        .filter_map(|p| p.quic.clone())
        .collect()
}

impl Hub {
    /// Decide o destino de um quadro vindo de `publisher`: acha a trilha pelo `header.track`
    /// (número) na sala do publicador, chama `TrackRelay::on_frame(layer, keyframe)`, resolve as
    /// conexões QUIC dos espectadores em `to`, e avisa `camada.atual` por WebSocket para quem
    /// trocou. Trilha desconhecida = plano vazio.
    pub fn plan_video(&self, publisher: &str, header: &VideoHeader) -> VideoPlan {
        let mut guard = self.inner.lock();
        let inner = &mut *guard;
        let vazio = || VideoPlan {
            targets: Vec::new(),
            switched: Vec::new(),
        };
        let Some(room_id) = inner.participants.get(publisher).and_then(|p| p.room_id.clone()) else {
            return vazio();
        };
        let Some(track) = inner.rooms.get_mut(&room_id).and_then(|r| {
            r.tracks
                .iter_mut()
                .find(|t| t.number == header.track && t.participant_id == publisher)
        }) else {
            return vazio();
        };

        let routing = track.relay.on_frame(header.layer, header.flags.is_keyframe());
        let spec = |camada: u8| track.layers.iter().find(|l| l.layer == camada).cloned();

        // Quem trocou: aviso por WebSocket e marca como anunciado (o ticker não repete).
        let mut avisos: Vec<(String, ControlMsg)> = Vec::new();
        let mut switched = Vec::new();
        for (num, camada) in &routing.switched {
            let Some(v) = track.relay.viewers.get_mut(num) else {
                continue;
            };
            v.announced = Some(*camada);
            let pid = v.participant_id.clone();
            if let Some(l) = spec(*camada) {
                avisos.push((
                    pid.clone(),
                    ControlMsg::LayerCurrent {
                        track_id: track.id.clone(),
                        layer: *camada,
                        width: l.width,
                        height: l.height,
                        fps: l.fps,
                        reason: "banda".into(),
                    },
                ));
            }
            switched.push((pid, *camada));
        }
        for (pid, msg) in avisos {
            if let Some(tx) = inner.participants.get(&pid).and_then(|p| p.ws.as_ref()) {
                let _ = tx.send(msg);
            }
        }

        VideoPlan {
            targets: resolve_targets(inner, &routing.to),
            switched,
        }
    }

    /// Conexões QUIC dos espectadores de áudio da trilha `track` (número) do `publisher`.
    pub fn audio_targets(&self, publisher: &str, track: u16) -> Vec<quinn::Connection> {
        let inner = self.inner.lock();
        let Some(room_id) = inner.participants.get(publisher).and_then(|p| p.room_id.as_ref()) else {
            return Vec::new();
        };
        let Some(t) = inner.rooms.get(room_id).and_then(|r| {
            r.tracks
                .iter()
                .find(|t| t.number == track && t.participant_id == publisher)
        }) else {
            return Vec::new();
        };
        resolve_targets(&inner, &t.relay.audio_targets())
    }
}

/// Repassa um quadro a todos os alvos: para cada conexão, `MediaSender::send_video` com o mesmo
/// cabeçalho (o `send_us` é reescrito dentro de `send_video`; `captura_us` e o resto ficam
/// intactos) e o prazo `sender.deadline()`. Falha em um alvo não afeta os outros (log em
/// `tracing::debug`). Os `MediaSender` são criados por chamada com `Clock` do `Hub`.
pub async fn forward_video(hub: &Hub, targets: Vec<quinn::Connection>, header: VideoHeader, data: Bytes) {
    // Uma tarefa por espectador: um alvo lento não segura os outros, e o prazo é de cada um.
    let tarefas: Vec<_> = targets
        .into_iter()
        .map(|conn| {
            let sender = sender_for(hub, conn);
            let data = data.clone();
            tokio::spawn(async move {
                let prazo = sender.deadline();
                match sender.send_video(header, data, prazo).await {
                    Ok(true) => {}
                    Ok(false) => tracing::debug!("quadro descartado: prazo estourado"),
                    Err(e) => tracing::debug!("falha ao repassar vídeo: {e}"),
                }
            })
        })
        .collect();
    for t in tarefas {
        let _ = t.await;
    }
}

/// Idem para um pacote de áudio (`MediaSender::send_audio`, que reescreve `send_us`).
pub fn forward_audio(hub: &Hub, targets: &[quinn::Connection], header: AudioHeader, opus: &[u8]) {
    for conn in targets {
        if let Err(e) = sender_for(hub, conn.clone()).send_audio(header, opus) {
            tracing::debug!("falha ao repassar áudio: {e}");
        }
    }
}

/// Criador de `MediaSender` (para os testes e `forward_*`).
pub fn sender_for(hub: &Hub, conn: quinn::Connection) -> MediaSender {
    MediaSender::new(conn, hub.clock.clone())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        relay::{TrackRelay, ViewerEntry},
        state::TrackData,
        Config,
    };
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};
    use zap_proto::{media::VideoFlags, Access, DelayState, LayerSpec, TileSize, TrackKind};

    fn entrar(hub: &Hub, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: format!("inst-{nome}"),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

    fn camada(n: u8) -> LayerSpec {
        LayerSpec {
            layer: n,
            width: 640,
            height: 360,
            fps: 30,
            kbps: 500,
        }
    }

    fn viewer(pid: &str, num: u64, current: u8, pending: Option<u8>, preview: bool) -> ViewerEntry {
        ViewerEntry {
            participant_id: pid.into(),
            num,
            size: TileSize::Main,
            max_layer_req: None,
            audio: true,
            preview,
            bandwidth_kbps: 0,
            state: DelayState::Normal,
            current,
            pending,
            announced: None,
        }
    }

    fn header(track: u16, layer: u8, keyframe: bool) -> VideoHeader {
        VideoHeader {
            flags: VideoFlags(if keyframe { VideoFlags::KEYFRAME } else { 0 }),
            track,
            layer,
            width: 640,
            height: 360,
            seq: 1,
            capture_us: 0,
            send_us: 0,
            size: 0,
        }
    }

    /// Ana publica a trilha 1; Beto assiste. Devolve o canal WS de Beto.
    fn cenario(
        viewer_entry: impl Fn(&str, u64) -> ViewerEntry,
    ) -> (Hub, String, String, mpsc::UnboundedReceiver<ControlMsg>) {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let beto = entrar(&hub, "Beto");
        let req = CreateRoomRequest {
            name: "Sala".into(),
            access: Access::Open,
            password: None,
        };
        let sala = hub.create_room(&ana, req).unwrap().id;
        let (tx, rx) = mpsc::unbounded_channel();
        {
            let mut inner = hub.inner.lock();
            let num_beto = inner.participants[&beto].num;
            inner.participants.get_mut(&ana).unwrap().room_id = Some(sala.clone());
            let b = inner.participants.get_mut(&beto).unwrap();
            b.room_id = Some(sala.clone());
            b.ws = Some(tx);
            let layers = vec![camada(0), camada(1), camada(2)];
            let mut relay = TrackRelay::new(layers.clone());
            relay.viewers.insert(num_beto, viewer_entry(&beto, num_beto));
            inner.rooms.get_mut(&sala).unwrap().tracks.push(TrackData {
                id: "t1".into(),
                number: 1,
                participant_id: ana.clone(),
                kind: TrackKind::Camera,
                title: "t".into(),
                layers,
                audio: None,
                relay,
            });
        }
        (hub, ana, beto, rx)
    }

    #[test]
    fn plan_video_acha_alvos() {
        let (hub, ana, _beto, _rx) = cenario(|pid, n| viewer(pid, n, 0, None, false));
        let plano = hub.plan_video(&ana, &header(1, 0, false));
        // Sem QUIC não há conexão, mas o resolvedor não quebra.
        assert!(plano.targets.is_empty());
        assert!(plano.switched.is_empty());
        let inner = hub.inner.lock();
        assert!(resolve_targets(&inner, &[1, 2, 3]).is_empty());
    }

    #[test]
    fn trilha_desconhecida_plano_vazio() {
        let (hub, ana, _beto, _rx) = cenario(|pid, n| viewer(pid, n, 0, None, false));
        let plano = hub.plan_video(&ana, &header(9, 0, true));
        assert!(plano.targets.is_empty() && plano.switched.is_empty());
        let plano = hub.plan_video("ninguem", &header(1, 0, true));
        assert!(plano.targets.is_empty() && plano.switched.is_empty());
    }

    #[test]
    fn keyframe_troca_e_anuncia() {
        let (hub, ana, beto, mut rx) = cenario(|pid, n| viewer(pid, n, 2, Some(1), false));
        let plano = hub.plan_video(&ana, &header(1, 1, true));
        assert_eq!(plano.switched, vec![(beto.clone(), 1)]);
        match rx.try_recv().unwrap() {
            ControlMsg::LayerCurrent {
                track_id,
                layer,
                reason,
                width,
                ..
            } => {
                assert_eq!(track_id, "t1");
                assert_eq!(layer, 1);
                assert_eq!(reason, "banda");
                assert_eq!(width, 640);
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
        let inner = hub.inner.lock();
        let r = &inner.rooms.values().next().unwrap().tracks[0].relay;
        assert!(r.viewers.values().all(|v| v.announced == Some(1) && v.current == 1));
    }

    #[test]
    fn audio_targets_nao_inclui_previa() {
        let (hub, ana, _beto, _rx) = cenario(|pid, n| viewer(pid, n, 0, None, true));
        let inner = hub.inner.lock();
        let t = &inner.rooms.values().next().unwrap().tracks[0];
        assert!(t.relay.audio_targets().is_empty());
        drop(inner);
        assert!(hub.audio_targets(&ana, 1).is_empty());
        assert!(hub.audio_targets(&ana, 7).is_empty());
    }
}
