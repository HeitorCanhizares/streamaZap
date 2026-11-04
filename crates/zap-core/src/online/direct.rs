//! Rota direta (`p2p.*`): tenta ligar espectador e publicador sem passar pela ponte.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use zap_proto::control::{ControlMsg, P2pRole, Route};
use zap_proto::{MediaAddress, TileSize, TrackKind};

use super::room::OnlineRoom;
use crate::subscribe::PubConn;
use crate::ticket::make_ticket;
use crate::CoreError;

/// Tempo da tentativa direta (docs/api/mensagens.md, "Rota direta").
pub const DIRECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Default)]
pub struct DirectState {
    /// Rota em uso por trilha assistida (ausente = ponte).
    pub(crate) routes: Mutex<HashMap<String, Route>>,
    /// Trilhas para as quais já mandei `midia.direta_ok` (visível aos testes).
    pub(crate) ok_sent: Mutex<std::collections::HashSet<String>>,
    /// Trilhas com tentativa direta em andamento (chave: `"<trilha>|<com>"`).
    pub(crate) trying: Mutex<std::collections::HashSet<String>>,
}

impl DirectState {
    pub fn route_of(&self, track_id: &str) -> Route {
        self.routes.lock().get(track_id).copied().unwrap_or(Route::Relay)
    }
}

/// Depois de entrar (e a cada reconexão): `P2pMyAddresses{addresses: "ip:porta" do
/// `MediaServer::address` (interfaces locais; `include_loopback` das opções), fingerprint}`
/// pelo WebSocket. Só se `media.direct_allowed`.
pub(crate) async fn send_my_addresses(room: &Arc<OnlineRoom>) -> Result<(), CoreError> {
    if !room.media.lock().direct_allowed {
        return Ok(());
    }
    let shared = room.shared()?;
    let ms = shared.ensure_media_server().await?;
    let interface = shared.settings.read().local.interface.clone();
    let addr = ms.address(interface.as_deref(), shared.options.include_loopback);
    let ws = room
        .session
        .ws()
        .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
    let port = addr.port;
    let msg = ControlMsg::P2pMyAddresses {
        addresses: addr.addresses.iter().map(|ip| format!("{ip}:{port}")).collect(),
        fingerprint: addr.fingerprint,
    };
    ws.send(msg).await.map_err(|e| CoreError::Connect(e.to_string()))
}

/// `p2p.candidatos`:
/// - `Client` (eu assisto): conecta direto no publicador (3 s, endereços em paralelo). Sucesso:
///   a trilha passa para a direta, o servidor recebe `cancelar` e `midia.direta_ok`. Falha:
///   `midia.direta_falhou` e segue pela ponte. Uma tentativa por vez por `"<trilha>|<com>"`.
///   A direta que cair some de `bridge.direct` e a trilha volta para a ponte;
/// - `Server` (eu publico): para cada endereço, `endpoint.connect(addr, "zap")` e solta
///   (só para abrir o NAT; o espectador é quem conecta de verdade).
pub(crate) fn on_candidates(
    room: &Arc<OnlineRoom>,
    track_id: String,
    with: String,
    role: P2pRole,
    addresses: Vec<String>,
    fingerprint: String,
) {
    if !room.media.lock().direct_allowed {
        return;
    }
    let Ok(shared) = room.shared() else { return };
    let room = room.clone();
    match role {
        P2pRole::Client => {
            // Só tenta se a trilha existe no modelo e eu a assisto.
            let existe = room.model.lock().tracks.iter().any(|t| t.id == track_id);
            let assisto = shared.subs.lock().values().any(|s| s.track.id == track_id);
            if !existe || !assisto {
                return;
            }
            let chave = format!("{track_id}|{with}");
            if !room.direct.trying.lock().insert(chave.clone()) {
                return;
            }
            let rt = shared.rt.clone();
            rt.spawn(async move {
                tentar_direta(&room, &shared, &track_id, &addresses, &fingerprint).await;
                room.direct.trying.lock().remove(&chave);
            });
        }
        P2pRole::Server => {
            let rt = shared.rt.clone();
            rt.spawn(async move {
                let Ok(ms) = shared.ensure_media_server().await else {
                    return;
                };
                for a in addresses.iter().filter_map(|a| a.parse::<std::net::SocketAddr>().ok()) {
                    if let Ok(connecting) = ms.endpoint.connect(a, "zap") {
                        // Só abre o NAT: espera pouco e descarta o resultado.
                        tokio::spawn(async move {
                            let _ = tokio::time::timeout(DIRECT_TIMEOUT, connecting).await;
                        });
                    }
                }
            });
        }
    }
}

