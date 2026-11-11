//! Atendimento de uma conexão de espectador no publicador.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use zap_net::control::{ControlRx, ControlTx};
use zap_net::media_tx::MediaSender;
use zap_proto::{ControlMsg, TileSize};

use super::viewer::{LayerInfo, ViewerLink};
use super::{MediaServer, PublishedTrack};

/// Laço de controle da conexão (`conn` já passou pelo aperto de mão). `label` = `instalacao_id`.
/// - `Subscribe{track_id, size, max_layer, audio}`: acha a trilha em `server.tracks`
///   (inexistente → `Error{codigo:"trilha_inexistente"}`); cria um `ViewerLink` (id de
///   `next_viewer`, `MediaSender::new(conn.clone(), clock.clone())` compartilhado pela conexão)
///   por trilha; se já existe para a trilha só chama `apply_subscribe`. Vídeo: `add_viewer` e
///   `camada.atual` inicial ("inicio") com a camada mais barata; se `audio == Some(true)`,
///   também liga o `ViewerLink` de áudio (primeira `PublishedTrack::Audio` registrada).
///   Assinar a trilha de áudio direto (quem compartilha só o som) também liga o áudio.
/// - `Unsubscribe{track_id}`: remove o link. Depois de cada assinar/cancelar, `ajustar_audio`
///   mantém o áudio ligado só enquanto alguém nesta conexão o quer. `KeyframeRequest{track_id}`: `request_keyframe` da
///   camada atual do link. `Report`: `Prober::on_report` (uma vez, com o relógio do publicador)
///   e `apply_report` em todos os links da conexão. `Ping`→`Pong`.
/// - Ao terminar (stream fechado, erro): remove todos os links da conexão dos publicadores.
pub(crate) async fn serve_viewer(
    server: Arc<MediaServer>,
    conn: quinn::Connection,
    tx: ControlTx,
    mut rx: ControlRx,
    label: String,
) {
    let sender = Arc::new(MediaSender::new(conn.clone(), server.clock.clone()));
    // Uma sondagem por conexão, compartilhada pelos links de vídeo dela.
    let prober = zap_net::probe::Prober::new(sender.clone());
    let mut links: HashMap<String, (Arc<ViewerLink>, PublishedTrack)> = HashMap::new();
    // Trilhas de áudio assinadas direto (não vieram junto com um vídeo).
    let mut audio_direto: HashSet<String> = HashSet::new();

    // Fim de stream, erro de leitura ou conexão fechada encerram o laço do mesmo jeito.
    while let Ok(Some(msg)) = rx.recv().await {
        match msg {
            ControlMsg::Subscribe {
                track_id,
                size,
                max_layer,
                audio,
            } => {
                assinar(
                    &server,
                    &sender,
                    &prober,
                    &tx,
                    &label,
                    &mut links,
                    &mut audio_direto,
                    track_id,
                    size,
                    max_layer,
                    audio,
                )
                .await;
                ajustar_audio(&server, &sender, &tx, &label, &mut links, &audio_direto, &prober);
            }
            ControlMsg::Unsubscribe { track_id } => {
                audio_direto.remove(&track_id);
                if let Some((link, track)) = links.remove(&track_id) {
                    remove_link(&link, &track, &prober);
                }
                ajustar_audio(&server, &sender, &tx, &label, &mut links, &audio_direto, &prober);
            }
            ControlMsg::KeyframeRequest { track_id, .. } => {
                if let Some((link, PublishedTrack::Video(p))) = links.get(&track_id) {
                    let camada = link.state.lock().current;
                    p.request_keyframe(camada);
                }
            }
            ControlMsg::Report(r) => {
                // Antes do apply: a sondagem usa as chegadas do relatório com o relógio deste lado.
                prober.on_report(server.clock.now_us(), &r);
                for (link, _) in links.values() {
                    link.state.lock().apply_report(&r);
                }
            }
            ControlMsg::Ping { ts_ms } if tx.send(&ControlMsg::Pong { ts_ms }).await.is_err() => break,
            ControlMsg::Ping { .. } => {}
            _ => {}
        }
    }

    // Sem isso o publicador continuaria enviando para uma conexão morta.
    for (link, track) in links.values() {
        remove_link(link, track, &prober);
    }
}

