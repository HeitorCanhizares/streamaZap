//! Fachada do núcleo: `Core` (API que o Tauri chama), `UiSink` e o estado compartilhado.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc::UnboundedSender;
use zap_net::discovery::Discovery;
use zap_net::tls::Identity;

use crate::audio_out::AudioOutHandle;
use crate::notify::Notifier;
use crate::playout::PlayoutHub;
use crate::publish::{MediaAuth, MediaServer};
use crate::room::{ActiveRoom, RoomCtx, RoomEvent};
use crate::settings::Settings;
use crate::share::ShareSession;
use crate::snapshot::{Me, Snapshot};
use crate::state::UiState;
use crate::subscribe::{AudioRegistry, ConnPool, Subscription};
use crate::types::*;
use crate::CoreError;

/// Saída para a interface. Chamado de threads do núcleo: não pode bloquear.
pub trait UiSink: Send + Sync + 'static {
    /// No máx. 4/s, e na hora em mudança de tela.
    fn state(&self, snapshot: Arc<Snapshot>);
    fn chat(&self, msg: ChatLine);
    /// Já no instante de exibir.
    fn video(&self, tile: TileId, packet: VideoPacket);
}

#[derive(Clone, Debug)]
pub struct CoreOptions {
    /// Sobe a descoberta UDP.
    pub discovery: bool,
    /// `DiscoveryConfig::broadcast` (false nos testes).
    pub broadcast: bool,
    pub discovery_port: u16,
    /// Inclui 127.0.0.1 nos endereços anunciados (testes).
    pub include_loopback: bool,
    /// Cria a saída de áudio (`AudioOutHandle::start`); false = stub.
    pub audio_out: bool,
    /// Destinos unicast extras da descoberta (`DiscoveryConfig.extra_targets`; testes).
    pub discovery_targets: Vec<std::net::SocketAddr>,
    /// Impressão digital (SHA-256 hex) aceita para o certificado da ponte QUIC Online
    /// (servidor com certificado próprio e testes). `None` = raízes web.
    pub dev_cert_fp: Option<String>,
    /// Força o endereço `host:porta` da ponte (proxy de teste); ignora o da resposta de `entrar`.
    pub dev_bridge_addr: Option<String>,
    /// Sobe a sessão com o servidor Online ao abrir (login se houver endereço). Falso nos testes
    /// que não usam o modo Online.
    pub online: bool,
    /// Endereço gravado em `settings.server.address` na primeira execução (sem `settings.json`).
    /// `Core::start` (o app) usa `settings::PRODUCTION_SERVER`; o padrão `None` deixa o endereço
    /// vazio, e assim nenhum teste fala com o servidor de produção.
    pub default_server: Option<String>,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self {
            discovery: true,
            broadcast: true,
            discovery_port: zap_proto::DISCOVERY_PORT,
            include_loopback: false,
            audio_out: true,
            discovery_targets: vec![],
            dev_cert_fp: None,
            dev_bridge_addr: None,
            online: true,
            default_server: None,
        }
    }
}

/// Estado compartilhado por todas as partes do núcleo.
pub struct Shared {
    pub rt: tokio::runtime::Handle,
    pub data_dir: PathBuf,
    pub options: CoreOptions,
    pub settings: RwLock<Settings>,
    pub identity: Arc<Identity>,
    pub ui: Arc<dyn UiSink>,
    pub state: Arc<Mutex<UiState>>,
    pub notifier: Notifier,
    pub clock: zap_net::Clock,
    pub discovery: Mutex<Option<Arc<Discovery>>>,
    pub room: Mutex<Option<Arc<ActiveRoom>>>,
    pub room_events: UnboundedSender<RoomEvent>,
    pub share: tokio::sync::Mutex<ShareSession>,
    pub media_server: tokio::sync::Mutex<Option<Arc<MediaServer>>>,
    pub subs: Mutex<HashMap<TileId, Arc<Subscription>>>,
    pub pool: ConnPool,
    pub playout: Arc<PlayoutHub>,
    pub audio_rx: AudioRegistry,
    pub audio_out: AudioOutHandle,
    pub online: Arc<crate::online::OnlineSession>,
    pub stop: AtomicBool,
    /// Resolução de timer de 1 ms enquanto há sala ativa (ver `set_active_room`).
    #[allow(dead_code)] // lido pelas tarefas core-01/03
    pub(crate) timer: Mutex<Option<zap_sys::platform::TimerResolution>>,
}

