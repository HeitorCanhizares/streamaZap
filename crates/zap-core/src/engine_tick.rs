//! Tarefa periódica (500 ms) que mantém o `UiState` em dia.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::{interval, MissedTickBehavior};
use zap_proto::{Track, TrackKind};

use crate::discovery_view::{network_status, peer_listings, presence_announcement, room_announcement, room_listings};
use crate::engine::Shared;
use crate::room::ActiveRoom;
use crate::snapshot::TileView;

/// Camada 0 da trilha de vídeo de maior `number`, como `(altura, fps)`.
fn pick_highlight(tracks: &[Track]) -> Option<(u16, u8)> {
    tracks
        .iter()
        .filter(|t| matches!(t.kind, TrackKind::Screen | TrackKind::Window | TrackKind::Camera))
        .max_by_key(|t| t.number)
        .and_then(|t| t.layers.iter().find(|l| l.layer == 0))
        .map(|l| (l.height, l.fps))
}

/// A cada 500 ms até `shared.stop`:
/// - descoberta: `rooms`/`peers`/`network` via `discovery_view` (+ `set_room`/`set_presence`
///   com o anúncio da minha sala se sou dono: `room_announcement`);
/// - tiles: para cada `Subscription`, `TileView` com `metrics.view`, ping do publicador
///   (`PubConn` RTT), `smoothing_ms` = `current_delay_ms` do relógio do participante;
/// - share: `ShareSession::view()`;
/// - sala: `refresh_room_view`;
///
/// e termina com `notifier.touch()`.
pub(crate) async fn run(shared: Arc<Shared>) {
    let mut ticker = interval(Duration::from_millis(500));
    // Se um tick atrasar, não tenta recuperar em rajada.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        tick_discovery(&shared);
        tick_tiles(&shared);
        // Trava assíncrona do share: nenhum outro lock é segurado aqui.
        let share_view = shared.share.lock().await.view();
        shared.state.lock().set_share(share_view);
        shared.refresh_room_view();
        shared.notifier.touch();
    }
}

fn tick_discovery(shared: &Arc<Shared>) {
    let discovery = shared.discovery.lock().clone();
    let Some(discovery) = discovery else { return };

    let room = shared.current_room();
    // Id da minha sala (só quando sou dono): não aparece na minha lista.
    let (my_room_id, announcement) = match room.as_deref() {
        Some(ActiveRoom::Owner(o)) => {
            let model = o.model.lock();
            let owner = model.participants.iter().find(|p| p.id == o.my_id).cloned();
            let info = owner.map(|owner| {
                (
                    model.room.clone(),
                    owner,
                    model.participants.len() as u32,
                    pick_highlight(&model.tracks),
                )
            });
            drop(model);
            (info.as_ref().map(|i| i.0.id.clone()), info.map(|i| (o.clone(), i)))
        }
        _ => (None, None),
    };

    let rooms: Vec<_> = discovery
        .rooms()
        .into_iter()
        .filter(|(a, _)| my_room_id.as_deref() != Some(a.room.id.as_str()))
        .collect();
    let peers = discovery.peers();
    let ifaces = zap_net::discovery::ipv4_interfaces();
    // Protocolo mais novo visto na rede: some sozinho quando o amigo deixa de ser visto.
    let maior = crate::diag_build::protocolo_maior(&discovery.other_versions(), zap_proto::PROTOCOL_VERSION);
    {
        let mut st = shared.state.lock();
        st.set_rooms(room_listings(&rooms));
        st.set_peers(peer_listings(&peers));
        st.set_network(network_status(&ifaces));
        st.set_newer_protocol(maior);
    }

    let announce = shared.settings.read().local.announce;
    match announcement {
        Some((o, (sala, dono, n, destaque))) if announce => {
            discovery.set_room(Some(room_announcement(&sala, &dono, o.control_address(), n, destaque)));
        }
        _ => discovery.set_room(None),
    }

    let (install_id, name) = {
        let s = shared.settings.read();
        (s.install_id.clone(), s.name.clone())
    };
    let hears = peers.iter().map(|p| p.install_id.clone()).collect();
    discovery.set_presence(presence_announcement(&install_id, &name, hears));
}

fn tick_tiles(shared: &Arc<Shared>) {
    // Clona a lista sem segurar o lock durante o resto do trabalho.
    let subs: Vec<_> = shared.subs.lock().iter().map(|(t, s)| (*t, s.clone())).collect();
    let now = shared.clock.now_us();
    for (tile, sub) in subs {
        // Só o som não tem camada nem quadro: não vira tile de vídeo.
        if sub.audio_only() {
            continue;
        }
        let v = sub.metrics().view(now);
        let smoothing_ms = shared
            .playout
            .clock_for(&sub.track.participant_id)
            .lock()
            .current_delay_ms()
            .round() as u32;
        let ping_ms = sub.conn.conn.rtt().as_millis() as u32;
        shared.state.lock().upsert_tile(TileView {
            tile,
            track_id: sub.track.id.clone(),
            layer: v.layer,
            width: v.width as u32,
            height: v.height as u32,
            fps: v.fps,
            kbps: v.kbps,
            ping_ms,
            smoothing_ms,
            reason: v.reason,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_proto::LayerSpec;

    fn trilha(number: u16, kind: TrackKind, height: u16, fps: u8) -> Track {
        Track {
            id: format!("t{number}"),
            participant_id: "p".into(),
            number,
            kind,
            title: String::new(),
            layers: vec![
                LayerSpec {
                    layer: 1,
                    width: 640,
                    height: 360,
                    fps: 30,
                    kbps: 800,
                },
                LayerSpec {
                    layer: 0,
                    width: height * 16 / 9,
                    height,
                    fps,
                    kbps: 6000,
                },
            ],
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    #[test]
    fn highlight_ultima_trilha() {
        let t = vec![
            trilha(1, TrackKind::Screen, 720, 30),
            trilha(3, TrackKind::Camera, 1080, 60),
            trilha(2, TrackKind::Audio, 0, 0),
        ];
        assert_eq!(pick_highlight(&t), Some((1080, 60)));
    }

    #[test]
    fn highlight_sem_video_none() {
        assert_eq!(pick_highlight(&[]), None);
        assert_eq!(pick_highlight(&[trilha(1, TrackKind::Audio, 0, 0)]), None);
    }

    #[test]
    fn tick_usa_protocolo_maior_da_descoberta() {
        use std::net::{IpAddr, Ipv4Addr};
        // Usa PROTOCOL_VERSION + 1 para o teste não quebrar quando o protocolo subir.
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let mais_novo = zap_proto::PROTOCOL_VERSION + 1;
        assert_eq!(
            crate::diag_build::protocolo_maior(&[(ip, mais_novo)], zap_proto::PROTOCOL_VERSION),
            Some(mais_novo)
        );
        assert_eq!(
            crate::diag_build::protocolo_maior(&[], zap_proto::PROTOCOL_VERSION),
            None
        );
    }
}