/// Tira o link do publicador; no vídeo também zera a sondagem da trilha.
fn remove_link(link: &ViewerLink, track: &PublishedTrack, prober: &zap_net::probe::Prober) {
    match track {
        PublishedTrack::Video(p) => {
            p.remove_viewer(link.id);
            prober.clear_track(p.track);
        }
        PublishedTrack::Audio(p) => p.remove_viewer(link.id),
    }
}

/// Cria o link de uma trilha e o registra no publicador.
fn novo_link(
    server: &MediaServer,
    sender: &Arc<MediaSender>,
    tx: &ControlTx,
    label: &str,
    track_id: &str,
    track: &PublishedTrack,
    cheapest: u8,
) -> Arc<ViewerLink> {
    let link = ViewerLink::new(
        server.next_viewer.fetch_add(1, Ordering::Relaxed),
        label.to_owned(),
        track_id.to_owned(),
        sender.clone(),
        tx.clone(),
        cheapest,
    );
    match track {
        PublishedTrack::Video(p) => p.add_viewer(link.clone()),
        PublishedTrack::Audio(p) => p.add_viewer(link.clone()),
    }
    link
}

#[allow(clippy::too_many_arguments)]
async fn assinar(
    server: &MediaServer,
    sender: &Arc<MediaSender>,
    prober: &Arc<zap_net::probe::Prober>,
    tx: &ControlTx,
    label: &str,
    links: &mut HashMap<String, (Arc<ViewerLink>, PublishedTrack)>,
    audio_direto: &mut HashSet<String>,
    track_id: String,
    size: TileSize,
    max_layer: Option<u8>,
    audio: Option<bool>,
) {
    // Copia a trilha e solta o lock na hora: nada de lock através de await.
    let track = server.tracks.lock().get(&track_id).cloned();
    let Some(track) = track else {
        let _ = tx
            .send(&ControlMsg::error(
                "trilha_inexistente",
                format!("trilha {track_id} não existe"),
                None,
            ))
            .await;
        return;
    };

    if let Some((link, _)) = links.get(&track_id) {
        link.state.lock().apply_subscribe(size, max_layer, audio);
    } else {
        let (cheapest, info) = match &track {
            PublishedTrack::Video(p) => match p.ladder().last() {
                Some(l) => (l.layer, Some(LayerInfo::from_spec(l))),
                None => (0, None),
            },
            PublishedTrack::Audio(_) => (0, None),
        };
        // Só o vídeo ganha sondagem (compartilhada pela conexão); áudio fica sem.
        let sondagem = matches!(track, PublishedTrack::Video(_)).then(|| prober.clone());
        // Cria sem registrar ainda: o "inicio" precisa sair antes de qualquer decisão do alocador.
        let link = ViewerLink::new_with_prober(
            server.next_viewer.fetch_add(1, Ordering::Relaxed),
            label.to_owned(),
            track_id.clone(),
            sender.clone(),
            tx.clone(),
            cheapest,
            sondagem,
        );
        link.state.lock().apply_subscribe(size, max_layer, audio);
        if let Some(info) = info {
            link.announce_layer(&info, "inicio").await;
        }
        match &track {
            PublishedTrack::Video(p) => p.add_viewer(link.clone()),
            PublishedTrack::Audio(p) => p.add_viewer(link.clone()),
        }
        links.insert(track_id.clone(), (link, track.clone()));
    }

    // Áudio assinado direto: vale até o `cancelar` dele (ou um `audio: false`).
    if matches!(track, PublishedTrack::Audio(_)) {
        if audio == Some(false) {
            audio_direto.remove(&track_id);
        } else {
            audio_direto.insert(track_id);
        }
    }
}

