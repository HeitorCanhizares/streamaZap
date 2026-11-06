//! Laço de recepção do membro, com reconexão.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use zap_net::control::{ControlRx, ControlTx};
use zap_net::handshake::{connect_handshake, Hello, Welcome};
use zap_net::NetError;
use zap_proto::ControlMsg;

use super::apply::{apply_owner_msg, Effect};
use super::member::MemberRoom;
use super::model::RoomModel;
use super::RoomEvent;
use crate::CoreError;

/// Tentativas de reconexão antes de desistir.
const TENTATIVAS: u32 = 6;

/// Limite de cada `connect` da reconexão (o mesmo do `join`).
#[cfg(not(test))]
const TEMPO_CONECTAR: Duration = Duration::from_secs(6);
/// Nos testes, limite curto: um dono derrubado pode não recusar na hora e 6 × 6 s travaria o teste.
#[cfg(test)]
const TEMPO_CONECTAR: Duration = Duration::from_millis(500);

/// Divide as esperas do backoff: 1 de verdade; nos testes 1/100 (os 31,5 s viram 315 ms).
#[cfg(not(test))]
const DIVISOR_ESPERA: u32 = 1;
#[cfg(test)]
const DIVISOR_ESPERA: u32 = 100;

/// Espera antes da tentativa `attempt` (a partir de 0): 0,5 s, 1, 2, 4, 8, 8 s…
fn backoff(attempt: u32) -> Duration {
    let ms = 500u64 << attempt.min(4);
    Duration::from_millis(ms.min(8_000))
}

/// Por que o laço de recepção parou.
enum Fim {
    /// `sala.encerrada`: não reconecta.
    Encerrada,
    /// Fim do stream, erro de leitura ou conexão fechada.
    Caiu,
}

enum Reconexao {
    Ok(ControlRx, quinn::Connection),
    /// `leave` foi chamado no meio: sai sem avisar nada.
    Saiu,
    Esgotou,
}

/// Recebe do dono: `apply_owner_msg`; `Ping` → `Pong`; `trilha.publicada` com `local_id` acorda
/// `pending_tracks`; `Effect` → `RoomEvent` (Changed/Chat/TrackRemoved/Closed/Error);
/// `conexao.reversa` → `RoomEvent::ReverseRequest`. Conexão perdida (e `left` falso) →
/// `RoomEvent::Reconnecting` e reconexão com backoff 0,5 s → 1 → 2 → 4 → 8 s (6 tentativas, mesmo
/// endereço/impressão digital/senha, novo handshake); sucesso → novo `RoomModel` state,
/// `RoomEvent::Reconnected`; esgotou → `RoomEvent::Closed("conexao")`.
pub(crate) async fn run(room: Arc<MemberRoom>, mut rx: ControlRx, mut conn: quinn::Connection) {
    loop {
        let fim = recebe(&room, &mut rx, &conn).await;
        // Quem espera `trilha.publicada` desta conexão não vai recebê-la: solta já (erro em vez
        // de esperar os 5 s do `publish_track`).
        room.pending_tracks.lock().clear();
        if let Fim::Encerrada = fim {
            conn.close(0u32.into(), b"sala encerrada");
            return;
        }
        // `leave` já mandou `sala.sair` e fechou a conexão: a queda é esperada.
        if room.left.load(Ordering::SeqCst) {
            return;
        }
        conn.close(0u32.into(), b"caiu");
        let _ = room.ctx.events.send(RoomEvent::Reconnecting);
        match reconecta(&room).await {
            Reconexao::Ok(novo_rx, nova) => {
                rx = novo_rx;
                conn = nova;
                let _ = room.ctx.events.send(RoomEvent::Reconnected);
            }
            Reconexao::Saiu => return,
            Reconexao::Esgotou => {
                let _ = room.ctx.events.send(RoomEvent::Closed("conexao".into()));
                return;
            }
        }
    }
}

/// Laço de recepção de uma conexão, até a sala acabar ou a conexão cair.
async fn recebe(room: &MemberRoom, rx: &mut ControlRx, conn: &quinn::Connection) -> Fim {
    let fechada = conn.closed();
    tokio::pin!(fechada);
    loop {
        let msg = tokio::select! {
            // `biased`: o que já chegou (ex.: `sala.encerrada`) vale antes do aviso de fechamento.
            biased;
            r = rx.recv() => match r {
                Ok(Some(m)) => m,
                Ok(None) => return Fim::Caiu,
                Err(e) => {
                    tracing::debug!(erro = %e, "controle do dono caiu");
                    return Fim::Caiu;
                }
            },
            _ = &mut fechada => return Fim::Caiu,
        };
        if trata(room, msg).await {
            return Fim::Encerrada;
        }
    }
}

