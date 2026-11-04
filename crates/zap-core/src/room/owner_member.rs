//! Atendimento de um membro pelo dono.

use std::sync::Arc;

use std::time::Duration;

use zap_net::control::{ControlRx, ControlTx};
use zap_net::handshake::Hello;
use zap_proto::{ControlMsg, Participant, Role, PROTOCOL_VERSION};

use super::dispatch::{handle_member_msg, on_member_joined, on_member_left, Outgoing};
use super::model::chat_message_msg;
use super::owner::{MemberConn, OwnerRoom};
use super::RoomEvent;
use crate::util::{iso8601_now, now_ms};

/// Intervalo entre pings ao membro.
const INTERVALO_PING: Duration = Duration::from_secs(2);

/// Depois do aperto de mão: monta o `Participant` (id = o que o handshake devolveu, papel membro,
/// `protocolo` = `zap_proto::PROTOCOL_VERSION`, `entrou_em` = agora), registra em `members`,
/// manda a ele `sala.estado` e as mensagens de chat do histórico, faz `on_member_joined` +
/// `send_all` + `RoomEvent::Changed`, e entra no laço:
/// - `rx.recv()` → `handle_member_msg` → `send_all`; `RoomLeave` → sai com "saiu";
///   `ReverseConnection` → `forward_reverse`; `Chat` do membro também vai ao `ctx.events`
///   como `RoomEvent::Chat`;
/// - a cada 2 s manda `Ping`; o `Pong` atualiza `latency_ms` (`ParticipantUpdated`);
/// - fim do stream/erro/conexão fechada → sai com "caiu".
///
/// Ao sair: tira de `members`, `on_member_left` + `send_all` + `Changed`.
pub(crate) async fn serve_member(
    room: Arc<OwnerRoom>,
    conn: quinn::Connection,
    tx: ControlTx,
    mut rx: ControlRx,
    hello: Hello,
    participant_id: String,
) {
    let id = participant_id;
    let participante = Participant {
        id: id.clone(),
        name: hello.name,
        color: hello.color,
        role: Role::Member,
        joined_at: iso8601_now(),
        app_version: hello.app_version,
        protocol: PROTOCOL_VERSION,
        latency_ms: None,
        watching: None,
    };
    room.members.lock().insert(
        id.clone(),
        MemberConn {
            tx: tx.clone(),
            conn: conn.clone(),
        },
    );

    // Estado e histórico são copiados antes dos awaits: o lock do modelo não atravessa `.await`.
    // O membro entra no modelo na mesma trava, para o estado já incluí-lo e nada se perder.
    let (estado, historico, entrada) = {
        let mut model = room.model.lock();
        let entrada = on_member_joined(&mut model, participante);
        let estado = model.state_msg();
        let historico: Vec<_> = model.chat.iter().cloned().collect();
        (estado, historico, entrada)
    };
    let _ = tx.send(&estado).await;
    for linha in &historico {
        let _ = tx.send(&chat_message_msg(linha, None)).await;
    }
    room.send_all(entrada).await;
    let _ = room.ctx.events.send(RoomEvent::Changed);

    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + INTERVALO_PING, INTERVALO_PING);
    let fechada = conn.closed();
    tokio::pin!(fechada);

    let motivo = loop {
        tokio::select! {
            _ = ping.tick() => {
                let _ = tx.send(&ControlMsg::Ping { ts_ms: now_ms() }).await;
            }
            _ = &mut fechada => break "caiu",
            recebido = rx.recv() => {
                let msg = match recebido {
                    Ok(Some(m)) => m,
                    Ok(None) | Err(_) => break "caiu",
                };
                match msg {
                    ControlMsg::RoomLeave {} => break "saiu",
                    ControlMsg::Pong { ts_ms } => {
                        let latencia = now_ms().saturating_sub(ts_ms).min(u32::MAX as u64) as u32;
                        let outs = {
                            let mut model = room.model.lock();
                            match model.participant(&id).cloned() {
                                Some(mut p) => {
                                    p.latency_ms = Some(latencia);
                                    model.upsert_participant(p.clone());
                                    vec![Outgoing {
                                        target: super::dispatch::Target::All,
                                        msg: ControlMsg::ParticipantUpdated { participant: p },
                                    }]
                                }
                                None => Vec::new(),
                            }
                        };
                        room.send_all(outs).await;
                        let _ = room.ctx.events.send(RoomEvent::Changed);
                    }
                    ControlMsg::ReverseConnection { track_id, address, token } => {
                        let _ = room.forward_reverse(&track_id, &address, &token).await;
                    }
                    outra => {
                        let eh_chat = matches!(outra, ControlMsg::ChatSend { .. });
                        let (outs, nova_linha) = {
                            let mut model = room.model.lock();
                            let antes = model.chat.back().map(|l| l.id.clone());
                            let outs = handle_member_msg(&mut model, &id, outra);
                            let nova = if eh_chat && model.chat.back().map(|l| l.id.clone()) != antes {
                                model.chat.back().cloned()
                            } else {
                                None
                            };
                            (outs, nova)
                        };
                        room.send_all(outs).await;
                        if let Some(linha) = nova_linha {
                            let _ = room.ctx.events.send(RoomEvent::Chat(linha));
                        }
                        let _ = room.ctx.events.send(RoomEvent::Changed);
                    }
                }
            }
        }
    };

    room.members.lock().remove(&id);
    let outs = {
        let mut model = room.model.lock();
        on_member_left(&mut model, &id, motivo)
    };
    room.send_all(outs).await;
    let _ = room.ctx.events.send(RoomEvent::Changed);
    conn.close(0u32.into(), motivo.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;
    use zap_net::endpoint::client_endpoint;
    use zap_net::handshake::connect_handshake;
    use zap_net::tls::{client_config, Identity};

    use crate::room::RoomCtx;
    use crate::types::{CreateRoom, RoomMode};

    type Erro = Box<dyn std::error::Error>;

    struct Cliente {
        tx: ControlTx,
        rx: ControlRx,
        conn: quinn::Connection,
        id: String,
        _ep: quinn::Endpoint,
    }

    async fn dono() -> Result<(Arc<OwnerRoom>, UnboundedReceiver<RoomEvent>), Erro> {
        let (etx, erx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = Arc::new(RoomCtx {
            identity: Arc::new(Identity::generate("zap")?),
            install_id: "inst_dono".into(),
            name: "Dono".into(),
            color: "#fff".into(),
            port: 0,
            interface: None,
            include_loopback: true,
            app_version: "1.0".into(),
            events: etx,
        });
        let sala = OwnerRoom::start(
            ctx,
            CreateRoom {
                name: "Sala".into(),
                mode: RoomMode::Local,
                password: None,
            },
        )
        .await?;
        Ok((sala, erx))
    }

    async fn entra(sala: &OwnerRoom, nome: &str) -> Result<Cliente, Erro> {
        let ender = sala.control_address();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", ender.port).parse()?;
        let ep = client_endpoint()?;
        let conn = ep
            .connect_with(client_config(Some(&ender.fingerprint))?, addr, "zap")?
            .await?;
        let hello = Hello {
            app_version: "1.0".into(),
            install_id: format!("inst_{nome}"),
            name: nome.into(),
            color: "#000".into(),
            ticket: None,
            room_id: None,
            preview: None,
        };
        let (tx, rx, w) = connect_handshake(&conn, hello, None).await?;
        Ok(Cliente {
            tx,
            rx,
            conn,
            id: w.participant_id,
            _ep: ep,
        })
    }

    async fn proxima(c: &mut Cliente) -> Result<ControlMsg, Erro> {
        let m = tokio::time::timeout(Duration::from_secs(5), c.rx.recv())
            .await?
            .map_err(|e| e.to_string())?
            .ok_or("stream terminou")?;
        Ok(m)
    }

    /// Lê até achar uma mensagem que satisfaça o filtro (ignora pings e afins).
    async fn espera<F: Fn(&ControlMsg) -> bool>(c: &mut Cliente, f: F) -> Result<ControlMsg, Erro> {
        for _ in 0..20 {
            let m = proxima(c).await?;
            if f(&m) {
                return Ok(m);
            }
        }
        Err("mensagem esperada não chegou".into())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn membro_recebe_estado_e_historico() -> Result<(), Erro> {
        let (sala, _ev) = dono().await?;
        sala.send_chat("um".into()).await?;
        sala.send_chat("dois".into()).await?;
        let mut a = entra(&sala, "Ana").await?;
        assert!(matches!(proxima(&mut a).await?, ControlMsg::RoomState { .. }));
        for esperado in ["um", "dois"] {
            match proxima(&mut a).await? {
                ControlMsg::ChatMessage { text, .. } => assert_eq!(text, esperado),
                outra => return Err(format!("esperava chat: {outra:?}").into()),
            }
        }
        sala.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ping_vira_pong_e_latencia_aparece() -> Result<(), Erro> {
        let (sala, _ev) = dono().await?;
        let mut a = entra(&sala, "Ana").await?;
        let limite = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut latencia = None;
        while latencia.is_none() && tokio::time::Instant::now() < limite {
            if let Ok(Ok(Some(ControlMsg::Ping { ts_ms }))) =
                tokio::time::timeout(Duration::from_millis(500), a.rx.recv()).await
            {
                a.tx.send(&ControlMsg::Pong { ts_ms }).await?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            latencia = sala.model.lock().participant(&a.id).and_then(|p| p.latency_ms);
        }
        assert!(latencia.is_some());
        sala.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sala_sair_remove_com_motivo_saiu() -> Result<(), Erro> {
        let (sala, _ev) = dono().await?;
        let mut a = entra(&sala, "Ana").await?;
        let b = entra(&sala, "Bia").await?;
        let id_b = b.id.clone();
        b.tx.send(&ControlMsg::RoomLeave {}).await?;
        let m = espera(&mut a, |m| matches!(m, ControlMsg::ParticipantLeft { .. })).await?;
        match m {
            ControlMsg::ParticipantLeft { participant_id, reason } => {
                assert_eq!(participant_id, id_b);
                assert_eq!(reason, "saiu");
            }
            _ => unreachable!(),
        }
        sala.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fechar_conexao_vira_caiu() -> Result<(), Erro> {
        let (sala, _ev) = dono().await?;
        let mut a = entra(&sala, "Ana").await?;
        let b = entra(&sala, "Bia").await?;
        b.conn.close(0u32.into(), b"x");
        let m = tokio::time::timeout(
            Duration::from_secs(3),
            espera(&mut a, |m| matches!(m, ControlMsg::ParticipantLeft { .. })),
        )
        .await??;
        match m {
            ControlMsg::ParticipantLeft { reason, .. } => assert_eq!(reason, "caiu"),
            _ => unreachable!(),
        }
        sala.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_de_membro_vai_para_os_outros_e_para_a_ui_do_dono() -> Result<(), Erro> {
        let (sala, mut ev) = dono().await?;
        let mut a = entra(&sala, "Ana").await?;
        let b = entra(&sala, "Bia").await?;
        b.tx.send(&ControlMsg::ChatSend {
            text: "oi".into(),
            client_id: "c1".into(),
        })
        .await?;
        let m = espera(&mut a, |m| matches!(m, ControlMsg::ChatMessage { .. })).await?;
        assert!(matches!(m, ControlMsg::ChatMessage { ref text, .. } if text == "oi"));
        let linha = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(RoomEvent::Chat(l)) = ev.recv().await {
                    return Some(l);
                }
            }
        })
        .await?
        .ok_or("sem evento de chat")?;
        assert_eq!(linha.text, "oi");
        sala.close().await;
        Ok(())
    }
}
