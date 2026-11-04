//! `impl Core`: salas.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::discovery_view::room_announcement;
use crate::engine::{Core, Shared};
use crate::room::{ActiveRoom, MemberRoom, OwnerRoom, RoomEvent};
use crate::share::ShareSession;
use crate::subscribe::{Subscription, WatchSpec};
use crate::types::*;
use crate::CoreError;
use zap_proto::discovery::RoomAnnouncement;
use zap_proto::{TileSize, TrackKind};

impl Core {
    /// Salta para o runtime do núcleo. Já em sala → `AlreadyInRoom`. Dono: `OwnerRoom::start`,
    /// anuncia na descoberta (`settings.local.announce`), `ensure_media_server` +
    /// `set_auth`, `refresh_room_view`, `notifier.flush()`.
    pub async fn create_room(&self, req: CreateRoom) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                if shared.current_room().is_some() {
                    return Err(CoreError::AlreadyInRoom);
                }
                if req.mode == zap_proto::RoomMode::Online {
                    return crate::online::entry::create_online(&shared, req).await;
                }
                let room = OwnerRoom::start(shared.room_ctx(), req).await?;
                *shared.room.lock() = Some(Arc::new(ActiveRoom::Owner(room.clone())));
                // Depois de guardar a sala, para o `set_auth` enxergá-la.
                shared.ensure_media_server().await?;
                let discovery = shared.discovery.lock().clone();
                if let (true, Some(d)) = (shared.settings.read().local.announce, discovery) {
                    let anuncio = {
                        let model = room.model.lock();
                        let dono = model
                            .participants
                            .iter()
                            .find(|p| p.id == model.room.owner_id)
                            .or(model.participants.first())
                            .cloned();
                        dono.map(|dono| room_announcement(&model.room, &dono, room.control_address(), 1, None))
                    };
                    d.set_room(anuncio);
                }
                shared.refresh_room_view();
                shared.notifier.flush();
                Ok(())
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
    /// `Discovered{room_id}`: acha na descoberta (`RoomNotFound` se não); `Address{ip,port}`:
    /// impressão digital desconhecida → consulta a descoberta pelo endereço; sem anúncio →
    /// `RoomNotFound`. Então `MemberRoom::join`, `ensure_media_server`, `set_auth`,
    /// `refresh_room_view`, `flush`.
    pub async fn join_room(&self, target: JoinTarget, password: Option<String>) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                if shared.current_room().is_some() {
                    return Err(CoreError::AlreadyInRoom);
                }
                // Sala Online: pelo código digitado ou pelo `room_id` da lista do servidor.
                let target = match target {
                    JoinTarget::Discovered { room_id } => match shared.online.code_for_room_id(&room_id) {
                        Some(code) => JoinTarget::Code { code },
                        None => JoinTarget::Discovered { room_id },
                    },
                    outro => outro,
                };
                if let JoinTarget::Code { code } = target {
                    return crate::online::entry::join_online(&shared, code, password).await;
                }
                let vistas = shared.discovery.lock().clone().map(|d| d.rooms()).unwrap_or_default();
                let (addr, fingerprint, room_id) = match target {
                    JoinTarget::Code { .. } => return Err(CoreError::RoomNotFound),
                    JoinTarget::Discovered { room_id } => {
                        let (an, origem) = vistas
                            .iter()
                            .find(|(an, _)| an.room.id == room_id)
                            .ok_or(CoreError::RoomNotFound)?;
                        let ip = endereco_do_anuncio(an, *origem);
                        (
                            SocketAddr::new(ip, an.address.port),
                            an.address.fingerprint.clone(),
                            room_id,
                        )
                    }
                    JoinTarget::Address { ip, port } => {
                        let ip: IpAddr = ip
                            .parse()
                            .map_err(|_| CoreError::Invalid(format!("endereço IP inválido: {ip}")))?;
                        // Sem impressão digital não há como confiar no certificado.
                        let (an, _) = vistas
                            .iter()
                            .find(|(an, origem)| {
                                an.address.port == port
                                    && (an
                                        .address
                                        .addresses
                                        .iter()
                                        .any(|a| a.parse::<IpAddr>().ok() == Some(ip))
                                        || *origem == ip)
                            })
                            .ok_or(CoreError::RoomNotFound)?;
                        (
                            SocketAddr::new(ip, port),
                            an.address.fingerprint.clone(),
                            an.room.id.clone(),
                        )
                    }
                };
                let room = MemberRoom::join(shared.room_ctx(), addr, fingerprint, password, Some(room_id)).await?;
                *shared.room.lock() = Some(Arc::new(ActiveRoom::Member(room)));
                shared.ensure_media_server().await?;
                shared.refresh_room_view();
                shared.notifier.flush();
                Ok(())
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
    /// Só para testes: entra na sala com endereço e impressão digital já conhecidos (sem descoberta).
    #[doc(hidden)]
    pub async fn join_room_pinned(
        &self,
        addr: SocketAddr,
        fingerprint: String,
        password: Option<String>,
    ) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                if shared.current_room().is_some() {
                    return Err(CoreError::AlreadyInRoom);
                }
                let room = MemberRoom::join(shared.room_ctx(), addr, fingerprint, password, None).await?;
                *shared.room.lock() = Some(Arc::new(ActiveRoom::Member(room)));
                shared.ensure_media_server().await?;
                shared.refresh_room_view();
                shared.notifier.flush();
                Ok(())
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
    /// Só para testes: porta de controle da sala própria.
    #[doc(hidden)]
    pub fn control_port(&self) -> Option<u16> {
        match self.shared.current_room()?.as_ref() {
            ActiveRoom::Owner(o) => Some(o.control_address().port),
            _ => None,
        }
    }
    /// Só para testes: impressão digital da sala própria.
    #[doc(hidden)]
    pub fn control_fingerprint(&self) -> Option<String> {
        match self.shared.current_room()?.as_ref() {
            ActiveRoom::Owner(o) => Some(o.control_address().fingerprint),
            _ => None,
        }
    }
    /// Para compartilhamento e assinaturas, `room.leave()`, remove o anúncio, limpa o estado,
    /// `flush`.
    pub async fn leave_room(&self) {
        let shared = self.shared.clone();
        let _ = shared
            .rt
            .clone()
            .spawn(async move {
                // Sai da sala (avisa os outros) depois de limpar; `clear_room` já a tirou do estado.
                if let Some(sala) = clear_room(&shared).await {
                    sala.leave().await;
                }
            })
            .await;
    }
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                let sala = shared.current_room().ok_or(CoreError::NoRoom)?;
                sala.send_chat(text).await
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                let sala = shared.current_room().ok_or(CoreError::NoRoom)?;
                sala.send_typing().await
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
}

