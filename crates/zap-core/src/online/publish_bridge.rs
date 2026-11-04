//! Publicar pela ponte: só as camadas que o servidor pede (`camadas.pedidas`).
//!
//! Mecanismo: cada trilha publicada ganha um `ViewerLink` "de ponte" (`ViewerState::bridge`)
//! que recebe **todos** os quadros das camadas pedidas pelo servidor (sem alocador por
//! espectador: quem escolhe a camada de cada espectador é o servidor).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use zap_net::{control::ControlTx, media_tx::MediaSender};
use zap_proto::control::LayerRequestMsg;
use zap_proto::Track;
use zap_rate::allocator::LayerRequest;

use crate::publish::{PublishedTrack, ViewerLink, ViewerState};

pub(crate) struct Feed {
    pub track: Track,
    pub published: PublishedTrack,
    /// Link de ponte da trilha (vídeo ou áudio); `None` enquanto não há ponte.
    pub link: Option<Arc<ViewerLink>>,
}

#[derive(Default)]
pub struct BridgePublish {
    pub(crate) feeds: Mutex<HashMap<String, Feed>>,
    pub(crate) next_id: AtomicU64,
}

/// Ids de ponte começam em `1 << 32` para não colidir com os do `MediaServer`.
const ID_BASE: u64 = 1 << 32;

fn add_link(published: &PublishedTrack, link: &Arc<ViewerLink>) {
    match published {
        PublishedTrack::Video(v) => v.add_viewer(link.clone()),
        PublishedTrack::Audio(a) => a.add_viewer(link.clone()),
    }
}

fn remove_link(published: &PublishedTrack, id: u64) {
    match published {
        PublishedTrack::Video(v) => v.remove_viewer(id),
        PublishedTrack::Audio(a) => a.remove_viewer(id),
    }
}

impl BridgePublish {
    fn make_link(
        &self,
        track_id: &str,
        published: &PublishedTrack,
        conn: quinn::Connection,
        ctrl: ControlTx,
        clock: zap_net::Clock,
    ) -> Arc<ViewerLink> {
        let sender = Arc::new(MediaSender::new(conn, clock));
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + ID_BASE;
        let link = ViewerLink::new(id, "ponte".into(), track_id.to_owned(), sender, ctrl, 0);
        *link.state.lock() = ViewerState::new_bridge();
        add_link(published, &link);
        link
    }

    /// Guarda a trilha e, se `bridge` é `Some((conn, ctrl, clock))`, cria o `ViewerLink` de
    /// ponte (`MediaSender::new(conn, clock)`, id de `next_id` a partir de `1 << 32` para não
    /// colidir com os de `MediaServer`) com `ViewerState::new_bridge` e o registra no
    /// publicador (`add_viewer`).
    pub fn attach(
        &self,
        track: &Track,
        published: PublishedTrack,
        bridge: Option<(quinn::Connection, ControlTx, zap_net::Clock)>,
    ) {
        let link = bridge.map(|(conn, ctrl, clock)| self.make_link(&track.id, &published, conn, ctrl, clock));
        let antigo = self.feeds.lock().insert(
            track.id.clone(),
            Feed {
                track: track.clone(),
                published,
                link,
            },
        );
        // Fora do lock de `feeds`: o publicador tem os próprios locks.
        if let Some(Feed {
            published,
            link: Some(l),
            ..
        }) = antigo
        {
            remove_link(&published, l.id);
        }
    }

    /// `camadas.pedidas`: define as camadas e bitrates pedidos ao pipeline
    /// (`VideoPublisher::set_bridge_requests`) e as camadas que o link de ponte deixa passar.
    /// Lista vazia = parar de codificar tudo. Trilha desconhecida: ignora.
    pub fn on_layers_requested(&self, track_id: &str, layers: &[LayerRequestMsg]) {
        let (published, link) = match self.feeds.lock().get(track_id) {
            Some(f) => (f.published.clone(), f.link.clone()),
            None => return,
        };
        // Áudio sempre flui: só o vídeo tem camadas pedidas.
        let PublishedTrack::Video(v) = published else {
            return;
        };
        v.set_bridge_requests(
            layers
                .iter()
                .map(|l| LayerRequest {
                    layer: l.layer,
                    kbps: l.kbps,
                })
                .collect(),
        );
        if let Some(l) = link {
            l.state
                .lock()
                .set_bridge_layers(layers.iter().map(|l| l.layer).collect());
        }
    }