/// Trata uma mensagem do dono; `true` = a sala acabou.
async fn trata(room: &MemberRoom, msg: ControlMsg) -> bool {
    let eventos = &room.ctx.events;
    let msg = match msg {
        ControlMsg::Ping { ts_ms } => {
            if let Err(e) = room.send(ControlMsg::Pong { ts_ms }).await {
                tracing::debug!(erro = %e, "pong não saiu");
            }
            return false;
        }
        ControlMsg::ReverseConnection {
            track_id,
            address,
            token,
        } => {
            let _ = eventos.send(RoomEvent::ReverseRequest {
                track_id,
                address,
                token,
            });
            return false;
        }
        outra => outra,
    };
    let acordar = match &msg {
        ControlMsg::TrackPublished {
            track,
            local_id: Some(local),
        } => room
            .pending_tracks
            .lock()
            .remove(local)
            .map(|espera| (espera, track.clone())),
        _ => None,
    };
    // O lock do modelo fica só neste bloco (nada de await com ele).
    let efeito = {
        let mut model = room.model.lock();
        if chat_repetido(&model, &msg) {
            Effect::None
        } else {
            apply_owner_msg(&mut model, msg)
        }
    };
    // Acorda `publish_track` depois de a trilha já estar no espelho.
    if let Some((espera, track)) = acordar {
        let _ = espera.send(track);
    }
    match efeito {
        Effect::None => {}
        Effect::Changed | Effect::ParticipantLeft { .. } => {
            let _ = eventos.send(RoomEvent::Changed);
        }
        Effect::Chat(linha) => {
            let _ = eventos.send(RoomEvent::Chat(linha));
        }
        Effect::TrackRemoved(id) => {
            let _ = eventos.send(RoomEvent::TrackRemoved(id));
            let _ = eventos.send(RoomEvent::Changed);
        }
        Effect::Closed(motivo) => {
            let _ = eventos.send(RoomEvent::Closed(motivo));
            return true;
        }
        Effect::Error { message, .. } => {
            let _ = eventos.send(RoomEvent::Error(message));
        }
    }
    false
}

/// Mensagem de chat que o espelho já tem: o dono reenvia o histórico a cada entrada e, na
/// reconexão, isso duplicaria as linhas e as notificações.
fn chat_repetido(model: &RoomModel, msg: &ControlMsg) -> bool {
    matches!(msg, ControlMsg::ChatMessage { id, .. } if model.chat.iter().any(|l| l.id == *id))
}

/// Até `TENTATIVAS` conexões novas ao mesmo dono, com `backoff` antes de cada uma.
async fn reconecta(room: &MemberRoom) -> Reconexao {
    for tentativa in 0..TENTATIVAS {
        tokio::time::sleep(backoff(tentativa) / DIVISOR_ESPERA).await;
        if room.left.load(Ordering::SeqCst) {
            return Reconexao::Saiu;
        }
        match conecta(room).await {
            Ok((tx, rx, conn, welcome)) => {
                *room.my_id.lock() = welcome.participant_id;
                if let Some(segredo) = welcome.media_secret {
                    *room.media_secret.lock() = segredo;
                }
                *room.tx.lock() = Some(tx);
                {
                    // Os participantes e trilhas antigos já não valem (eu mesmo tenho id novo e
                    // minhas trilhas caíram com a conexão): o `sala.estado` que vem a seguir repovoa.
                    // O chat fica; o histórico reenviado é filtrado por id.
                    let mut model = room.model.lock();
                    model.room = welcome.room;
                    model.participants.clear();
                    model.tracks.clear();
                }
                room.set_conn(conn.clone());
                // `leave` pode ter rodado durante o aperto de mão: a conexão nova não fica aberta.
                if room.left.load(Ordering::SeqCst) {
                    conn.close(0u32.into(), b"saiu");
                    return Reconexao::Saiu;
                }
                return Reconexao::Ok(rx, conn);
            }
            Err(e) => {
                tracing::info!(tentativa = tentativa + 1, erro = %e, "reconexão ao dono falhou");
            }
        }
    }
    Reconexao::Esgotou
}

