//! Lógica pura do updater (sem Tauri): tipos do evento `zap://update`, escolha do endpoint por
//! canal, detecção do placeholder da chave pública e as regras "nunca reiniciar numa sala".

use serde::Serialize;
use zap_core::UpdateChannel;

/// Nome do evento de estado da atualização.
pub const EVENT_UPDATE: &str = "zap://update";
/// Valor de `plugins.updater.pubkey` enquanto o dono não colou a chave real.
pub const PUBKEY_PLACEHOLDER: &str = "COLE_AQUI_A_CHAVE_PUBLICA";
/// Endpoint do canal estável.
pub const ENDPOINT_ESTAVEL: &str =
    "https://github.com/HeitorCanhizares/streamaZap/releases/latest/download/latest.json";
/// Endpoint do canal beta (arquivo anexado à Release fixa `canal-beta`).
pub const ENDPOINT_BETA: &str =
    "https://github.com/HeitorCanhizares/streamaZap/releases/download/canal-beta/latest-beta.json";

/// Fase da atualização (serializa em minúsculas: nada, baixando, pronta, instalando, erro).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Fase {
    Nada,
    Baixando,
    Pronta,
    Instalando,
    Erro,
}

/// Payload do evento `zap://update`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct EstadoUpdate {
    pub fase: Fase,
    /// 0 a 100.
    pub progresso: u8,
    pub versao: Option<String>,
    pub erro: Option<String>,
}

/// Resposta de `update_check`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub disponivel: bool,
    pub versao: Option<String>,
    pub notas: Option<String>,
    pub tamanho: Option<u64>,
}

impl EstadoUpdate {
    pub fn nada() -> Self {
        Self {
            fase: Fase::Nada,
            progresso: 0,
            versao: None,
            erro: None,
        }
    }
    pub fn baixando(progresso: u8, versao: &str) -> Self {
        // Limita em 100 para o front nunca receber barra além do total.
        Self {
            fase: Fase::Baixando,
            progresso: progresso.min(100),
            versao: Some(versao.to_string()),
            erro: None,
        }
    }
    pub fn pronta(versao: &str) -> Self {
        Self {
            fase: Fase::Pronta,
            progresso: 100,
            versao: Some(versao.to_string()),
            erro: None,
        }
    }
    pub fn instalando(versao: &str) -> Self {
        Self {
            fase: Fase::Instalando,
            progresso: 100,
            versao: Some(versao.to_string()),
            erro: None,
        }
    }
    pub fn erro(msg: &str) -> Self {
        Self {
            fase: Fase::Erro,
            progresso: 0,
            versao: None,
            erro: Some(msg.to_string()),
        }
    }
}

/// Endpoint do canal escolhido.
pub fn endpoint_do_canal(canal: UpdateChannel) -> &'static str {
    match canal {
        UpdateChannel::Estavel => ENDPOINT_ESTAVEL,
        UpdateChannel::Beta => ENDPOINT_BETA,
    }
}

/// `true` se a chave pública ainda é o placeholder (ou vazia): atualização não configurada.
pub fn pubkey_configurada(pubkey: &str) -> bool {
    let t = pubkey.trim();
    !t.is_empty() && t != PUBKEY_PLACEHOLDER
}

/// Porcentagem 0–100 dado bytes baixados e total (total desconhecido/0 -> 0; limita em 100).
pub fn percentual(baixado: u64, total: Option<u64>) -> u8 {
    match total {
        None | Some(0) => 0,
        // u128 evita estouro em baixado * 100 quando os valores são enormes.
        Some(t) => {
            let p = (u128::from(baixado) * 100) / u128::from(t);
            p.min(100) as u8
        }
    }
}

/// Erro (pt-BR) de `update_install_now` quando há sala ativa; `Ok(())` fora de sala.
pub fn checar_instalar_agora(em_sala: bool) -> Result<(), String> {
    if em_sala {
        Err("Saia da sala antes de reiniciar para atualizar.".to_string())
    } else {
        Ok(())
    }
}

