//! Conta falsa do servidor de teste (só para os testes do `zap-core`; **sem JWT de verdade**).
//!
//! Contrato de produção: `docs/api/rest.md` (`acesso_conta`, `requer_conta`, `renovar`). Aqui o
//! "JWT" é um texto `teste:<sub>:<plano>` (ex.: `teste:u1:fullhd`); qualquer outro formato é
//! inválido. Este módulo é puro (sem I/O, sem relógio): `state.rs` guarda o resultado no
//! participante e `rest.rs` só repassa o texto do token.

use zap_proto::plan::{PlanId, PlanInfo};

use crate::state::HubError;

/// Token falso já lido: `teste:<sub>:<plano>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestToken {
    /// Id da conta (vira `conta_id`).
    pub sub: String,
    /// Plano do claim (`PlanId::from_claim`; texto desconhecido = `PlanId::None`).
    pub plan: PlanId,
}

/// Conta ligada a um participante, mais o contador de renovações (para os testes provarem que o
/// `renovar` aconteceu sem reconectar o WebSocket).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    /// `None` = sessão sem conta (sem limite de plano).
    pub account_id: Option<String>,
    pub plan: Option<PlanId>,
    /// Quantos `POST /v1/sessoes/renovar` o participante já recebeu (com ou sem token de conta).
    pub renew_count: u32,
}

impl AccountState {
    /// `plano` da resposta da sessão: `Some(PlanInfo::of(plano))` só quando há conta.
    pub fn plan_info(&self) -> Option<PlanInfo> {
        self.account_id.as_ref()?;
        self.plan.map(PlanInfo::of)
    }
}

/// Lê `teste:<sub>:<plano>`. `None` se o formato não bate: prefixo diferente de `teste`, `sub`
/// vazio, número de partes diferente de 3. Plano desconhecido ou vazio não invalida (vira
/// `PlanId::None`, como o claim ausente em produção).
pub fn parse_test_token(raw: &str) -> Option<TestToken> {
    let mut partes = raw.splitn(3, ':');
    if partes.next()? != "teste" {
        return None;
    }
    let sub = partes.next()?;
    let plano = partes.next()?;
    if sub.is_empty() {
        return None;
    }
    Some(TestToken {
        sub: sub.to_string(),
        plan: PlanId::from_claim(Some(plano)),
    })
}

/// Conta de uma sessão nova (`POST /v1/sessoes`, depois de protocolo e senha).
/// - token válido: `account_id = sub`, `plan = plano do token`;
/// - sem token ou inválido e `requires_account`: `Err(401 "conta_necessaria")`;
/// - sem token ou inválido e sem `requires_account`: `AccountState::default()`.
///
/// `renew_count` do resultado é sempre 0 (quem chama preserva o do participante existente).
pub fn account_for_login(requires_account: bool, token: Option<&str>) -> Result<AccountState, HubError> {
    match token.and_then(parse_test_token) {
        Some(t) => Ok(AccountState {
            account_id: Some(t.sub),
            plan: Some(t.plan),
            renew_count: 0,
        }),
        None if requires_account => Err(HubError::new(
            401,
            "conta_necessaria",
            "Entre com sua conta para usar este servidor",
        )),
        None => Ok(AccountState::default()),
    }
}