impl Shared {
    /// Troca `room` e pega/solta o timer de 1 ms (`Some` guarda um se não houver; `None` solta).
    /// Devolve a sala anterior.
    #[allow(dead_code)] // tarefa core-01 liga os chamadores (core-03)
    pub(crate) fn set_active_room(&self, room: Option<Arc<ActiveRoom>>) -> Option<Arc<ActiveRoom>> {
        let tem_sala = room.is_some();
        // Ordem room -> timer em todo lugar que pegar os dois: a troca e o timer andam juntos.
        let mut atual = self.room.lock();
        let anterior = std::mem::replace(&mut *atual, room);
        let mut timer = self.timer.lock();
        if tem_sala {
            // Já com guarda: não cria outro (o sistema conta os `timeBeginPeriod` por processo).
            if timer.is_none() {
                *timer = Some(zap_sys::platform::TimerResolution::high());
            }
        } else {
            // Solta o guarda (Drop faz o `timeEndPeriod`).
            *timer = None;
        }
        anterior
    }
    /// Sala atual (clone do `Arc`).
    pub fn current_room(&self) -> Option<Arc<ActiveRoom>> {
        self.room.lock().clone()
    }
    /// Valores atuais das configurações para uma sala nova.
    pub fn room_ctx(&self) -> Arc<RoomCtx> {
        let s = self.settings.read();
        Arc::new(RoomCtx {
            identity: self.identity.clone(),
            install_id: s.install_id.clone(),
            name: s.name.clone(),
            color: s.color.clone(),
            port: s.local.port,
            interface: s.local.interface.clone(),
            include_loopback: self.options.include_loopback,
            app_version: env!("CARGO_PKG_VERSION").into(),
            events: self.room_events.clone(),
        })
    }
    /// Cria o `MediaServer` na primeira vez (porta preferida = `settings.local.port + 1`) e o
    /// reaproveita; configura `set_auth` com a sala atual (`room`, `media_secret`, `members`).
    pub async fn ensure_media_server(self: &Arc<Self>) -> Result<Arc<MediaServer>, CoreError> {
        // Trava assíncrona: duas chamadas concorrentes não criam dois servidores.
        let mut guard = self.media_server.lock().await;
        let server = match guard.as_ref() {
            Some(existente) => existente.clone(),
            None => {
                let porta = self.settings.read().local.port.saturating_add(1);
                let novo =
                    MediaServer::start(self.identity.clone(), porta, self.clock.clone(), self.rt.clone()).await?;
                *guard = Some(novo.clone());
                novo
            }
        };
        // Sem sala, nenhum espectador é aceito (`set_auth(None)` recusa o ingresso).
        let auth = match self.current_room() {
            Some(sala) => {
                let model = sala.model();
                let room = model.lock().room.clone();
                let para_membros = model.clone();
                Some(MediaAuth {
                    room,
                    secret: sala.media_secret(),
                    members: Arc::new(move || para_membros.lock().participants.iter().map(|p| p.id.clone()).collect()),
                })
            }
            None => None,
        };
        server.set_auth(auth);
        Ok(server)
    }
    /// Reconstrói `UiState.room` a partir do modelo da sala (ou `None`) e faz `notifier.touch()`.
    pub fn refresh_room_view(&self) {
        // Gera o `RoomView` antes de travar o estado: nunca os dois locks ao mesmo tempo.
        let view = match self.current_room() {
            Some(sala) => {
                let model = sala.model();
                let mut v = model.lock().to_view();
                // No Online o id de participante vem do servidor (não é o `install_id`).
                v.me_id = sala.my_id();
                Some(v)
            }
            None => None,
        };
        self.state.lock().set_room(view);
        self.notifier.touch();
    }
    /// Reconstrói `UiState.me` a partir das configurações.
    pub fn refresh_me(&self) {
        let me = {
            let s = self.settings.read();
            Me {
                name: s.name.clone(),
                color: s.color.clone(),
                install_id: s.install_id.clone(),
            }
        };
        self.state.lock().set_me(me);
        self.notifier.touch();
    }
}

pub struct Core {
    pub(crate) shared: Arc<Shared>,
    /// Runtime próprio do núcleo (2 threads de trabalho); todo `async fn` público salta para ele
    /// com `shared.rt.spawn(..).await`.
    pub(crate) runtime: Option<tokio::runtime::Runtime>,
}

