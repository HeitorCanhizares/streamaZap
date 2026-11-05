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
    /// Só direta: tiles esperando a rota direta.
    // Provisório: quem lê/escreve é a core-06 (fora da lib por enquanto; sem isso o clippy -D warnings falha).
    #[allow(dead_code)]
    pub(crate) pending: Mutex<HashMap<crate::types::TileId, crate::subscribe::WatchSpec>>,
    /// Só direta: conexão direta por trilha (com ponte elas ficam em `bridge.direct`).
    pub(crate) conns: Mutex<HashMap<String, Arc<PubConn>>>,
}

/// Sem ponte: guarda o pedido e manda `assinar` pelo WebSocket (é o que faz o servidor mandar
/// `p2p.candidatos`). Mesmo tile com a mesma trilha: troca tamanho/teto e reenvia.
///
/// Sem WebSocket ativo, o pedido fica guardado em `pending` mesmo assim e a função devolve
/// `Err(Connect)`. A reconexão (`reconnect.rs`) ainda não reenvia `pending`, só `shared.subs`.
#[allow(dead_code)] // Provisório até a core-06 chamar.
pub(crate) async fn watch_pending(room: &Arc<OnlineRoom>, spec: crate::subscribe::WatchSpec) -> Result<(), CoreError> {
    // Campos extraídos antes do `insert`, que move o `spec`.
    let copia = spec.clone();
    room.direct.pending.lock().insert(spec.tile, spec);
    enviar_assinar(room, &copia).await
}

/// Manda `assinar` pelo WebSocket para o pedido dado (é o que faz o servidor mandar `p2p.candidatos`).
async fn enviar_assinar(room: &Arc<OnlineRoom>, spec: &crate::subscribe::WatchSpec) -> Result<(), CoreError> {
    let ws = room
        .session
        .ws()
        .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
    ws.send(ControlMsg::Subscribe {
        track_id: spec.track.id.clone(),
        size: spec.size,
        max_layer: spec.max_layer,
        audio: Some(spec.audio_track.is_some()),
    })
    .await
    .map_err(|e| CoreError::Connect(e.to_string()))
}

/// Tira o tile de `pending` (e manda `cancelar` pelo WebSocket). `false` se não estava lá.
#[allow(dead_code)] // Provisório até a core-06 chamar.
pub(crate) async fn unwatch_pending(room: &Arc<OnlineRoom>, tile: crate::types::TileId) -> bool {
    // Guarda solto antes de qualquer `await`.
    let removido = room.direct.pending.lock().remove(&tile);
    let Some(spec) = removido else {
        return false;
    };
    if let Some(ws) = room.session.ws() {
        // Erro ao cancelar é ignorado: o WebSocket pode ter caído (como em `Subscription::stop`).
        let _ = ws
            .send(ControlMsg::Unsubscribe {
                track_id: spec.track.id,
            })
            .await;
    }
    true
}

