//! Comandos da música da sala (`docs/planos/musica-da-sala.md`).

use serde::Deserialize;
use zap_net::youtube::{oembed, OembedError};
use zap_proto::ControlMsg;

use crate::engine::Core;
use crate::music::video_id_from_link;
use crate::room::{music_unsupported, ActiveRoom};
use crate::CoreError;

/// O que o player da interface descobriu sobre um item (`musica.info`).
#[derive(Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicReport {
    pub item_id: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub live: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub ended: bool,
    #[serde(default)]
    pub failed: bool,
}

impl Core {
    /// Põe um vídeo do YouTube na fila da sala. Lê o id do link, consulta o oEmbed (título e
    /// canal; recusa vídeo que não toca fora do YouTube) e manda `musica.adicionar` ao dono. Sem
    /// internet para o oEmbed, adiciona assim mesmo (o player completa o título).
    pub async fn music_add(&self, link: String) -> Result<(), CoreError> {
        let video_id = video_id_from_link(&link)
            .ok_or_else(|| CoreError::Invalid("cole o link de um vídeo ou de uma live do YouTube".into()))?;
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                let sala = shared.current_room().ok_or(CoreError::NoRoom)?;
                // Antes de consultar o YouTube: no Online não adianta.
                if matches!(*sala, ActiveRoom::Online(_)) {
                    return Err(music_unsupported());
                }
                let (title, channel) = match oembed(&video_id).await {
                    Ok(v) => (Some(v.title), v.channel),
                    Err(e @ (OembedError::NotEmbeddable | OembedError::NotFound)) => {
                        return Err(CoreError::Invalid(e.to_string()));
                    }
                    Err(OembedError::Network(e)) => {
                        tracing::warn!(erro = %e, "oEmbed do YouTube falhou; adicionando sem título");
                        (None, None)
                    }
                };
                sala.music(ControlMsg::MusicAdd {
                    video_id,
                    title,
                    channel,
                })
                .await
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }

    /// Tira um item da fila (o atual = pular direto: só quem pôs ou o dono da sala).
    pub async fn music_remove(&self, item_id: String) -> Result<(), CoreError> {
        self.music_send(ControlMsg::MusicRemove { item_id }).await
    }

    /// Voto para pular o item atual (`false` desfaz).
    pub async fn music_vote(&self, item_id: String, vote: bool) -> Result<(), CoreError> {
        self.music_send(ControlMsg::MusicSkip { item_id, vote }).await
    }

    /// O que o player descobriu (duração, live, título, fim, erro).
    pub async fn music_report(&self, r: MusicReport) -> Result<(), CoreError> {
        self.music_send(ControlMsg::MusicInfo {
            item_id: r.item_id,
            duration_ms: r.duration_ms,
            live: r.live,
            title: r.title,
            ended: r.ended,
            failed: r.failed,
        })
        .await
    }

    async fn music_send(&self, msg: ControlMsg) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                let sala = shared.current_room().ok_or(CoreError::NoRoom)?;
                sala.music(msg).await
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
}
