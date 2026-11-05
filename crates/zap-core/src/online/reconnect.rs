//! Reconexão do WebSocket e da ponte durante uma sala Online.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use zap_proto::{ControlMsg, TileSize};

use super::backoff::Backoff;
use super::bridge::connect_bridge;
use super::direct;
use super::room::OnlineRoom;
use super::room_events;
use crate::engine::Shared;
use crate::room::RoomEvent;
use crate::subscribe::PubConn;
use crate::CoreError;

/// Espera pelo `sala.estado` depois de um novo `sala.entrar`.
const TEMPO_ESTADO: Duration = Duration::from_secs(10);

fn saiu(room: &OnlineRoom) -> bool {
    room.left.load(Ordering::SeqCst)
}

/// Pede um ingresso novo (`client.join`), conecta a ponte (`bridge::connect_bridge`), monta a
/// `PubConn` da ponte (`PubConn::connect_online`) e a guarda em `room.bridge` e no `ConnPool`;
/// `publish.reattach_all` (ou `attach` das trilhas guardadas) na conexão nova. Usada na entrada
/// e na reconexão. Sem `use_relay` ou sem `media.bridge`: `Ok(())` sem fazer nada.
pub(crate) async fn connect_bridge_for(room: &Arc<OnlineRoom>) -> Result<(), CoreError> {
    let shared = room.shared()?;
    if !room.bridge_enabled(&shared) {
        return Ok(());
    }
    let client = room
        .session
        .client()
        .ok_or_else(|| CoreError::Connect("servidor não conectado".into()))?;
    // `entrar` é idempotente: devolve um ingresso novo e a `midia` atual.
    let resp = client.join(&room.code, room.password.as_deref()).await?;
    *room.ticket.lock() = resp.ticket.clone();
    *room.media.lock() = resp.media.clone();
    let Some(bridge) = resp.media.bridge else {
        return Ok(());
    };
    let ws = room
        .session
        .ws()
        .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
    let room_id = room.model.lock().room.id.clone();
    let link = connect_bridge(&shared, &bridge.address, &resp.ticket, &room_id, false).await?;
    // `camadas.pedidas`/`keyframe.pedir`/`camada.atual` chegam pelo controle da ponte. `Weak`
    // para a ponte não segurar a sala viva depois de `leave`.
    let fraca = Arc::downgrade(room);
    let on_media: Arc<dyn Fn(ControlMsg) + Send + Sync> = Arc::new(move |m| {
        if let Some(r) = fraca.upgrade() {
            room_events::handle_media_msg(&r, m);
        }
    });
    let pc = PubConn::connect_online(&shared, link, ws, Some(on_media));

    let anterior = room.bridge.lock().await.replace(pc.clone());
    *shared.pool.online.lock() = Some(pc.clone());
    if let Some(a) = anterior {
        // As rotas diretas continuam valendo: passam para a conexão nova antes de fechar a velha.
        let diretas: Vec<_> = a.direct.lock().drain().collect();
        pc.direct.lock().extend(diretas);
        a.close();
    }
    room.publish.reattach_all(&pc.conn, &pc.ctrl, &shared.clock);
    Ok(())
}

/// Registra no `Router` da ponte nova os receptores das trilhas assistidas e repete o `assinar`
/// de cada uma. (O `Router` antigo morre com a `PubConn` antiga; os `VideoRx`/`AudioRx` vivem nas
/// assinaturas e em `shared.audio_rx`, então basta registrá-los de novo.)
async fn reassina_na_ponte(shared: &Arc<Shared>, pc: &Arc<PubConn>) {
    let subs: Vec<_> = shared.subs.lock().values().cloned().collect();
    for sub in subs {
        if let Some(v) = &sub.video {
            pc.router.register(sub.track.number, v.clone());
        }
        let audio = sub.audio_number.is_some();
        if let (Some(n), Ok(rx)) = (
            sub.audio_number,
            shared.audio_rx.get_or_create(&sub.track.participant_id),
        ) {
            // Uma contagem por assinatura, como no `start`.
            pc.router.acquire(n, rx);
        }
        // Só áudio não tem tamanho nem camada; o vídeo mantém o pedido que tinha.
        let (size, max_layer) = if sub.audio_only() {
            (TileSize::Thumbnail, None)
        } else {
            let sp = sub.spec();
            (sp.size, sp.max_layer)
        };
        // Erro aqui só significa que a ponte caiu de novo: `watch_bridge` trata.
        let _ = pc.subscribe(&sub.track.id, size, max_layer, audio).await;
    }
}

