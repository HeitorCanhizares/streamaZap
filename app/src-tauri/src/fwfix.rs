//! Modo `--corrigir-firewall`: processo elevado, sem janela, que corrige o firewall e grava o
//! resultado em arquivo para o app principal ler.

use zap_sys::elevate::{escrever_resultado, ARG_CORRIGIR_FIREWALL};
use zap_sys::Resultado;

/// `true` se a linha de comando contém `zap_sys::elevate::ARG_CORRIGIR_FIREWALL`.
pub fn pedido(args: &[String]) -> bool {
    // `args[0]` é o caminho do executável; não conta como argumento.
    args.iter().skip(1).any(|a| a.as_str() == ARG_CORRIGIR_FIREWALL)
}

/// Executa a correção (`zap_sys::corrigir(exe)`), grava `zap_sys::elevate::escrever_resultado`
/// (em erro grava `Resultado { ok: false, mensagem, .. }`) e devolve o código de saída
/// (0 = ok, 1 = falha).
pub fn executar() -> i32 {
    let (resultado, codigo) = match std::env::current_exe() {
        Ok(exe) => resultado_de(zap_sys::corrigir(&exe)),
        Err(e) => (falha(format!("Não foi possível achar o executável: {e}")), 1),
    };
    // Sem o arquivo, o app principal só vê "sem resultado"; não há como reportar mais que isso.
    if let Err(e) = escrever_resultado(&resultado) {
        tracing::warn!("falha ao gravar o resultado do firewall: {e}");
    }
    codigo
}

/// Converte o retorno de `zap_sys::corrigir` no `Resultado` a gravar e no código de saída.
/// Função pura, para teste.
pub fn resultado_de(r: zap_sys::Result<Resultado>) -> (Resultado, i32) {
    let resultado = match r {
        Ok(r) => r,
        Err(e) => falha(e.to_string()),
    };
    let codigo = if resultado.ok { 0 } else { 1 };
    (resultado, codigo)
}

/// `Resultado` de falha, sem regras removidas nem perfis liberados.
fn falha(mensagem: String) -> Resultado {
    Resultado {
        ok: false,
        mensagem,
        removidas: 0,
        perfis_liberados: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_sys::ErroSys;

    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pedido_sem_flag_e_falso() {
        assert!(!pedido(&sv(&["x"])));
    }

    #[test]
    fn pedido_com_flag_e_verdadeiro() {
        assert!(pedido(&sv(&["x", "--corrigir-firewall"])));
    }

    #[test]
    fn pedido_ignora_primeiro_argumento() {
        assert!(!pedido(&sv(&["--corrigir-firewall"])));
    }

    #[test]
    fn resultado_ok_da_codigo_zero() {
        let r = Resultado {
            ok: true,
            mensagem: "ok".into(),
            removidas: 2,
            perfis_liberados: vec![],
        };
        let (out, codigo) = resultado_de(Ok(r.clone()));
        assert_eq!(out, r);
        assert_eq!(codigo, 0);
    }

    #[test]
    fn resultado_nao_ok_da_codigo_um() {
        let r = Resultado {
            ok: false,
            mensagem: "não".into(),
            removidas: 0,
            perfis_liberados: vec![],
        };
        let (_, codigo) = resultado_de(Ok(r));
        assert_eq!(codigo, 1);
    }

    #[test]
    fn erro_vira_falha_com_mensagem() {
        let (out, codigo) = resultado_de(Err(ErroSys::SemPermissao));
        assert!(!out.ok);
        assert!(!out.mensagem.is_empty());
        assert_eq!(out.removidas, 0);
        assert!(out.perfis_liberados.is_empty());
        assert_eq!(codigo, 1);
    }
}