async fn tentar_direta(
    room: &Arc<OnlineRoom>,
    shared: &Arc<crate::engine::Shared>,
    track_id: &str,
    addresses: &[String],
    fingerprint: &str,
) {
    let Some(bridge) = room.bridge.lock().await.clone() else {
        falhou(room, track_id).await;
        return;
    };
    let ticket = make_ticket(&room.media_secret, &room.my_id);
    let room_id = room.model.lock().room.id.clone();
    let direta =
        PubConn::connect_direct(shared, addresses, fingerprint, &ticket, &room_id, bridge.router.clone()).await;
    let Some(direta) = direta else {
        falhou(room, track_id).await;
        return;
    };
    // Áudio: só se o dono da trilha tem uma trilha de áudio no modelo.
    let audio = {
        let m = room.model.lock();
        m.tracks.iter().find(|t| t.id == track_id).is_some_and(|t| {
            m.tracks
                .iter()
                .any(|o| o.kind == TrackKind::Audio && o.participant_id == t.participant_id)
        })
    };
    if direta.subscribe(track_id, TileSize::Main, None, audio).await.is_err() {
        direta.close();
        falhou(room, track_id).await;
        return;
    }
    bridge.direct.lock().insert(track_id.to_owned(), direta.clone());
    room.direct.routes.lock().insert(track_id.to_owned(), Route::Direct);
    room.direct.ok_sent.lock().insert(track_id.to_owned());
    if let Some(ws) = room.session.ws() {
        let _ = ws
            .send(ControlMsg::Unsubscribe {
                track_id: track_id.to_owned(),
            })
            .await;
        let _ = ws
            .send(ControlMsg::DirectOk {
                track_id: track_id.to_owned(),
            })
            .await;
    }
    // Vigia a queda: a trilha volta para a ponte.
    let (room2, track, bridge2) = (room.clone(), track_id.to_owned(), bridge.clone());
    tokio::spawn(async move {
        direta.conn.closed().await;
        let atual = {
            let mut d = bridge2.direct.lock();
            match d.get(&track) {
                Some(c) if Arc::ptr_eq(c, &direta) => {
                    d.remove(&track);
                    true
                }
                _ => false,
            }
        };
        if !atual {
            return;
        }
        room2.direct.routes.lock().remove(&track);
        let _ = bridge2.subscribe(&track, TileSize::Main, None, audio).await;
    });
}

async fn falhou(room: &Arc<OnlineRoom>, track_id: &str) {
    if let Some(ws) = room.session.ws() {
        let _ = ws
            .send(ControlMsg::DirectFailed {
                track_id: track_id.to_owned(),
            })
            .await;
    }
}

/// `midia.rota`: guarda a rota (`ponte`/`direta`) em `routes`. Com `direta` e `address`, o
/// servidor está mandando usar a rota direta já negociada: não faz nada além de guardar.
pub(crate) fn on_media_route(room: &Arc<OnlineRoom>, track_id: &str, route: Route, address: Option<MediaAddress>) {
    let _ = address;
    room.direct.routes.lock().insert(track_id.to_owned(), route);
}

#[cfg(test)]
mod tests {
    use zap_net::online::JoinMedia;
    use zap_proto::{Access, Participant, Role, Room, RoomCode, RoomMode};

    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::room::RoomModel;
    use crate::testutil::TestSink;

    fn core() -> Core {
        let dir = std::env::temp_dir().join(format!("zapcore-direta-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        Core::start_with(dir, TestSink::new(), opts).unwrap()
    }

    fn sala(core: &Core, direct_allowed: bool) -> Arc<OnlineRoom> {
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
                direct_allowed,
            },
            String::new(),
        )
    }

    #[test]
    fn route_of_padrao_e_ponte() {
        assert_eq!(DirectState::default().route_of("t1"), Route::Relay);
    }

    #[test]
    fn on_media_route_guarda_rota() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        on_media_route(&room, "t1", Route::Direct, None);
        assert_eq!(room.direct.route_of("t1"), Route::Direct);
        on_media_route(&room, "t1", Route::Relay, None);
        assert_eq!(room.direct.route_of("t1"), Route::Relay);
    }

    #[test]
    fn send_my_addresses_sem_direct_allowed_nao_envia() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, false);
        assert!(rt.block_on(send_my_addresses(&room)).is_ok());
    }

    #[test]
    fn candidatos_cliente_ignorados_sem_assinatura() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        on_candidates(
            &room,
            "t1".into(),
            "p2".into(),
            P2pRole::Client,
            vec!["127.0.0.1:1".into()],
            "fp".into(),
        );
        rt.block_on(tokio::time::sleep(std::time::Duration::from_millis(100)));
        assert!(room.direct.trying.lock().is_empty());
        assert_eq!(room.direct.route_of("t1"), Route::Relay);
    }

    #[test]
    fn candidatos_servidor_abre_nat_sem_panico() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        on_candidates(
            &room,
            "t1".into(),
            "p2".into(),
            P2pRole::Server,
            vec!["127.0.0.1:9".into(), "lixo".into(), "".into()],
            "fp".into(),
        );
        rt.block_on(tokio::time::sleep(std::time::Duration::from_millis(200)));
        assert!(room.direct.trying.lock().is_empty());
    }
}