/// Tarefa da sala: espera `session.epoch_rx()` mudar (WebSocket voltou depois de cair).
/// Então: `ws.send(RoomJoin{room_id})`, espera `RoomState` pelo canal da sala (`set_route` com
/// canal novo se o antigo fechou) e substitui o modelo (`replace_state`, mantém o chat);
/// `RoomEvent::Reconnected`; manda `p2p.meus_enderecos` de novo; reassina todas as trilhas
/// assistidas (`Subscription::update` de cada tile). Sala inexistente/encerrada
/// (`sala_nao_encontrada`, `sala.encerrada`) → `RoomEvent::Closed`. Reenvia `assinar` também dos
/// pedidos de `direct.pending`. Sai se `room.left`.
pub(crate) async fn watch_ws(room: Arc<OnlineRoom>) {
    // `epoch_rx()` já nasce com o valor atual marcado como visto: só mudanças futuras acordam.
    let mut epoch = room.session.epoch_rx();
    loop {
        if epoch.changed().await.is_err() || saiu(&room) {
            return;
        }
        let Ok(shared) = room.shared() else {
            return;
        };
        match volta_ws(&room, &shared).await {
            Ok(true) => {}
            // Sala acabou no servidor.
            Ok(false) => {
                room.left.store(true, Ordering::SeqCst);
                room.session.set_route(None);
                let _ = room.events.send(RoomEvent::Closed("servidor".into()));
                return;
            }
            Err(e) => {
                let _ = room.events.send(RoomEvent::Error(e.to_string()));
            }
        }
    }
}

/// Reentra na sala pelo WebSocket novo. `Ok(false)` = a sala não existe mais.
async fn volta_ws(room: &Arc<OnlineRoom>, shared: &Arc<Shared>) -> Result<bool, CoreError> {
    let ws = room
        .session
        .ws()
        .ok_or_else(|| CoreError::Connect("websocket indisponível".into()))?;
    let room_id = room.model.lock().room.id.clone();
    let (tx, mut rx) = mpsc::channel::<ControlMsg>(1024);
    room.session.set_route(Some(tx));
    ws.send(ControlMsg::RoomJoin { room_id })
        .await
        .map_err(|e| CoreError::Connect(e.to_string()))?;

    let espera = async {
        while let Some(msg) = rx.recv().await {
            match msg {
                ControlMsg::RoomState {
                    room,
                    participants,
                    tracks,
                } => return Some(Some((room, participants, tracks))),
                ControlMsg::RoomClosed { .. } => return Some(None),
                ControlMsg::Error { code, .. } if code == "sala_nao_encontrada" => return Some(None),
                _ => {}
            }
        }
        None
    };
    let estado = match tokio::time::timeout(TEMPO_ESTADO, espera).await {
        Ok(Some(r)) => r,
        _ => {
            return Err(CoreError::Connect(
                "o servidor não respondeu ao reentrar na sala".into(),
            ))
        }
    };
    let Some((sala, participantes, trilhas)) = estado else {
        return Ok(false);
    };
    room.model.lock().replace_state(sala, participantes, trilhas);

    // A tarefa antiga termina sozinha: o `Sender` dela foi trocado em `set_route`.
    let nova = tokio::spawn(room_events::run(room.clone(), rx));
    room.tasks.lock().push(nova);

    let _ = room.events.send(RoomEvent::Changed);
    let _ = room.events.send(RoomEvent::Reconnected);
    let _ = direct::send_my_addresses(room).await;

    // O `PubConn` da ponte guarda o `WsTx` antigo: o `assinar` vai direto pelo WebSocket novo.
    let subs: Vec<_> = shared.subs.lock().values().cloned().collect();
    for sub in subs {
        let _ = ws
            .send(ControlMsg::Subscribe {
                track_id: sub.track.id.clone(),
                size: TileSize::Main,
                max_layer: None,
                audio: Some(sub.audio_number.is_some()),
            })
            .await;
    }
    // Pedidos ainda sem rota direta (`direct.pending`): `watch_pending` regrava e manda `assinar`.
    // Lista copiada antes dos `await`, para não segurar o lock.
    let pendentes: Vec<_> = room.direct.pending.lock().values().cloned().collect();
    for spec in pendentes {
        let _ = direct::watch_pending(room, spec).await;
    }
    Ok(true)
}

