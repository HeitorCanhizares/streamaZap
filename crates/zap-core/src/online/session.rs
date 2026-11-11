//! Sessão com o servidor Online: login, WebSocket, ping e lista de salas.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::{mpsc, watch};
use zap_net::online::{LoginRequest, RoomListing as NetListing, ServerClient, ServerInfo, WsTx};
use zap_proto::ControlMsg;

use super::preview::PreviewRegistry;
use super::status::ServerStatus;
use crate::engine::Shared;
use crate::CoreError;

/// Estado compartilhado da conexão com o servidor (um por `Core`; vive em `Shared::online`).
pub struct OnlineSession {
    /// `None` = sem endereço configurado ("não configurado").
    pub(crate) status: Mutex<Option<ServerStatus>>,
    pub(crate) info: Mutex<Option<ServerInfo>>,
    pub(crate) client: Mutex<Option<ServerClient>>,
    pub(crate) ws: Mutex<Option<WsTx>>,
    /// Última `salas.lista` (já convertida).
    pub(crate) rooms: Mutex<Vec<NetListing>>,
    /// Destino das mensagens do WebSocket que não são `salas.lista` (a sala ativa).
    pub(crate) route: Mutex<Option<mpsc::Sender<ControlMsg>>>,
    /// Soma 1 a cada WebSocket (re)conectado com sucesso.
    pub(crate) epoch: watch::Sender<u64>,
    /// Acorda o laço de `session_run::run` (configurações mudaram).
    pub(crate) wake: tokio::sync::Notify,
    pub(crate) task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(crate) previews: PreviewRegistry,
    /// Token de acesso da conta (JWT). **Só em memória**: nunca em settings, snapshot ou log.
    pub(crate) account_token: Mutex<Option<String>>,
    /// `senha_servidor` / `conta_necessaria` quando o último login falhou por isso.
    pub(crate) login_error: Mutex<Option<String>>,
}

impl OnlineSession {
    pub fn new() -> Arc<Self> {
        let (epoch, _) = watch::channel(0u64);
        Arc::new(Self {
            status: Mutex::new(None),
            info: Mutex::new(None),
            client: Mutex::new(None),
            ws: Mutex::new(None),
            rooms: Mutex::new(Vec::new()),
            route: Mutex::new(None),
            epoch,
            wake: tokio::sync::Notify::new(),
            task: Mutex::new(None),
            previews: PreviewRegistry::default(),
            account_token: Mutex::new(None),
            login_error: Mutex::new(None),
        })
    }
    pub fn status(&self) -> Option<ServerStatus> {
        self.status.lock().clone()
    }
    pub fn client(&self) -> Option<ServerClient> {
        self.client.lock().clone()
    }
    pub fn ws(&self) -> Option<WsTx> {
        self.ws.lock().clone()
    }
    pub fn rooms(&self) -> Vec<NetListing> {
        self.rooms.lock().clone()
    }
    pub fn info(&self) -> Option<ServerInfo> {
        self.info.lock().clone()
    }
    pub(crate) fn account_token(&self) -> Option<String> {
        self.account_token.lock().clone()
    }
    #[cfg(test)]
    pub(crate) fn login_error(&self) -> Option<String> {
        self.login_error.lock().clone()
    }
    /// Recalcula `account::account_view(info, client, login_error)` e grava em
    /// `state.set_account`; `notifier.touch()`. Nunca segura lock do state e de sessão juntos.
    pub(crate) fn publish_account(&self, shared: &Arc<Shared>) {
        // Cada lock é solto antes do próximo; o do state só entra depois.
        let info = self.info.lock().clone();
        let client = self.client.lock().clone();
        let login_error = self.login_error.lock().clone();
        let view = super::account::account_view(info.as_ref(), client.as_ref(), login_error.as_deref());
        shared.state.lock().set_account(view);
        shared.notifier.touch();
    }
    /// Define (ou limpa) o canal que recebe as mensagens da sala ativa.
    pub fn set_route(&self, tx: Option<mpsc::Sender<ControlMsg>>) {
        *self.route.lock() = tx;
    }
    pub fn epoch_rx(&self) -> watch::Receiver<u64> {
        self.epoch.subscribe()
    }
    /// Código (`ZAP-XXXX`) da sala Online com esse `id` na última lista; `None` se não está.
    pub fn code_for_room_id(&self, room_id: &str) -> Option<String> {
        self.rooms
            .lock()
            .iter()
            .find(|r| r.id == room_id)
            .map(|r| r.code.to_string())
    }