    /// `keyframe.pedir` do servidor: `VideoPublisher::request_keyframe(layer)` (`None` = todas
    /// as camadas pedidas).
    pub fn on_keyframe_request(&self, track_id: &str, layer: Option<u8>) {
        let (published, link) = match self.feeds.lock().get(track_id) {
            Some(f) => (f.published.clone(), f.link.clone()),
            None => return,
        };
        let PublishedTrack::Video(v) = published else {
            return;
        };
        match layer {
            Some(l) => v.request_keyframe(l),
            None => {
                let camadas = link.map(|l| l.state.lock().bridge_layers.clone()).unwrap_or_default();
                for l in camadas {
                    v.request_keyframe(l);
                }
            }
        }
    }

    fn release(feed: Feed) {
        if let Some(l) = &feed.link {
            remove_link(&feed.published, l.id);
        }
        if let PublishedTrack::Video(v) = &feed.published {
            v.set_bridge_requests(vec![]);
        }
    }

    /// Tira o link do publicador, zera o pedido de camadas e esquece a trilha.
    pub fn detach(&self, track_id: &str) {
        let feed = self.feeds.lock().remove(track_id);
        if let Some(f) = feed {
            Self::release(f);
        }
    }

    pub fn detach_all(&self) {
        let todos: Vec<Feed> = self.feeds.lock().drain().map(|(_, f)| f).collect();
        for f in todos {
            Self::release(f);
        }
    }

