//! Diagnóstico e correção de firewall.

use std::sync::Arc;

use tauri::State;
use zap_core::{Core, DiagReport};

use crate::state::AppState;

/// Executável atual (`std::env::current_exe`); erro vira `String`.
pub fn exe_atual() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Não foi possível achar o executável: {e}"))
}

/// Roda `core.diagnostics(&exe)` numa thread de bloqueio (a coleta usa COM/WMI).
async fn coletar_relatorio(core: Arc<Core>) -> Result<DiagReport, String> {
    let exe = exe_atual()?;
    tauri::async_runtime::spawn_blocking(move || core.diagnostics(&exe))
        .await
        .map_err(|e| format!("Falha ao coletar o diagnóstico: {e}"))
}

/// Relatório completo: `core.diagnostics(&exe)` em `spawn_blocking` (coleta usa COM/WMI).
#[tauri::command]
pub async fn diagnostics_run(state: State<'_, Arc<AppState>>) -> Result<DiagReport, String> {
    coletar_relatorio(state.core.clone()).await
}

/// Corrige o firewall: `zap_sys::corrigir_elevado(exe)` em `spawn_blocking`; devolve o texto de
/// `Resultado.mensagem` (ou erro em pt-BR; UAC cancelado -> mensagem clara).
#[tauri::command]
pub async fn firewall_fix() -> Result<zap_sys::Resultado, String> {
    let exe = exe_atual()?;
    // Relança o próprio exe elevado (UAC); `corrigir` direto exigiria admin já no processo.
    tauri::async_runtime::spawn_blocking(move || zap_sys::corrigir_elevado(&exe))
        .await
        .map_err(|e| format!("Falha ao corrigir o firewall: {e}"))?
        .map_err(|e| erro_corrigir(&e))
}

/// Mensagem em pt-BR para cada erro de `corrigir_elevado`.
fn erro_corrigir(e: &zap_sys::ErroSys) -> String {
    match e {
        zap_sys::ErroSys::SemPermissao => {
            "A correção do firewall precisa de permissão de administrador e foi cancelada.".into()
        }
        zap_sys::ErroSys::NaoSuportado => "Corrigir o firewall só é possível no Windows.".into(),
        zap_sys::ErroSys::Outro(m) => m.clone(),
    }
}

/// Texto do relatório para "Copiar relatório" (`zap_core::diag_text::relatorio_texto`).
#[tauri::command]
pub async fn diagnostics_text(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let relatorio = coletar_relatorio(state.core.clone()).await?;
    Ok(zap_core::diag_text::relatorio_texto(&relatorio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erro_corrigir_mensagens() {
        use zap_sys::ErroSys;
        assert_eq!(
            erro_corrigir(&ErroSys::SemPermissao),
            "A correção do firewall precisa de permissão de administrador e foi cancelada."
        );
        assert_eq!(
            erro_corrigir(&ErroSys::NaoSuportado),
            "Corrigir o firewall só é possível no Windows."
        );
        assert_eq!(erro_corrigir(&ErroSys::Outro("x".into())), "x");
    }

    #[test]
    fn exe_atual_devolve_caminho_existente() {
        let caminho = exe_atual().unwrap();
        assert!(caminho.exists(), "caminho do executável não existe: {caminho:?}");
    }
}