impl Core {
    /// `start_with` com `CoreOptions::default()` e o servidor de produção como endereço da
    /// primeira execução. É o ponto de entrada do app; os testes usam `start_with`.
    pub fn start(data_dir: PathBuf, ui: Arc<dyn UiSink>) -> Result<Self, CoreError> {
        let options = CoreOptions {
            default_server: Some(crate::settings::PRODUCTION_SERVER.into()),
            ..CoreOptions::default()
        };
        Self::start_with(data_dir, ui, options)
    }

    /// Cria o runtime, carrega `settings::load` + identidade, monta `Shared` (UiState com `Me`,
    /// `Notifier::spawn`, `PlayoutHub::start`, `AudioOutHandle` conforme `options`), sobe a
    /// descoberta (`Discovery::start` com `DiscoveryConfig{port, broadcast}` + presença) se
    /// habilitada (falha de descoberta = só log + `push_error`), `engine_room::pump_room_events`
    /// e `engine_tick::run`. Primeiro snapshot já entregue no fim.
    pub fn start_with(data_dir: PathBuf, ui: Arc<dyn UiSink>, options: CoreOptions) -> Result<Self, CoreError> {
        // MTA do COM fixado antes de qualquer thread do núcleo tocar em COM.
        zap_sys::platform::pin_com_mta();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("zap-core")
            .build()?;
        let rt = runtime.handle().clone();
        // Contexto do runtime ativo: `Notifier::spawn` e `PlayoutHub::start` usam timers/spawn.
        let _guard = rt.enter();

        std::fs::create_dir_all(&data_dir)?;
        let mut settings = crate::settings::load(&data_dir);
        // Grava na primeira execução para o `install_id` gerado não mudar na próxima. O servidor
        // padrão só entra aqui: quem apagar o endereço depois continua sem servidor.
        if !data_dir.join("settings.json").exists() {
            if let Some(endereco) = &options.default_server {
                settings.server.address = endereco.clone();
            }
            crate::settings::save(&data_dir, &settings)?;
        }
        let identity = Arc::new(crate::settings::load_or_create_identity(
            &data_dir,
            &settings.install_id,
        )?);

        let state = Arc::new(Mutex::new(UiState::new(Me {
            name: settings.name.clone(),
            color: settings.color.clone(),
            install_id: settings.install_id.clone(),
        })));
        let notifier = Notifier::spawn(&rt, ui.clone(), state.clone());
        let clock = zap_net::Clock::new();
        let playout = Arc::new(PlayoutHub::start(clock.clone(), ui.clone()));
        playout.set_smoothing(settings.video.smoothing_auto, settings.video.smoothing_ms);
        // Fora do Windows `start()` já é um stub sem thread.
        let audio_out = AudioOutHandle::start();
        let audio_rx = AudioRegistry::new(
            audio_out.clone(),
            playout.clone(),
            clock.clone(),
            settings.sound.initial_volume,
        );
        let (room_events, rx) = tokio::sync::mpsc::unbounded_channel();

        let install_id = settings.install_id.clone();
        let nome = settings.name.clone();
        let shared = Arc::new(Shared {
            rt: rt.clone(),
            data_dir,
            options: options.clone(),
            settings: RwLock::new(settings),
            identity,
            ui,
            state,
            notifier,
            clock,
            discovery: Mutex::new(None),
            room: Mutex::new(None),
            room_events,
            share: tokio::sync::Mutex::new(ShareSession::default()),
            media_server: tokio::sync::Mutex::new(None),
            subs: Mutex::new(HashMap::new()),
            pool: ConnPool::default(),
            playout,
            audio_rx,
            audio_out,
            online: crate::online::OnlineSession::new(),
            stop: AtomicBool::new(false),
            timer: Mutex::new(None),
        });

        if options.discovery {
            let cfg = zap_net::discovery::DiscoveryConfig {
                port: options.discovery_port,
                broadcast: options.broadcast,
                extra_targets: options.discovery_targets.clone(),
                ..Default::default()
            };
            match rt.block_on(Discovery::start(cfg, install_id.clone())) {
                Ok(d) => {
                    d.set_presence(crate::discovery_view::presence_announcement(&install_id, &nome, vec![]));
                    *shared.discovery.lock() = Some(Arc::new(d));
                }
                Err(e) => {
                    tracing::warn!("descoberta indisponível: {e}");
                    shared
                        .state
                        .lock()
                        .push_error(format!("Descoberta de salas indisponível: {e}"));
                }
            }
        }

        rt.spawn(crate::engine_room::pump_room_events(shared.clone(), rx));
        rt.spawn(crate::engine_tick::run(shared.clone()));
        // Sessão Online: só sobe com a opção ligada (testes sem o modo Online deixam desligada).
        if options.online {
            shared.online.start(&shared);
        }
        shared.notifier.flush();
        Ok(Core {
            shared,
            runtime: Some(runtime),
        })
    }