    /// Reconexão da ponte: recria os links de todas as trilhas guardadas na conexão nova (os
    /// antigos saem dos publicadores). O servidor repete `camadas.pedidas` ao reconectar.
    pub fn reattach_all(&self, conn: &quinn::Connection, ctrl: &ControlTx, clock: &zap_net::Clock) {
        let atuais: Vec<(String, PublishedTrack, Option<Arc<ViewerLink>>)> = self
            .feeds
            .lock()
            .iter()
            .map(|(k, f)| (k.clone(), f.published.clone(), f.link.clone()))
            .collect();
        for (id, published, antigo) in atuais {
            if let Some(l) = antigo {
                remove_link(&published, l.id);
            }
            let novo = self.make_link(&id, &published, conn.clone(), ctrl.clone(), clock.clone());
            if let Some(f) = self.feeds.lock().get_mut(&id) {
                f.link = Some(novo);
            } else {
                // Trilha saiu enquanto recriávamos: desfaz o link novo.
                remove_link(&published, novo.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use zap_media::{EncoderChoiceSpec, VideoPipelineConfig, VideoSourceSpec};
    use zap_net::{
        endpoint,
        tls::{self, Identity},
        Clock,
    };
    use zap_rate::ladder::{screen_ladder, Preset};

    use crate::publish::VideoPublisher;

    fn publicador() -> Arc<VideoPublisher> {
        let cfg = VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Synthetic {
                width: 1280,
                height: 720,
            },
            ladder: screen_ladder(Preset::P720p30, 1280, 720),
            encoder: EncoderChoiceSpec::Auto,
        };
        VideoPublisher::start(cfg, tokio::runtime::Handle::current(), Clock::new()).unwrap()
    }

    /// Conexão QUIC de loopback; devolve também o lado do servidor para manter vivo.
    async fn conexao() -> (quinn::Connection, ControlTx, quinn::Connection) {
        let id = Identity::generate("zap").unwrap();
        let (servidor, porta) = endpoint::bind_server(&id, 0).await.unwrap();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let cliente = endpoint::client_endpoint().unwrap();
        let conectando = cliente
            .connect_with(tls::client_config(Some(&id.fingerprint())).unwrap(), addr, "zap")
            .unwrap();
        let inc = servidor.accept().await.unwrap();
        let srv = inc.await.unwrap();
        let cli = conectando.await.unwrap();
        let (send, _recv) = cli.open_bi().await.unwrap();
        // Mantém endpoints vivos até o fim do processo de teste.
        std::mem::forget(servidor);
        std::mem::forget(cliente);
        (cli, ControlTx::new(send), srv)
    }

    fn trilha(id: &str) -> Track {
        serde_json::from_value(serde_json::json!({
            "id": id, "participante_id": "p1", "numero": 1, "tipo": "tela", "titulo": "t"
        }))
        .unwrap()
    }

    fn pedido(layer: u8, kbps: u32) -> LayerRequestMsg {
        LayerRequestMsg { layer, kbps }
    }

    fn viewers(p: &VideoPublisher) -> usize {
        p.shared.viewers.lock().len()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attach_sem_ponte_guarda_a_trilha() {
        let p = publicador();
        let b = BridgePublish::default();
        b.attach(&trilha("v1"), PublishedTrack::Video(p.clone()), None);
        let feeds = b.feeds.lock();
        assert!(feeds.get("v1").unwrap().link.is_none());
        assert_eq!(viewers(&p), 0);
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn layers_requested_define_camadas_do_link() {
        let (cli, ctrl, _srv) = conexao().await;
        let p = publicador();
        let b = BridgePublish::default();
        b.attach(
            &trilha("v1"),
            PublishedTrack::Video(p.clone()),
            Some((cli, ctrl, Clock::new())),
        );
        b.on_layers_requested("v1", &[pedido(0, 500), pedido(1, 300)]);
        let link = b.feeds.lock().get("v1").unwrap().link.clone().unwrap();
        assert!(link.id >= 1 << 32);
        assert_eq!(link.state.lock().bridge_layers, vec![0, 1]);
        assert_eq!(viewers(&p), 1);
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn layers_requested_vazio_para_de_codificar() {
        let (cli, ctrl, _srv) = conexao().await;
        let p = publicador();
        let b = BridgePublish::default();
        b.attach(
            &trilha("v1"),
            PublishedTrack::Video(p.clone()),
            Some((cli, ctrl, Clock::new())),
        );
        b.on_layers_requested("v1", &[pedido(0, 500)]);
        let ini = Instant::now();
        while p.stats().layers.is_empty() && ini.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!p.stats().layers.is_empty(), "deveria codificar a camada pedida");
        b.on_layers_requested("v1", &[]);
        let ini = Instant::now();
        while !p.stats().layers.is_empty() && ini.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(p.stats().layers.is_empty());
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detach_remove_o_link() {
        let (cli, ctrl, _srv) = conexao().await;
        let p = publicador();
        let b = BridgePublish::default();
        b.attach(
            &trilha("v1"),
            PublishedTrack::Video(p.clone()),
            Some((cli, ctrl, Clock::new())),
        );
        assert_eq!(viewers(&p), 1);
        b.detach("v1");
        assert_eq!(viewers(&p), 0);
        assert!(b.feeds.lock().is_empty());
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reattach_all_troca_o_link() {
        let (cli, ctrl, _srv) = conexao().await;
        let p = publicador();
        let b = BridgePublish::default();
        b.attach(&trilha("v1"), PublishedTrack::Video(p.clone()), None);
        b.reattach_all(&cli, &ctrl, &Clock::new());
        let id1 = b.feeds.lock().get("v1").unwrap().link.clone().unwrap().id;
        assert_eq!(viewers(&p), 1);
        b.reattach_all(&cli, &ctrl, &Clock::new());
        let l2 = b.feeds.lock().get("v1").unwrap().link.clone().unwrap();
        assert_ne!(l2.id, id1);
        assert!(l2.state.lock().bridge_layers.is_empty());
        assert_eq!(viewers(&p), 1);
        p.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn trilha_desconhecida_ignorada() {
        let b = BridgePublish::default();
        b.on_layers_requested("nada", &[pedido(0, 100)]);
        b.on_keyframe_request("nada", None);
        b.detach("nada");
        assert!(b.feeds.lock().is_empty());
    }
}