/// IP do anúncio: primeiro endereço válido anunciado ou, na falta, o ip de origem do pacote.
fn endereco_do_anuncio(an: &RoomAnnouncement, origem: IpAddr) -> IpAddr {
    an.address
        .addresses
        .first()
        .and_then(|a| a.parse().ok())
        .unwrap_or(origem)
}

/// Encerra tudo da sala atual SEM chamar `room.leave()`; devolve a sala para quem quiser sair
/// dela (o `leave_room`). Quando a sala já fechou (`Closed`) basta descartar o retorno.
async fn clear_room(shared: &Arc<Shared>) -> Option<Arc<ActiveRoom>> {
    let sala = shared.room.lock().take();
    ShareSession::stop(shared).await;
    let subs: Vec<_> = shared.subs.lock().drain().map(|(_, s)| s).collect();
    for s in subs {
        s.stop(shared).await;
    }
    shared.pool.close_all().await;
    if let Some(ms) = shared.media_server.lock().await.clone() {
        ms.set_auth(None);
    }
    let discovery = shared.discovery.lock().clone();
    if let Some(d) = discovery {
        d.set_room(None);
    }
    {
        let mut st = shared.state.lock();
        st.set_room(None);
        let tiles: Vec<_> = st.tiles().iter().map(|t| t.tile).collect();
        for t in tiles {
            st.remove_tile(t);
        }
        st.set_share(None);
    }
    shared.notifier.touch();
    shared.notifier.flush();
    sala
}

/// Separa `ip:porta#fingerprint` (formato que `PubConn::connect` monta no pedido reverso).
fn parse_reverse_address(address: &str) -> Option<(SocketAddr, String)> {
    let (addr, fp) = address.split_once('#')?;
    if fp.is_empty() {
        return None;
    }
    Some((addr.parse().ok()?, fp.to_owned()))
}

