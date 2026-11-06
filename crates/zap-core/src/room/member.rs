//! Lado do membro: conecta ao dono, mantém o espelho da sala.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::oneshot;
use zap_net::control::ControlTx;
use zap_net::handshake::{connect_handshake, Hello};
use zap_net::NetError;
use zap_proto::{ControlMsg, Track};

use super::model::{RoomModel, TrackSpec};
use super::{member_run, RoomCtx};
use crate::util::new_id;
use crate::CoreError;

pub struct MemberRoom {
    pub(crate) ctx: Arc<RoomCtx>,
    pub(crate) model: Arc<Mutex<RoomModel>>,
    pub(crate) my_id: Mutex<String>,
    pub(crate) media_secret: Mutex<String>,
    pub(crate) tx: Mutex<Option<ControlTx>>,
    pub(crate) owner_addr: SocketAddr,
    pub(crate) fingerprint: String,
    pub(crate) password: Option<String>,
    /// `local_id` → quem espera o `trilha.publicada`.
    pub(crate) pending_tracks: Mutex<HashMap<String, oneshot::Sender<Track>>>,
    pub(crate) left: AtomicBool,
    /// Conexão atual com o dono (`join` guarda a primeira; a reconexão troca); `leave` fecha esta.
    pub(crate) conn: Mutex<Option<quinn::Connection>>,
}