/// Nova conta depois de um `POST /v1/sessoes/renovar` (semântica de `rest.md`).
/// - `token` ausente ou inválido: devolve `atual` clonado (mantém conta e plano);
/// - válido e `atual` sem conta: liga a conta (`account_id = sub`, `plan`);
/// - válido e mesmo `sub`: atualiza o plano;
/// - válido e `sub` diferente: `Err(403 "proibido")`.
///
/// Em todos os casos de sucesso `renew_count = atual.renew_count + 1`.
pub fn account_for_renew(atual: &AccountState, token: Option<&str>) -> Result<AccountState, HubError> {
    let mut novo = atual.clone();
    novo.renew_count = atual.renew_count.saturating_add(1);
    if let Some(t) = token.and_then(parse_test_token) {
        match &atual.account_id {
            Some(id) if *id != t.sub => {
                return Err(HubError::new(
                    403,
                    "proibido",
                    "Token de outra conta; abra uma sessão nova",
                ));
            }
            _ => {
                novo.account_id = Some(t.sub);
                novo.plan = Some(t.plan);
            }
        }
    }
    Ok(novo)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn conta(sub: &str, plan: PlanId, n: u32) -> AccountState {
        AccountState {
            account_id: Some(sub.into()),
            plan: Some(plan),
            renew_count: n,
        }
    }

    #[test]
    fn parse_validos() {
        for (txt, plano) in [
            ("teste:u1:fullhd", PlanId::FullHd),
            ("teste:u1:hd", PlanId::Hd),
            ("teste:u1:espectador", PlanId::Viewer),
            ("teste:u1:nenhum", PlanId::None),
        ] {
            let t = parse_test_token(txt).unwrap();
            assert_eq!(t.sub, "u1");
            assert_eq!(t.plan, plano, "{txt}");
        }
    }

    #[test]
    fn parse_plano_desconhecido_vira_none() {
        for txt in ["teste:u1:ouro", "teste:u1:", "teste:u1:hd:x"] {
            let t = parse_test_token(txt).unwrap();
            assert_eq!(t.plan, PlanId::None, "{txt}");
        }
    }

    #[test]
    fn parse_invalidos() {
        for txt in ["", "eyJabc", "teste:u1", "outro:u1:hd", "teste::hd"] {
            assert!(parse_test_token(txt).is_none(), "{txt}");
        }
    }

    #[test]
    fn plan_info_so_com_conta() {
        assert!(AccountState::default().plan_info().is_none());
        assert_eq!(conta("u1", PlanId::Hd, 0).plan_info(), Some(PlanInfo::of(PlanId::Hd)));
    }

    #[test]
    fn login_token_valido() {
        let a = account_for_login(false, Some("teste:u1:hd")).unwrap();
        assert_eq!(a, conta("u1", PlanId::Hd, 0));
    }

    #[test]
    fn login_sem_token_sem_exigir_conta() {
        assert_eq!(account_for_login(false, None).unwrap(), AccountState::default());
    }

    #[test]
    fn login_token_invalido_ignorado_sem_exigir() {
        assert_eq!(account_for_login(false, Some("lixo")).unwrap(), AccountState::default());
    }

    #[test]
    fn login_exige_conta() {
        for tok in [None, Some("lixo")] {
            let e = account_for_login(true, tok).unwrap_err();
            assert_eq!(e.status, 401);
            assert_eq!(e.code, "conta_necessaria");
        }
        assert!(account_for_login(true, Some("teste:u1:hd")).is_ok());
    }

    #[test]
    fn renew_sem_token_mantem_e_conta() {
        let atual = conta("u1", PlanId::Hd, 2);
        assert_eq!(account_for_renew(&atual, None).unwrap(), conta("u1", PlanId::Hd, 3));
    }

    #[test]
    fn renew_invalido_ignorado() {
        let atual = conta("u1", PlanId::Hd, 0);
        assert_eq!(
            account_for_renew(&atual, Some("lixo")).unwrap(),
            conta("u1", PlanId::Hd, 1)
        );
    }

    #[test]
    fn renew_atualiza_plano_mesma_conta() {
        let atual = conta("u1", PlanId::Hd, 0);
        let novo = account_for_renew(&atual, Some("teste:u1:fullhd")).unwrap();
        assert_eq!(novo, conta("u1", PlanId::FullHd, 1));
    }

    #[test]
    fn renew_liga_conta_em_sessao_sem_conta() {
        let novo = account_for_renew(&AccountState::default(), Some("teste:u2:hd")).unwrap();
        assert_eq!(novo, conta("u2", PlanId::Hd, 1));
    }

    #[test]
    fn renew_outra_conta_403_proibido() {
        let atual = conta("u1", PlanId::Hd, 4);
        let e = account_for_renew(&atual, Some("teste:u2:hd")).unwrap_err();
        assert_eq!(e.status, 403);
        assert_eq!(e.code, "proibido");
        assert_eq!(atual, conta("u1", PlanId::Hd, 4));
    }
}
