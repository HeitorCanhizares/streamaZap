//! Aplica um token de conta à sessão Online viva (troca sem derrubar a sala).

use std::sync::Arc;

use super::session::OnlineSession;
use crate::engine::Shared;

impl OnlineSession {
    /// Chamada por `Core::online_set_account`. Guarda o token (só em memória) e reage conforme
    /// `account::decide_token_action`:
    /// - `Renew`: `client.renew(Some(token))`; sucesso → `publish_account`; erro com código
    ///   `proibido` (outra conta) → `restart`; outro erro → `tracing::warn!` (sem o token);
    /// - `Restart`: `restart(shared)`;
    /// - `Wake`: `wake.notify_one()`;
    /// - `Nothing`: nada.
    pub(crate) async fn apply_account(self: &Arc<Self>, shared: &Arc<Shared>, token: Option<String>) {
        use super::account::{decide_token_action, TokenAction};

        let current = self.account_token();
        let client = self.client();
        let has_client = client.is_some();
        let session_has_account = client.as_ref().and_then(|c| c.account_id()).is_some();
        // Só em memória: nunca em settings nem em log.
        *self.account_token.lock() = token.clone();

        match decide_token_action(has_client, session_has_account, current.as_deref(), token.as_deref()) {
            TokenAction::Nothing => {}
            TokenAction::Wake => self.wake.notify_one(),
            TokenAction::Restart => self.restart(shared),
            TokenAction::Renew => {
                let Some(client) = client else { return };
                match client.renew(token.as_deref()).await {
                    // Sala e WebSocket ficam como estão; só a conta/plano mudam.
                    Ok(_) => self.publish_account(shared),
                    // O servidor não troca a conta de uma sessão: é outra conta, recomeça.
                    Err(e) if e.code() == Some("proibido") => self.restart(shared),
                    Err(e) => {
                        tracing::warn!("não foi possível renovar a conta: {}", e.code().unwrap_or("rede"));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;
    use zap_proto::plan::PlanId;
    use zap_test_server::Config;

    fn core_teste() -> (Core, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("zapcore-apply-{}", uuid::Uuid::new_v4()));
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
    async fn renew_ok_nao_reinicia() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        let online = &core.shared.online;
        *online.account_token.lock() = Some("teste:u1:hd".into());
        let _rx = online.connect(&core.shared).await.unwrap();
        online.apply_account(&core.shared, Some("teste:u1:fullhd".into())).await;
        let c = online.client().unwrap();
        assert_eq!(c.plan().unwrap().id, PlanId::FullHd);
        assert_eq!(c.account_id().as_deref(), Some("u1"));
        assert!(online.task.lock().is_none());
        assert_eq!(online.account_token().as_deref(), Some("teste:u1:fullhd"));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn mesmo_token_nao_faz_nada() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core().await;
        apontar(&core, srv.base_url.clone());
        let online = &core.shared.online;
        *online.account_token.lock() = Some("teste:u1:hd".into());
        let _rx = online.connect(&core.shared).await.unwrap();
        let novo = Some("teste:u1:fullhd".to_string());
        online.apply_account(&core.shared, novo.clone()).await;
        online.apply_account(&core.shared, novo).await;
        let pid = online.client().unwrap().participant_id();
        assert_eq!(srv.hub.account_of(&pid).unwrap().renew_count, 1);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sem_cliente_acorda() {
        let (core, d) = novo_core().await;
        let online = &core.shared.online;
        online.apply_account(&core.shared, Some("teste:u1:hd".into())).await;
        assert_eq!(online.account_token().as_deref(), Some("teste:u1:hd"));
        encerrar(core, d).await;
    }
}
