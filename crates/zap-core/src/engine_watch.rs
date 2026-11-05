//! `impl Core`: assistir trilhas.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use zap_proto::TrackKind;

use crate::engine::Core;
use crate::online::direct;
use crate::room::ActiveRoom;
use crate::subscribe::{Subscription, WatchSpec};
use crate::types::*;
use crate::CoreError;

/// `watch` em andamento num tile: a geração, a trilha e o último tamanho/teto pedido enquanto
/// conectava.
struct Pending {
    generation: u64,
    track_id: String,
    /// Novo `watch` da mesma trilha durante a conexão: aplicado quando ela terminar.
    update: Option<(TileSize, Option<u8>)>,
}

/// `watch` em andamento por `(Shared, tile)`. Evita a corrida entre a conexão assíncrona e um
/// `unwatch` (ou novo `watch`) no mesmo tile: cada `watch` de outra trilha grava uma geração nova;
/// `unwatch` apaga a entrada. Ao terminar de conectar, a tarefa só publica a assinatura em
/// `shared.subs` se a geração ainda for a dela (conferido sob este mesmo lock, e `unwatch` apaga a
/// entrada antes de mexer em `subs`); senão a assinatura é encerrada. Um `watch` da MESMA trilha
/// durante a conexão só guarda o tamanho novo: duas assinaturas da mesma trilha na mesma conexão
/// brigariam (o `cancelar` de uma derruba a outra no publicador). A chave usa o endereço do
/// `Shared` para não misturar Cores diferentes (ex.: testes); a entrada some ao fim de cada tarefa.
fn generations() -> &'static Mutex<HashMap<(usize, TileId), Pending>> {
    static G: OnceLock<Mutex<HashMap<(usize, TileId), Pending>>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Contador global para gerações únicas.
fn next_generation() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

impl Core {
    /// Acha a trilha no modelo da sala (`TrackNotFound`); trilha própria → `Invalid`;
    /// `tile` já em uso → substitui (para a anterior); acha a trilha de áudio do mesmo dono;
    /// `Subscription::start` numa tarefa do runtime (função síncrona: dispara e devolve `Ok`;
    /// falha de conexão vira `push_error` + remove o tile). Se já assistindo a mesma trilha no
    /// mesmo tile, só `Subscription::update` (tamanho/teto).
    pub fn watch(&self, track_id: &str, tile: TileId, size: TileSize, max_layer: Option<u8>) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        let room = shared.current_room().ok_or(CoreError::NoRoom)?;
        let (track, audio_track) = {
            let model = room.model();
            let model = model.lock();
            let track = model.track_by_id(track_id).cloned().ok_or(CoreError::TrackNotFound)?;
            let so_audio = track.kind == TrackKind::Audio;
            if track.participant_id == room.my_id() {
                if so_audio {
                    return Err(CoreError::Invalid("não dá para ouvir o próprio som".into()));
                }
                // Minha trilha: prévia local, sem rede.
                drop(model);
                self.watch_local(track.id, tile, size);
                return Ok(());
            }
            // Trilha de áudio sozinha (a pessoa compartilha só o som): assinatura só de áudio.
            // Vídeo: o áudio do mesmo participante vem junto.
            let audio = if so_audio {
                None
            } else {
                model
                    .tracks
                    .iter()
                    .find(|t| t.participant_id == track.participant_id && t.kind == TrackKind::Audio)
                    .cloned()
            };
            (track, audio)
        };

        // Online sem ponte: só rota direta. O pedido fica pendente e o tile aparece quando a
        // assinatura nascer (não mexe no UiState aqui).
        if let ActiveRoom::Online(r) = &*room {
            if !r.bridge_enabled(&shared) {
                let antiga = shared.subs.lock().remove(&tile);
                if let Some(old) = antiga {
                    let sh = shared.clone();
                    shared.rt.spawn(async move { old.stop(&sh).await });
                }
                // Mesmo padrão do caminho normal: um `unwatch` ou `watch` posterior invalida esta geração.
                let chave = (Arc::as_ptr(&shared) as usize, tile);
                generations().lock().insert(
                    chave,
                    Pending {
                        generation: next_generation(),
                        track_id: track.id.clone(),
                        update: None,
                    },
                );
                let spec = WatchSpec {
                    track,
                    audio_track,
                    tile,
                    size,
                    max_layer,
                };
                let r = r.clone();
                let sh = shared.clone();
                shared.rt.spawn(async move {
                    if let Err(e) = direct::watch_pending(&r, spec).await {
                        direct::unwatch_pending(&r, tile).await;
                        sh.state.lock().push_error(e.to_string());
                        sh.notifier.touch();
                    }
                });
                return Ok(());
            }
        }