    /// Fluxo completo de (re)conexão, usado pelo laço de `session_run`:
    /// 1. endereço de `settings.server.address` vazio → `Err(Invalid("Servidor não configurado"))`;
    /// 2. `ServerClient::info(base)` (guarda em `info`); `PROTOCOL_VERSION` fora de
    ///    `[protocol_min, protocol_max]` → `Err(Server{code:"protocolo_incompativel", message:
    ///    "Este app e o servidor usam versões diferentes do protocolo. Atualize o StreamaZap."})`
    ///    e, se `protocol_min > PROTOCOL_VERSION`, `state.set_newer_protocol(Some(protocol_min))`;
    /// 3. `ServerClient::login(base, LoginRequest{name, color, install_id, protocol,
    ///    app_version, server_password})` (senha vazia → `None`); guarda em `client`;
    /// 4. `client.connect_ws()`; guarda `WsTx` em `ws` e devolve o `WsRx`.
    ///
    /// `base` = endereço com `https://` na frente se não houver esquema (`http://` é aceito).
    pub async fn connect(&self, shared: &Arc<Shared>) -> Result<zap_net::online::WsRx, CoreError> {
        let token = self.account_token();
        let (address, password, name, color, install_id) = {
            let s = shared.settings.read();
            (
                s.server.address.clone(),
                s.server.password.clone(),
                s.name.clone(),
                s.color.clone(),
                s.install_id.clone(),
            )
        };
        let base = base_url(&address);
        if base.is_empty() {
            return Err(CoreError::Invalid("Servidor não configurado".into()));
        }
        // Estado de uma tentativa anterior não deve sobreviver a uma falha desta.
        *self.client.lock() = None;
        *self.ws.lock() = None;
        *self.login_error.lock() = None;

        let info = ServerClient::info(&base).await?;
        *self.info.lock() = Some(info.clone());
        // A tela já pode saber `requires_account`/`conta` antes do login.
        self.publish_account(shared);

        let versao = zap_proto::PROTOCOL_VERSION;
        if versao < info.protocol_min || versao > info.protocol_max {
            // Só quando o servidor é mais novo: serve de pista para "atualize o app".
            // Compatível não apaga o valor (ele também vem da descoberta).
            if info.protocol_min > versao {
                shared.state.lock().set_newer_protocol(Some(info.protocol_min));
                shared.notifier.touch();
            }
            return Err(CoreError::Server {
                code: "protocolo_incompativel".into(),
                message: "Este app e o servidor usam versões diferentes do protocolo. Atualize o StreamaZap.".into(),
            });
        }

        let req = LoginRequest {
            name,
            color,
            install_id,
            protocol: versao,
            app_version: env!("CARGO_PKG_VERSION").into(),
            server_password: if password.is_empty() { None } else { Some(password) },
            // Servidor sem contas nunca recebe o token.
            account_token: if info.account.is_some() { token.clone() } else { None },
        };
        let client = match ServerClient::login(&base, req).await {
            Ok(c) => c,
            Err(e) => {
                let e: CoreError = e.into();
                *self.login_error.lock() = super::account::login_error_code(&e);
                self.publish_account(shared);
                return Err(e);
            }
        };
        // Só guarda `client` depois que o WebSocket abriu: falha não deixa estado parcial.
        let (tx, rx) = client.connect_ws().await?;
        *self.client.lock() = Some(client);
        *self.ws.lock() = Some(tx);
        self.publish_account(shared);
        Ok(rx)
    }
    /// Ping em ms: tempo de um `ServerClient::info(base)` (timeout 5 s → `Err(Connect)`).
    pub async fn measure_ping(&self, shared: &Arc<Shared>) -> Result<u32, CoreError> {
        let base = base_url(&shared.settings.read().server.address);
        let t0 = Instant::now();
        match tokio::time::timeout(Duration::from_secs(5), ServerClient::info(&base)).await {
            Err(_) => Err(CoreError::Connect("O servidor não respondeu ao ping".into())),
            Ok(Err(e)) => Err(CoreError::Connect(e.to_string())),
            Ok(Ok(_)) => {
                let ms = u32::try_from(t0.elapsed().as_millis()).unwrap_or(u32::MAX);
                Ok(ms.max(1))
            }
        }
    }
    /// Sobe `session_run::run` numa tarefa do runtime do `Core` (guarda o handle em `task`).
    pub fn start(self: &Arc<Self>, shared: &Arc<Shared>) {
        let handle = shared.rt.spawn(super::session_run::run(shared.clone()));
        if let Some(antiga) = self.task.lock().replace(handle) {
            antiga.abort();
        }
    }
    /// Aborta a tarefa atual, zera `client`/`ws`/`info`/`rooms`/`status`/`route` e chama `start`
    /// (as configurações do servidor mudaram).
    pub fn restart(self: &Arc<Self>, shared: &Arc<Shared>) {
        self.clear(shared);
        self.start(shared);
    }
    /// Parte comum de `restart` e `stop_and_clear`: aborta a tarefa e zera o estado da sessão
    /// e o que a interface mostra dela. Idempotente.
    fn clear(&self, shared: &Arc<Shared>) {
        if let Some(t) = self.task.lock().take() {
            t.abort();
        }
        *self.client.lock() = None;
        // Sem nenhum `WsTx` vivo, a tarefa do WebSocket fecha a conexão sozinha.
        *self.ws.lock() = None;
        *self.info.lock() = None;
        self.rooms.lock().clear();
        *self.status.lock() = None;
        *self.route.lock() = None;
        // O `account_token` fica: sobrevive ao reinício (só `apply_account` o troca).
        *self.login_error.lock() = None;
        {
            let mut st = shared.state.lock();
            st.set_server(None);
            st.set_account(None);
            st.clear_plan_notice();
            st.set_online_rooms(vec![]);
        }
        shared.notifier.touch();
    }
    /// Desliga o Online (interruptor "Usar o modo Online" desligado): aborta a tarefa, fecha o
    /// WebSocket e zera `client`/`ws`/`info`/`rooms`/`status`/`route`, o `server` e as salas Online
    /// do estado e as prévias abertas; avisa a interface. **Não** sobe a sessão de novo (diferente de
    /// `restart`). Tarefa core-03.
    pub fn stop_and_clear(self: &Arc<Self>, shared: &Arc<Shared>) {
        // Copia os tiles antes: `preview::stop` toma o mesmo lock.
        let tiles: Vec<_> = self.previews.active.lock().keys().copied().collect();
        for tile in tiles {
            super::preview::stop(shared, tile);
        }
        self.clear(shared);
    }
    /// Aborta a tarefa e fecha o WebSocket (desligamento do `Core`).
    pub fn stop(&self) {
        if let Some(t) = self.task.lock().take() {
            t.abort();
        }
        *self.ws.lock() = None;
        *self.client.lock() = None;
    }
}

