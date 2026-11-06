//! Lógica pura do updater (sem Tauri): tipos do evento `zap://update`, escolha do endpoint por
//! canal, detecção do placeholder da chave pública e as regras "nunca reiniciar numa sala".

use std::time::Duration;

use serde::Serialize;
use zap_core::{InstallMode, UpdateChannel};

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

/// Evento do aviso de instalação ("Atualizando para a vX em 30 s · Adiar 1 h").
pub const EVENT_UPDATE_AVISO: &str = "zap://update-aviso";
/// Evento "Atualizado para a vX" (emitido na abertura seguinte a uma instalação).
pub const EVENT_UPDATE_APLICADA: &str = "zap://update-aplicada";
/// Espera depois de abrir o app antes da primeira checagem.
pub const ESPERA_INICIAL: Duration = Duration::from_secs(10);
/// Intervalo entre checagens com o app aberto.
pub const INTERVALO_CHECAGEM: Duration = Duration::from_secs(6 * 3600);
/// Teto do sorteio somado ao intervalo (para não bater todo mundo no GitHub na mesma hora).
pub const SORTEIO_MAX: Duration = Duration::from_secs(5 * 60);
/// Nova tentativa depois de uma checagem que falhou.
pub const INTERVALO_FALHA: Duration = Duration::from_secs(30 * 60);
/// De quanto em quanto tempo se confere se já é seguro instalar.
pub const TICK_INSTALACAO: Duration = Duration::from_secs(30);
/// Tempo ocioso mínimo (sem teclado/mouse) para instalar com o app aberto e visível.
pub const OCIOSO_MIN: Duration = Duration::from_secs(120);
/// Duração do aviso antes de instalar.
pub const AVISO_DURACAO: Duration = Duration::from_secs(30);
/// Quanto o botão "Adiar" empurra a instalação.
pub const ADIAR: Duration = Duration::from_secs(3600);

/// Payload do evento `zap://update-aviso`.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AvisoInstalacao {
    pub versao: String,
    /// Segundos até instalar (30 ao abrir o aviso). `0` = fechar o aviso (adiado ou ficou inseguro).
    pub segundos: u32,
}

/// Fotografia do que importa para decidir se é seguro instalar agora.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextoInstalacao {
    pub em_sala: bool,
    /// Há transmissão minha em andamento (`snapshot.share` presente).
    pub transmitindo: bool,
    /// Há prévia de sala aberta na lista de salas.
    pub previa_aberta: bool,
    pub minimizado: bool,
    /// Tempo sem teclado nem mouse (`zap_core::idle_time()`).
    pub ocioso: Duration,
}

/// `true` quando é seguro instalar sozinho: fora de sala, sem transmitir, sem prévia aberta E
/// (app minimizado OU ocioso por pelo menos `OCIOSO_MIN`).
pub fn pode_instalar_agora(ctx: &ContextoInstalacao) -> bool {
    !ctx.em_sala && !ctx.transmitindo && !ctx.previa_aberta && (ctx.minimizado || ctx.ocioso >= OCIOSO_MIN)
}

/// O que o laço de instalação deve fazer neste tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passo {
    /// Nada a fazer (ou cancelar um aviso em curso se `aviso_em` estava preenchido).
    Esperar,
    /// Abrir o aviso de 30 s.
    Avisar,
    /// O aviso está aberto e ainda seguro: deixar a contagem correr, sem mexer no aviso.
    Contando,
    /// O aviso venceu sem resposta: instalar.
    Instalar,
}

