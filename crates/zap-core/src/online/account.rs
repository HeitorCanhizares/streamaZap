//! Conta e plano no modo Online: tipos para a interface e decisões puras (sem I/O, sem tokio).
//!
//! O token de acesso da conta (JWT do Supabase) **nunca** sai da memória do `OnlineSession`:
//! não vai para `settings.json`, para o `Snapshot` nem para o log.

use serde::{Deserialize, Serialize};
use zap_net::online::{AccountProvider, ServerClient, ServerInfo};
use zap_proto::plan::PlanInfo;

/// Onde o app faz login (cópia de `ServerInfo.account` com nomes em inglês para a interface).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccountProviderView {
    pub provider: String,
    pub url: String,
    pub key: String,
}

impl From<&AccountProvider> for AccountProviderView {
    fn from(p: &AccountProvider) -> Self {
        Self {
            provider: p.provider.clone(),
            url: p.url.clone(),
            key: p.key.clone(),
        }
    }
}

/// `Snapshot.account`: o que a interface precisa para a conta e a senha do servidor.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AccountView {
    /// `ServerInfo.account`; `None` = o servidor não confere contas (a interface não mostra login).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_provider: Option<AccountProviderView>,
    pub requires_account: bool,
    pub requires_password: bool,
    /// `conta_id` da sessão atual (`None` = sessão sem conta).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// `plano` da sessão atual (`None` = sem limite de plano).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanInfo>,
    /// `senha_servidor` ou `conta_necessaria` quando o último login falhou por isso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_error: Option<String>,
}

/// O que fazer quando o front entrega um token novo (ou `None` = saiu da conta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenAction {
    /// Nada a fazer (mesmo token de novo, ou continua sem conta).
    Nothing,
    /// `client.renew(Some(token))` na sessão atual, sem derrubar sala nem WebSocket.
    Renew,
    /// Reinicia a sessão (`POST /v1/sessoes` novo).
    Restart,
    /// Sessão ainda não logou (caiu por `conta_necessaria` ou está tentando): acorda o laço.
    Wake,
}

/// Monta o `AccountView` a partir da sessão. `info = None` → `None` (sem servidor, sem visão).
/// Usa `client.account_id()` e `client.plan()` quando há cliente logado.
pub fn account_view(
    info: Option<&ServerInfo>,
    client: Option<&ServerClient>,
    login_error: Option<&str>,
) -> Option<AccountView> {
    let info = info?;
    Some(AccountView {
        account_provider: info.account.as_ref().map(AccountProviderView::from),
        requires_account: info.requires_account,
        requires_password: info.requires_password,
        account_id: client.and_then(|c| c.account_id()),
        plan: client.and_then(|c| c.plan()),
        login_error: login_error.map(str::to_string),
    })
}

/// Código a guardar em `login_error`: `Some("senha_servidor" | "conta_necessaria")` para esses dois
/// códigos de `CoreError::Server`; qualquer outro erro → `None`.
pub fn login_error_code(e: &crate::CoreError) -> Option<String> {
    match e {
        crate::CoreError::Server { code, .. } if code == "senha_servidor" || code == "conta_necessaria" => {
            Some(code.clone())
        }
        _ => None,
    }
}

