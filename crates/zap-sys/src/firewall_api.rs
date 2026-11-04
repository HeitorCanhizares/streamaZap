//! API pública do firewall: junta a coleta COM (Windows) com a interpretação pura de `firewall.rs`.

use std::path::Path;

use crate::firewall::{FirewallStatus, Resultado};
use crate::Result;

/// Lê o estado do firewall para o executável. Nunca entra em pânico: erro vai em `erro`.
/// Fora do Windows devolve `FirewallStatus::nao_suportado()`.
#[cfg(windows)]
pub fn status(exe: &Path) -> FirewallStatus {
    match crate::firewall_com::coletar() {
        Ok(c) => {
            let st = crate::firewall::interpretar(exe, &c);
            tracing::info!(
                "firewall lido: bloqueado={} liberado={} bloquear_tudo={:?}",
                st.bloqueado,
                st.liberado,
                st.bloquear_tudo_por_perfil
            );
            st
        }
        // Falha na leitura COM: o status sai como suportado, mas com o erro explicado.
        Err(e) => FirewallStatus {
            erro: Some(e.to_string()),
            suportado: true,
            ..FirewallStatus::nao_suportado()
        },
    }
}

/// Lê o estado do firewall para o executável. Fora do Windows não há firewall a consultar.
#[cfg(not(windows))]
pub fn status(_exe: &Path) -> FirewallStatus {
    FirewallStatus::nao_suportado()
}

/// Corrige o firewall NESTE processo (exige administrador). Fora do Windows: `NaoSuportado`.
#[cfg(windows)]
pub fn corrigir(exe: &Path) -> Result<Resultado> {
    let c = crate::firewall_com::coletar()?;
    let plano = crate::firewall::planejar_correcao(exe, &c);
    crate::firewall_com::aplicar(exe, &plano, plano.remover.len() as u32)
}

/// Corrige o firewall NESTE processo (exige administrador). Fora do Windows: `NaoSuportado`.
#[cfg(not(windows))]
pub fn corrigir(_exe: &Path) -> Result<Resultado> {
    Err(crate::ErroSys::NaoSuportado)
}

/// Corrige o firewall pedindo elevação: se já é admin chama `corrigir`; senão relança o app com
/// `--corrigir-firewall` via `elevate::relaunch_elevated` e lê o resultado do arquivo JSON.
pub fn corrigir_elevado(exe: &Path) -> Result<Resultado> {
    use crate::elevate;

    if elevate::is_elevated() {
        let r = corrigir(exe)?;
        return Ok(r);
    }
    // Apaga resultado antigo para não confundir com o da nova execução elevada.
    let _ = std::fs::remove_file(elevate::caminho_resultado());
    let args = vec![elevate::ARG_CORRIGIR_FIREWALL.to_string()];
    let codigo = elevate::relaunch_elevated(&args)?;
    match elevate::ler_resultado() {
        Some(r) => Ok(r),
        None => Ok(Resultado {
            ok: false,
            mensagem: format!("sem resultado (código {codigo}; permissão negada?)"),
            removidas: 0,
            perfis_liberados: vec![],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn status_fora_do_windows_nao_suportado() {
        assert_eq!(status(Path::new("/tmp/app")), FirewallStatus::nao_suportado());
    }

    #[cfg(not(windows))]
    #[test]
    fn corrigir_fora_do_windows_erra() {
        assert!(matches!(
            corrigir(Path::new("/tmp/app")),
            Err(crate::ErroSys::NaoSuportado)
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn corrigir_elevado_fora_do_windows_erra() {
        assert!(matches!(
            corrigir_elevado(Path::new("/tmp/app")),
            Err(crate::ErroSys::NaoSuportado)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn status_do_proprio_exe_nao_da_erro() {
        let exe = std::env::current_exe().unwrap();
        assert!(status(&exe).erro.is_none());
    }
}