        // Mesma trilha no tile: só atualiza; outra trilha: tira a antiga.
        let antiga = {
            let mut subs = shared.subs.lock();
            match subs.get(&tile) {
                Some(sub) if sub.track.id == track.id => {
                    let sub = sub.clone();
                    drop(subs);
                    shared.rt.spawn(async move {
                        let _ = sub.update(size, max_layer).await;
                    });
                    return Ok(());
                }
                Some(_) => subs.remove(&tile),
                None => None,
            }
        };
        if let Some(old) = antiga {
            let sh = shared.clone();
            shared.rt.spawn(async move { old.stop(&sh).await });
        }

        let chave = (Arc::as_ptr(&shared) as usize, tile);
        let geracao = next_generation();
        {
            let mut gens = generations().lock();
            // A mesma trilha ainda conectando neste tile: só guarda o tamanho novo.
            if let Some(p) = gens.get_mut(&chave) {
                if p.track_id == track.id {
                    p.update = Some((size, max_layer));
                    return Ok(());
                }
            }
            gens.insert(
                chave,
                Pending {
                    generation: geracao,
                    track_id: track.id.clone(),
                    update: None,
                },
            );
        }
        let spec = WatchSpec {
            track,
            audio_track,
            tile,
            size,
            max_layer,
        };
        let sh = shared.clone();
        shared.rt.spawn(async move {
            let resultado = Subscription::start(&sh, spec).await;
            // Decide sob o lock das gerações; `unwatch` apaga a entrada antes de tocar em `subs`.
            let (vigente, sobra, atualizar) = {
                let mut gens = generations().lock();
                let vigente = gens.get(&chave).is_some_and(|p| p.generation == geracao);
                let atualizar = if vigente {
                    gens.remove(&chave).and_then(|p| p.update)
                } else {
                    None
                };
                // Outro `watch` da mesma trilha já assumiu este tile?
                let mesma_segue = |track_id: &str| {
                    gens.get(&chave).is_some_and(|p| p.track_id == track_id)
                        || sh.subs.lock().get(&tile).is_some_and(|s| s.track.id == track_id)
                };
                match resultado {
                    Ok(sub) => {
                        let sub = Arc::new(sub);
                        if vigente {
                            sh.subs.lock().insert(tile, sub.clone());
                            (vigente, Ok(atualizar.map(|_| (sub, false))), atualizar)
                        } else {
                            let segue = mesma_segue(&sub.track.id);
                            (vigente, Ok(Some((sub, !segue))), None)
                        }
                    }
                    Err(e) => (vigente, Err(e), None),
                }
            };
            match sobra {
                // Pediram outro tamanho enquanto conectava.
                Ok(Some((sub, false))) if vigente => {
                    if let Some((size, max_layer)) = atualizar {
                        let _ = sub.update(size, max_layer).await;
                    }
                }
                // Cancelado enquanto conectava: não deixa assinatura órfã. Se a mesma trilha
                // continua assistida neste tile por outra assinatura, só solta o lado de cá (o
                // `cancelar` derrubaria a outra no publicador) e devolve o tratador de vídeo a ela.
                Ok(Some((sub, cancelar))) => {
                    if cancelar {
                        sub.stop(&sh).await;
                    } else {
                        sub.stop_local(&sh);
                        let atual = sh.subs.lock().get(&tile).cloned();
                        if let Some(atual) = atual.filter(|a| a.track.id == sub.track.id) {
                            if let Some(v) = &atual.video {
                                atual.conn.router.register(atual.track.number, v.clone());
                            }
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    // Se outro watch/unwatch assumiu o tile, ele cuida do estado.
                    if vigente {
                        sh.state.lock().push_error(e.to_string());
                        sh.state.lock().remove_tile(tile);
                        sh.notifier.touch();
                    }
                }
            }
        });
        Ok(())
    }
    /// Para a assinatura do tile, remove de `UiState.tiles`, `touch()`.
    /// Prévia local da minha trilha `track_id` no `tile`: o publicador dela entrega a camada mais
    /// leve direto à interface (`VideoPublisher::set_local_preview`). Trilha que não é mais minha
    /// (parei de transmitir): nada.
    fn watch_local(&self, track_id: String, tile: TileId, size: TileSize) {
        let sh = self.shared.clone();
        self.shared.rt.spawn(async move {
            let publicador = {
                let sessao = sh.share.lock().await;
                let achado = [sessao.screen.as_ref(), sessao.camera.as_ref()]
                    .into_iter()
                    .flatten()
                    .find(|v| v.track.id == track_id)
                    .map(|v| v.publisher.clone());
                achado
            };
            if let Some(p) = publicador {
                p.set_local_preview(tile, sh.playout.clone(), size);
            }
        });
    }

    /// Desliga a prévia local que estiver no `tile` (tela ou câmera).
    fn unwatch_local(&self, tile: TileId) {
        let sh = self.shared.clone();
        self.shared.rt.spawn(async move {
            let sessao = sh.share.lock().await;
            for v in [sessao.screen.as_ref(), sessao.camera.as_ref()].into_iter().flatten() {
                v.publisher.clear_local_preview(tile);
            }
            drop(sessao);
            sh.playout.remove_tile(tile);
        });
    }

    pub fn unwatch(&self, tile: TileId) {
        self.unwatch_local(tile);
        let shared = self.shared.clone();
        // Primeiro invalida o watch em andamento, depois tira de `subs` (ver `generations`).
        generations().lock().remove(&(Arc::as_ptr(&shared) as usize, tile));
        let sub = shared.subs.lock().remove(&tile);
        if let Some(sub) = sub {
            let sh = shared.clone();
            shared.rt.spawn(async move { sub.stop(&sh).await });
        }
        shared.state.lock().remove_tile(tile);
        shared.notifier.touch();
        // Sala Online: tira também o pedido pendente (rota direta ainda não pronta).
        if let Some(room) = shared.current_room() {
            if let ActiveRoom::Online(r) = &*room {
                let r = r.clone();
                shared.rt.spawn(async move {
                    direct::unwatch_pending(&r, tile).await;
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;
    use std::path::PathBuf;
    use std::time::Duration;
    use zap_proto::{MediaAddress, RoomMode, Track};

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("zapcore-watch-{}", uuid::Uuid::new_v4()))
    }
    fn novo() -> (Core, PathBuf) {
        let d = dir();
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        (Core::start_with(d.clone(), TestSink::new(), opts).expect("core"), d)
    }
    async fn encerrar(core: Core, d: PathBuf) {
        let _ = tokio::task::spawn_blocking(move || drop(core)).await;
        let _ = std::fs::remove_dir_all(d);
    }
    async fn sala(core: &Core) {
        core.create_room(crate::types::CreateRoom {
            name: "S".into(),
            mode: RoomMode::Local,
            password: None,
        })
        .await
        .expect("sala");
    }
    fn trilha(id: &str, dono: &str, numero: u16, kind: TrackKind) -> Track {
        Track {
            id: id.into(),
            participant_id: dono.into(),
            number: numero,
            kind,
            title: "t".into(),
            layers: vec![],
            audio: None,
            address: Some(MediaAddress {
                addresses: vec!["127.0.0.1".into()],
                port: 1,
                fingerprint: "00".into(),
            }),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_sem_sala_no_room() {
        let (core, d) = novo();
        assert!(matches!(
            core.watch("x", 1, TileSize::Main, None),
            Err(CoreError::NoRoom)
        ));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_trilha_inexistente() {
        let (core, d) = novo();
        sala(&core).await;
        assert!(matches!(
            core.watch("nao_existe", 1, TileSize::Main, None),
            Err(CoreError::TrackNotFound)
        ));
        encerrar(core, d).await;
    }

    /// Minha trilha vira prévia local: a camada mais leve chega ao tile, começando num keyframe,
    /// sem nenhum espectador na rede; `unwatch` corta a entrega.
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_propria_trilha_vira_previa_local() {
        let d = dir();
        let sink = TestSink::new();
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        let core = Core::start_with(d.clone(), sink.clone(), opts).expect("core");
        sala(&core).await;
        core.start_share(ShareRequest {
            source: "synthetic:640x360".into(),
            audio: AudioChoice::Keyword("none".into()),
            preset: Preset::P720p30,
            encoder: "auto".into(),
        })
        .await
        .expect("compartilhar");
        let trilha_id = {
            let sessao = core.shared.share.lock().await;
            sessao.screen.as_ref().expect("tela publicada").track.id.clone()
        };
        const TILE: TileId = 9000;
        core.watch(&trilha_id, TILE, TileSize::Thumbnail, Some(2))
            .expect("prévia");
        let chegou = sink
            .wait_for(Duration::from_secs(5), |s| s.video_count(TILE) >= 3)
            .await;
        assert!(chegou, "a prévia local não entregou quadros");
        {
            let videos = sink.videos.lock();
            let primeiro = videos.iter().find(|(_, t, _)| *t == TILE).expect("quadro");
            assert!(primeiro.2.keyframe, "a prévia tem que começar num keyframe");
        }
        core.unwatch(TILE);
        tokio::time::sleep(Duration::from_millis(400)).await;
        let antes = sink.video_count(TILE);
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(sink.video_count(TILE), antes, "unwatch devia parar a prévia");
        core.stop_share().await;
        encerrar(core, d).await;
    }

    /// O som de outra pessoa pode ser assinado sozinho; o meu, não.
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_audio_de_outro_aceita_e_o_meu_nao() {
        let (core, d) = novo();
        sala(&core).await;
        let room = core.shared.current_room().expect("sala");
        let eu = room.my_id();
        room.model()
            .lock()
            .upsert_track(trilha("t_a", "outro", 2, TrackKind::Audio));
        room.model()
            .lock()
            .upsert_track(trilha("t_meu", &eu, 3, TrackKind::Audio));
        assert!(core.watch("t_a", 1, TileSize::Thumbnail, None).is_ok());
        assert!(matches!(
            core.watch("t_meu", 2, TileSize::Thumbnail, None),
            Err(CoreError::Invalid(_))
        ));
        core.unwatch(1);
        encerrar(core, d).await;
    }

    /// Novo `watch` da mesma trilha enquanto a primeira ainda conecta: não abre outra assinatura,
    /// só guarda o tamanho para aplicar quando conectar.
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_repetido_durante_conexao_so_guarda_o_tamanho() {
        let (core, d) = novo();
        sala(&core).await;
        let room = core.shared.current_room().expect("sala");
        room.model()
            .lock()
            .upsert_track(trilha("t_v", "outro", 2, TrackKind::Screen));
        core.watch("t_v", 3, TileSize::Thumbnail, Some(2)).expect("1º");
        let chave = (Arc::as_ptr(&core.shared) as usize, 3);
        let geracao = generations().lock().get(&chave).map(|p| p.generation);
        core.watch("t_v", 3, TileSize::Main, None).expect("2º");
        // O endereço da trilha não responde: a primeira ainda está conectando.
        let (g, upd) = {
            let gens = generations().lock();
            let p = gens.get(&chave).expect("ainda conectando");
            (p.generation, p.update)
        };
        assert_eq!(Some(g), geracao, "não pode ter aberto outra assinatura");
        assert_eq!(upd, Some((TileSize::Main, None)));
        core.unwatch(3);
        assert!(generations().lock().get(&chave).is_none());
        encerrar(core, d).await;
    }

    /// Ponta a ponta numa sala Local: quem só assina a trilha de áudio recebe o som (sem vídeo).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn assinatura_so_de_audio_recebe_o_som() {
        let (dono, d1) = novo();
        sala(&dono).await;
        let (b, d2) = novo();
        let porta = dono.control_port().expect("porta");
        let fp = dono.control_fingerprint().expect("impressão");
        b.join_room_pinned(std::net::SocketAddr::from(([127, 0, 0, 1], porta)), fp, None)
            .await
            .expect("entra");
        // Tela sintética com som: o som também é sintético (não depende da placa de som).
        dono.start_share(ShareRequest {
            source: "synthetic:640x360".into(),
            audio: AudioChoice::Keyword("all".into()),
            preset: Preset::P720p30,
            encoder: "auto".into(),
        })
        .await
        .expect("compartilhar");
        let mut achado = None;
        for _ in 0..100 {
            let sala_b = b.shared.current_room().expect("sala de B");
            achado = sala_b
                .model()
                .lock()
                .tracks
                .iter()
                .find(|t| t.kind == TrackKind::Audio)
                .map(|t| (t.id.clone(), t.participant_id.clone()));
            if achado.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let (audio_id, dono_id) = achado.expect("B não viu a trilha de áudio");
        const TILE: TileId = 5;
        b.watch(&audio_id, TILE, TileSize::Thumbnail, None).expect("só o som");
        let recebidos = || {
            b.shared
                .audio_rx
                .by_participant
                .lock()
                .get(&dono_id)
                .and_then(|rx| *rx.last_seq.lock())
                .unwrap_or(0)
        };
        let mut ok = false;
        for _ in 0..100 {
            if recebidos() >= 10 {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(ok, "o som não chegou: {}", recebidos());
        // Só áudio: não aparece como tile de vídeo.
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(b.shared.state.lock().snapshot().tiles.iter().all(|t| t.tile != TILE));
        b.unwatch(TILE);
        dono.stop_share().await;
        encerrar(b, d2).await;
        encerrar(dono, d1).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unwatch_de_tile_inexistente_nao_panica() {
        let (core, d) = novo();
        core.unwatch(42);
        sala(&core).await;
        core.unwatch(42);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unwatch_durante_conexao_nao_deixa_assinatura_orfa() {
        let (core, d) = novo();
        sala(&core).await;
        let room = core.shared.current_room().expect("sala");
        room.model()
            .lock()
            .upsert_track(trilha("t_v", "outro", 3, TrackKind::Screen));
        core.watch("t_v", 7, TileSize::Main, None).expect("watch");
        core.unwatch(7);
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(core.shared.subs.lock().is_empty());
        assert!(core.shared.state.lock().tiles().is_empty());
        encerrar(core, d).await;
    }

    /// Monta uma sala Online sem ponte (nem WebSocket) e a instala como sala atual.
    fn sala_online(core: &Core) -> Arc<crate::online::OnlineRoom> {
        use zap_net::online::JoinMedia;
        use zap_proto::{Access, Participant, Role, Room, RoomCode};
        let room = Room {
            id: "s_1".into(),
            code: None,
            name: "O".into(),
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
        let r = crate::online::OnlineRoom::new(
            &core.shared,
            crate::room::RoomModel::new(room, owner),
            "p1".into(),
            RoomCode::from_entropy([1, 2, 3, 4]),
            None,
            JoinMedia {
                bridge: None,
                direct_allowed: true,
            },
            String::new(),
        );
        *core.shared.room.lock() = Some(Arc::new(ActiveRoom::Online(r.clone())));
        r
    }

    // Limitação: sem WebSocket em unitário, `watch_pending` falha e o tile sai de `pending` logo
    // depois; por isso confere-se o caminho pendente (sem erro síncrono, `subs` vazio, UiState
    // intocido, erro de conexão publicado), e não a permanência do tile em `pending`.
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_sem_ponte_fica_pendente() {
        let (core, d) = novo();
        let r = sala_online(&core);
        r.model
            .lock()
            .upsert_track(trilha("t_v", "outro", 3, TrackKind::Screen));
        core.watch("t_v", 7, TileSize::Main, None).expect("watch");
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(core.shared.subs.lock().is_empty());
        assert!(core.shared.state.lock().tiles().is_empty());
        // Sem WebSocket o pedido falha e é desfeito.
        assert!(r.direct.pending.lock().is_empty());
        assert!(!core.shared.state.lock().snapshot().errors.is_empty());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unwatch_remove_pendente() {
        let (core, d) = novo();
        let r = sala_online(&core);
        let track = trilha("t_v", "outro", 3, TrackKind::Screen);
        r.direct.pending.lock().insert(
            7,
            WatchSpec {
                track,
                audio_track: None,
                tile: 7,
                size: TileSize::Main,
                max_layer: None,
            },
        );
        core.unwatch(7);
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(r.direct.pending.lock().is_empty());
        encerrar(core, d).await;
    }
}