/// Decide a reação à chegada de um token.
/// - `has_client`: já existe `ServerClient` logado;
/// - `session_has_account`: a sessão atual tem `account_id`;
/// - `current`: token que o `OnlineSession` já guarda; `new`: o que chegou.
pub fn decide_token_action(
    has_client: bool,
    session_has_account: bool,
    current: Option<&str>,
    new: Option<&str>,
) -> TokenAction {
    if current == new {
        return TokenAction::Nothing;
    }
    if !has_client {
        return TokenAction::Wake;
    }
    match new {
        None if session_has_account => TokenAction::Restart,
        None => TokenAction::Nothing,
        Some(_) => TokenAction::Renew,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoreError;
    use zap_net::online::LoginRequest;
    use zap_proto::plan::PlanId;
    use zap_test_server::Config;

    fn login_req(token: Option<&str>) -> LoginRequest {
        LoginRequest {
            name: "t".into(),
            color: "#ffffff".into(),
            install_id: "inst-1".into(),
            protocol: zap_proto::PROTOCOL_VERSION,
            app_version: "0.0.0".into(),
            server_password: None,
            account_token: token.map(str::to_string),
        }
    }

    fn srv_err(code: &str) -> CoreError {
        CoreError::Server {
            code: code.into(),
            message: "m".into(),
        }
    }

    #[test]
    fn info_none_da_none() {
        assert!(account_view(None, None, None).is_none());
    }

    #[tokio::test]
    async fn view_sem_client() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let info = ServerClient::info(&srv.base_url).await.unwrap();
        let v = account_view(Some(&info), None, None).unwrap();
        assert!(!v.requires_account);
        assert!(!v.requires_password);
        assert_eq!(v.account_id, None);
        assert!(v.plan.is_none());
        assert!(v.account_provider.is_none());
        assert_eq!(v.login_error, None);
        let v = account_view(Some(&info), None, Some("conta_necessaria")).unwrap();
        assert_eq!(v.login_error.as_deref(), Some("conta_necessaria"));
    }

    #[tokio::test]
    async fn view_com_provedor() {
        let srv = zap_test_server::spawn(Config {
            requires_account: true,
            account: Some(zap_net::online::AccountProvider {
                provider: "supabase".into(),
                url: "https://x.supabase.co".into(),
                key: "sb_publishable_x".into(),
            }),
            ..Default::default()
        })
        .await
        .unwrap();
        let info = ServerClient::info(&srv.base_url).await.unwrap();
        let v = account_view(Some(&info), None, None).unwrap();
        assert!(v.requires_account);
        let p = v.account_provider.unwrap();
        assert_eq!(p.provider, "supabase");
        assert_eq!(p.url, "https://x.supabase.co");
        assert_eq!(p.key, "sb_publishable_x");
    }

    #[tokio::test]
    async fn view_com_client_logado() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let info = ServerClient::info(&srv.base_url).await.unwrap();
        let client = ServerClient::login(&srv.base_url, login_req(Some("teste:u1:hd")))
            .await
            .unwrap();
        let v = account_view(Some(&info), Some(&client), None).unwrap();
        assert_eq!(v.account_id.as_deref(), Some("u1"));
        assert_eq!(v.plan.unwrap().id, PlanId::Hd);
    }

    #[test]
    fn login_error_codes() {
        assert_eq!(
            login_error_code(&srv_err("senha_servidor")).as_deref(),
            Some("senha_servidor")
        );
        assert_eq!(
            login_error_code(&srv_err("conta_necessaria")).as_deref(),
            Some("conta_necessaria")
        );
        assert_eq!(login_error_code(&srv_err("sala_cheia")), None);
        assert_eq!(login_error_code(&CoreError::NoRoom), None);
    }

    #[test]
    fn tabela_decide_token_action() {
        use TokenAction::*;
        let casos = [
            (false, false, None, None, Nothing),
            (false, false, None, Some("a"), Wake),
            (false, false, Some("a"), Some("b"), Wake),
            (true, true, Some("a"), Some("a"), Nothing),
            (true, false, None, Some("a"), Renew),
            (true, true, Some("a"), Some("b"), Renew),
            (true, true, Some("a"), None, Restart),
            (true, false, Some("a"), None, Nothing),
        ];
        for (c, a, cur, new, esperado) in casos {
            assert_eq!(decide_token_action(c, a, cur, new), esperado, "{c} {a} {cur:?} {new:?}");
        }
    }

    #[test]
    fn view_default_serializa_so_obrigatorios() {
        let v = serde_json::to_value(AccountView::default()).unwrap();
        let o = v.as_object().unwrap();
        assert_eq!(o.len(), 2);
        assert!(o.contains_key("requires_account"));
        assert!(o.contains_key("requires_password"));
    }
}