/// Uma tentativa: endpoint novo, `connect` (limite `TEMPO_CONECTAR`) e o mesmo `ola` da entrada.
async fn conecta(room: &MemberRoom) -> Result<(ControlTx, ControlRx, quinn::Connection, Welcome), CoreError> {
    let endpoint = zap_net::endpoint::client_endpoint()?;
    let cfg = zap_net::tls::client_config(Some(&room.fingerprint))?;
    let conectando = endpoint
        .connect_with(cfg, room.owner_addr, "zap")
        .map_err(NetError::from)?;
    let conn = tokio::time::timeout(TEMPO_CONECTAR, conectando)
        .await
        .map_err(|_| CoreError::Connect("sem resposta".into()))?
        .map_err(NetError::from)?;
    let room_id = room.model.lock().room.id.clone();
    let hello = Hello {
        app_version: room.ctx.app_version.clone(),
        install_id: room.ctx.install_id.clone(),
        name: room.ctx.name.clone(),
        color: room.ctx.color.clone(),
        ticket: None,
        room_id: Some(room_id),
        preview: None,
    };
    match connect_handshake(&conn, hello, room.password.as_deref()).await {
        Ok((tx, rx, welcome)) => Ok((tx, rx, conn, welcome)),
        Err(e) => {
            conn.close(0u32.into(), b"recusado");
            Err(CoreError::from_handshake(e))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use parking_lot::Mutex;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
    use tokio::sync::oneshot;
    use tokio::task::JoinHandle;
    use zap_net::tls::Identity;
    use zap_proto::{RoomMode, TrackKind};

    use crate::room::{Outgoing, OwnerRoom, RoomCtx, Target};
    use crate::types::CreateRoom;
    use crate::util::now_ms;

    fn ctx(nome: &str) -> (Arc<RoomCtx>, UnboundedReceiver<RoomEvent>) {
        let (events, rx) = unbounded_channel();
        let ctx = Arc::new(RoomCtx {
            identity: Arc::new(Identity::generate("zap").expect("identidade")),
            install_id: format!("inst-{nome}"),
            name: nome.into(),
            color: "#123456".into(),
            port: 0,
            interface: None,
            include_loopback: true,
            app_version: "0.1.0".into(),
            events,
        });
        (ctx, rx)
    }

    async fn dono() -> Arc<OwnerRoom> {
        let (ctx, rx) = ctx("Dono");
        // O receptor do dono fica vivo: o envio de eventos dele não deve falhar.
        std::mem::forget(rx);
        let req = CreateRoom {
            name: "Sala".into(),
            mode: RoomMode::Local,
            password: None,
        };
        OwnerRoom::start(ctx, req).await.expect("start")
    }

    /// Faz o que `MemberRoom::join` faz (connect + aperto de mão + espelho sem participantes) e
    /// sobe `run`: assim o laço é testado contra um dono de verdade.
    async fn entra(sala: &OwnerRoom) -> (Arc<MemberRoom>, UnboundedReceiver<RoomEvent>, JoinHandle<()>) {
        let (ctx, eventos) = ctx("Ana");
        let addr = SocketAddr::from(([127, 0, 0, 1], sala.port));
        let fingerprint = sala.ctx.identity.fingerprint();
        let ep = zap_net::endpoint::client_endpoint().expect("endpoint");
        let cfg = zap_net::tls::client_config(Some(&fingerprint)).expect("config");
        let conn = ep
            .connect_with(cfg, addr, "zap")
            .expect("connect")
            .await
            .expect("conexão");
        let hello = Hello {
            app_version: ctx.app_version.clone(),
            install_id: ctx.install_id.clone(),
            name: ctx.name.clone(),
            color: ctx.color.clone(),
            ticket: None,
            room_id: None,
            preview: None,
        };
        let (tx, rx, w) = connect_handshake(&conn, hello, None).await.expect("aperto de mão");
        let membro = Arc::new(MemberRoom {
            ctx,
            model: Arc::new(Mutex::new(RoomModel::from_state(w.room, Vec::new(), Vec::new()))),
            my_id: Mutex::new(w.participant_id),
            media_secret: Mutex::new(w.media_secret.unwrap_or_default()),
            tx: Mutex::new(Some(tx)),
            owner_addr: addr,
            fingerprint,
            password: None,
            pending_tracks: Mutex::new(HashMap::new()),
            left: AtomicBool::new(false),
            conn: Mutex::new(Some(conn.clone())),
        });
        let tarefa = tokio::spawn(run(membro.clone(), rx, conn));
        (membro, eventos, tarefa)
    }

    /// Lê eventos até `f` aceitar um; devolve todos os lidos (o último é o aceito).
    async fn espera_evento(
        rx: &mut UnboundedReceiver<RoomEvent>,
        limite: Duration,
        f: impl Fn(&RoomEvent) -> bool,
    ) -> Vec<RoomEvent> {
        tokio::time::timeout(limite, async {
            let mut vistos = Vec::new();
            loop {
                let e = rx.recv().await.expect("canal de eventos fechou");
                let achou = f(&e);
                vistos.push(e);
                if achou {
                    return vistos;
                }
            }
        })
        .await
        .expect("evento não chegou a tempo")
    }

    /// Repete `cond` a cada 10 ms até ser verdadeira ou o prazo vencer.
    async fn ate(prazo: Instant, mut cond: impl FnMut() -> bool) -> bool {
        loop {
            if cond() {
                return true;
            }
            if Instant::now() >= prazo {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn eh_reconexao(e: &RoomEvent) -> bool {
        matches!(e, RoomEvent::Reconnecting | RoomEvent::Reconnected)
    }

    #[test]
    fn backoff_sequencia() {
        let ms: Vec<u128> = (0..6).map(|t| backoff(t).as_millis()).collect();
        assert_eq!(ms, vec![500, 1_000, 2_000, 4_000, 8_000, 8_000]);
        // Além das 6 tentativas continua no teto (sem estouro de deslocamento).
        assert_eq!(backoff(40), Duration::from_secs(8));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn membro_recebe_estado_inicial() {
        let sala = dono().await;
        let prazo = Instant::now() + Duration::from_secs(1);
        let (membro, mut eventos, _t) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        assert!(ate(prazo, || membro.model.lock().participants.len() == 2).await);
        let eu = membro.my_id.lock().clone();
        assert!(membro.model.lock().participant(&eu).is_some());
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dono_fecha_sala_membro_recebe_closed() {
        let sala = dono().await;
        let (_membro, mut eventos, tarefa) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        sala.close().await;
        let vistos = espera_evento(&mut eventos, Duration::from_secs(3), |e| {
            matches!(e, RoomEvent::Closed(_))
        })
        .await;
        assert!(matches!(vistos.last(), Some(RoomEvent::Closed(m)) if m == "dono"));
        assert!(!vistos.iter().any(eh_reconexao));
        // O laço termina sem tentar reconectar.
        tokio::time::timeout(Duration::from_secs(2), tarefa)
            .await
            .expect("run não terminou")
            .expect("run entrou em pânico");
        while let Ok(e) = eventos.try_recv() {
            assert!(!eh_reconexao(&e), "evento depois do fim: {e:?}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn queda_do_dono_gera_reconnecting_e_closed() {
        let sala = dono().await;
        let (_membro, mut eventos, tarefa) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        // Queda sem `sala.encerrada`: o endpoint do dono some de uma vez.
        sala.endpoint.close(0u32.into(), b"queda");
        espera_evento(&mut eventos, Duration::from_secs(3), |e| {
            matches!(e, RoomEvent::Reconnecting)
        })
        .await;
        let vistos = espera_evento(&mut eventos, Duration::from_secs(15), |e| {
            matches!(e, RoomEvent::Closed(_))
        })
        .await;
        assert!(matches!(vistos.last(), Some(RoomEvent::Closed(m)) if m == "conexao"));
        assert!(!vistos.iter().any(|e| matches!(e, RoomEvent::Reconnected)));
        tokio::time::timeout(Duration::from_secs(2), tarefa)
            .await
            .expect("run não terminou")
            .expect("run entrou em pânico");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ping_do_dono_recebe_pong() {
        let sala = dono().await;
        let (membro, mut eventos, _t) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        let eu = membro.my_id.lock().clone();
        // Ping na hora (o automático do dono só sai depois de 2 s).
        sala.send_all(vec![Outgoing {
            target: Target::Only(eu.clone()),
            msg: ControlMsg::Ping { ts_ms: now_ms() },
        }])
        .await;
        let prazo = Instant::now() + Duration::from_secs(1);
        let latencia = || sala.model.lock().participant(&eu).and_then(|p| p.latency_ms);
        assert!(ate(prazo, || latencia().is_some()).await, "o dono não recebeu o pong");
        // O pong devolve o mesmo `ts_ms`: a latência medida pelo dono é pequena.
        assert!(latencia().unwrap_or(u32::MAX) < 1_000);
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn trilha_publicada_acorda_pendente_e_reversa_vira_evento() {
        let sala = dono().await;
        let (membro, mut eventos, _t) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        let (espera, recebe_trilha) = oneshot::channel();
        membro.pending_tracks.lock().insert("l_1".into(), espera);
        membro
            .send(ControlMsg::TrackPublish {
                local_id: "l_1".into(),
                kind: TrackKind::Camera,
                title: "Câmera".into(),
                layers: Vec::new(),
                audio: None,
                address: None,
                camera_layout: None,
            })
            .await
            .expect("envio");
        let trilha = tokio::time::timeout(Duration::from_secs(2), recebe_trilha)
            .await
            .expect("trilha.publicada não chegou")
            .expect("pendente descartado");
        assert_eq!(trilha.participant_id, *membro.my_id.lock());
        assert!(membro.model.lock().track_by_id(&trilha.id).is_some());
        assert!(membro.pending_tracks.lock().is_empty());

        sala.forward_reverse(&trilha.id, "10.0.0.2:7000", "tok")
            .await
            .expect("reversa");
        let vistos = espera_evento(&mut eventos, Duration::from_secs(2), |e| {
            matches!(e, RoomEvent::ReverseRequest { .. })
        })
        .await;
        match vistos.last() {
            Some(RoomEvent::ReverseRequest {
                track_id,
                address,
                token,
            }) => {
                assert_eq!(track_id, &trilha.id);
                assert_eq!(address, "10.0.0.2:7000");
                assert_eq!(token, "tok");
            }
            outro => panic!("esperava ReverseRequest, veio {outro:?}"),
        }
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconecta_quando_o_dono_derruba_a_conexao() {
        let sala = dono().await;
        let (membro, mut eventos, _t) = entra(&sala).await;
        espera_evento(&mut eventos, Duration::from_secs(1), |e| {
            matches!(e, RoomEvent::Changed)
        })
        .await;
        sala.send_chat("oi".into()).await.expect("chat");
        espera_evento(&mut eventos, Duration::from_secs(2), |e| {
            matches!(e, RoomEvent::Chat(_))
        })
        .await;

        // O dono continua no ar; só a conexão deste membro cai.
        let antigo = membro.my_id.lock().clone();
        let conn = sala
            .members
            .lock()
            .get(&antigo)
            .map(|m| m.conn.clone())
            .expect("membro no dono");
        conn.close(0u32.into(), b"derrubada");

        espera_evento(&mut eventos, Duration::from_secs(3), |e| {
            matches!(e, RoomEvent::Reconnecting)
        })
        .await;
        let vistos = espera_evento(&mut eventos, Duration::from_secs(5), |e| {
            matches!(e, RoomEvent::Reconnected | RoomEvent::Closed(_))
        })
        .await;
        assert!(matches!(vistos.last(), Some(RoomEvent::Reconnected)));

        let novo = membro.my_id.lock().clone();
        assert_ne!(novo, antigo);
        // O `sala.estado` repovoa o espelho: dono + eu com o id novo, sem o id antigo.
        let prazo = Instant::now() + Duration::from_secs(2);
        assert!(
            ate(prazo, || {
                let m = membro.model.lock();
                m.participants.len() == 2 && m.participant(&novo).is_some() && m.participant(&antigo).is_none()
            })
            .await
        );
        // O histórico reenviado não duplica o chat.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(membro.model.lock().chat.len(), 1);
        while let Ok(e) = eventos.try_recv() {
            assert!(!matches!(e, RoomEvent::Chat(_)), "chat repetido virou evento");
        }
        // A conexão guardada é a nova (é ela que o `leave` fecha).
        let guardada = membro.conn.lock().clone().expect("conexão guardada");
        assert!(guardada.close_reason().is_none());
        sala.close().await;
    }
}