/// Endereço das configurações → URL base: vazio fica vazio; sem esquema vira `https://`;
/// `http://` é mantido (testes e redes locais); a barra final sai.
pub(crate) fn base_url(address: &str) -> String {
    let a = address.trim();
    if a.is_empty() {
        return String::new();
    }
    let com_esquema = if a.contains("://") {
        a.to_owned()
    } else {
        format!("https://{a}")
    };
    com_esquema.trim_end_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;
    use zap_test_server::Config;

    fn core_teste() -> (Core, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("zapcore-session-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            online: false,
            ..Default::default()
        };
        let core = Core::start_with(d.clone(), TestSink::new(), opts).unwrap();
        (core, d)
    }

    async fn novo_core() -> (Core, std::path::PathBuf) {
        tokio::task::spawn_blocking(core_teste).await.unwrap()
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        // O runtime do Core não pode ser derrubado dentro de um contexto assíncrono.
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    #[test]
    fn base_url_variacoes() {
        assert_eq!(base_url(""), "");
        assert_eq!(base_url("   "), "");
        assert_eq!(base_url(" exemplo.com "), "https://exemplo.com");
        assert_eq!(base_url("exemplo.com/"), "https://exemplo.com");
        assert_eq!(base_url("http://127.0.0.1:80/"), "http://127.0.0.1:80");
        assert_eq!(base_url("https://a.b"), "https://a.b");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_endereco_vazio_invalido() {
        let (core, d) = novo_core().await;
        let r = core.shared.online.connect(&core.shared).await;
        assert!(matches!(r, Err(CoreError::Invalid(ref m)) if m == "Servidor não configurado"));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_ok_preenche_client_ws_info() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        let url = srv.base_url.clone();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        assert!(core.shared.online.client().is_some());
        assert!(core.shared.online.ws().is_some());
        assert!(core.shared.online.info().is_some());
        core.shared.online.stop();
        assert!(core.shared.online.client().is_none());
        assert!(core.shared.online.ws().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_senha_servidor_errada() {
        let srv = zap_test_server::spawn(Config {
            server_password: Some("x".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let (core, d) = novo_core().await;
        let url = srv.base_url.clone();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        let r = core.shared.online.connect(&core.shared).await;
        assert!(matches!(r, Err(CoreError::Server { ref code, .. }) if code == "senha_servidor"));
        assert!(core.shared.online.client().is_none());
        assert!(core.shared.online.ws().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn protocolo_incompativel() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        // Pega um `ServerInfo` real e sobe o `protocolo_min` além do nosso.
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let mut info = ServerClient::info(&srv.base_url).await.unwrap();
        let min = zap_proto::PROTOCOL_VERSION + 1;
        info.protocol_min = min;
        info.protocol_max = min;
        let corpo = serde_json::to_string(&info).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                let corpo = corpo.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let _ = sock.read(&mut buf).await;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        corpo.len(),
                        corpo
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });

        let (core, d) = novo_core().await;
        core.update_settings(|s| {
            s.server.address = format!("http://127.0.0.1:{porta}");
            s.server.enabled = true;
        });
        let r = core.shared.online.connect(&core.shared).await;
        match r {
            Err(CoreError::Server { code, message }) => {
                assert_eq!(code, "protocolo_incompativel");
                assert_eq!(
                    message,
                    "Este app e o servidor usam versões diferentes do protocolo. Atualize o StreamaZap."
                );
            }
            _ => panic!("esperava protocolo_incompativel"),
        }
        assert_eq!(core.snapshot().newer_protocol, Some(min));
        assert!(core.shared.online.client().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn measure_ping_positivo() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        let url = srv.base_url.clone();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        let ms = core.shared.online.measure_ping(&core.shared).await.unwrap();
        assert!(ms >= 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn restart_zera_estado() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        let url = srv.base_url.clone();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        *core.shared.online.status.lock() = Some(ServerStatus::connecting(None));
        // Sem endereço, a tarefa nova (quando `run` existir) não reconecta e não mexe no estado.
        core.update_settings(|s| s.server.address = String::new());
        // `session_run::run` pode ser `todo!` (outra tarefa): a tarefa nova entra em pânico
        // sozinha, sem afetar o estado que o `restart` zerou.
        core.shared.online.restart(&core.shared);
        assert!(core.shared.online.client().is_none());
        assert!(core.shared.online.ws().is_none());
        assert!(core.shared.online.info().is_none());
        assert!(core.shared.online.rooms().is_empty());
        assert!(core.shared.online.status().is_none());
        assert!(core.shared.online.task.lock().is_some());
        core.shared.online.stop();
        assert!(core.shared.online.task.lock().is_none());
        encerrar(core, d).await;
    }

    async fn sessao_online(srv: &zap_test_server::Handle) -> (Core, std::path::PathBuf) {
        let (core, d) = novo_core().await;
        let url = srv.base_url.clone();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        core.shared.online.start(&core.shared);
        for _ in 0..100 {
            if core.shared.online.status().is_some() && core.shared.online.ws().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        (core, d)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_and_clear_zera_estado() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = sessao_online(&srv).await;
        assert!(core.shared.online.status().is_some());
        core.shared.online.stop_and_clear(&core.shared);
        assert!(core.shared.online.status().is_none());
        assert!(core.shared.online.task.lock().is_none());
        assert!(core.snapshot().server.is_none());
        assert!(core.shared.online.rooms().is_empty());
        assert!(core.shared.online.ws().is_none());
        assert!(core.shared.online.client().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_and_clear_e_idempotente() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = sessao_online(&srv).await;
        core.shared.online.stop_and_clear(&core.shared);
        core.shared.online.stop_and_clear(&core.shared);
        assert!(core.shared.online.status().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_and_clear_nao_sobe_de_novo() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = sessao_online(&srv).await;
        core.shared.online.stop_and_clear(&core.shared);
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(core.shared.online.task.lock().is_none());
        assert!(core.shared.online.status().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_and_clear_fecha_websocket() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = sessao_online(&srv).await;
        let com_ws = |srv: &zap_test_server::Handle| srv.hub.inner.lock().participants.values().any(|p| p.ws.is_some());
        assert!(com_ws(&srv), "o servidor deveria ter a sessão ativa");
        core.shared.online.stop_and_clear(&core.shared);
        let mut fechou = false;
        for _ in 0..60 {
            if !com_ws(&srv) {
                fechou = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(fechou, "o WebSocket continuou aberto");
        encerrar(core, d).await;
    }

    fn conta_cfg() -> Config {
        Config {
            requires_account: true,
            account: Some(zap_net::online::AccountProvider {
                provider: "supabase".into(),
                url: "https://x.supabase.co".into(),
                key: "k".into(),
            }),
            ..Default::default()
        }
    }

    fn apontar(core: &Core, url: String) {
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_com_token_envia_acesso_conta() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        *core.shared.online.account_token.lock() = Some("teste:u1:fullhd".into());
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        assert_eq!(core.shared.online.client().unwrap().account_id().as_deref(), Some("u1"));
        let a = core.snapshot().account.clone().unwrap();
        assert_eq!(a.account_id.as_deref(), Some("u1"));
        assert!(a.requires_account);
        assert_eq!(a.plan.unwrap().id, zap_proto::plan::PlanId::FullHd);
        assert_eq!(a.login_error, None);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_sem_token_requer_conta() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        let r = core.shared.online.connect(&core.shared).await;
        assert!(matches!(r, Err(CoreError::Server { ref code, .. }) if code == "conta_necessaria"));
        assert_eq!(core.shared.online.login_error().as_deref(), Some("conta_necessaria"));
        let a = core.snapshot().account.clone().unwrap();
        assert_eq!(a.login_error.as_deref(), Some("conta_necessaria"));
        assert!(a.requires_account);
        assert_eq!(a.account_id, None);
        assert!(core.shared.online.client().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_servidor_sem_conta_nao_manda_token() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        *core.shared.online.account_token.lock() = Some("teste:u1:fullhd".into());
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        assert_eq!(core.shared.online.client().unwrap().account_id(), None);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn connect_senha_servidor_errada_publica_login_error() {
        let srv = zap_test_server::spawn(Config {
            server_password: Some("x".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        let r = core.shared.online.connect(&core.shared).await;
        assert!(matches!(r, Err(CoreError::Server { ref code, .. }) if code == "senha_servidor"));
        let a = core.snapshot().account.clone().unwrap();
        assert_eq!(a.login_error.as_deref(), Some("senha_servidor"));
        assert!(a.requires_password);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_and_clear_zera_conta() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        *core.shared.online.account_token.lock() = Some("teste:u1:fullhd".into());
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        assert!(core.snapshot().account.is_some());
        core.shared.online.stop_and_clear(&core.shared);
        assert!(core.snapshot().account.is_none());
        assert_eq!(core.shared.online.login_error(), None);
        assert_eq!(core.shared.online.account_token().as_deref(), Some("teste:u1:fullhd"));
        encerrar(core, d).await;
    }
}
