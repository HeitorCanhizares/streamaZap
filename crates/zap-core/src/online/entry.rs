//! Pontos de entrada do `Core` para o modo Online.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{convert, preview, room::OnlineRoom};
use crate::engine::{Core, Shared};
use crate::room::ActiveRoom;
use crate::types::{CreateRoom, TileId};
use crate::CoreError;

/// Resultado de "Testar conexão" (Configurações). Sempre devolvido (falha vira `ok = false`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerTestResult {
    pub ok: bool,
    /// `nome` do servidor.
    pub name: Option<String>,
    pub ping_ms: Option<u32>,
    /// Código do erro (`protocolo_incompativel`, `senha_servidor`, `conexao`) quando `!ok`.
    pub code: Option<String>,
    /// Texto em português para a interface (sucesso ou erro).
    pub message: String,
}

/// `Core::create_room` com `mode == Online`: exige que não haja sala; `OnlineRoom::create`,
/// guarda `ActiveRoom::Online` em `shared.room`, `ensure_media_server` se
/// `media.direct_allowed`, `refresh_room_view`, `notifier.flush()`. Falha: nada fica guardado.
pub(crate) async fn create_online(shared: &Arc<Shared>, req: CreateRoom) -> Result<(), CoreError> {
    if shared.current_room().is_some() {
        return Err(CoreError::AlreadyInRoom);
    }
    let room = OnlineRoom::create(shared, req).await?;
    guarda_sala(shared, room).await
}

/// `JoinTarget::Code`: `convert::parse_code` + `OnlineRoom::join`; mesmo acabamento de
/// `create_online`. Já em sala → `AlreadyInRoom`.
pub(crate) async fn join_online(shared: &Arc<Shared>, code: String, password: Option<String>) -> Result<(), CoreError> {
    if shared.current_room().is_some() {
        return Err(CoreError::AlreadyInRoom);
    }
    let code = convert::parse_code(&code)?;
    let password = password.filter(|p| !p.is_empty());
    let room = OnlineRoom::join(shared, code, password).await?;
    guarda_sala(shared, room).await
}

/// Acabamento comum: guarda a sala, sobe o servidor de mídia (se a rota direta vale) e
/// atualiza a interface. Se o servidor de mídia falhar, a sala sai e o campo volta a `None`.
async fn guarda_sala(shared: &Arc<Shared>, room: Arc<OnlineRoom>) -> Result<(), CoreError> {
    shared.set_active_room(Some(Arc::new(ActiveRoom::Online(room.clone()))));
    // Depois de guardar a sala, para o `set_auth` enxergá-la.
    let direta = room.media.lock().direct_allowed;
    if direta {
        if let Err(e) = shared.ensure_media_server().await {
            shared.set_active_room(None);
            room.leave().await;
            return Err(e);
        }
    }
    shared.refresh_room_view();
    shared.notifier.flush();
    Ok(())
}

impl Core {
    /// "Testar conexão": **sem** mexer na sessão atual. `ServerClient::info(base)` (mede o
    /// tempo), confere o protocolo (mesma regra e mensagem de `OnlineSession::connect`), e se
    /// `requer_senha` ou `password` não é vazia, tenta `login` com `LoginRequest` das
    /// configurações (`senha_servidor` → `ok = false, code = "senha_servidor", message = "Senha
    /// do servidor incorreta"`). Endereço vazio → `ok = false, code = "invalido", message =
    /// "Informe o endereço do servidor"`. Rede → `code = "conexao"`.
    pub async fn server_test(&self, address: String, password: String) -> ServerTestResult {
        let shared = self.shared.clone();
        let falha = ServerTestResult {
            ok: false,
            code: Some("conexao".into()),
            message: "Teste interrompido".into(),
            ..Default::default()
        };
        shared
            .rt
            .clone()
            .spawn(async move { testa_servidor(&shared, address, password).await })
            .await
            .unwrap_or(falha)
    }
    /// Abre a prévia da sala (`code` digitado/da lista) no `tile`. Ver `preview::start`.
    pub async fn room_preview(&self, code: String, tile: TileId) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        shared
            .rt
            .clone()
            .spawn(async move {
                let code = convert::parse_code(&code)?;
                preview::start(&shared, code, tile).await
            })
            .await
            .map_err(|_| CoreError::Stopped)?
    }
    pub fn room_preview_stop(&self, tile: TileId) {
        preview::stop(&self.shared, tile);
    }
    /// Só para testes: rota atual (`ponte`/`direta`) da trilha assistida, se em sala Online.
    #[doc(hidden)]
    pub fn online_route(&self, track_id: &str) -> Option<zap_proto::control::Route> {
        match self.shared.current_room().as_deref() {
            Some(ActiveRoom::Online(o)) => Some(o.direct.route_of(track_id)),
            _ => None,
        }
    }
    /// Só para testes: endereço (`ip:porta` + impressão digital) do meu `MediaServer`, para a
    /// rota direta; `None` se ainda não existe.
    #[doc(hidden)]
    pub fn online_media_address(&self) -> Option<zap_proto::MediaAddress> {
        let guarda = self.shared.media_server.try_lock().ok()?;
        let ms = guarda.as_ref()?;
        let interface = self.shared.settings.read().local.interface.clone();
        Some(ms.address(interface.as_deref(), self.shared.options.include_loopback))
    }
    /// Só para testes: já mandei `midia.direta_ok` para essa trilha?
    #[doc(hidden)]
    pub fn online_direct_ok_sent(&self, track_id: &str) -> bool {
        match self.shared.current_room().as_deref() {
            Some(ActiveRoom::Online(o)) => o.direct.ok_sent.lock().contains(track_id),
            _ => false,
        }
    }
    /// Só para testes: fecha a conexão QUIC da ponte como numa queda de rede (a sala deve
    /// reconectar sozinha).
    #[doc(hidden)]
    pub fn online_drop_bridge(&self) {
        if let Some(ActiveRoom::Online(o)) = self.shared.current_room().as_deref() {
            if let Ok(ponte) = o.bridge.try_lock() {
                if let Some(pc) = ponte.as_ref() {
                    pc.conn.close(0u32.into(), b"queda");
                }
            }
        }
    }
    /// Só para testes: `OnlineSession::restart` (derruba o WebSocket e refaz o login).
    #[doc(hidden)]
    pub fn online_restart_session(&self) {
        self.shared.online.restart(&self.shared);
    }
    /// Só para testes: entrega `msg` à sala Online ativa como se tivesse vindo do WebSocket
    /// (o servidor de teste não manda `p2p.candidatos`).
    #[doc(hidden)]
    pub fn online_inject(&self, msg: zap_proto::ControlMsg) {
        // A rota vigente (a sala troca o canal ao reconectar), não uma cópia antiga.
        let tx = self.shared.online.route.lock().clone();
        if let Some(tx) = tx {
            let _ = tx.try_send(msg);
        }
    }
}

