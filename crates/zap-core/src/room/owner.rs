//! Lado do dono: endpoint QUIC, aceita membros, mantém o `RoomModel` e repassa eventos.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use zap_net::control::ControlTx;
use zap_net::handshake::ServerAuth;
use zap_proto::types::Access;
use zap_proto::{ControlMsg, MediaAddress, Participant, Role, Room, RoomMode, Track};

use super::dispatch::handle_member_msg;
use super::dispatch::{Outgoing, Target};
use super::model::{RoomModel, TrackSpec};
use super::{RoomCtx, RoomEvent};
use crate::types::CreateRoom;
use crate::util::{iso8601_now, local_addresses, new_id};
use crate::{ticket, CoreError};

/// Um membro conectado.
pub(crate) struct MemberConn {
    pub tx: ControlTx,
    pub conn: quinn::Connection,
}

pub struct OwnerRoom {
    pub(crate) ctx: Arc<RoomCtx>,
    pub(crate) model: Arc<Mutex<RoomModel>>,
    pub(crate) my_id: String,
    pub(crate) media_secret: String,
    pub(crate) password: Option<String>,
    pub(crate) endpoint: quinn::Endpoint,
    pub(crate) port: u16,
    pub(crate) members: Mutex<HashMap<String, MemberConn>>,
    pub(crate) closed: std::sync::atomic::AtomicBool,
}

impl OwnerRoom {
    /// Sobe o endpoint QUIC, cria o modelo com o dono e inicia o aceite de membros.
    pub async fn start(ctx: Arc<RoomCtx>, req: CreateRoom) -> Result<Arc<Self>, CoreError> {
        if req.mode != RoomMode::Local {
            return Err(CoreError::Unsupported("modo online chega no M4".into()));
        }
        let (endpoint, port) = zap_net::endpoint::bind_server(&ctx.identity, ctx.port).await?;
        let room_id = new_id("s_");
        let my_id = new_id("p_");
        let password = req.password.filter(|p| !p.is_empty());
        let room = Room {
            id: room_id,
            code: None,
            name: req.name,
            owner_id: my_id.clone(),
            access: if password.is_some() {
                Access::Password
            } else {
                Access::Open
            },
            mode: RoomMode::Local,
            created_at: iso8601_now(),
        };
        let owner = Participant {
            id: my_id.clone(),
            name: ctx.name.clone(),
            color: ctx.color.clone(),
            role: Role::Owner,
            joined_at: iso8601_now(),
            app_version: ctx.app_version.clone(),
            protocol: zap_proto::PROTOCOL_VERSION,
            latency_ms: None,
            watching: None,
        };
        let this = Arc::new(Self {
            ctx,
            model: Arc::new(Mutex::new(RoomModel::new(room, owner))),
            my_id,
            media_secret: ticket::new_media_secret(),
            password,
            endpoint,
            port,
            members: Mutex::new(HashMap::new()),
            closed: std::sync::atomic::AtomicBool::new(false),
        });
        tokio::spawn(accept_loop(this.clone()));
        tokio::spawn(music_loop(Arc::downgrade(&this)));
        Ok(this)
    }

    /// Endereço de controle para anunciar na descoberta.
    pub fn control_address(&self) -> MediaAddress {
        MediaAddress {
            addresses: local_addresses(self.ctx.interface.as_deref(), self.ctx.include_loopback),
            port: self.port,
            fingerprint: self.ctx.identity.fingerprint(),
        }
    }

    /// Envia a cada destino conforme `Target`; falha de envio a um membro é ignorada (a tarefa
    /// dele cuida da queda).
    pub async fn send_all(&self, outs: Vec<Outgoing>) {
        for out in outs {
            // Clona os `ControlTx` antes: o lock de `members` não pode atravessar o await.
            let destinos: Vec<ControlTx> = {
                let membros = self.members.lock();
                membros
                    .iter()
                    .filter(|(id, _)| match &out.target {
                        Target::All => true,
                        Target::Only(alvo) => *id == alvo,
                        Target::AllBut(excluido) => *id != excluido,
                    })
                    .map(|(_, m)| m.tx.clone())
                    .collect()
            };
            for tx in destinos {
                let _ = tx.send(&out.msg).await;
            }
        }
    }

