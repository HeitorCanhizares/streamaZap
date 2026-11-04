//! Integração do StreamaZap com o Windows: regras do firewall (COM `INetFwPolicy2`), elevação
//! (`runas`), interfaces de rede, Radmin VPN, outras VPNs e antivírus com firewall próprio.
//! Porte de `streamazap/firewall.py` e `streamazap/netutil.py`.
//!
//! Em outros sistemas as funções que dependem do Windows são stubs (`suportado == false`),
//! e a lógica de interpretação (pura) é testável em qualquer sistema.

pub mod antivirus;
pub mod elevate;
pub mod firewall;
mod firewall_api;
#[cfg(windows)]
mod firewall_com;
pub mod net;
pub mod vpn;

pub use firewall::{Acao, Coleta, Direcao, FirewallStatus, PerfilBruto, RegraBruta, RegraResumo, Resultado};
pub use firewall_api::{corrigir, corrigir_elevado, status};
pub use net::Interface;

/// Erro das operações de sistema.
#[derive(Debug, thiserror::Error)]
pub enum ErroSys {
    #[error("não suportado neste sistema")]
    NaoSuportado,
    #[error("é preciso executar como administrador")]
    SemPermissao,
    #[error("{0}")]
    Outro(String),
}

pub type Result<T> = std::result::Result<T, ErroSys>;