fn erro_do_teste(e: CoreError) -> ServerTestResult {
    match e {
        CoreError::Server { code, message } => ServerTestResult {
            ok: false,
            code: Some(code),
            message,
            ..Default::default()
        },
        outro => ServerTestResult {
            ok: false,
            code: Some("conexao".into()),
            message: outro.to_string(),
            ..Default::default()
        },
    }
}

async fn testa_servidor(shared: &Arc<Shared>, address: String, password: String) -> ServerTestResult {
    use std::time::Instant;
    use zap_net::online::{LoginRequest, ServerClient};

    let base = super::session::base_url(&address);
    if base.is_empty() {
        return ServerTestResult {
            ok: false,
            code: Some("invalido".into()),
            message: "Informe o endereço do servidor".into(),
            ..Default::default()
        };
    }
    let t0 = Instant::now();
    let info = match ServerClient::info(&base).await {
        Ok(i) => i,
        Err(e) => return erro_do_teste(e.into()),
    };
    let ping = u32::try_from(t0.elapsed().as_millis()).unwrap_or(u32::MAX).max(1);

    let versao = zap_proto::PROTOCOL_VERSION;
    if versao < info.protocol_min || versao > info.protocol_max {
        return ServerTestResult {
            ok: false,
            name: Some(info.name),
            ping_ms: Some(ping),
            code: Some("protocolo_incompativel".into()),
            message: "Este app e o servidor usam versões diferentes do protocolo. Atualize o StreamaZap.".into(),
        };
    }
    if info.requires_password || !password.is_empty() {
        let (name, color, install_id) = {
            let s = shared.settings.read();
            (s.name.clone(), s.color.clone(), s.install_id.clone())
        };
        let req = LoginRequest {
            name,
            color,
            install_id,
            protocol: versao,
            app_version: env!("CARGO_PKG_VERSION").into(),
            server_password: if password.is_empty() { None } else { Some(password) },
            account_token: None,
        };
        // O cliente logado só serve para validar a senha; é descartado.
        if let Err(e) = ServerClient::login(&base, req).await {
            let mut r = erro_do_teste(e.into());
            r.name = Some(info.name);
            r.ping_ms = Some(ping);
            if r.code.as_deref() == Some("senha_servidor") {
                r.message = "Senha do servidor incorreta".into();
            }
            return r;
        }
    }
    ServerTestResult {
        ok: true,
        message: format!("Conectado a {}", info.name),
        name: Some(info.name),
        ping_ms: Some(ping),
        code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;
    use crate::types::JoinTarget;
    use std::time::{Duration, Instant};
    use zap_proto::RoomMode;
    use zap_test_server::Config;

    async fn novo_core(url: Option<String>) -> (Core, std::path::PathBuf) {
        let (core, d) = tokio::task::spawn_blocking(|| {
            let d = std::env::temp_dir().join(format!("zapcore-entry-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap();
        if let Some(url) = url {
            core.update_settings(|s| {
                s.server.address = url;
                s.server.use_relay = false;
            });
            core.shared.online.start(&core.shared);
            assert!(ate(|| core.shared.online.ws().is_some()).await, "sem websocket");
        }
        (core, d)
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    async fn ate(mut ok: impl FnMut() -> bool) -> bool {
        let fim = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ok()
    }

    fn pedido(nome: &str) -> CreateRoom {
        CreateRoom {
            name: nome.into(),
            mode: RoomMode::Online,
            password: None,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn server_test_ok_mede_ping_e_nome() {
        let srv = zap_test_server::spawn(Config {
            name: "Servidor X".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let (core, d) = novo_core(None).await;
        let r = core.server_test(srv.base_url.clone(), String::new()).await;
        assert!(r.ok, "{r:?}");
        assert_eq!(r.name.as_deref(), Some("Servidor X"));
        assert!(r.ping_ms.unwrap_or(0) >= 1);
        assert_eq!(r.message, "Conectado a Servidor X");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn server_test_endereco_vazio() {
        let (core, d) = novo_core(None).await;
        let r = core.server_test("  ".into(), String::new()).await;
        assert!(!r.ok);
        assert_eq!(r.code.as_deref(), Some("invalido"));
        assert_eq!(r.message, "Informe o endereço do servidor");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn server_test_senha_errada() {
        let srv = zap_test_server::spawn(Config {
            server_password: Some("x".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let (core, d) = novo_core(None).await;
        let r = core.server_test(srv.base_url.clone(), String::new()).await;
        assert!(!r.ok);
        assert_eq!(r.code.as_deref(), Some("senha_servidor"));
        assert_eq!(r.message, "Senha do servidor incorreta");
        let r = core.server_test(srv.base_url.clone(), "x".into()).await;
        assert!(r.ok, "{r:?}");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn server_test_rede_falha() {
        // Porta que acabou de ser liberada: ninguém escuta.
        let porta = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let (core, d) = novo_core(None).await;
        let r = core
            .server_test(format!("http://127.0.0.1:{porta}"), String::new())
            .await;
        assert!(!r.ok);
        assert_eq!(r.code.as_deref(), Some("conexao"));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn server_test_nao_mexe_na_sessao() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let outro = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.base_url.clone())).await;
        assert!(ate(|| core.snapshot().server.is_some()).await);
        let antes = core.snapshot().server.clone();
        let r = core.server_test(outro.base_url.clone(), String::new()).await;
        assert!(r.ok);
        assert_eq!(core.snapshot().server, antes);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn criar_sala_online_vai_para_snapshot() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.base_url.clone())).await;
        core.create_room(pedido("Minha sala")).await.unwrap();
        let snap = core.snapshot();
        let sala = snap.room.as_ref().expect("sem sala no snapshot");
        assert_eq!(sala.mode, RoomMode::Online);
        assert!(sala.code.is_some());
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn entrar_por_codigo_com_variacoes_de_caixa() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(Some(srv.base_url.clone())).await;
        let (b, db) = novo_core(Some(srv.base_url.clone())).await;
        a.create_room(pedido("Sala")).await.unwrap();
        let codigo = a.snapshot().room.as_ref().and_then(|r| r.code.clone()).unwrap();
        let digitado = format!(" {} ", codigo.to_lowercase());
        b.join_room(JoinTarget::Code { code: digitado }, None).await.unwrap();
        assert_eq!(b.snapshot().room.as_ref().and_then(|r| r.code.clone()), Some(codigo));
        b.leave_room().await;
        a.leave_room().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ja_em_sala_erro() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.base_url.clone())).await;
        core.create_room(pedido("Sala")).await.unwrap();
        let r = core.create_room(pedido("Outra")).await;
        assert!(matches!(r, Err(CoreError::AlreadyInRoom)));
        let codigo = core.snapshot().room.as_ref().and_then(|r| r.code.clone()).unwrap();
        let r = core.join_room(JoinTarget::Code { code: codigo }, None).await;
        assert!(matches!(r, Err(CoreError::AlreadyInRoom)));
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn join_target_discovered_cai_para_codigo_online() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (a, da) = novo_core(Some(srv.base_url.clone())).await;
        let (b, db) = novo_core(Some(srv.base_url.clone())).await;
        // Cria a sala pelo cliente HTTP para ela aparecer na lista do servidor.
        a.create_room(pedido("Sala da lista")).await.unwrap();
        let codigo = a.snapshot().room.as_ref().and_then(|r| r.code.clone()).unwrap();
        assert!(ate(|| !b.shared.online.rooms().is_empty()).await, "lista vazia");
        let id = b
            .shared
            .online
            .rooms()
            .iter()
            .find(|r| r.code.to_string() == codigo)
            .map(|r| r.id.clone())
            .expect("sala não listada");
        b.join_room(JoinTarget::Discovered { room_id: id }, None).await.unwrap();
        assert_eq!(b.snapshot().room.as_ref().map(|r| r.mode), Some(RoomMode::Online));
        b.leave_room().await;
        a.leave_room().await;
        encerrar(a, da).await;
        encerrar(b, db).await;
    }
}