    /// Pedido de música do próprio dono: passa pela mesma regra dos membros e avisa todos.
    pub async fn music(&self, msg: ControlMsg) -> Result<(), CoreError> {
        let outs = {
            let mut model = self.model.lock();
            handle_member_msg(&mut model, &self.my_id, msg)
        };
        for out in &outs {
            if let ControlMsg::Error { message, .. } = &out.msg {
                return Err(CoreError::Invalid(message.clone()));
            }
        }
        self.send_all(outs).await;
        let _ = self.ctx.events.send(RoomEvent::Changed);
        Ok(())
    }

    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        let (outs, linha) = {
            let mut model = self.model.lock();
            let outs = handle_member_msg(
                &mut model,
                &self.my_id,
                ControlMsg::ChatSend {
                    text,
                    client_id: new_id("c_"),
                },
            );
            (outs, model.chat.back().cloned())
        };
        // Erro de validação vem como `Error` só para o próprio dono.
        for out in &outs {
            if let ControlMsg::Error { message, .. } = &out.msg {
                return Err(CoreError::Invalid(message.clone()));
            }
        }
        self.send_all(outs).await;
        if let Some(linha) = linha {
            let _ = self.ctx.events.send(RoomEvent::Chat(linha));
        }
        Ok(())
    }

    /// Dono publica trilha própria: `model.add_track(my_id, spec)` + `send_all(TrackPublished)`.
    pub async fn publish_track(&self, spec: TrackSpec) -> Result<Track, CoreError> {
        let track = self.model.lock().add_track(&self.my_id, spec)?;
        self.send_all(vec![Outgoing {
            target: Target::All,
            msg: ControlMsg::TrackPublished {
                track: track.clone(),
                local_id: None,
            },
        }])
        .await;
        let _ = self.ctx.events.send(RoomEvent::Changed);
        Ok(track)
    }

    pub async fn update_track(
        &self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<zap_proto::LayerSpec>>,
        camera_layout: Option<zap_proto::CameraLayout>,
    ) -> Result<(), CoreError> {
        let track = {
            let mut model = self.model.lock();
            match model.track_by_id(track_id) {
                Some(t) if t.participant_id == self.my_id => {}
                _ => return Err(CoreError::TrackNotFound),
            }
            model
                .update_track(track_id, title, layers, camera_layout)
                .ok_or(CoreError::TrackNotFound)?
        };
        self.send_all(vec![Outgoing {
            target: Target::All,
            msg: ControlMsg::TrackUpdated { track },
        }])
        .await;
        let _ = self.ctx.events.send(RoomEvent::Changed);
        Ok(())
    }

    pub async fn remove_track(&self, track_id: &str) -> Result<(), CoreError> {
        {
            let mut model = self.model.lock();
            match model.track_by_id(track_id) {
                Some(t) if t.participant_id == self.my_id => {}
                _ => return Err(CoreError::TrackNotFound),
            }
            model.remove_track(track_id);
        }
        self.send_all(vec![Outgoing {
            target: Target::All,
            msg: ControlMsg::TrackRemoved {
                track_id: track_id.to_owned(),
            },
        }])
        .await;
        let _ = self.ctx.events.send(RoomEvent::Changed);
        Ok(())
    }

    /// Encaminha `conexao.reversa` ao dono da trilha (se for o próprio dono, emite
    /// `RoomEvent::ReverseRequest`).
    pub async fn forward_reverse(&self, track_id: &str, address: &str, token: &str) -> Result<(), CoreError> {
        let dono = self
            .model
            .lock()
            .track_by_id(track_id)
            .map(|t| t.participant_id.clone())
            .ok_or(CoreError::TrackNotFound)?;
        if dono == self.my_id {
            let _ = self.ctx.events.send(RoomEvent::ReverseRequest {
                track_id: track_id.to_owned(),
                address: address.to_owned(),
                token: token.to_owned(),
            });
        } else {
            self.send_all(vec![Outgoing {
                target: Target::Only(dono),
                msg: ControlMsg::ReverseConnection {
                    track_id: track_id.to_owned(),
                    address: address.to_owned(),
                    token: token.to_owned(),
                },
            }])
            .await;
        }
        Ok(())
    }

    /// `sala.encerrada{motivo:"dono"}` para todos, fecha conexões e o endpoint.
    pub async fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
        self.send_all(vec![Outgoing {
            target: Target::All,
            msg: ControlMsg::RoomClosed { reason: "dono".into() },
        }])
        .await;
        // Dá tempo de a mensagem sair antes de derrubar as conexões.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let conns: Vec<quinn::Connection> = self.members.lock().drain().map(|(_, m)| m.conn).collect();
        for conn in conns {
            conn.close(0u32.into(), b"sala encerrada");
        }
        self.endpoint.close(0u32.into(), b"sala encerrada");
    }
}

