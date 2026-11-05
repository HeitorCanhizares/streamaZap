//! Repasse de mídia da ponte (`docs/api/midia.md#ponte-servidor--o-que-repassar`).

use std::sync::Arc;

use bytes::Bytes;
use zap_net::media_tx::MediaSender;
use zap_proto::media::{AudioHeader, VideoHeader};
use zap_proto::ControlMsg;

use crate::state::{Hub, Inner};

/// Resultado do planejamento de um quadro: a quem enviar e quais trocas de camada avisar.
pub struct VideoPlan {
    /// `MediaSender` dos espectadores que devem receber (`ParticipantData::media_tx`).
    pub targets: Vec<Arc<MediaSender>>,
    /// `(participante, camada nova)` para anunciar `camada.atual`.
    pub switched: Vec<(String, u8)>,
}

/// `MediaSender` dos participantes com esses números. Quem não tem `media_tx` (sem conexão de
/// mídia) fica de fora.
fn resolve_targets(inner: &Inner, nums: &[u64]) -> Vec<Arc<MediaSender>> {
    nums.iter()
        .filter_map(|n| inner.participants.values().find(|p| p.num == *n))
        .filter_map(|p| p.media_tx.clone())
        .collect()
}

impl Hub {
    /// Decide o destino de um quadro vindo de `publisher`: acha a trilha pelo `header.track`
    /// (número) na sala do publicador, chama `TrackRelay::on_frame(layer, keyframe)`, resolve os
    /// `MediaSender` dos espectadores em `to`, e avisa `camada.atual` a quem trocou (pelo QUIC do
    /// espectador: é mensagem da mídia). Trilha desconhecida = plano vazio.
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

        // Quem trocou: aviso pelo QUIC e marca como anunciado (o ticker não repete). O primeiro
        // anúncio de um espectador tem motivo `inicio`; os seguintes, `banda`.
        let mut avisos: Vec<(String, ControlMsg)> = Vec::new();
        let mut switched = Vec::new();
        for (num, camada) in &routing.switched {
            let Some(v) = track.relay.viewers.get_mut(num) else {
                continue;
            };
            let motivo = if v.announced.is_none() { "inicio" } else { "banda" };
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
                        reason: motivo.into(),
                    },
                ));
            }
            switched.push((pid, *camada));
        }
        for (pid, msg) in avisos {
            inner.send_quic(&pid, msg);
        }

        VideoPlan {
            targets: resolve_targets(inner, &routing.to),
            switched,
        }
    }

    /// `MediaSender` dos espectadores de áudio da trilha `track` (número) do `publisher`.
    pub fn audio_targets(&self, publisher: &str, track: u16) -> Vec<Arc<MediaSender>> {
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

/// Repassa um quadro a todos os alvos: para cada `MediaSender`, `send_video` com o mesmo cabeçalho
/// (o `send_us` é reescrito dentro de `send_video`; `captura_us` e o resto ficam intactos) e o
/// prazo `sender.deadline()`. Falha em um alvo não afeta os outros (log em `tracing::debug`).
/// Os `MediaSender` vêm de `ParticipantData::media_tx`: são o mesmo `Arc` da conexão em todos os
/// quadros, porque o prazo do keyframe e a admissão têm estado por conexão. `hub` não é usado
/// aqui; fica na assinatura para não mudar a chamada em `bridge.rs`.
pub async fn forward_video(_hub: &Hub, targets: Vec<Arc<MediaSender>>, header: VideoHeader, data: Bytes) {
    // Uma tarefa por espectador: um alvo lento não segura os outros, e o prazo é de cada um.
    let tarefas: Vec<_> = targets
        .into_iter()
        .map(|sender| {
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

/// Idem para um pacote de áudio (`MediaSender::send_audio`, que reescreve `send_us`). `hub` não é
/// usado (ver `forward_video`).
pub fn forward_audio(_hub: &Hub, targets: &[Arc<MediaSender>], header: AudioHeader, opus: &[u8]) {
    for sender in targets {
        if let Err(e) = sender.send_audio(header, opus) {
            tracing::debug!("falha ao repassar áudio: {e}");
        }
    }
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

    /// Ana publica a trilha 1; Beto assiste. Devolve o canal de controle QUIC de Beto.
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
            b.quic_ctl = Some(tx);
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
                assert_eq!(reason, "inicio", "primeiro anúncio do espectador");
                assert_eq!(width, 640);
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
        {
            let inner = hub.inner.lock();
            let r = &inner.rooms.values().next().unwrap().tracks[0].relay;
            assert!(r.viewers.values().all(|v| v.announced == Some(1) && v.current == 1));
        }

        // Segunda troca (já anunciado antes): motivo `banda`.
        {
            let mut inner = hub.inner.lock();
            let r = &mut inner.rooms.values_mut().next().unwrap().tracks[0].relay;
            r.viewers.values_mut().for_each(|v| v.pending = Some(0));
        }
        let plano = hub.plan_video(&ana, &header(1, 0, true));
        assert_eq!(plano.switched, vec![(beto.clone(), 0)]);
        assert!(matches!(
            rx.try_recv().unwrap(),
            ControlMsg::LayerCurrent { layer: 0, reason, .. } if reason == "banda"
        ));
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

    #[test]
    fn resolve_targets_ignora_quem_nao_tem_media_tx() {
        let (hub, _ana, beto, _rx) = cenario(|pid, n| viewer(pid, n, 0, None, false));
        let inner = hub.inner.lock();
        let num_beto = inner.participants[&beto].num;
        assert!(inner.participants[&beto].media_tx.is_none());
        let alvos: Vec<Arc<MediaSender>> = resolve_targets(&inner, &[num_beto]);
        assert!(alvos.is_empty());
    }
}