/// Instalar ao fechar só vale se a atualização está pronta (`Fase::Pronta`) e foi marcada.
pub fn deve_instalar_ao_sair(fase: Fase, marcada: bool) -> bool {
    marcada && fase == Fase::Pronta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construtores_preenchem_campos() {
        assert_eq!(
            EstadoUpdate::nada(),
            EstadoUpdate {
                fase: Fase::Nada,
                progresso: 0,
                versao: None,
                erro: None
            }
        );
        assert_eq!(
            EstadoUpdate::baixando(40, "1.2.3"),
            EstadoUpdate {
                fase: Fase::Baixando,
                progresso: 40,
                versao: Some("1.2.3".into()),
                erro: None
            }
        );
        assert_eq!(
            EstadoUpdate::pronta("1.0"),
            EstadoUpdate {
                fase: Fase::Pronta,
                progresso: 100,
                versao: Some("1.0".into()),
                erro: None
            }
        );
        assert_eq!(
            EstadoUpdate::instalando("1.0"),
            EstadoUpdate {
                fase: Fase::Instalando,
                progresso: 100,
                versao: Some("1.0".into()),
                erro: None
            }
        );
        assert_eq!(
            EstadoUpdate::erro("falhou"),
            EstadoUpdate {
                fase: Fase::Erro,
                progresso: 0,
                versao: None,
                erro: Some("falhou".into())
            }
        );
    }

    #[test]
    fn baixando_limita_progresso_em_100() {
        assert_eq!(EstadoUpdate::baixando(100, "1").progresso, 100);
        assert_eq!(EstadoUpdate::baixando(255, "1").progresso, 100);
    }

    #[test]
    fn endpoint_por_canal() {
        assert_eq!(endpoint_do_canal(UpdateChannel::Estavel), ENDPOINT_ESTAVEL);
        assert_eq!(endpoint_do_canal(UpdateChannel::Beta), ENDPOINT_BETA);
        assert_ne!(ENDPOINT_ESTAVEL, ENDPOINT_BETA);
    }

    #[test]
    fn pubkey_placeholder_e_vazia_nao_configuradas() {
        assert!(!pubkey_configurada(""));
        assert!(!pubkey_configurada("   "));
        assert!(!pubkey_configurada(PUBKEY_PLACEHOLDER));
        assert!(!pubkey_configurada(&format!("  {PUBKEY_PLACEHOLDER}\n")));
    }

    #[test]
    fn pubkey_real_configurada() {
        assert!(pubkey_configurada("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmtleQ=="));
    }

    #[test]
    fn percentual_casos() {
        assert_eq!(percentual(0, None), 0);
        assert_eq!(percentual(50, Some(0)), 0);
        assert_eq!(percentual(50, Some(100)), 50);
        assert_eq!(percentual(200, Some(100)), 100);
        assert_eq!(percentual(u64::MAX, Some(u64::MAX)), 100);
    }

    #[test]
    fn instalar_agora_recusa_em_sala() {
        assert_eq!(
            checar_instalar_agora(true),
            Err("Saia da sala antes de reiniciar para atualizar.".to_string())
        );
        assert_eq!(checar_instalar_agora(false), Ok(()));
    }

    #[test]
    fn instalar_ao_sair_so_pronta_e_marcada() {
        assert!(deve_instalar_ao_sair(Fase::Pronta, true));
        assert!(!deve_instalar_ao_sair(Fase::Pronta, false));
        assert!(!deve_instalar_ao_sair(Fase::Baixando, true));
        assert!(!deve_instalar_ao_sair(Fase::Nada, true));
        assert!(!deve_instalar_ao_sair(Fase::Instalando, true));
        assert!(!deve_instalar_ao_sair(Fase::Erro, true));
    }

    #[test]
    fn serializa_json_fase_minuscula() {
        let v = serde_json::to_value(EstadoUpdate::pronta("1.0")).expect("serializa");
        assert_eq!(v["fase"], "pronta");
        assert_eq!(v["progresso"], 100);
        assert_eq!(v["versao"], "1.0");
    }
}