/// Com música tocando, o estado vai de novo a todos a cada tantos tiques do `music_loop`: conserta
/// um membro que ficou com um estado velho (dois envios cruzados) e o atraso acumulado no espelho.
const REENVIO_MUSICA_TIQUES: u32 = 10;

/// A cada 1 s, avança a música quando o vídeo atual acaba e avisa todos (`musica.estado`); com
/// música tocando, reenvia o estado a cada 10 s. Para quando a sala fecha ou deixa de existir.
async fn music_loop(room: std::sync::Weak<OwnerRoom>) {
    let mut tique = tokio::time::interval(std::time::Duration::from_secs(1));
    tique.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut n: u32 = 0;
    loop {
        tique.tick().await;
        n = n.wrapping_add(1);
        let Some(room) = room.upgrade() else {
            return;
        };
        if room.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let outs = {
            let mut model = room.model.lock();
            let agora = std::time::Instant::now();
            let reenviar = n % REENVIO_MUSICA_TIQUES == 0 && !model.music.is_empty();
            if model.music.tick(agora) || reenviar {
                let msg = model.music.msg(model.participants.len(), agora);
                vec![Outgoing {
                    target: Target::All,
                    msg,
                }]
            } else {
                Vec::new()
            }
        };
        if !outs.is_empty() {
            room.send_all(outs).await;
            let _ = room.ctx.events.send(RoomEvent::Changed);
        }
    }
}