impl MemberRoom {
    /// Troca a conexão atual e fecha a anterior, se ainda houver (reconexão).
    pub(crate) fn set_conn(&self, conn: quinn::Connection) {
        let antiga = self.conn.lock().replace(conn);
        if let Some(antiga) = antiga {
            antiga.close(0u32.into(), b"reconectou");
        }
    }
    /// Conecta (`client_endpoint` + `connect_with(client_config(fingerprint))`), faz
    /// `connect_handshake` (`Hello` com `room_id`), guarda `segredo_midia` do `Welcome`, cria o
    /// `RoomModel` a partir de `Welcome.room` (sem participantes até o `sala.estado`) e sobe
    /// `member_run::run`. Falha de senha → `BadPassword`; timeout de 6 s no connect →
    /// `Connect("sem resposta")`.
    pub async fn join(
        ctx: Arc<RoomCtx>,
        addr: SocketAddr,
        fingerprint: String,
        password: Option<String>,
        room_id: Option<String>,
    ) -> Result<Arc<Self>, CoreError> {
        let ep = zap_net::endpoint::client_endpoint()?;
        let cfg = zap_net::tls::client_config(Some(&fingerprint))?;
        let conectando = ep.connect_with(cfg, addr, "zap").map_err(NetError::from)?;
        let conn = tokio::time::timeout(Duration::from_secs(6), conectando)
            .await
            .map_err(|_| CoreError::Connect("sem resposta".into()))?
            .map_err(NetError::from)?;
        let hello = Hello {
            app_version: ctx.app_version.clone(),
            install_id: ctx.install_id.clone(),
            name: ctx.name.clone(),
            color: ctx.color.clone(),
            ticket: None,
            room_id,
            preview: None,
        };
        let (tx, rx, welcome) = match connect_handshake(&conn, hello, password.as_deref()).await {
            Ok(r) => r,
            Err(e) => {
                conn.close(0u32.into(), b"recusado");
                return Err(CoreError::from_handshake(e));
            }
        };
        let room = Arc::new(MemberRoom {
            ctx,
            model: Arc::new(Mutex::new(RoomModel::from_state(welcome.room, vec![], vec![]))),
            my_id: Mutex::new(welcome.participant_id),
            media_secret: Mutex::new(welcome.media_secret.unwrap_or_default()),
            tx: Mutex::new(Some(tx)),
            owner_addr: addr,
            fingerprint,
            password,
            pending_tracks: Mutex::new(HashMap::new()),
            left: AtomicBool::new(false),
            conn: Mutex::new(Some(conn.clone())),
        });
        tokio::spawn(member_run::run(room.clone(), rx, conn));
        Ok(room)
    }
    /// Envia ao dono pelo `tx` atual (cópia: o lock não atravessa o await).
    pub(crate) async fn send(&self, msg: ControlMsg) -> Result<(), CoreError> {
        let tx = self.tx.lock().clone().ok_or(CoreError::NoRoom)?;
        tx.send(&msg).await?;
        Ok(())
    }
    /// Pedido de música para o dono (a resposta chega como `musica.estado` ou `erro`).
    pub async fn music(&self, msg: ControlMsg) -> Result<(), CoreError> {
        self.send(msg).await
    }
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        if text.trim().is_empty() {
            return Err(CoreError::Invalid("mensagem vazia".into()));
        }
        if text.chars().count() > 1000 {
            return Err(CoreError::Invalid("mensagem longa demais (máx. 1000)".into()));
        }
        self.send(ControlMsg::ChatSend {
            text,
            client_id: new_id("c_"),
        })
        .await
    }
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        self.send(ControlMsg::ChatTyping { participant_id: None }).await
    }
    /// Manda `trilha.publicar` e espera o `trilha.publicada` com o mesmo `local_id` (5 s).
    pub async fn publish_track(&self, spec: TrackSpec) -> Result<Track, CoreError> {
        let local_id = spec.local_id.clone();
        let (aviso, espera) = oneshot::channel();
        self.pending_tracks.lock().insert(local_id.clone(), aviso);
        let enviado = self
            .send(ControlMsg::TrackPublish {
                local_id: local_id.clone(),
                kind: spec.kind,
                title: spec.title,
                layers: spec.layers,
                audio: spec.audio,
                address: spec.address,
            })
            .await;
        if let Err(e) = enviado {
            self.pending_tracks.lock().remove(&local_id);
            return Err(e);
        }
        let resposta = tokio::time::timeout(Duration::from_secs(5), espera).await;
        // Em qualquer desfecho o pendente sai do mapa.
        self.pending_tracks.lock().remove(&local_id);
        match resposta {
            Ok(Ok(track)) => Ok(track),
            _ => Err(CoreError::Connect("sem resposta do dono".into())),
        }
    }
    pub async fn update_track(
        &self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<zap_proto::LayerSpec>>,
    ) -> Result<(), CoreError> {
        self.send(ControlMsg::TrackUpdate {
            track_id: track_id.into(),
            title,
            layers,
        })
        .await
    }
    pub async fn remove_track(&self, track_id: &str) -> Result<(), CoreError> {
        self.send(ControlMsg::TrackRemove {
            track_id: track_id.into(),
        })
        .await
    }
    pub async fn request_reverse(&self, track_id: &str, address: &str, token: &str) -> Result<(), CoreError> {
        self.send(ControlMsg::ReverseConnection {
            track_id: track_id.into(),
            address: address.into(),
            token: token.into(),
        })
        .await
    }
    /// `sala.sair`, marca `left` e fecha a conexão.
    pub async fn leave(&self) {
        self.left.store(true, Ordering::SeqCst);
        // Erro ignorado: se a conexão já caiu, sair é só fechar.
        let _ = self.send(ControlMsg::RoomLeave {}).await;
        // Dá tempo de o aviso chegar antes de fechar a conexão.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = self.conn.lock().take();
        if let Some(conn) = conn {
            conn.close(0u32.into(), b"saiu");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tokio::sync::mpsc::unbounded_channel;
    use zap_net::tls::Identity;
    use zap_proto::{LayerSpec, RoomMode, TrackKind};

    use crate::room::OwnerRoom;
    use crate::types::CreateRoom;

    fn ctx(nome: &str) -> Arc<RoomCtx> {
        let (events, rx) = unbounded_channel();
        std::mem::forget(rx);
        Arc::new(RoomCtx {
            identity: Arc::new(Identity::generate("zap").expect("identidade")),
            install_id: format!("inst-{nome}"),
            name: nome.into(),
            color: "#123456".into(),
            port: 0,
            interface: None,
            include_loopback: true,
            app_version: "0.1.0".into(),
            events,
        })
    }

    async fn dono(senha: Option<&str>) -> Arc<OwnerRoom> {
        let req = CreateRoom {
            name: "Sala".into(),
            mode: RoomMode::Local,
            password: senha.map(String::from),
        };
        OwnerRoom::start(ctx("Dono"), req).await.expect("start")
    }

    async fn entra(sala: &OwnerRoom, senha: Option<&str>) -> Result<Arc<MemberRoom>, CoreError> {
        let addr = SocketAddr::from(([127, 0, 0, 1], sala.port));
        MemberRoom::join(
            ctx("Ana"),
            addr,
            sala.ctx.identity.fingerprint(),
            senha.map(String::from),
            None,
        )
        .await
    }

    async fn ate(mut cond: impl FnMut() -> bool) -> bool {
        let prazo = Instant::now() + Duration::from_secs(2);
        while Instant::now() < prazo {
            if cond() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        cond()
    }

    fn spec(local: &str, kind: TrackKind) -> TrackSpec {
        TrackSpec {
            local_id: local.into(),
            kind,
            title: "Minha tela".into(),
            layers: vec![LayerSpec {
                layer: 0,
                width: 1280,
                height: 720,
                fps: 30,
                kbps: 2000,
            }],
            audio: None,
            address: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_ok_recebe_id_e_segredo() {
        let sala = dono(None).await;
        let m = entra(&sala, None).await.expect("join");
        assert!(!m.media_secret.lock().is_empty());
        assert!(!m.my_id.lock().is_empty());
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_senha_errada_bad_password() {
        let sala = dono(Some("certa")).await;
        let r = entra(&sala, Some("errada")).await;
        assert!(matches!(r, Err(CoreError::BadPassword)));
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn join_porta_fechada_erro_connect() {
        // Reserva uma porta UDP e a libera: ninguém escuta nela.
        let s = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
        let addr = s.local_addr().expect("addr");
        drop(s);
        let ident = Identity::generate("zap").expect("identidade");
        let ini = Instant::now();
        let r = MemberRoom::join(ctx("Ana"), addr, ident.fingerprint(), None, None).await;
        assert!(r.is_err());
        assert!(ini.elapsed() < Duration::from_secs(7));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_do_membro_chega_ao_dono() {
        let sala = dono(None).await;
        let m = entra(&sala, None).await.expect("join");
        m.send_chat("oi".into()).await.expect("chat");
        assert!(ate(|| sala.model.lock().chat.iter().any(|l| l.text == "oi")).await);
        assert!(matches!(m.send_chat("  ".into()).await, Err(CoreError::Invalid(_))));
        assert!(matches!(
            m.send_chat("x".repeat(1001)).await,
            Err(CoreError::Invalid(_))
        ));
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publish_track_devolve_trilha_com_numero() {
        let sala = dono(None).await;
        let m = entra(&sala, None).await.expect("join");
        let t = m.publish_track(spec("l_1", TrackKind::Screen)).await.expect("trilha");
        assert!(t.number >= 1);
        assert!(sala.model.lock().tracks.iter().any(|x| x.id == t.id));
        assert!(m.pending_tracks.lock().is_empty());
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publish_track_duplicada_erro_timeout_ou_invalid() {
        let sala = dono(None).await;
        let m = entra(&sala, None).await.expect("join");
        m.publish_track(spec("l_1", TrackKind::Screen)).await.expect("primeira");
        let r = m.publish_track(spec("l_2", TrackKind::Screen)).await;
        assert!(r.is_err());
        assert!(m.pending_tracks.lock().is_empty());
        sala.close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn leave_remove_o_membro_no_dono() {
        let sala = dono(None).await;
        let m = entra(&sala, None).await.expect("join");
        assert!(ate(|| sala.model.lock().participants.len() == 2).await);
        m.leave().await;
        assert!(ate(|| sala.model.lock().participants.len() == 1).await);
        sala.close().await;
    }
}