/// Reassina as trilhas que eu assistia (ids/endereços podem ter mudado após reconectar).
async fn reassinar_tiles(shared: &Arc<Shared>) {
    let Some(sala) = shared.current_room() else { return };
    let antigas: Vec<_> = shared.subs.lock().drain().map(|(_, s)| s).collect();
    for antiga in antigas {
        let tile = antiga.tile;
        let track_id = antiga.track.id.clone();
        antiga.stop(shared).await;
        let trilhas = sala.model().lock().tracks.clone();
        let Some(track) = trilhas.iter().find(|t| t.id == track_id).cloned() else {
            shared.state.lock().remove_tile(tile);
            continue;
        };
        let audio_track = trilhas
            .iter()
            .find(|t| t.participant_id == track.participant_id && t.kind == TrackKind::Audio)
            .cloned();
        let spec = WatchSpec {
            track,
            audio_track,
            tile,
            size: TileSize::Main,
            max_layer: None,
        };
        match Subscription::start(shared, spec).await {
            Ok(sub) => {
                shared.subs.lock().insert(tile, Arc::new(sub));
            }
            Err(e) => {
                let mut st = shared.state.lock();
                st.remove_tile(tile);
                st.push_error(format!("Não foi possível reassinar a trilha: {e}"));
            }
        }
    }
    shared.notifier.touch();
}

