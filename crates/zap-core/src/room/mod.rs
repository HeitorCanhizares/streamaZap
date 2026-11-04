//! Sala do modo Local: o dono (servidor QUIC + descoberta) e os membros.

mod apply;
mod dispatch;
mod member;
mod member_run;
mod model;
mod owner;
mod owner_member;

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use zap_net::tls::Identity;
use zap_proto::{ControlMsg, Track};

pub use apply::{apply_owner_msg, Effect};
pub use dispatch::{handle_member_msg, on_member_joined, on_member_left, Outgoing, Target};
pub use member::MemberRoom;
pub use model::{chat_line_from_msg, chat_message_msg, RoomModel, TrackSpec, CHAT_HISTORY};
pub use owner::OwnerRoom;

use crate::types::ChatLine;
use crate::CoreError;

/// Eventos da sala para o `Core`.
#[derive(Debug, Clone)]
pub enum RoomEvent {
    /// Participantes/trilhas mudaram.
    Changed,
    Chat(ChatLine),
    /// A sala acabou (dono saiu/encerrou) ou o membro desistiu de reconectar.
    Closed(String),
    Error(String),
    Reconnecting,
    /// Reconectou ao dono: o `Core` republica o que eu compartilhava e volta a assistir.
    Reconnected,
    /// Alguém quer receber minha trilha por conexão reversa.
    ReverseRequest {
        track_id: String,
        address: String,
        token: String,
    },
    /// Trilha de outro participante foi removida (para encerrar assinaturas).
    TrackRemoved(String),
}

/// O que as salas precisam do `Core` (cópia dos valores, sem referência ao `Core`).
pub struct RoomCtx {
    pub identity: Arc<Identity>,
    pub install_id: String,
    pub name: String,
    pub color: String,
    /// Porta preferida do endpoint de controle (só o dono).
    pub port: u16,
    pub interface: Option<String>,
    pub include_loopback: bool,
    pub app_version: String,
    pub events: UnboundedSender<RoomEvent>,
}

pub enum ActiveRoom {
    Owner(Arc<OwnerRoom>),
    Member(Arc<MemberRoom>),
}

impl ActiveRoom {
    pub fn model(&self) -> Arc<Mutex<RoomModel>> {
        match self {
            ActiveRoom::Owner(o) => o.model.clone(),
            ActiveRoom::Member(m) => m.model.clone(),
        }
    }
    pub fn my_id(&self) -> String {
        match self {
            ActiveRoom::Owner(o) => o.my_id.clone(),
            ActiveRoom::Member(m) => m.my_id.lock().clone(),
        }
    }
    /// Segredo de mídia da sala (dono: o que gerou; membro: o do `bem_vindo`).
    pub fn media_secret(&self) -> String {
        match self {
            ActiveRoom::Owner(o) => o.media_secret.clone(),
            ActiveRoom::Member(m) => m.media_secret.lock().clone(),
        }
    }
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError> {
        match self {
            ActiveRoom::Owner(o) => o.send_chat(text).await,
            ActiveRoom::Member(m) => m.send_chat(text).await,
        }
    }
    pub async fn send_typing(&self) -> Result<(), CoreError> {
        match self {
            // O dono não tem "digitando" por conexão: avisa todos menos ele mesmo.
            ActiveRoom::Owner(o) => {
                let me = o.my_id.clone();
                o.send_all(vec![Outgoing {
                    target: Target::AllBut(me.clone()),
                    msg: ControlMsg::ChatTyping {
                        participant_id: Some(me),
                    },
                }])
                .await;
                Ok(())
            }
            ActiveRoom::Member(m) => m.send_typing().await,
        }
    }
    pub async fn publish_track(&self, spec: TrackSpec) -> Result<Track, CoreError> {
        match self {
            ActiveRoom::Owner(o) => o.publish_track(spec).await,
            ActiveRoom::Member(m) => m.publish_track(spec).await,
        }
    }
    pub async fn update_track(
        &self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<zap_proto::LayerSpec>>,
    ) -> Result<(), CoreError> {
        match self {
            ActiveRoom::Owner(o) => o.update_track(track_id, title, layers).await,
            ActiveRoom::Member(m) => m.update_track(track_id, title, layers).await,
        }
    }
    pub async fn remove_track(&self, track_id: &str) -> Result<(), CoreError> {
        match self {
            ActiveRoom::Owner(o) => o.remove_track(track_id).await,
            ActiveRoom::Member(m) => m.remove_track(track_id).await,
        }
    }
    /// Pede conexão reversa pelo dono da sala (`conexao.reversa`).
    pub async fn request_reverse(&self, track_id: &str, address: &str, token: &str) -> Result<(), CoreError> {
        match self {
            ActiveRoom::Owner(o) => o.forward_reverse(track_id, address, token).await,
            ActiveRoom::Member(m) => m.request_reverse(track_id, address, token).await,
        }
    }
    /// Dono: encerra a sala (`sala.encerrada{dono}`); membro: manda `sala.sair` e fecha.
    pub async fn leave(&self) {
        match self {
            ActiveRoom::Owner(o) => o.close().await,
            ActiveRoom::Member(m) => m.leave().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tokio::sync::mpsc::unbounded_channel;
    use zap_net::tls::Identity;
    use zap_proto::RoomMode;

    use crate::types::CreateRoom;

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

    async fn dono() -> ActiveRoom {
        let req = CreateRoom {
            name: "Sala".into(),
            mode: RoomMode::Local,
            password: None,
        };
        ActiveRoom::Owner(OwnerRoom::start(ctx(), req).await.expect("sala do dono"))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn my_id_e_media_secret_do_dono() {
        let sala = dono().await;
        assert!(!sala.my_id().is_empty());
        let segredo = STANDARD.decode(sala.media_secret()).expect("base64");
        assert_eq!(segredo.len(), 32);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_do_dono_chega_ao_modelo() {
        let sala = dono().await;
        sala.send_chat("oi".into()).await.expect("chat");
        assert_eq!(sala.model().lock().chat.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn leave_do_dono_nao_panica() {
        let sala = dono().await;
        sala.leave().await;
    }
}