/// Tarefa da sala: espera `PubConn::conn.closed()` da ponte; se não foi `leave`, mostra
/// `RoomEvent::Reconnecting`, e tenta `connect_bridge_for` com `Backoff` (1, 2, 4, 8, 15 s) até
/// conseguir; ao voltar, reassina as trilhas assistidas e emite `RoomEvent::Reconnected`.
pub(crate) async fn watch_bridge(room: Arc<OnlineRoom>) {
    loop {
        if saiu(&room) {
            return;
        }
        let atual = room.bridge.lock().await.clone();
        let Some(pc) = atual else {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        };
        pc.conn.closed().await;
        if saiu(&room) {
            return;
        }
        let Ok(shared) = room.shared() else {
            return;
        };
        // Outra tarefa já pôs uma ponte nova no lugar: só acompanha a nova.
        let trocada = room.bridge.lock().await.as_ref().is_some_and(|b| !Arc::ptr_eq(b, &pc));
        if trocada {
            continue;
        }
        // Sem relay não há o que reconectar: esquece a ponte morta (evita girar em falso).
        if !room.bridge_enabled(&shared) {
            room.bridge.lock().await.take();
            shared.pool.online.lock().take();
            continue;
        }

        let _ = room.events.send(RoomEvent::Reconnecting);
        let mut backoff = Backoff::new();
        loop {
            if saiu(&room) {
                return;
            }
            match connect_bridge_for(&room).await {
                Ok(()) => break,
                Err(CoreError::Stopped) => return,
                Err(_) => tokio::time::sleep(backoff.next_delay()).await,
            }
        }
        let nova = room.bridge.lock().await.clone();
        if let Some(nova) = nova.filter(|n| !Arc::ptr_eq(n, &pc)) {
            reassina_na_ponte(&shared, &nova).await;
            let _ = room.events.send(RoomEvent::Reconnected);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use zap_net::online::{CreateRoomRequest, JoinMedia, JoinResponse, LoginRequest, ServerClient, WsRx};
    use zap_proto::{Access, Participant, Role, Room, RoomMode};
    use zap_test_server::{Config, Handle};

    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::room::RoomModel;
    use crate::testutil::TestSink;

    async fn novo_core(fp: Option<String>) -> (Core, std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-reconn-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                dev_cert_fp: fp,
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap()
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    fn modelo(id: &str, owner: &str) -> RoomModel {
        let room = Room {
            id: id.into(),
            code: None,
            name: "Sala".into(),
            owner_id: owner.into(),
            access: Access::Open,
            mode: RoomMode::Online,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let p = Participant {
            id: owner.into(),
            name: "Teste".into(),
            color: "#3BF08A".into(),
            role: Role::Member,
            joined_at: "2026-01-01T00:00:00Z".into(),
            app_version: "1.0.0".into(),
            protocol: 1,
            latency_ms: None,
            watching: None,
        };
        RoomModel::new(room, p)
    }

    /// Sala de teste sem servidor.
    fn sala_vazia(core: &Core, media: JoinMedia) -> Arc<OnlineRoom> {
        OnlineRoom::new(
            &core.shared,
            modelo("s_1", "p1"),
            "p1".into(),
            zap_proto::RoomCode::from_entropy([1, 2, 3, 4]),
            None,
            media,
            String::new(),
        )
    }

    /// Login + sala + entrada no servidor de teste; instala `client` e `ws` na sessão do Core.
    /// O `WsRx` devolvido precisa ficar vivo durante o teste.
    async fn sala_no_servidor(
        core: &Core,
        srv: &Handle,
    ) -> (Arc<OnlineRoom>, WsRx, tokio::sync::mpsc::UnboundedReceiver<RoomEvent>) {
        let cli = ServerClient::login(
            &srv.base_url,
            LoginRequest {
                name: "Teste".into(),
                color: "#3BF08A".into(),
                install_id: uuid::Uuid::new_v4().to_string(),
                protocol: zap_proto::PROTOCOL_VERSION,
                app_version: "teste".into(),
                server_password: None,
            },
        )
        .await
        .unwrap();
        let listing = cli
            .create_room(CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            })
            .await
            .unwrap();
        let join: JoinResponse = cli.join(&listing.code, None).await.unwrap();
        let (wtx, wrx) = cli.connect_ws().await.unwrap();
        *core.shared.online.client.lock() = Some(cli);
        *core.shared.online.ws.lock() = Some(wtx);

        let mut room = OnlineRoom::new(
            &core.shared,
            modelo(&join.room.id, &join.participant_id),
            join.participant_id.clone(),
            listing.code.clone(),
            None,
            join.media.clone(),
            join.ticket.clone(),
        );
        // Eventos da sala vão para um canal que o teste lê.
        let (etx, erx) = tokio::sync::mpsc::unbounded_channel();
        Arc::get_mut(&mut room).unwrap().events = etx;
        (room, wrx, erx)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_bridge_for_sem_use_relay_nao_faz_nada() {
        let (core, d) = novo_core(None).await;
        core.shared.settings.write().server.use_relay = false;
        let room = sala_vazia(
            &core,
            JoinMedia {
                bridge: Some(zap_net::online::BridgeInfo {
                    address: "x:1".into(),
                    alpn: "zap/4".into(),
                }),
                direct_allowed: false,
            },
        );
        assert!(connect_bridge_for(&room).await.is_ok());
        assert!(room.bridge.lock().await.is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_bridge_for_sem_ponte_no_media_ok() {
        let (core, d) = novo_core(None).await;
        let room = sala_vazia(
            &core,
            JoinMedia {
                bridge: None,
                direct_allowed: false,
            },
        );
        assert!(connect_bridge_for(&room).await.is_ok());
        assert!(room.bridge.lock().await.is_none());
        assert!(core.shared.pool.online.lock().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_bridge_for_conecta_e_guarda_pubconn() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.cert_fingerprint.clone())).await;
        let (room, _wrx, _erx) = sala_no_servidor(&core, &srv).await;
        connect_bridge_for(&room).await.unwrap();
        let pc = room.bridge.lock().await.clone().expect("ponte guardada");
        assert!(!pc.is_closed());
        let no_pool = core.shared.pool.online.lock().clone().expect("pool.online");
        assert!(Arc::ptr_eq(&pc, &no_pool));
        pc.close();
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn watch_bridge_reconecta_apos_queda() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.cert_fingerprint.clone())).await;
        let (room, _wrx, mut erx) = sala_no_servidor(&core, &srv).await;
        connect_bridge_for(&room).await.unwrap();
        let antiga = room.bridge.lock().await.clone().unwrap();
        let tarefa = tokio::spawn(watch_bridge(room.clone()));

        antiga.conn.close(0u32.into(), b"queda");

        let limite = Instant::now() + Duration::from_secs(10);
        let nova = loop {
            let atual = room.bridge.lock().await.clone();
            if let Some(p) = atual {
                if !Arc::ptr_eq(&p, &antiga) && !p.is_closed() {
                    break p;
                }
            }
            assert!(Instant::now() < limite, "a ponte não reconectou em 10 s");
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        // `Reconnected` sai logo depois de a ponte nova ser guardada.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut vistos = Vec::new();
        while let Ok(ev) = erx.try_recv() {
            vistos.push(ev);
        }
        assert!(vistos.iter().any(|e| matches!(e, RoomEvent::Reconnecting)));
        assert!(vistos.iter().any(|e| matches!(e, RoomEvent::Reconnected)));

        room.left.store(true, Ordering::SeqCst);
        tarefa.abort();
        nova.close();
        encerrar(core, d).await;
    }
}