/// Tarefa que consome `RoomEvent`: `Changed` → `refresh_room_view`; `Chat` → `ui.chat` +
/// `refresh`; `Closed` → limpa a sala como `leave_room` e `push_error` com o motivo;
/// `Reconnected` → `ShareSession::republish` e reassina os tiles; `TrackRemoved(id)` → encerra a
/// assinatura dessa trilha; `ReverseRequest` → `media_server.connect_back`; `Error` → `push_error`.
pub(crate) async fn pump_room_events(shared: Arc<Shared>, mut rx: UnboundedReceiver<RoomEvent>) {
    while let Some(ev) = rx.recv().await {
        if shared.stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        match ev {
            RoomEvent::Changed => shared.refresh_room_view(),
            RoomEvent::Chat(linha) => {
                shared.ui.chat(linha);
                shared.refresh_room_view();
            }
            RoomEvent::Closed(motivo) => {
                clear_room(&shared).await;
                shared.state.lock().push_error(format!("Sala encerrada ({motivo})"));
                shared.notifier.touch();
                shared.notifier.flush();
            }
            RoomEvent::Reconnecting => {
                shared.state.lock().push_error("Reconectando à sala…".into());
                shared.notifier.touch();
            }
            RoomEvent::Reconnected => {
                shared.state.lock().clear_errors();
                if let Err(e) = ShareSession::republish(&shared).await {
                    shared
                        .state
                        .lock()
                        .push_error(format!("Não foi possível retomar o compartilhamento: {e}"));
                }
                reassinar_tiles(&shared).await;
                shared.refresh_room_view();
            }
            RoomEvent::TrackRemoved(id) => {
                let alvo = {
                    let mut subs = shared.subs.lock();
                    let tile = subs.iter().find(|(_, s)| s.track.id == id).map(|(t, _)| *t);
                    tile.and_then(|t| subs.remove(&t).map(|s| (t, s)))
                };
                if let Some((tile, sub)) = alvo {
                    sub.stop(&shared).await;
                    shared.state.lock().remove_tile(tile);
                    shared.notifier.touch();
                }
            }
            RoomEvent::ReverseRequest { address, token, .. } => {
                // O espectador manda `ip:porta#fingerprint`; sem a impressão digital não há como
                // pinar o certificado dele.
                let Some((addr, fp)) = parse_reverse_address(&address) else {
                    shared
                        .state
                        .lock()
                        .push_error(format!("Pedido de conexão reversa malformado: {address}"));
                    shared.notifier.touch();
                    continue;
                };
                let ms = shared.media_server.lock().await.clone();
                let Some(ms) = ms else {
                    shared
                        .state
                        .lock()
                        .push_error("Conexão reversa sem servidor de mídia".into());
                    shared.notifier.touch();
                    continue;
                };
                let sh = shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = ms.connect_back(addr, &fp, &token).await {
                        sh.state.lock().push_error(format!("Conexão reversa falhou: {e}"));
                        sh.notifier.touch();
                    }
                });
            }
            RoomEvent::Error(msg) => {
                shared.state.lock().push_error(msg);
                shared.notifier.touch();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TestSink;
    use std::path::PathBuf;
    use std::time::Duration;
    use zap_proto::RoomMode;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("zapcore-room-{}", uuid::Uuid::new_v4()))
    }
    fn opts() -> crate::engine::CoreOptions {
        crate::engine::CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        }
    }
    fn req() -> CreateRoom {
        CreateRoom {
            name: "Sala X".into(),
            mode: RoomMode::Local,
            password: None,
        }
    }
    fn novo() -> (Core, Arc<TestSink>, PathBuf) {
        let d = dir();
        let sink = TestSink::new();
        let core = Core::start_with(d.clone(), sink.clone(), opts()).expect("core");
        (core, sink, d)
    }

    /// `Core::drop` faz `block_on`: fora do contexto assíncrono do teste.
    async fn encerrar(core: Core, d: PathBuf) {
        let _ = tokio::task::spawn_blocking(move || drop(core)).await;
        let _ = std::fs::remove_dir_all(d);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn criar_sala_aparece_no_snapshot() {
        let (core, _s, d) = novo();
        core.create_room(req()).await.expect("criar");
        let snap = core.snapshot();
        let room = snap.room.as_ref().expect("sala");
        assert_eq!(room.participants.len(), 1);
        assert_eq!(room.name, "Sala X");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn criar_duas_vezes_already_in_room() {
        let (core, _s, d) = novo();
        core.create_room(req()).await.expect("criar");
        assert!(matches!(core.create_room(req()).await, Err(CoreError::AlreadyInRoom)));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn entrar_por_endereco_sem_descoberta_room_not_found() {
        let (core, _s, d) = novo();
        let r = core
            .join_room(
                JoinTarget::Address {
                    ip: "127.0.0.1".into(),
                    port: 47800,
                },
                None,
            )
            .await;
        assert!(matches!(r, Err(CoreError::RoomNotFound)));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sair_limpa_snapshot_room_none() {
        let (core, _s, d) = novo();
        core.create_room(req()).await.expect("criar");
        core.leave_room().await;
        assert!(core.snapshot().room.is_none());
        encerrar(core, d).await;
    }

    #[test]
    fn reverse_address_ok() {
        let (a, fp) = parse_reverse_address("10.0.0.2:4000#abc").expect("ok");
        assert_eq!(a, SocketAddr::from(([10, 0, 0, 2], 4000)));
        assert_eq!(fp, "abc");
    }
    #[test]
    fn reverse_address_sem_fingerprint() {
        assert!(parse_reverse_address("10.0.0.2:4000").is_none());
        assert!(parse_reverse_address("10.0.0.2:4000#").is_none());
    }
    #[test]
    fn reverse_address_ipv6_invalido() {
        assert!(parse_reverse_address("::zz:4000#abc").is_none());
        assert!(parse_reverse_address("[::1]:4000#abc").is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn evento_chat_chega_ao_sink() {
        let (core, sink, d) = novo();
        let linha = ChatLine {
            id: "m1".into(),
            author_id: "p".into(),
            author: "x".into(),
            color: "#fff".into(),
            text: "oi".into(),
            ts: 1,
        };
        let _ = core.shared.room_events.send(RoomEvent::Chat(linha));
        assert!(
            sink.wait_for(Duration::from_secs(1), |s| !s.chats.lock().is_empty())
                .await
        );
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn evento_closed_limpa_sala_e_registra_erro() {
        let (core, _s, d) = novo();
        core.create_room(req()).await.expect("criar");
        let _ = core.shared.room_events.send(RoomEvent::Closed("dono".into()));
        for _ in 0..50 {
            if core.snapshot().room.is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let snap = core.snapshot();
        assert!(snap.room.is_none());
        assert!(!snap.errors.is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reverse_request_malformado_vira_erro() {
        let (core, _s, d) = novo();
        let _ = core.shared.room_events.send(RoomEvent::ReverseRequest {
            track_id: "t".into(),
            address: "10.0.0.2:4000".into(),
            token: "k".into(),
        });
        for _ in 0..50 {
            if !core.snapshot().errors.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!core.snapshot().errors.is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_do_dono_chega_ao_sink_do_dono() {
        let (core, sink, d) = novo();
        core.create_room(req()).await.expect("criar");
        core.send_chat("oi".into()).await.expect("chat");
        assert!(
            sink.wait_for(Duration::from_secs(1), |s| !s.chats.lock().is_empty())
                .await
        );
        encerrar(core, d).await;
    }
}