/// Decide o próximo passo da instalação sozinha. Tempos (`adiado_ate`, `aviso_em`, `agora`) são
/// relógio monotônico desde a abertura do app.
///
/// - `modo != Auto` ou `fase != Pronta` -> `Esperar`;
/// - `adiado_ate > agora` -> `Esperar`;
/// - `!pode_instalar_agora(ctx)` -> `Esperar`;
/// - seguro e sem aviso (`aviso_em == None`) -> `Avisar`;
/// - seguro e `agora >= aviso_em + AVISO_DURACAO` -> `Instalar`; antes disso -> `Contando` (o
///   aviso continua aberto, sem reiniciar a contagem).
pub fn proximo_passo(
    modo: InstallMode,
    fase: Fase,
    ctx: &ContextoInstalacao,
    adiado_ate: Option<Duration>,
    aviso_em: Option<Duration>,
    agora: Duration,
) -> Passo {
    if modo != InstallMode::Auto || fase != Fase::Pronta {
        return Passo::Esperar;
    }
    // Adiamento vencido (`t <= agora`) não vale mais.
    if matches!(adiado_ate, Some(t) if t > agora) {
        return Passo::Esperar;
    }
    if !pode_instalar_agora(ctx) {
        return Passo::Esperar;
    }
    match aviso_em {
        None => Passo::Avisar,
        Some(t) if agora >= t + AVISO_DURACAO => Passo::Instalar,
        Some(_) => Passo::Contando,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_seguro() -> ContextoInstalacao {
        ContextoInstalacao {
            em_sala: false,
            transmitindo: false,
            previa_aberta: false,
            minimizado: true,
            ocioso: Duration::ZERO,
        }
    }

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn pode_instalar_tabela() {
        let casos: Vec<(ContextoInstalacao, bool)> = vec![
            (ctx_seguro(), true),
            (
                ContextoInstalacao {
                    minimizado: false,
                    ocioso: s(119),
                    ..ctx_seguro()
                },
                false,
            ),
            (
                ContextoInstalacao {
                    minimizado: false,
                    ocioso: s(120),
                    ..ctx_seguro()
                },
                true,
            ),
            (
                ContextoInstalacao {
                    minimizado: false,
                    ocioso: s(300),
                    ..ctx_seguro()
                },
                true,
            ),
            (
                ContextoInstalacao {
                    em_sala: true,
                    ..ctx_seguro()
                },
                false,
            ),
            (
                ContextoInstalacao {
                    transmitindo: true,
                    ..ctx_seguro()
                },
                false,
            ),
            (
                ContextoInstalacao {
                    previa_aberta: true,
                    ..ctx_seguro()
                },
                false,
            ),
            (
                ContextoInstalacao {
                    em_sala: true,
                    minimizado: false,
                    ocioso: s(300),
                    ..ctx_seguro()
                },
                false,
            ),
            (
                ContextoInstalacao {
                    minimizado: false,
                    ocioso: s(0),
                    ..ctx_seguro()
                },
                false,
            ),
        ];
        for (i, (ctx, esperado)) in casos.iter().enumerate() {
            assert_eq!(pode_instalar_agora(ctx), *esperado, "caso {i}: {ctx:?}");
        }
    }

    #[test]
    fn passo_so_no_modo_auto() {
        let c = ctx_seguro();
        for modo in [InstallMode::Ask, InstallMode::OnExit] {
            assert_eq!(
                proximo_passo(modo, Fase::Pronta, &c, None, None, s(100)),
                Passo::Esperar
            );
        }
        assert_eq!(
            proximo_passo(InstallMode::Auto, Fase::Pronta, &c, None, None, s(100)),
            Passo::Avisar
        );
    }

    #[test]
    fn passo_exige_fase_pronta() {
        let c = ctx_seguro();
        for fase in [Fase::Nada, Fase::Baixando, Fase::Instalando, Fase::Erro] {
            assert_eq!(
                proximo_passo(InstallMode::Auto, fase, &c, None, None, s(100)),
                Passo::Esperar,
                "fase {fase:?}"
            );
        }
        assert_eq!(
            proximo_passo(InstallMode::Auto, Fase::Pronta, &c, None, None, s(100)),
            Passo::Avisar
        );
    }

    #[test]
    fn passo_respeita_adiar() {
        let c = ctx_seguro();
        let adiado = Some(s(3600));
        let passo = |agora| proximo_passo(InstallMode::Auto, Fase::Pronta, &c, adiado, None, agora);
        assert_eq!(passo(s(100)), Passo::Esperar);
        assert_eq!(passo(s(3600)), Passo::Avisar);
        assert_eq!(passo(s(4000)), Passo::Avisar);
    }

    #[test]
    fn passo_inseguro_cancela() {
        let c = ContextoInstalacao {
            em_sala: true,
            ..ctx_seguro()
        };
        assert_eq!(
            proximo_passo(InstallMode::Auto, Fase::Pronta, &c, None, Some(s(10)), s(45)),
            Passo::Esperar
        );
    }

    #[test]
    fn passo_aviso_30s() {
        let c = ctx_seguro();
        let passo = |agora| proximo_passo(InstallMode::Auto, Fase::Pronta, &c, None, Some(s(100)), agora);
        assert_eq!(passo(s(100)), Passo::Contando);
        assert_eq!(passo(s(129)), Passo::Contando);
        assert_eq!(passo(s(130)), Passo::Instalar);
        assert_eq!(passo(s(500)), Passo::Instalar);
    }

    #[test]
    fn passo_sem_aviso_avisa_antes_de_instalar() {
        assert_eq!(
            proximo_passo(InstallMode::Auto, Fase::Pronta, &ctx_seguro(), None, None, s(1000)),
            Passo::Avisar
        );
    }

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