/// A trilha está assistida (em `shared.subs` ou em `pending`)?
#[allow(dead_code)] // Provisório até a core-06 chamar.
pub(crate) fn is_watched(room: &OnlineRoom, shared: &crate::engine::Shared, track_id: &str) -> bool {
    shared.subs.lock().values().any(|s| s.track.id == track_id)
        || room.direct.pending.lock().values().any(|w| w.track.id == track_id)
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
            if !deve_tentar(&room, &shared, &track_id) {
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

/// Vale tentar a rota direta? A trilha existe no modelo e eu a assisto (em `subs` ou `pending`).
fn deve_tentar(room: &OnlineRoom, shared: &crate::engine::Shared, track_id: &str) -> bool {
    let existe = room.model.lock().tracks.iter().any(|t| t.id == track_id);
    existe && is_watched(room, shared, track_id)
}

async fn tentar_direta(
    room: &Arc<OnlineRoom>,
    shared: &Arc<crate::engine::Shared>,
    track_id: &str,
    addresses: &[String],
    fingerprint: &str,
) {
    // Sem ponte habilitada, a direta é a única rota e usa um `Router` próprio.
    let bridge = if room.bridge_enabled(shared) {
        room.bridge.lock().await.clone()
    } else {
        None
    };
    let ticket = make_ticket(&room.media_secret, &room.my_id);
    let room_id = room.model.lock().room.id.clone();
    let router = match &bridge {
        Some(b) => b.router.clone(),
        None => Arc::new(crate::subscribe::Router::new()),
    };
    let direta = PubConn::connect_direct(shared, addresses, fingerprint, &ticket, &room_id, router).await;
    let Some(direta) = direta else {
        falhou(room, track_id).await;
        return;
    };
    let Some(bridge) = bridge else {
        tentar_direta_sem_ponte(room, shared, track_id, direta).await;
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
    marcar_direta_ok(room, track_id).await;
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

/// Só direta: assina em `direta` cada tile pendente desta trilha e promove o tile a `shared.subs`.
async fn tentar_direta_sem_ponte(
    room: &Arc<OnlineRoom>,
    shared: &Arc<crate::engine::Shared>,
    track_id: &str,
    direta: Arc<PubConn>,
) {
    // Cópia dos pedidos: nenhum lock fica preso durante os `await`.
    let specs: Vec<crate::subscribe::WatchSpec> = room
        .direct
        .pending
        .lock()
        .values()
        .filter(|w| w.track.id == track_id)
        .cloned()
        .collect();
    let mut nasceu = false;
    for spec in specs {
        let tile = spec.tile;
        let trilha = spec.track.id.clone();
        let Ok(sub) = crate::subscribe::Subscription::start_on(shared, spec, direta.clone()).await else {
            continue;
        };
        // O tile pode ter saído (unwatch) ou mudado de trilha enquanto assinava.
        let ainda = {
            let mut p = room.direct.pending.lock();
            if p.get(&tile).is_some_and(|w| w.track.id == trilha) {
                p.remove(&tile);
                true
            } else {
                false
            }
        };
        if ainda {
            shared.subs.lock().insert(tile, Arc::new(sub));
            nasceu = true;
        } else {
            sub.stop(shared).await;
        }
    }
    if !nasceu {
        direta.close();
        falhou(room, track_id).await;
        return;
    }
    room.direct.conns.lock().insert(track_id.to_owned(), direta);
    marcar_direta_ok(room, track_id).await;
    // Vigia a queda: os tiles voltam a pendentes e o `assinar` é reenviado pelo WebSocket.
    let (room2, shared2, track) = (room.clone(), shared.clone(), track_id.to_owned());
    let conn = room.direct.conns.lock().get(track_id).cloned();
    if let Some(direta) = conn {
        tokio::spawn(async move {
            direta.conn.closed().await;
            let specs = voltar_a_pendente(&room2, &shared2, &track, &direta);
            if room2.left.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            for spec in specs {
                let _ = enviar_assinar(&room2, &spec).await;
            }
        });
    }
}

/// Queda da direta sem ponte: se `direta` ainda é a conexão da trilha, tira a trilha de `conns` e
/// `routes`, para as assinaturas dela e devolve cada tile para `pending`. Devolve os pedidos
/// que precisam de novo `assinar`. Se a conexão não é mais a atual, não faz nada.
fn voltar_a_pendente(
    room: &OnlineRoom,
    shared: &Arc<crate::engine::Shared>,
    track_id: &str,
    direta: &Arc<PubConn>,
) -> Vec<crate::subscribe::WatchSpec> {
    {
        let mut c = room.direct.conns.lock();
        match c.get(track_id) {
            Some(atual) if Arc::ptr_eq(atual, direta) => {
                c.remove(track_id);
            }
            _ => return Vec::new(),
        }
    }
    room.direct.routes.lock().remove(track_id);
    let caidas: Vec<Arc<crate::subscribe::Subscription>> = {
        let mut subs = shared.subs.lock();
        let tiles: Vec<_> = subs
            .iter()
            .filter(|(_, s)| s.track.id == track_id && Arc::ptr_eq(&s.conn, direta))
            .map(|(t, _)| *t)
            .collect();
        tiles.iter().filter_map(|t| subs.remove(t)).collect()
    };
    let mut specs = Vec::new();
    for sub in caidas {
        sub.stop_local(shared);
        shared.state.lock().remove_tile(sub.tile);
        shared.notifier.touch();
        let spec = sub.spec();
        room.direct.pending.lock().insert(sub.tile, spec.clone());
        specs.push(spec);
    }
    specs
}

/// Registra a rota direta e avisa o servidor (`cancelar` na ponte + `midia.direta_ok`).
async fn marcar_direta_ok(room: &Arc<OnlineRoom>, track_id: &str) {
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
}

/// Texto do aviso quando a direta falha e não há ponte.
fn msg_falha_direta(nome: &str) -> String {
    format!("Não deu para conectar direto em {nome}. Ligue “Usar o servidor como ponte” em Configurações para assistir pelo servidor.")
}

async fn falhou(room: &Arc<OnlineRoom>, track_id: &str) {
    if let Ok(shared) = room.shared() {
        if !room.bridge_enabled(&shared) {
            let nome = {
                let m = room.model.lock();
                m.tracks.iter().find(|t| t.id == track_id).map(|t| {
                    m.participants
                        .iter()
                        .find(|p| p.id == t.participant_id)
                        .map_or_else(|| t.participant_id.clone(), |p| p.name.clone())
                })
            };
            if let Some(nome) = nome {
                shared.state.lock().push_error(msg_falha_direta(&nome));
                shared.notifier.touch();
            }
        }
    }
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
    fn mensagem_direta_falhou_formato() {
        assert_eq!(
            msg_falha_direta("Ana"),
            "Não deu para conectar direto em Ana. Ligue “Usar o servidor como ponte” em Configurações para assistir pelo servidor."
        );
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

    fn spec_pendente(track_id: &str, tile: crate::types::TileId, size: TileSize) -> crate::subscribe::WatchSpec {
        let track: zap_proto::Track = serde_json::from_value(serde_json::json!({
            "id": track_id, "participante_id": "p2", "numero": 1, "tipo": "tela", "titulo": "t"
        }))
        .unwrap();
        crate::subscribe::WatchSpec {
            track,
            audio_track: None,
            tile,
            size,
            max_layer: None,
        }
    }

    #[test]
    fn is_watched_ve_subs_e_pending() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        assert!(!is_watched(&room, &core.shared, "t1"));
        // Pending: a trilha conta como assistida.
        room.direct
            .pending
            .lock()
            .insert(7, spec_pendente("t1", 7, TileSize::Main));
        assert!(is_watched(&room, &core.shared, "t1"));
        assert!(!is_watched(&room, &core.shared, "outra"));
        // `shared.subs` não é exercitado aqui: montar uma `Subscription` exige `PubConn` com
        // ponte viva (coberto nos testes de `subscribe`).
    }

    #[test]
    fn deve_tentar_considera_modelo_e_pending() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        assert!(!deve_tentar(&room, &core.shared, "t1"));
        room.direct
            .pending
            .lock()
            .insert(7, spec_pendente("t1", 7, TileSize::Main));
        // Assistida mas fora do modelo: não tenta.
        assert!(!deve_tentar(&room, &core.shared, "t1"));
        let track = spec_pendente("t1", 7, TileSize::Main).track;
        room.model.lock().tracks.push(track);
        assert!(deve_tentar(&room, &core.shared, "t1"));
        assert!(!deve_tentar(&room, &core.shared, "outra"));
    }

    #[test]
    fn unwatch_pending_false_se_ausente() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        assert!(!rt.block_on(unwatch_pending(&room, 7)));
        room.direct
            .pending
            .lock()
            .insert(7, spec_pendente("t1", 7, TileSize::Main));
        assert!(rt.block_on(unwatch_pending(&room, 7)));
        assert!(room.direct.pending.lock().is_empty());
        // Segunda vez: já saiu.
        assert!(!rt.block_on(unwatch_pending(&room, 7)));
    }

    #[test]
    fn watch_pending_guarda_e_troca_spec() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let core = core();
        let room = sala(&core, true);
        // Sem WebSocket ativo: erro, mas o pedido já ficou guardado.
        let r = rt.block_on(watch_pending(&room, spec_pendente("t1", 7, TileSize::Main)));
        assert!(matches!(r, Err(CoreError::Connect(_))));
        {
            let p = room.direct.pending.lock();
            assert_eq!(p.len(), 1);
            assert_eq!(p[&7].size, TileSize::Main);
        }
        // Mesmo tile e trilha: o insert troca tamanho e teto (continua sendo um único tile).
        let mut troca = spec_pendente("t1", 7, TileSize::Thumbnail);
        troca.max_layer = Some(1);
        let _ = rt.block_on(watch_pending(&room, troca));
        let p = room.direct.pending.lock();
        assert_eq!(p.len(), 1);
        assert_eq!(p[&7].size, TileSize::Thumbnail);
        assert_eq!(p[&7].max_layer, Some(1));
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