/// Liga ou desliga o áudio desta conexão. Fica ligado enquanto um vídeo assinado pedir áudio
/// (`audio: true`) ou a trilha de áudio estiver assinada direto; sem ninguém, sai (antes ele
/// continuava chegando depois de fechar todas as telas).
fn ajustar_audio(
    server: &MediaServer,
    sender: &Arc<MediaSender>,
    tx: &ControlTx,
    label: &str,
    links: &mut HashMap<String, (Arc<ViewerLink>, PublishedTrack)>,
    audio_direto: &HashSet<String>,
    prober: &zap_net::probe::Prober,
) {
    let quer = !audio_direto.is_empty()
        || links
            .values()
            .any(|(l, t)| matches!(t, PublishedTrack::Video(_)) && l.state.lock().audio);
    let tem = links.values().any(|(_, t)| matches!(t, PublishedTrack::Audio(_)));
    if quer && !tem {
        let candidata = {
            let tracks = server.tracks.lock();
            tracks
                .iter()
                .filter(|(_, t)| matches!(t, PublishedTrack::Audio(_)))
                .min_by(|a, b| a.0.cmp(b.0))
                .map(|(id, t)| (id.clone(), t.clone()))
        };
        if let Some((id, t)) = candidata {
            let link = novo_link(server, sender, tx, label, &id, &t, 0);
            link.state.lock().audio = true;
            links.insert(id, (link, t));
        }
    } else if !quer && tem {
        let ids: Vec<String> = links
            .iter()
            .filter(|(_, (_, t))| matches!(t, PublishedTrack::Audio(_)))
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some((link, t)) = links.remove(&id) {
                remove_link(&link, &t, prober);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    use parking_lot::Mutex;
    use tokio::sync::mpsc;
    use zap_media::{EncoderChoiceSpec, VideoPipelineConfig, VideoSourceSpec};
    use zap_net::handshake::{accept_handshake_with, connect_handshake, Hello, ServerAuth};
    use zap_net::media_rx::{spawn_receiver, MediaEvent};
    use zap_net::report::ReportBuilder;
    use zap_net::tls::{self, Identity};
    use zap_net::{endpoint, Clock};
    use zap_proto::{Access, Room, RoomMode};
    use zap_rate::ladder::{screen_ladder, Preset};

    use crate::publish::{AudioPublisher, AudioSource, MediaAuth, VideoPublisher};
    use crate::ticket::{make_ticket, verify_ticket};

    const SEGREDO: &str = "c2VncmVkby1kZS10ZXN0ZQ==";

    fn sala() -> Room {
        Room {
            id: "r1".into(),
            code: None,
            name: "Sala".into(),
            owner_id: "alice".into(),
            access: Access::Open,
            mode: RoomMode::Local,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    struct Cenario {
        server: Arc<MediaServer>,
        video: Arc<VideoPublisher>,
        audio: Arc<AudioPublisher>,
        cli: quinn::Connection,
        tx: ControlTx,
        rx: ControlRx,
        eventos: mpsc::Receiver<MediaEvent>,
        _cli_ep: quinn::Endpoint,
    }

    /// Monta o `MediaServer` na mão (sem `start`), com laço de aceite próprio, e conecta um cliente cru.
    async fn cenario() -> Cenario {
        let id = Arc::new(Identity::generate("zap").unwrap());
        let (endpoint, port) = endpoint::bind_server(&id, 0).await.unwrap();
        let clock = Clock::new();
        let server = Arc::new(MediaServer {
            endpoint: endpoint.clone(),
            port,
            fingerprint: id.fingerprint(),
            tracks: Mutex::new(HashMap::new()),
            auth: Mutex::new(Some(MediaAuth {
                room: sala(),
                secret: SEGREDO.into(),
                members: Arc::new(|| vec!["bob".to_string()]),
            })),
            clock: clock.clone(),
            next_viewer: AtomicU64::new(1),
            identity: id.clone(),
            rt: tokio::runtime::Handle::current(),
        });

        let video = VideoPublisher::start(
            VideoPipelineConfig {
                track: 1,
                source: VideoSourceSpec::Synthetic {
                    width: 1280,
                    height: 720,
                },
                ladder: screen_ladder(Preset::P720p30, 1280, 720),
                encoder: EncoderChoiceSpec::Auto,
            },
            tokio::runtime::Handle::current(),
            clock.clone(),
        )
        .unwrap();
        let audio = AudioPublisher::start(2, AudioSource::Synthetic).unwrap();
        server
            .tracks
            .lock()
            .insert("v1".into(), PublishedTrack::Video(video.clone()));
        server
            .tracks
            .lock()
            .insert("a1".into(), PublishedTrack::Audio(audio.clone()));

        let s = server.clone();
        tokio::spawn(async move {
            while let Some(inc) = endpoint.accept().await {
                let s = s.clone();
                tokio::spawn(async move {
                    let Ok(conn) = inc.await else { return };
                    let s2 = s.clone();
                    let auth = ServerAuth::Ticket(Box::new(move |t| {
                        verify_ticket(SEGREDO, &["bob".to_string()], t) && s2.auth.lock().is_some()
                    }));
                    let Ok((tx, rx, hello)) = accept_handshake_with(&conn, &auth, &sala(), "alice".into(), None).await
                    else {
                        return;
                    };
                    serve_viewer(s, conn, tx, rx, hello.install_id).await;
                });
            }
        });

        let cli_ep = endpoint::client_endpoint().unwrap();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let cli = cli_ep
            .connect_with(tls::client_config(Some(&id.fingerprint())).unwrap(), addr, "zap")
            .unwrap()
            .await
            .unwrap();
        let (tx, rx, _w) = connect_handshake(
            &cli,
            Hello {
                app_version: "1.0".into(),
                install_id: "inst-bob".into(),
                name: "Bob".into(),
                color: "#fff".into(),
                ticket: Some(make_ticket(SEGREDO, "bob")),
                room_id: None,
                preview: None,
            },
            None,
        )
        .await
        .unwrap();
        let (etx, eventos) = mpsc::channel(256);
        spawn_receiver(cli.clone(), clock, etx, Arc::new(Mutex::new(ReportBuilder::new(false))));
        Cenario {
            server,
            video,
            audio,
            cli,
            tx,
            rx,
            eventos,
            _cli_ep: cli_ep,
        }
    }

    fn assinar(track: &str, size: TileSize, audio: Option<bool>) -> ControlMsg {
        ControlMsg::Subscribe {
            track_id: track.into(),
            size,
            max_layer: None,
            audio,
        }
    }

    fn encerra(c: &Cenario) {
        c.video.stop();
        c.audio.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn assinar_trilha_inexistente_erro() {
        let mut c = cenario().await;
        c.tx.send(&assinar("nao_existe", TileSize::Main, None)).await.unwrap();
        let m = tokio::time::timeout(Duration::from_secs(3), c.rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match m {
            ControlMsg::Error { code, .. } => assert_eq!(code, "trilha_inexistente"),
            outra => panic!("esperava erro, veio {outra:?}"),
        }
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn assinar_video_recebe_camada_atual_inicio_e_quadros() {
        let mut c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(false))).await.unwrap();
        let m = tokio::time::timeout(Duration::from_secs(3), c.rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match m {
            ControlMsg::LayerCurrent { track_id, reason, .. } => {
                assert_eq!(track_id, "v1");
                assert_eq!(reason, "inicio");
            }
            outra => panic!("esperava camada.atual, veio {outra:?}"),
        }
        let mut seqs: Vec<u32> = Vec::new();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while seqs.len() < 3 && tokio::time::Instant::now() < fim {
            if let Ok(Some(MediaEvent::Video { header, .. })) =
                tokio::time::timeout(Duration::from_millis(500), c.eventos.recv()).await
            {
                seqs.push(header.seq);
            }
        }
        assert!(seqs.len() >= 3, "poucos quadros: {seqs:?}");
        assert!(seqs.windows(2).all(|w| w[0] < w[1]), "seq não cresce: {seqs:?}");
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn assinar_com_audio_recebe_datagramas() {
        let mut c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(true))).await.unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut audio = 0;
        while audio < 3 && tokio::time::Instant::now() < fim {
            if let Ok(Some(MediaEvent::Audio { .. })) =
                tokio::time::timeout(Duration::from_millis(500), c.eventos.recv()).await
            {
                audio += 1;
            }
        }
        assert!(audio >= 3, "poucos datagramas de áudio: {audio}");
        encerra(&c);
    }

    /// Conta datagramas de áudio durante `janela`.
    async fn conta_audio(c: &mut Cenario, janela: Duration) -> usize {
        let fim = tokio::time::Instant::now() + janela;
        let mut n = 0;
        while tokio::time::Instant::now() < fim {
            if let Ok(Some(MediaEvent::Audio { .. })) =
                tokio::time::timeout(Duration::from_millis(100), c.eventos.recv()).await
            {
                n += 1;
            }
        }
        n
    }

    /// Quem compartilha só o som não tem vídeo: assinar a trilha de áudio direto recebe o som.
    #[tokio::test(flavor = "multi_thread")]
    async fn assinar_so_o_audio_recebe_datagramas() {
        let mut c = cenario().await;
        c.tx.send(&assinar("a1", TileSize::Thumbnail, Some(true)))
            .await
            .unwrap();
        assert!(
            conta_audio(&mut c, Duration::from_secs(2)).await >= 3,
            "o áudio direto não chegou"
        );
        assert!(c.video.shared.viewers.lock().is_empty(), "não devia assinar vídeo");
        encerra(&c);
    }

    /// Cancelar o único vídeo que pedia áudio desliga o áudio da conexão.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancelar_o_video_desliga_o_audio() {
        let mut c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(true))).await.unwrap();
        assert!(conta_audio(&mut c, Duration::from_secs(2)).await >= 3);
        c.tx.send(&ControlMsg::Unsubscribe { track_id: "v1".into() })
            .await
            .unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while !c.audio.viewers.lock().is_empty() && tokio::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(c.audio.viewers.lock().is_empty(), "o áudio continuou ligado");
        encerra(&c);
    }

    /// Cancelar o áudio direto não corta o áudio que um vídeo assinado ainda pede.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancelar_audio_direto_mantem_o_do_video() {
        let mut c = cenario().await;
        c.tx.send(&assinar("a1", TileSize::Thumbnail, Some(true)))
            .await
            .unwrap();
        c.tx.send(&assinar("v1", TileSize::Main, Some(true))).await.unwrap();
        c.tx.send(&ControlMsg::Unsubscribe { track_id: "a1".into() })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        while c.eventos.try_recv().is_ok() {}
        assert!(
            conta_audio(&mut c, Duration::from_secs(2)).await >= 3,
            "o vídeo ficou sem áudio"
        );
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn miniatura_so_recebe_camada_2() {
        let mut c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Thumbnail, Some(false)))
            .await
            .unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut n = 0;
        while n < 5 && tokio::time::Instant::now() < fim {
            if let Ok(Some(MediaEvent::Video { header, .. })) =
                tokio::time::timeout(Duration::from_millis(500), c.eventos.recv()).await
            {
                assert_eq!(header.layer, 2);
                n += 1;
            }
        }
        assert!(n >= 5, "poucos quadros: {n}");
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancelar_para_de_receber() {
        let mut c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(false))).await.unwrap();
        let m = tokio::time::timeout(Duration::from_secs(3), c.eventos.recv()).await;
        assert!(matches!(m, Ok(Some(MediaEvent::Video { .. }))));
        c.tx.send(&ControlMsg::Unsubscribe { track_id: "v1".into() })
            .await
            .unwrap();
        // Dá tempo de o remove chegar e dos quadros em voo esgotarem.
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while !c.video.shared.viewers.lock().is_empty() && tokio::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(c.video.shared.viewers.lock().len(), 0);
        tokio::time::sleep(Duration::from_millis(500)).await;
        while c.eventos.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(c.eventos.try_recv().is_err(), "ainda chegam quadros");
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn link_de_video_tem_prober_e_audio_nao() {
        let c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(true))).await.unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while (c.video.shared.viewers.lock().is_empty() || c.audio.viewers.lock().is_empty())
            && tokio::time::Instant::now() < fim
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let v = c.video.shared.viewers.lock()[0].clone();
        let a = c.audio.viewers.lock()[0].clone();
        assert!(v.prober.is_some());
        assert!(a.prober.is_none());
        // Segundo vídeo da mesma conexão compartilha o mesmo Arc.
        let video2 = VideoPublisher::start(
            VideoPipelineConfig {
                track: 3,
                source: VideoSourceSpec::Synthetic {
                    width: 1280,
                    height: 720,
                },
                ladder: screen_ladder(Preset::P720p30, 1280, 720),
                encoder: EncoderChoiceSpec::Auto,
            },
            tokio::runtime::Handle::current(),
            c.server.clock.clone(),
        )
        .unwrap();
        c.server
            .tracks
            .lock()
            .insert("v2".into(), PublishedTrack::Video(video2.clone()));
        c.tx.send(&assinar("v2", TileSize::Main, Some(false))).await.unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while video2.shared.viewers.lock().is_empty() && tokio::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let v2 = video2.shared.viewers.lock()[0].clone();
        assert!(Arc::ptr_eq(v.prober.as_ref().unwrap(), v2.prober.as_ref().unwrap()));
        video2.stop();
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relatorio_do_espectador_alimenta_o_prober() {
        use zap_rate::probe::ProbeNeed;

        let c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(false))).await.unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while c.video.shared.viewers.lock().is_empty() && tokio::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let link = c.video.shared.viewers.lock()[0].clone();
        let prober = link.prober.clone().unwrap();

        // Planejador com necessidade e uma rajada conhecida (mesmo roteiro de zap-net: poll após 1,1 s).
        prober.set_need(
            1,
            Some(ProbeNeed {
                next_kbps: 2500,
                reserve_kbps: 0,
            }),
        );
        assert!(prober.poll(0, 400).is_none());
        let rajada = prober.poll(1_100_000, 400).unwrap();

        // 60 sondas de 1200 B a cada 3 ms: 1200 * 8 / 3 ms = 3200 kbps no grupo.
        let sondas = (0..60)
            .map(|i| (rajada.cluster, i, 0u32, 1_000_000 + i as u64 * 3000, 1200u32))
            .collect();
        let com_sondas = zap_proto::Report {
            arrivals: vec![],
            probes: sondas,
            lost: vec![],
            loss: 0.0,
            bandwidth_kbps: Some(2000),
            state: None,
            buffer_ms: None,
        };
        // Relatório sem sondas fecha o grupo aberto (`finish`).
        let sem_sondas = zap_proto::Report {
            arrivals: vec![],
            probes: vec![],
            lost: vec![],
            loss: 0.0,
            bandwidth_kbps: None,
            state: None,
            buffer_ms: None,
        };
        c.tx.send(&ControlMsg::Report(com_sondas)).await.unwrap();
        c.tx.send(&ControlMsg::Report(sem_sondas)).await.unwrap();

        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut medida = None;
        while medida.is_none() && tokio::time::Instant::now() < fim {
            medida = prober.last_measure(c.server.clock.now_us());
            if medida.is_none() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        let (kbps, _) = medida.expect("o prober não recebeu o relatório");
        assert!((2880..=3520).contains(&kbps), "kbps = {kbps}");
        // O `apply_report` continua valendo no mesmo relatório.
        assert_eq!(link.state.lock().bandwidth_kbps, 2000);
        encerra(&c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fechar_conexao_remove_links() {
        let c = cenario().await;
        c.tx.send(&assinar("v1", TileSize::Main, Some(true))).await.unwrap();
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while c.video.stats().viewers_per_layer.iter().sum::<u32>() == 0 && tokio::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(c.video.stats().viewers_per_layer.iter().sum::<u32>() > 0);
        c.cli.close(0u32.into(), b"fim");
        let fim = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < fim
            && (c.video.stats().viewers_per_layer.iter().sum::<u32>() > 0 || !c.audio.viewers.lock().is_empty())
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(c.video.stats().viewers_per_layer.iter().sum::<u32>(), 0);
        assert!(c.audio.viewers.lock().is_empty());
        let _ = &c.server;
        encerra(&c);
    }
}
