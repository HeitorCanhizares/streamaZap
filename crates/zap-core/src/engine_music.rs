//! Comandos da música da sala (`docs/planos/musica-da-sala.md`).

use serde::Deserialize;
use zap_net::youtube::{oembed, OembedError};
use zap_proto::ControlMsg;

use crate::engine::Core;
use crate::music::video_id_from_link;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;
    use crate::types::{CreateRoom, JoinTarget, RoomMode};

    async fn criar(fp: String) -> (Core, PathBuf) {
        tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-musica-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                dev_cert_fp: Some(fp),
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap()
    }

    async fn encerrar(core: Core, d: PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    async fn ate(limite_s: u64, mut ok: impl FnMut() -> bool) -> bool {
        let fim = std::time::Instant::now() + Duration::from_secs(limite_s);
        while std::time::Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ok()
    }

    fn musica(core: &Core) -> Option<crate::snapshot::MusicView> {
        core.snapshot().room.as_ref().and_then(|r| r.music.clone())
    }

    fn online(core: &Core) -> bool {
        core.snapshot()
            .server
            .as_ref()
            .is_some_and(|s| s.state == crate::online::ServerState::Online)
    }

    /// Duas pessoas numa sala Online pelo servidor de teste: a música de uma chega à outra pelo
    /// servidor (a fonte no Online), e o voto da dona pula.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn musica_numa_sala_online_passa_pelo_servidor() {
        let srv = zap_test_server::spawn(zap_test_server::Config::default())
            .await
            .unwrap();
        let (ana, da) = criar(srv.cert_fingerprint.clone()).await;
        let (bia, db) = criar(srv.cert_fingerprint.clone()).await;
        for c in [&ana, &bia] {
            c.update_settings(|s| s.server.address = srv.base_url.clone());
        }
        assert!(ate(10, || online(&ana) && online(&bia)).await, "os dois no servidor");

        ana.create_room(CreateRoom {
            name: "Sala".into(),
            mode: RoomMode::Online,
            password: None,
        })
        .await
        .unwrap();
        let codigo = ana
            .snapshot()
            .room
            .as_ref()
            .and_then(|r| r.code.clone())
            .expect("código");
        bia.join_room(JoinTarget::Code { code: codigo }, None).await.unwrap();
        // Entrar já traz a música da sala (vazia): a interface sabe que a sala tem música.
        assert!(
            ate(5, || musica(&bia).is_some()).await,
            "Bia recebeu o estado da música"
        );

        // Bia põe uma música (sem passar pelo oEmbed, que iria à internet).
        bia.music_send(ControlMsg::MusicAdd {
            video_id: "aaaaaaaaaaa".into(),
            title: Some("Primeira".into()),
            channel: None,
        })
        .await
        .unwrap();
        let bia_id = bia.snapshot().room.as_ref().map(|r| r.me_id.clone()).unwrap();
        assert!(
            ate(5, || musica(&ana).and_then(|m| m.current).is_some_and(|c| c
                .item
                .added_by
                == bia_id
                && c.item.title == "Primeira"))
            .await,
            "Ana vê a música que Bia pôs"
        );
        let atual = musica(&bia).and_then(|m| m.current).expect("Bia também vê");
        assert_eq!(atual.needed, 1, "2 pessoas: 1 voto");

        // O voto de Ana pula (e a fila acaba para as duas).
        ana.music_vote(atual.item.id.clone(), true).await.unwrap();
        assert!(ate(5, || musica(&bia).is_some_and(|m| m.current.is_none())).await);
        assert!(musica(&ana).is_some_and(|m| m.current.is_none()));

        encerrar(ana, da).await;
        encerrar(bia, db).await;
    }
}
