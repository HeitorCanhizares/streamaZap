//! Tarefa que consome as mensagens do WebSocket destinadas à sala ativa.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::mpsc;
use zap_proto::ControlMsg;

use super::direct;
use super::room::OnlineRoom;
use super::ws_apply::apply_ws_msg;
use crate::plan_gate::{self, PlanAction};
use crate::room::RoomEvent;

/// Para cada mensagem de `rx` (até o canal fechar ou `room.left`):
/// 1. `TrackPublished{track, local_id: Some(l)}` com `l` em `room.pending` → completa o
///    `oneshot` (antes de aplicar);
/// 2. `ws_apply::apply_ws_msg(&mut model, msg.clone())` → cada `RoomEvent` vai para
///    `room.events` (`Closed` também marca `room.left`);
/// 3. mensagens de mídia: `LayersRequested` → `room.publish.on_layers_requested`;
///    `KeyframeRequest{track_id, layer: Some(l)}` → `room.publish.on_keyframe_request`;
///    `MediaRoute` → `direct::on_media_route`; `P2pCandidates` → `direct::on_candidates`;
///    `LayerCurrent` → guarda em `PubConn::last_layer` da ponte e atualiza o motivo do tile
///    (`TileMetrics::set_reason` da assinatura da trilha, se houver);
/// 4. `Error{ref: Some(r)}` com `r` em `room.pending` → completa o `oneshot` com
///    `Err(plan_gate::error_from_server(..))` e não gera `RoomEvent`; `Error` sem `ref` (ou com
///    `ref` desconhecida) segue para o passo 2 (`plano_insuficiente` → `PlanRefused`, o resto →
///    `RoomEvent::Error(message)`).
///
/// Depois de aplicar `ParticipantLeft` de quem eu assistia, nada especial (o `Core` limpa pelo
///   `RoomEvent::TrackRemoved`).
pub(crate) async fn run(room: Arc<OnlineRoom>, mut rx: mpsc::Receiver<ControlMsg>) {
    while let Some(msg) = rx.recv().await {
        // 1. Publicação local confirmada: completa o `oneshot` antes de aplicar ao modelo.
        if let ControlMsg::TrackPublished {
            track,
            local_id: Some(l),
        } = &msg
        {
            if let Some(tx) = room.pending.lock().remove(l) {
                let _ = tx.send(Ok(track.clone()));
            }
        }
        // 1b. Erro com `ref` de uma publicação pendente: quem publicou recebe o erro (o
        // fechamento do share registra a recusa); não vira `RoomEvent`.
        if let ControlMsg::Error {
            code,
            message,
            reference: Some(r),
            plan,
            field,
            ..
        } = &msg
        {
            // O lock de `pending` é solto no fim da instrução, antes de `room.shared()`.
            let tx = room.pending.lock().remove(r);
            if let Some(tx) = tx {
                let fallback = room.shared().ok().and_then(|s| s.session_plan()).map(|p| p.id);
                let _ = tx.send(Err(plan_gate::error_from_server(
                    code,
                    message,
                    *plan,
                    field.clone(),
                    fallback,
                    PlanAction::Publish,
                )));
                continue;
            }
        }
        // 2. Aplica ao modelo; o lock é solto antes de enviar os eventos.
        let evs = {
            let mut model = room.model.lock();
            apply_ws_msg(&mut model, msg.clone())
        };
        let mut fechada = false;
        for ev in evs {
            if matches!(ev, RoomEvent::Closed(_)) {
                fechada = true;
            }
            let _ = room.events.send(ev);
        }
        if fechada {
            room.left.store(true, Ordering::SeqCst);
            break;
        }
        // 3. Mensagens de mídia (sem `Shared` vivo, o Core foi desligado: encerra).
        if room.shared().is_err() {
            break;
        }
        handle_media_msg(&room, msg);
    }
}

