//! Laço de fundo da sessão Online: login, ping, leitura do WebSocket e reconexão.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use zap_proto::ControlMsg;

use super::backoff::Backoff;
use super::convert::{listing_to_ui, parse_ws_rooms};
use super::session::base_url;
use super::status::ServerStatus;
use crate::engine::Shared;

/// Roda até `shared.stop` ou a tarefa ser abortada (`OnlineSession::restart/stop`).
/// A cada volta:
/// 1. endereço vazio → `session.status = None`, `state.set_server(None)`, `notifier.touch()` e
///    espera `session.wake` (sem sondar);
/// 2. senão status `connecting`; `session.connect(&shared)`; erro → status `error(mensagem)`
///    (+ `push_error` só na primeira falha seguida), espera `Backoff::next_delay` (ou `wake`)
///    e tenta de novo; sucesso → `Backoff::reset`, `epoch += 1`, status `online(nome, ping)`;
/// 3. com o WebSocket aberto: lê `WsRx::recv` e despacha — `RoomList{salas}` →
///    `convert::parse_ws_rooms` → `session.rooms` e `state.set_online_rooms(listings
///    convertidas com listing_to_ui)` + `touch`; `Error` → `push_error`; demais mensagens →
///    `session.route` (`try_send`; cheio = descarta com `tracing::warn!`);
///    em paralelo, a cada 5 s `measure_ping` atualiza o status (`state.set_server`, `touch`);
/// 4. `recv` devolver `None` (ou erro): status `connecting`, zera `ws`, volta ao passo 2.
///    Toda mudança de status faz `shared.state.lock().set_server(status.clone())`.
///
/// Erro de login por senha ou conta fica em `login_error` (veja `connect`); `wake` (token novo)
/// acorda a espera e tenta de novo.
pub(crate) async fn run(shared: Arc<Shared>) {
    let session = shared.online.clone();
    let mut backoff = Backoff::new();
    // Já empilhamos o erro desta sequência de falhas?
    let mut erro_avisado = false;

    while !shared.stop.load(Ordering::Relaxed) {
        let endereco = shared.settings.read().server.address.clone();
        if base_url(&endereco).is_empty() {
            *session.status.lock() = None;
            session.rooms.lock().clear();
            *session.login_error.lock() = None;
            {
                let mut st = shared.state.lock();
                st.set_server(None);
                st.set_online_rooms(vec![]);
                st.set_account(None);
            }
            shared.notifier.touch();
            session.wake.notified().await;
            continue;
        }

        let nome = session.info().map(|i| i.name);
        publicar(&shared, Some(ServerStatus::connecting(nome)));

        let mut rx = match session.connect(&shared).await {
            Ok(rx) => rx,
            Err(e) => {
                let msg = match &e {
                    crate::CoreError::Server { message, .. } => message.clone(),
                    outro => outro.to_string(),
                };
                publicar(&shared, Some(ServerStatus::error(msg.clone())));
                if !erro_avisado {
                    erro_avisado = true;
                    shared.state.lock().push_error(format!("Servidor Online: {msg}"));
                    shared.notifier.touch();
                }
                esperar_retry(&session, &mut backoff).await;
                continue;
            }
        };

        backoff.reset();
        erro_avisado = false;
        // `connect` já guardou `ws`: quem espera a época pode usá-lo.
        session.epoch.send_modify(|e| *e += 1);
        let nome = session.info().map(|i| i.name).unwrap_or_default();
        let mut ping = session.measure_ping(&shared).await.ok();
        publicar(&shared, Some(ServerStatus::online(nome.clone(), ping)));

        let mut online_mostrado = true;
        let mut tick = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(5),
            Duration::from_secs(5),
        );
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(Ok(m)) => despachar(&shared, m),
                    Some(Err(e)) => tracing::debug!("mensagem inválida do servidor: {e}"),
                    None => break,
                },
                _ = tick.tick() => {
                    // Falha de ping não derruba a conexão (o WebSocket pode seguir aberto), mas a
                    // barra deixa de dizer "online" até o próximo ping responder.
                    match session.measure_ping(&shared).await {
                        Ok(ms) => {
                            if ping != Some(ms) || !online_mostrado {
                                ping = Some(ms);
                                online_mostrado = true;
                                publicar(&shared, Some(ServerStatus::online(nome.clone(), ping)));
                            }
                        }
                        Err(_) if online_mostrado => {
                            online_mostrado = false;
                            publicar(&shared, Some(ServerStatus::connecting(Some(nome.clone()))));
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        *session.ws.lock() = None;
        let nome = session.info().map(|i| i.name);
        publicar(&shared, Some(ServerStatus::connecting(nome)));
        esperar_retry(&session, &mut backoff).await;
    }
}

/// Grava o status na sessão e na interface (sem segurar o lock do state em `.await`).
fn publicar(shared: &Arc<Shared>, status: Option<ServerStatus>) {
    *shared.online.status.lock() = status.clone();
    shared.state.lock().set_server(status);
    shared.notifier.touch();
}

/// Espera o próximo atraso do backoff ou um `wake`, o que vier primeiro.
async fn esperar_retry(session: &super::session::OnlineSession, backoff: &mut Backoff) {
    let atraso = backoff.next_delay();
    tokio::select! {
        _ = tokio::time::sleep(atraso) => {}
        _ = session.wake.notified() => {}
    }
}

fn despachar(shared: &Arc<Shared>, msg: ControlMsg) {
    let session = &shared.online;
    match msg {
        ControlMsg::RoomList { salas } => {
            let lista = parse_ws_rooms(&salas);
            let ui: Vec<_> = lista.iter().map(listing_to_ui).collect();
            *session.rooms.lock() = lista;
            shared.state.lock().set_online_rooms(ui);
            shared.notifier.touch();
        }
        outra => {
            let rota = session.route.lock().clone();
            match rota {
                Some(tx) => {
                    if tx.try_send(outra).is_err() {
                        tracing::warn!("canal da sala cheio ou fechado: mensagem do servidor descartada");
                    }
                }
                None => {
                    if let ControlMsg::Error { message, .. } = outra {
                        shared.state.lock().push_error(message);
                        shared.notifier.touch();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use zap_net::online::{CreateRoomRequest, LoginRequest, ServerClient};
    use zap_proto::Access;
    use zap_test_server::Config;

    use crate::engine::{Core, CoreOptions};
    use crate::online::ServerState;
    use crate::testutil::TestSink;
    use crate::types::RoomMode;

    async fn novo_core() -> (Core, std::path::PathBuf) {
        tokio::task::spawn_blocking(|| {
            let d = std::env::temp_dir().join(format!("zapcore-run-{}", uuid::Uuid::new_v4()));
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
        .unwrap()
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    async fn ate(limite: u64, mut ok: impl FnMut() -> bool) -> bool {
        let fim = Instant::now() + Duration::from_secs(limite);
        while Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ok()
    }

    fn iniciar(core: &Core, url: String) {
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
        });
        core.shared.online.start(&core.shared);
    }

    fn esta_online(core: &Core) -> bool {
        core.snapshot()
            .server
            .as_ref()
            .is_some_and(|s| s.state == ServerState::Online)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sem_endereco_server_none() {
        let (core, d) = novo_core().await;
        core.shared.online.start(&core.shared);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(core.snapshot().server.is_none());
        assert!(core.shared.online.status().is_none());
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn loga_e_mostra_online_com_ping() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        let ok = ate(5, || {
            core.snapshot()
                .server
                .as_ref()
                .is_some_and(|s| s.state == ServerState::Online && s.ping_ms.is_some())
        })
        .await;
        assert!(ok, "status: {:?}", core.snapshot().server);
        let label = core.snapshot().server.as_ref().unwrap().label();
        assert!(label.starts_with("Servidor online"), "{label}");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn lista_de_salas_online_no_snapshot() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        assert!(ate(5, || esta_online(&core)).await);

        let outro = ServerClient::login(
            &srv.base_url,
            LoginRequest {
                name: "Outro".into(),
                color: "#ff0000".into(),
                install_id: uuid::Uuid::new_v4().to_string(),
                protocol: zap_proto::PROTOCOL_VERSION,
                app_version: "teste".into(),
                server_password: None,
                account_token: None,
            },
        )
        .await
        .unwrap();
        let sala = outro
            .create_room(CreateRoomRequest {
                name: "Sala de teste".into(),
                access: Access::Open,
                password: None,
            })
            .await
            .unwrap();
        let codigo = sala.code.to_string();
        let ok = ate(5, || {
            core.snapshot()
                .rooms
                .iter()
                .any(|r| r.mode == RoomMode::Online && r.addr.as_deref() == Some(codigo.as_str()))
        })
        .await;
        assert!(ok, "salas: {:?}", core.snapshot().rooms);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn senha_servidor_errada_vira_erro() {
        let srv = zap_test_server::spawn(Config {
            server_password: Some("x".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        let ok = ate(5, || {
            core.snapshot()
                .server
                .as_ref()
                .is_some_and(|s| s.state == ServerState::Error && s.message.as_deref().is_some_and(|m| !m.is_empty()))
        })
        .await;
        assert!(ok, "status: {:?}", core.snapshot().server);
        // Deixa passar mais tentativas (1 s + 2 s de espera): o erro não se repete.
        tokio::time::sleep(Duration::from_millis(3500)).await;
        assert_eq!(core.snapshot().errors.len(), 1, "{:?}", core.snapshot().errors);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn servidor_caiu_vira_connecting() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        assert!(ate(5, || esta_online(&core)).await);
        drop(srv);
        let ok = ate(8, || !esta_online(&core)).await;
        assert!(ok, "status: {:?}", core.snapshot().server);
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

    fn id_da_conta(core: &Core) -> Option<String> {
        core.snapshot().account.as_ref().and_then(|a| a.account_id.clone())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn endereco_vazio_limpa_conta() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        // Sem token: o laco fica em `esperar_retry` com o erro de login.
        iniciar(&core, srv.base_url.clone());
        let ok = ate(5, || {
            core.snapshot()
                .account
                .as_ref()
                .is_some_and(|a| a.login_error.as_deref() == Some("conta_necessaria"))
        })
        .await;
        assert!(ok, "conta: {:?}", core.snapshot().account);
        // Zera o endereco e acorda o laco (sem `restart`/`clear`): so o ramo
        // 'endereco vazio' de `run` pode limpar a conta.
        core.update_settings(|s| s.server.address = String::new());
        core.shared.online.wake.notify_one();
        let ok = ate(5, || core.snapshot().account.is_none()).await;
        assert!(ok, "conta: {:?}", core.snapshot().account);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn conta_necessaria_tenta_de_novo_com_wake() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        let ok = ate(5, || {
            let s = core.snapshot();
            s.account
                .as_ref()
                .is_some_and(|a| a.login_error.as_deref() == Some("conta_necessaria"))
                && s.server.as_ref().is_some_and(|x| x.state == ServerState::Error)
        })
        .await;
        assert!(ok, "conta: {:?}", core.snapshot().account);
        *core.shared.online.account_token.lock() = Some("teste:u2:hd".into());
        core.shared.online.wake.notify_one();
        let ok = ate(2, || esta_online(&core) && id_da_conta(&core).as_deref() == Some("u2")).await;
        assert!(ok, "conta: {:?}", core.snapshot().account);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn restart_com_endereco_vazio_limpa_status() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core().await;
        iniciar(&core, srv.base_url.clone());
        assert!(ate(5, || esta_online(&core)).await);
        core.update_settings(|s| s.server.address = String::new());
        core.shared.online.restart(&core.shared);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(core.snapshot().server.is_none());
        assert!(core.shared.online.status().is_none());
        encerrar(core, d).await;
    }
}