/// Aceita conexões até a sala fechar; para cada uma: `accept_handshake_with` (aberta/senha,
/// `media_secret`), recusa se a sala fechou, e dispara `owner_member::serve_member`.
pub(crate) async fn accept_loop(room: Arc<OwnerRoom>) {
    use std::sync::atomic::Ordering;
    while let Some(incoming) = room.endpoint.accept().await {
        if room.closed.load(Ordering::SeqCst) {
            break;
        }
        let room = room.clone();
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!(erro = %e, "conexão de entrada falhou");
                    return;
                }
            };
            let participant_id = new_id("p_");
            let auth = match &room.password {
                Some(pw) => ServerAuth::Password(pw.clone()),
                None => ServerAuth::Open,
            };
            // Cópia do `Room` atual: o lock não atravessa o await do handshake.
            let snapshot = room.model.lock().room.clone();
            match zap_net::handshake::accept_handshake_with(
                &conn,
                &auth,
                &snapshot,
                participant_id.clone(),
                Some(&room.media_secret),
            )
            .await
            {
                Ok((tx, rx, hello)) => {
                    if room.closed.load(Ordering::SeqCst) {
                        conn.close(0u32.into(), b"sala encerrada");
                        return;
                    }
                    super::owner_member::serve_member(room, conn, tx, rx, hello, participant_id).await;
                }
                Err(e) => {
                    tracing::info!(erro = %e, "aperto de mão recusado");
                    conn.close(0u32.into(), b"recusado");
                }
            }
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc::unbounded_channel;
    use zap_net::control::ControlRx;
    use zap_net::handshake::{connect_handshake, Hello};
    use zap_net::tls::Identity;
    use zap_net::NetError;

    fn ctx() -> Arc<RoomCtx> {
        let (events, rx) = unbounded_channel();
        // Mantém o receptor vivo: o envio de eventos não deve falhar nos testes.
        std::mem::forget(rx);
        Arc::new(RoomCtx {
            identity: Arc::new(Identity::generate("zap").expect("identidade")),
            install_id: "i".into(),
            name: "Dono".into(),
            color: "#fff".into(),
            port: 0,
            interface: None,
            include_loopback: true,
            app_version: "0.1.0".into(),
            events,
        })
    }

    fn req(password: Option<&str>) -> CreateRoom {
        CreateRoom {
            name: "Sala".into(),
            mode: RoomMode::Local,
            password: password.map(String::from),
        }
    }

    fn hello(nome: &str) -> Hello {
        Hello {
            app_version: "0.1.0".into(),
            install_id: format!("inst-{nome}"),
            name: nome.into(),
            color: "#000".into(),
            ticket: None,
            room_id: None,
            preview: None,
        }
    }

    async fn entra(
        sala: &OwnerRoom,
        nome: &str,
        senha: Option<&str>,
    ) -> Result<(quinn::Connection, ControlTx, ControlRx), NetError> {
        let ep = zap_net::endpoint::client_endpoint()?;
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], sala.port));
        let cfg = zap_net::tls::client_config(Some(&sala.ctx.identity.fingerprint()))?;
        let conn = ep
            .connect_with(cfg, addr, "zap")
            .expect("connect")
            .await
            .expect("conexão");
        let (tx, rx, _w) = connect_handshake(&conn, hello(nome), senha).await?;
        Ok((conn, tx, rx))
    }

    async fn espera_membros(sala: &OwnerRoom, n: usize) {
        for _ in 0..40 {
            if sala.model.lock().participants.len() == n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("membros não chegaram a {n}");
    }

    /// Lê mensagens até achar uma que satisfaça `f` (ignora estado inicial, pings etc.).
    async fn le_ate(rx: &mut ControlRx, f: impl Fn(&ControlMsg) -> bool) -> ControlMsg {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match rx.recv().await {
                    Ok(Some(m)) if f(&m) => return m,
                    Ok(Some(_)) => {}
                    other => panic!("stream terminou: {other:?}"),
                }
            }
        })
        .await
        .expect("timeout")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_abre_porta_e_anuncia_endereco() {
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let end = sala.control_address();
        assert_ne!(end.port, 0);
        assert_eq!(end.fingerprint.len(), 64);
        assert!(end.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_modo_online_unsupported() {
        let r = CreateRoom {
            name: "x".into(),
            mode: RoomMode::Online,
            password: None,
        };
        assert!(matches!(
            OwnerRoom::start(ctx(), r).await,
            Err(CoreError::Unsupported(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn membro_entra_e_dono_ve() {
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let _m = entra(&sala, "Ana", None).await.expect("entrar");
        espera_membros(&sala, 2).await;
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn senha_errada_recusada() {
        let sala = OwnerRoom::start(ctx(), req(Some("certa"))).await.expect("start");
        match entra(&sala, "Ana", Some("errada")).await {
            Err(NetError::Rejected(c)) => assert_eq!(c, "senha_sala"),
            Err(e) => panic!("erro inesperado: {e:?}"),
            Ok(_) => panic!("não devia entrar"),
        }
        let _ok = entra(&sala, "Bia", Some("certa")).await.expect("senha certa");
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_envia_sala_encerrada() {
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let (_c, _tx, mut rx) = entra(&sala, "Ana", None).await.expect("entrar");
        espera_membros(&sala, 2).await;
        sala.close().await;
        let m = le_ate(&mut rx, |m| matches!(m, ControlMsg::RoomClosed { .. })).await;
        assert!(matches!(m, ControlMsg::RoomClosed { reason } if reason == "dono"));
    }

    fn spec_camera() -> TrackSpec {
        TrackSpec {
            local_id: "local".into(),
            kind: zap_proto::TrackKind::Camera,
            title: "Câmera".into(),
            layers: Vec::new(),
            audio: None,
            address: None,
            camera_layout: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_track_com_camera_layout_muda_so_o_jeito() {
        use zap_proto::CameraLayout;
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let t = sala.publish_track(spec_camera()).await.expect("publicar");
        sala.update_track(&t.id, None, None, Some(CameraLayout::Free))
            .await
            .expect("atualizar");
        let depois = sala.model.lock().track_by_id(&t.id).cloned().expect("trilha");
        assert_eq!(depois.id, t.id);
        assert_eq!(depois.camera_layout, Some(CameraLayout::Free));
        assert_eq!(depois.title, t.title);
        assert_eq!(depois.layers, t.layers);
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_track_sem_camera_layout_preserva() {
        use zap_proto::CameraLayout;
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let t = sala.publish_track(spec_camera()).await.expect("publicar");
        sala.update_track(&t.id, None, None, Some(CameraLayout::Free))
            .await
            .expect("atualizar");
        sala.update_track(&t.id, Some("X".into()), None, None)
            .await
            .expect("atualizar título");
        let depois = sala.model.lock().track_by_id(&t.id).cloned().expect("trilha");
        assert_eq!(depois.title, "X");
        assert_eq!(depois.camera_layout, Some(CameraLayout::Free));
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_track_de_trilha_alheia_falha() {
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let (_c, _t, _rx) = entra(&sala, "Ana", None).await.expect("ana");
        espera_membros(&sala, 2).await;
        let ana = sala
            .model
            .lock()
            .participants
            .iter()
            .find(|p| p.name == "Ana")
            .map(|p| p.id.clone())
            .expect("ana");
        let t = sala.model.lock().add_track(&ana, spec_camera()).expect("trilha");
        let r = sala
            .update_track(&t.id, None, None, Some(zap_proto::CameraLayout::Free))
            .await;
        assert!(matches!(r, Err(CoreError::TrackNotFound)));
        assert_eq!(sala.model.lock().track_by_id(&t.id).expect("t").camera_layout, None);
        let r = sala
            .update_track("t_nao_existe", None, None, Some(zap_proto::CameraLayout::Free))
            .await;
        assert!(matches!(r, Err(CoreError::TrackNotFound)));
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_all_only_e_allbut() {
        let sala = OwnerRoom::start(ctx(), req(None)).await.expect("start");
        let (_c1, _t1, mut rx1) = entra(&sala, "Ana", None).await.expect("a");
        let (_c2, _t2, mut rx2) = entra(&sala, "Bia", None).await.expect("b");
        espera_membros(&sala, 3).await;
        let ids: Vec<String> = sala
            .model
            .lock()
            .participants
            .iter()
            .filter(|p| p.name == "Ana")
            .map(|p| p.id.clone())
            .collect();
        let ana = ids[0].clone();

        let eh = |t: &'static str| move |m: &ControlMsg| matches!(m, ControlMsg::Error { code, .. } if code == t);
        sala.send_all(vec![Outgoing {
            target: Target::Only(ana.clone()),
            msg: ControlMsg::error("so_ana", "x", None),
        }])
        .await;
        le_ate(&mut rx1, eh("so_ana")).await;

        sala.send_all(vec![Outgoing {
            target: Target::AllBut(ana),
            msg: ControlMsg::error("menos_ana", "x", None),
        }])
        .await;
        // Bia recebe as duas? Não: só a segunda; a primeira não era para ela.
        let m = le_ate(&mut rx2, |m| matches!(m, ControlMsg::Error { .. })).await;
        assert!(matches!(m, ControlMsg::Error { code, .. } if code == "menos_ana"));
        sala.close().await;
    }
}