    pub fn settings(&self) -> Settings {
        self.shared.settings.read().clone()
    }
    /// Aplica `f`, salva (`settings::save`), atualiza `Me` e o nome/presença da descoberta e
    /// faz `touch()`. Erro de gravação: `push_error`.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let (novo, servidor_mudou) = {
            let mut guard = self.shared.settings.write();
            let antes = (guard.server.address.clone(), guard.server.password.clone());
            f(&mut guard);
            let mudou = antes != (guard.server.address.clone(), guard.server.password.clone());
            (guard.clone(), mudou)
        };
        if let Err(e) = crate::settings::save(&self.shared.data_dir, &novo) {
            self.shared
                .state
                .lock()
                .push_error(format!("Não foi possível salvar as configurações: {e}"));
        }
        // Só endereço e senha do servidor reiniciam a sessão (trocar `use_relay` não).
        if servidor_mudou && self.shared.options.online {
            self.shared.online.restart(&self.shared);
        }
        self.shared.refresh_me();
        let discovery = self.shared.discovery.lock().clone();
        if let Some(d) = discovery {
            d.set_presence(crate::discovery_view::presence_announcement(
                &novo.install_id,
                &novo.name,
                vec![],
            ));
        }
        self.shared.notifier.touch();
    }
    pub fn sources(&self) -> Sources {
        crate::sources::list_sources()
    }
    /// Miniatura RGBA `w`×`h`. No Windows, foto de verdade da tela, janela ou câmera
    /// (`zap_media::snapshot`); fonte sintética (e fora do Windows): o degradê de
    /// `sources::thumbnail`. Bloqueia até a foto chegar (até 1,5 s; câmera até 4 s).
    pub fn thumbnail(&self, src: &SourceRef, w: u32, h: u32) -> Result<Rgba, CoreError> {
        if matches!(src, SourceRef::Synthetic { .. }) || !cfg!(windows) {
            return crate::sources::thumbnail(src, w, h);
        }
        crate::sources::check_thumbnail_size(w, h)?;
        let foto = zap_media::snapshot(&crate::sources::to_video_spec(src), w, h)?;
        Ok(Rgba {
            width: foto.width,
            height: foto.height,
            data: foto.rgba,
        })
    }
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.shared.state.lock().snapshot()
    }
    /// Só para testes: derruba a conexão do membro com o dono **sem** `sala.sair`, como numa queda
    /// de rede. `left` vai para `true` para o laço do membro não tentar reconectar.
    #[doc(hidden)]
    pub fn simulate_drop(&self) {
        if let Some(sala) = self.shared.current_room() {
            if let ActiveRoom::Member(m) = sala.as_ref() {
                m.left.store(true, std::sync::atomic::Ordering::SeqCst);
                let conn = m.conn.lock().take();
                if let Some(conn) = conn {
                    conn.close(0u32.into(), b"caiu");
                }
            }
        }
    }
    /// `audio_rx.set_volume(participant_id, volume, muted)`.
    pub fn set_volume(&self, participant_id: &str, volume: f32, muted: bool) {
        self.shared.audio_rx.set_volume(participant_id, volume, muted);
    }
    /// Guarda em `settings.video` (sem salvar de novo se igual) e `playout.set_smoothing`.
    pub fn set_smoothing(&self, auto: bool, ms: u32) {
        self.shared.playout.set_smoothing(auto, ms);
        let mudou = {
            let s = self.shared.settings.read();
            s.video.smoothing_auto != auto || s.video.smoothing_ms != ms
        };
        if mudou {
            self.update_settings(|s| {
                s.video.smoothing_auto = auto;
                s.video.smoothing_ms = ms;
            });
        }
    }
    /// Para tudo (sala, compartilhamento, assinaturas) e derruba o runtime.
    pub fn shutdown(&mut self) {
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        self.shared.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let shared = self.shared.clone();
        let core_view = Core {
            shared: shared.clone(),
            runtime: None,
        };
        // Thread própria: `block_on` e derrubar um runtime entram em pânico dentro de contexto
        // assíncrono, e o `Drop` pode rodar de qualquer lugar (inclusive de tarefa tokio).
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name("zap-core-shutdown".into())
            .spawn(move || {
                runtime.block_on(async {
                    if shared.current_room().is_some() {
                        core_view.leave_room().await;
                    }
                    // Depois da sala: `leave` ainda precisa do WebSocket e do cliente para avisar o servidor.
                    shared.online.stop();
                    shared.pool.close_all().await;
                    ShareSession::stop(&shared).await;
                    if let Some(ms) = shared.media_server.lock().await.take() {
                        ms.shutdown();
                    }
                });
                runtime.shutdown_timeout(std::time::Duration::from_secs(2));
                let _ = tx.send(());
            });
        match spawned {
            Ok(_) => {
                if std::thread::panicking() {
                    // Durante um pânico não vale travar: a thread termina sozinha.
                    let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
                } else {
                    let _ = rx.recv();
                }
            }
            Err(e) => tracing::warn!("não foi possível criar a thread de encerramento: {e}"),
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TestSink;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("zapcore-eng-{}", uuid::Uuid::new_v4()))
    }
    fn opts() -> CoreOptions {
        CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        }
    }

    #[test]
    fn start_cria_arquivos_e_primeiro_snapshot() {
        let d = dir();
        let sink = TestSink::new();
        let core = Core::start_with(d.clone(), sink.clone(), opts()).unwrap();
        assert!(!sink.snapshots.lock().is_empty());
        assert!(!sink.last_snapshot().unwrap().me.install_id.is_empty());
        assert!(d.join("settings.json").exists());
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn servidor_padrao_so_na_primeira_execucao() {
        let d = dir();
        // `online: false`: o endereço é só gravado, ninguém tenta logar nele.
        let com_padrao = CoreOptions {
            online: false,
            default_server: Some("https://servidor.invalido:7443".into()),
            ..opts()
        };
        let core = Core::start_with(d.clone(), TestSink::new(), com_padrao.clone()).unwrap();
        assert_eq!(core.settings().server.address, "https://servidor.invalido:7443");
        // Quem apaga o endereço continua sem servidor nas próximas execuções.
        core.update_settings(|s| s.server.address.clear());
        drop(core);
        let core2 = Core::start_with(d.clone(), TestSink::new(), com_padrao).unwrap();
        assert_eq!(core2.settings().server.address, "");
        drop(core2);
        let _ = std::fs::remove_dir_all(d);

        // Sem `default_server` (os testes), a primeira execução fica sem endereço.
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        assert_eq!(core.settings().server.address, "");
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn update_settings_persiste_e_atualiza_me() {
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        core.update_settings(|s| s.name = "Novo".into());
        assert_eq!(core.snapshot().me.name, "Novo");
        drop(core);
        let core2 = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        assert_eq!(core2.settings().name, "Novo");
        drop(core2);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn set_smoothing_guarda_nas_configuracoes() {
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        core.set_smoothing(false, 350);
        let s = core.settings();
        assert!(!s.video.smoothing_auto);
        assert_eq!(s.video.smoothing_ms, 350);
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn set_volume_nao_panica_sem_participante() {
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        core.set_volume("ninguem", 0.5, false);
        core.set_volume("ninguem", f32::NAN, true);
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn shutdown_idempotente() {
        let d = dir();
        let mut core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        core.shutdown();
        core.shutdown();
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn drop_dentro_de_tarefa_tokio() {
        let (core, d) = criar(opts()).await;
        // Solta direto na tarefa assíncrona, sem spawn_blocking.
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn panico_com_core_vivo_nao_aborta() {
        let (core, d) = criar(opts()).await;
        let r = tokio::spawn(async move {
            let _c = core;
            panic!("x");
        })
        .await;
        assert!(r.unwrap_err().is_panic());
        let _ = std::fs::remove_dir_all(d);
    }

    /// Sala Local real (sem montar `ActiveRoom` na mão): `create_room` grava a sala em `shared.room`
    /// e a devolvemos com `current_room`; `create_room` já chama `set_active_room`.
    async fn sala_local(core: &Core) -> Arc<ActiveRoom> {
        core.create_room(CreateRoom {
            name: "Sala timer".into(),
            mode: RoomMode::Local,
            password: None,
        })
        .await
        .expect("criar sala");
        core.shared.current_room().expect("sala")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn set_active_room_pega_e_solta_timer() {
        let (core, d) = criar(opts()).await;
        let sala = sala_local(&core).await;
        let shared = core.shared.clone();
        // `create_room` já passa por `set_active_room`: o timer está pego aqui.
        assert!(shared.timer.lock().is_some());
        // Some -> Some: continua com o mesmo guarda (nenhum novo).
        shared.set_active_room(Some(sala.clone()));
        assert!(shared.timer.lock().is_some());
        // None: solta o timer.
        shared.set_active_room(None);
        assert!(shared.timer.lock().is_none());
        // Devolve a sala ao núcleo para o `shutdown` sair da sala de forma limpa.
        shared.set_active_room(Some(sala));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn set_active_room_devolve_anterior() {
        let (core, d) = criar(opts()).await;
        let sala = sala_local(&core).await;
        let shared = core.shared.clone();
        // Devolve a sala que estava guardada.
        let anterior = shared.set_active_room(None).expect("anterior");
        assert!(Arc::ptr_eq(&anterior, &sala));
        // Sem sala antes: devolve None.
        assert!(shared.set_active_room(Some(sala.clone())).is_none());
        // Trocar por outra devolve a que saiu.
        let anterior = shared.set_active_room(None).expect("anterior");
        assert!(Arc::ptr_eq(&anterior, &sala));
        shared.set_active_room(Some(sala));
        encerrar(core, d).await;
    }

    #[test]
    fn descoberta_desligada_nao_sobe_socket() {
        let d = dir();
        let core = Core::start_with(d.clone(), TestSink::new(), opts()).unwrap();
        assert!(core.shared.discovery.lock().is_none());
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    /// Espera até `limite` segundos por `ok` (verifica a cada 50 ms).
    async fn ate(limite: u64, mut ok: impl FnMut() -> bool) -> bool {
        let fim = std::time::Instant::now() + std::time::Duration::from_secs(limite);
        while std::time::Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        ok()
    }

    /// `Core` criado fora do contexto assíncrono (derrubar o runtime dentro dele dá pânico).
    async fn criar(options: CoreOptions) -> (Core, PathBuf) {
        tokio::task::spawn_blocking(move || {
            let d = dir();
            let core = Core::start_with(d.clone(), TestSink::new(), options).unwrap();
            (core, d)
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

    fn esta_online(core: &Core) -> bool {
        core.snapshot()
            .server
            .as_ref()
            .is_some_and(|s| s.state == crate::online::ServerState::Online)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn start_com_online_false_nao_sobe_sessao() {
        let (core, d) = criar(CoreOptions {
            online: false,
            ..opts()
        })
        .await;
        assert!(core.shared.online.status().is_none());
        assert!(core.shared.online.task.lock().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn update_settings_reinicia_sessao_quando_endereco_muda() {
        let srv = zap_test_server::spawn(zap_test_server::Config::default())
            .await
            .unwrap();
        let (core, d) = criar(opts()).await;
        core.update_settings(|s| s.server.address = srv.base_url.clone());
        assert!(
            ate(5, || esta_online(&core)).await,
            "status: {:?}",
            core.snapshot().server
        );
        core.update_settings(|s| s.server.address = String::new());
        assert!(ate(5, || core.snapshot().server.is_none()).await);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn update_settings_use_relay_nao_reinicia() {
        let srv = zap_test_server::spawn(zap_test_server::Config::default())
            .await
            .unwrap();
        let (core, d) = criar(opts()).await;
        core.update_settings(|s| s.server.address = srv.base_url.clone());
        assert!(ate(5, || esta_online(&core)).await);
        // Deixa o primeiro WebSocket assentar antes de medir a época.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let epoca = *core.shared.online.epoch_rx().borrow();
        core.update_settings(|s| s.server.use_relay = !s.server.use_relay);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert_eq!(*core.shared.online.epoch_rx().borrow(), epoca);
        assert!(esta_online(&core));
        encerrar(core, d).await;
    }
}