/// Tratamento das mensagens de mídia (passo 3). Também é o ponto de entrada para o leitor de
/// controle da ponte: `camadas.pedidas`, `keyframe.pedir` e `camada.atual` não vêm pelo WebSocket.
pub(crate) fn handle_media_msg(room: &Arc<OnlineRoom>, msg: ControlMsg) {
    match msg {
        ControlMsg::LayersRequested { track_id, layers } => room.publish.on_layers_requested(&track_id, &layers),
        ControlMsg::KeyframeRequest { track_id, layer } => room.publish.on_keyframe_request(&track_id, layer),
        ControlMsg::MediaRoute {
            track_id,
            route,
            address,
        } => direct::on_media_route(room, &track_id, route, address),
        ControlMsg::P2pCandidates {
            track_id,
            with,
            role,
            addresses,
            fingerprint,
        } => direct::on_candidates(room, track_id, with, role, addresses, fingerprint),
        m @ ControlMsg::LayerCurrent { .. } => {
            let ControlMsg::LayerCurrent { track_id, reason, .. } = &m else {
                return;
            };
            let (track_id, reason) = (track_id.clone(), reason.clone());
            if let Ok(bridge) = room.bridge.try_lock() {
                if let Some(conn) = bridge.as_ref() {
                    *conn.last_layer.lock() = Some(m);
                }
            }
            if let Ok(shared) = room.shared() {
                let subs = shared.subs.lock();
                for sub in subs.values().filter(|s| s.track.id == track_id) {
                    sub.metrics.set_reason(&reason);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use zap_net::online::JoinMedia;
    use zap_proto::{Access, Participant, Role, Room, RoomCode, RoomMode, Track};

    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::room::RoomModel;
    use crate::testutil::TestSink;

    fn core() -> Core {
        let dir = std::env::temp_dir().join(format!("zapcore-evt-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        Core::start_with(dir, TestSink::new(), opts).unwrap()
    }

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

    fn msg(v: serde_json::Value) -> ControlMsg {
        serde_json::from_value(v).unwrap()
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn track_publicada_completa_pending_e_entra_no_modelo() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (otx, orx) = tokio::sync::oneshot::channel();
            room.pending.lock().insert("local-1".into(), otx);
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(ControlMsg::TrackPublished {
                track: trilha("t1"),
                local_id: Some("local-1".into()),
            })
            .await
            .unwrap();
            let t = tokio::time::timeout(Duration::from_secs(2), orx)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(t.expect("publicação ok").id, "t1");
            drop(tx);
            h.await.unwrap();
            assert!(room.pending.lock().is_empty());
            assert!(room.model.lock().tracks.iter().any(|t| t.id == "t1"));
        });
    }

    /// Sala com um canal de eventos próprio (o do `Core` é consumido pelo pump).
    fn sala_com_eventos(core: &Core) -> (Arc<OnlineRoom>, mpsc::UnboundedReceiver<RoomEvent>) {
        let mut room = sala(core);
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::get_mut(&mut room).expect("sala sem outras referências").events = tx;
        (room, rx)
    }

    fn erro(code: &str, r: Option<&str>, plan: Option<&str>, campo: Option<&str>) -> ControlMsg {
        let mut v = json!({"t": "erro", "codigo": code, "mensagem": "recusado"});
        if let Some(r) = r {
            v["ref"] = json!(r);
        }
        if let Some(p) = plan {
            v["plano"] = json!(p);
        }
        if let Some(c) = campo {
            v["campo"] = json!(c);
        }
        msg(v)
    }

    #[test]
    fn erro_com_ref_completa_pending_sem_evento() {
        let core = core();
        let (room, mut evs) = sala_com_eventos(&core);
        rt().block_on(async {
            let (otx, orx) = tokio::sync::oneshot::channel();
            room.pending.lock().insert("l_9".into(), otx);
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(erro("plano_insuficiente", Some("l_9"), Some("hd"), Some("camadas")))
                .await
                .unwrap();
            let r = tokio::time::timeout(Duration::from_secs(2), orx)
                .await
                .unwrap()
                .unwrap();
            match r {
                Err(crate::CoreError::Plan(p)) => {
                    assert_eq!(p.plan, zap_proto::plan::PlanId::Hd);
                    assert_eq!(p.field.as_deref(), Some("camadas"));
                    assert_eq!(p.action, PlanAction::Publish);
                }
                outro => panic!("esperava Plan, veio {outro:?}"),
            }
            drop(tx);
            h.await.unwrap();
            assert!(room.pending.lock().is_empty());
            assert!(evs.try_recv().is_err());
        });
    }

    #[test]
    fn erro_sem_ref_vira_plan_refused_e_mantem_pending() {
        let core = core();
        let (room, mut evs) = sala_com_eventos(&core);
        rt().block_on(async {
            let (otx, _orx) = tokio::sync::oneshot::channel();
            room.pending.lock().insert("l_9".into(), otx);
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(erro("plano_insuficiente", None, Some("hd"), Some("camadas")))
                .await
                .unwrap();
            drop(tx);
            h.await.unwrap();
            assert!(room.pending.lock().contains_key("l_9"));
            match evs.try_recv() {
                Ok(RoomEvent::PlanRefused(p)) => {
                    assert_eq!(p.plan, zap_proto::plan::PlanId::Hd);
                    assert_eq!(p.field.as_deref(), Some("camadas"));
                    assert_eq!(p.action, PlanAction::Publish);
                }
                outro => panic!("esperava PlanRefused, veio {outro:?}"),
            }
        });
    }

    #[test]
    fn erro_comum_com_ref_vira_server() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (otx, orx) = tokio::sync::oneshot::channel();
            room.pending.lock().insert("l_9".into(), otx);
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(erro("invalido", Some("l_9"), None, None)).await.unwrap();
            let r = tokio::time::timeout(Duration::from_secs(2), orx)
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(r, Err(crate::CoreError::Server { ref code, .. }) if code == "invalido"));
            drop(tx);
            h.await.unwrap();
        });
    }

    #[test]
    fn chat_gera_room_event_chat() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(msg(json!({
                "t": "chat.mensagem", "id": "m1", "participante_id": "p_1",
                "nome": "Ana", "cor": "#ff0000", "texto": "oi", "em": "2026-01-01T00:00:00Z"
            })))
            .await
            .unwrap();
            drop(tx);
            h.await.unwrap();
            assert!(room.model.lock().chat.iter().any(|c| c.id == "m1"));
        });
    }

    #[test]
    fn sala_encerrada_marca_left_e_encerra_o_laco() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(msg(json!({"t": "sala.encerrada", "motivo": "dono"})))
                .await
                .unwrap();
            // O laço termina sozinho, sem fechar o canal.
            tokio::time::timeout(Duration::from_secs(2), h).await.unwrap().unwrap();
            assert!(room.left.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn layers_requested_chega_ao_bridge_publish() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(ControlMsg::LayersRequested {
                track_id: "inexistente".into(),
                layers: Vec::new(),
            })
            .await
            .unwrap();
            tx.send(ControlMsg::KeyframeRequest {
                track_id: "inexistente".into(),
                layer: Some(1),
            })
            .await
            .unwrap();
            drop(tx);
            tokio::time::timeout(Duration::from_secs(2), h).await.unwrap().unwrap();
        });
    }

    #[test]
    fn mensagem_desconhecida_nao_para_o_laco() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (tx, rx) = mpsc::channel(8);
            let h = tokio::spawn(run(room.clone(), rx));
            tx.send(ControlMsg::Ping { ts_ms: 1 }).await.unwrap();
            tx.send(ControlMsg::TrackPublished {
                track: trilha("t9"),
                local_id: None,
            })
            .await
            .unwrap();
            drop(tx);
            h.await.unwrap();
            assert!(room.model.lock().tracks.iter().any(|t| t.id == "t9"));
        });
    }

    #[test]
    fn canal_fechado_encerra() {
        let core = core();
        let room = sala(&core);
        rt().block_on(async {
            let (tx, rx) = mpsc::channel::<ControlMsg>(1);
            drop(tx);
            tokio::time::timeout(Duration::from_secs(2), run(room, rx))
                .await
                .unwrap();
        });
    }
}
