//! Planos do modo Online: o que cada conta pode fazer (`docs/planos/contas-planos-camera.md`, §2,
//! e `docs/api/rest.md`, `POST /v1/sessoes`).
//!
//! Os números ficam só aqui e valem para o app e para o servidor. Os limites valem **só no
//! Online** e só para sessões com conta: no Local, e no Online sem conta, nada é limitado por plano.
//! Assistir não depende do plano: o espectador recebe o que quem transmite manda, até o teto de
//! banda [`VIEWER_MAX_KBPS`], igual para todos.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{LayerSpec, TrackKind};

/// Teto de banda por espectador na ponte do Online (kbps), para todos: o servidor usa
/// `min(relatorio.banda_kbps, VIEWER_MAX_KBPS)` antes do alocador e da sondagem.
pub const VIEWER_MAX_KBPS: u32 = 12_000;

/// Plano da conta. No fio: `"nenhum" | "espectador" | "hd" | "fullhd"` (claim `plano` do JWT e
/// `plano.id` da sessão).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanId {
    /// Conta sem plano: vê a lista de salas e a prévia, não entra nem transmite.
    #[default]
    #[serde(rename = "nenhum")]
    None,
    /// Espectador: entra e assiste a tudo, não transmite.
    #[serde(rename = "espectador")]
    Viewer,
    /// Transmissor HD: até 720p60.
    #[serde(rename = "hd")]
    Hd,
    /// Transmissor Full HD: até 1080p60.
    #[serde(rename = "fullhd")]
    FullHd,
}

impl PlanId {
    /// Texto do fio (`"nenhum"`, `"espectador"`, `"hd"`, `"fullhd"`).
    pub fn as_str(self) -> &'static str {
        match self {
            PlanId::None => "nenhum",
            PlanId::Viewer => "espectador",
            PlanId::Hd => "hd",
            PlanId::FullHd => "fullhd",
        }
    }

    /// Lê o claim `plano` do JWT. Claim ausente ou desconhecido = [`PlanId::None`].
    pub fn from_claim(claim: Option<&str>) -> PlanId {
        match claim {
            Some("espectador") => PlanId::Viewer,
            Some("hd") => PlanId::Hd,
            Some("fullhd") => PlanId::FullHd,
            _ => PlanId::None,
        }
    }
}

/// Teto da camada 0 de cada trilha para quem transmite. Altura = lado menor do quadro (uma janela
/// em pé ou uma tela ultralarga contam pelo lado menor).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishLimit {
    #[serde(rename = "altura_max")]
    pub max_height: u16,
    #[serde(rename = "fps_max")]
    pub max_fps: u8,
    /// Tela e janela.
    #[serde(rename = "tela_kbps")]
    pub screen_kbps: u32,
    #[serde(rename = "camera_kbps")]
    pub camera_kbps: u32,
}

/// O que um plano permite no Online.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Criar e entrar em sala Online.
    #[serde(rename = "pode_entrar")]
    pub can_join: bool,
    /// `None` = não transmite no Online (nem tela, nem câmera, nem só o som).
    #[serde(rename = "transmitir", default)]
    pub publish: Option<PublishLimit>,
}

/// Plano no formato da resposta de `POST /v1/sessoes`:
/// `{"id": "hd", "pode_entrar": true, "transmitir": {...} | null}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInfo {
    pub id: PlanId,
    #[serde(flatten)]
    pub limits: PlanLimits,
}

impl PlanInfo {
    /// O plano com os limites de [`limits`].
    pub fn of(id: PlanId) -> PlanInfo {
        PlanInfo { id, limits: limits(id) }
    }
}

/// Transmissor HD: 720p60; tela 4000 kbps, câmera 2500.
const HD: PublishLimit = PublishLimit {
    max_height: 720,
    max_fps: 60,
    screen_kbps: 4_000,
    camera_kbps: 2_500,
};

/// Transmissor Full HD: 1080p60; tela 8000 kbps, câmera 4500.
const FULL_HD: PublishLimit = PublishLimit {
    max_height: 1080,
    max_fps: 60,
    screen_kbps: 8_000,
    camera_kbps: 4_500,
};

/// Limites de cada plano (tabela da §2 do plano).
pub fn limits(id: PlanId) -> PlanLimits {
    match id {
        PlanId::None => PlanLimits {
            can_join: false,
            publish: None,
        },
        PlanId::Viewer => PlanLimits {
            can_join: true,
            publish: None,
        },
        PlanId::Hd => PlanLimits {
            can_join: true,
            publish: Some(HD),
        },
        PlanId::FullHd => PlanLimits {
            can_join: true,
            publish: Some(FULL_HD),
        },
    }
}

/// Por que uma publicação não cabe no plano. Vira o erro `plano_insuficiente` (`campo: "camadas"`).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PlanError {
    #[error("o plano não permite transmitir")]
    CannotPublish,
    #[error("a camada {layer} passa do plano ({what})")]
    AboveLimit { layer: u8, what: &'static str },
}

impl PlanError {
    /// O `campo` do erro `plano_insuficiente`.
    pub fn field(&self) -> &'static str {
        "camadas"
    }
}

/// Confere se uma trilha cabe no plano. Áudio só pede plano de transmissor; vídeo (tela, janela ou
/// câmera) pede que **toda** camada caiba no teto (numa escada honesta, isso é a camada 0 caber).
pub fn check_publish(kind: TrackKind, layers: &[LayerSpec], limits: &PlanLimits) -> Result<(), PlanError> {
    let Some(limit) = limits.publish else {
        return Err(PlanError::CannotPublish);
    };
    let max_kbps = match kind {
        TrackKind::Audio => return Ok(()),
        TrackKind::Screen | TrackKind::Window => limit.screen_kbps,
        TrackKind::Camera => limit.camera_kbps,
    };
    for l in layers {
        let what = if l.width.min(l.height) > limit.max_height {
            "altura"
        } else if l.fps > limit.max_fps {
            "fps"
        } else if l.kbps > max_kbps {
            "kbps"
        } else {
            continue;
        };
        return Err(PlanError::AboveLimit { layer: l.layer, what });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn camada(layer: u8, width: u16, height: u16, fps: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer,
            width,
            height,
            fps,
            kbps,
        }
    }

    /// Escada de tela do app (`balanceamento.md` §1) com a camada 0 dada.
    fn tela(width: u16, height: u16, fps: u8, kbps: u32) -> Vec<LayerSpec> {
        vec![camada(0, width, height, fps, kbps), camada(2, 640, 360, 15, 400)]
    }

    #[test]
    fn texto_do_fio() {
        for (id, texto) in [
            (PlanId::None, "nenhum"),
            (PlanId::Viewer, "espectador"),
            (PlanId::Hd, "hd"),
            (PlanId::FullHd, "fullhd"),
        ] {
            assert_eq!(serde_json::to_value(id).unwrap(), json!(texto));
            assert_eq!(serde_json::from_value::<PlanId>(json!(texto)).unwrap(), id);
            assert_eq!(id.as_str(), texto);
            assert_eq!(PlanId::from_claim(Some(texto)), id);
        }
    }

    #[test]
    fn claim_ausente_ou_desconhecido_e_nenhum() {
        assert_eq!(PlanId::from_claim(None), PlanId::None);
        assert_eq!(PlanId::from_claim(Some("ouro")), PlanId::None);
        assert_eq!(PlanId::from_claim(Some("")), PlanId::None);
    }

    #[test]
    fn tabela_de_limites() {
        assert!(!limits(PlanId::None).can_join);
        assert_eq!(limits(PlanId::None).publish, None);
        assert!(limits(PlanId::Viewer).can_join);
        assert_eq!(limits(PlanId::Viewer).publish, None);
        assert_eq!(limits(PlanId::Hd).publish, Some(HD));
        assert_eq!(limits(PlanId::FullHd).publish, Some(FULL_HD));
        assert_eq!(VIEWER_MAX_KBPS, 12_000);
    }

    #[test]
    fn plan_info_no_fio() {
        let hd = serde_json::to_value(PlanInfo::of(PlanId::Hd)).unwrap();
        assert_eq!(
            hd,
            json!({
                "id": "hd",
                "pode_entrar": true,
                "transmitir": {"altura_max": 720, "fps_max": 60, "tela_kbps": 4000, "camera_kbps": 2500}
            })
        );
        let esp = serde_json::to_value(PlanInfo::of(PlanId::Viewer)).unwrap();
        assert_eq!(
            esp,
            json!({"id": "espectador", "pode_entrar": true, "transmitir": null})
        );
        for id in [PlanId::None, PlanId::Viewer, PlanId::Hd, PlanId::FullHd] {
            let v = serde_json::to_value(PlanInfo::of(id)).unwrap();
            assert_eq!(serde_json::from_value::<PlanInfo>(v).unwrap(), PlanInfo::of(id));
        }
        // `transmitir` ausente = não transmite.
        let sem: PlanInfo = serde_json::from_value(json!({"id": "espectador", "pode_entrar": true})).unwrap();
        assert_eq!(sem.limits.publish, None);
    }

    #[test]
    fn sem_plano_de_transmissor_nao_publica_nada() {
        for id in [PlanId::None, PlanId::Viewer] {
            let l = limits(id);
            assert_eq!(check_publish(TrackKind::Audio, &[], &l), Err(PlanError::CannotPublish));
            assert_eq!(
                check_publish(TrackKind::Camera, &[camada(0, 640, 360, 15, 300)], &l),
                Err(PlanError::CannotPublish)
            );
        }
    }

    #[test]
    fn transmissor_publica_so_o_som() {
        assert_eq!(check_publish(TrackKind::Audio, &[], &limits(PlanId::Hd)), Ok(()));
    }

    #[test]
    fn hd_aceita_720p60_e_recusa_1080p() {
        let hd = limits(PlanId::Hd);
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(1280, 720, 60, 4000), &hd),
            Ok(())
        );
        assert_eq!(
            check_publish(TrackKind::Window, &tela(1280, 720, 30, 2500), &hd),
            Ok(())
        );
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(1920, 1080, 60, 8000), &hd),
            Err(PlanError::AboveLimit {
                layer: 0,
                what: "altura"
            })
        );
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(1280, 720, 60, 4001), &hd),
            Err(PlanError::AboveLimit { layer: 0, what: "kbps" })
        );
        // Câmera tem o próprio teto de kbps.
        let cam720p60 = vec![camada(0, 1280, 720, 60, 2500), camada(1, 1280, 720, 30, 1500)];
        assert_eq!(check_publish(TrackKind::Camera, &cam720p60, &hd), Ok(()));
        let cam_cara = vec![camada(0, 1280, 720, 60, 4000)];
        assert_eq!(
            check_publish(TrackKind::Camera, &cam_cara, &hd),
            Err(PlanError::AboveLimit { layer: 0, what: "kbps" })
        );
    }

    #[test]
    fn fullhd_aceita_1080p60_e_recusa_acima() {
        let full = limits(PlanId::FullHd);
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(1920, 1080, 60, 8000), &full),
            Ok(())
        );
        let cam1080p60 = vec![camada(0, 1920, 1080, 60, 4500), camada(1, 1280, 720, 30, 1500)];
        assert_eq!(check_publish(TrackKind::Camera, &cam1080p60, &full), Ok(()));
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(2560, 1440, 60, 8000), &full),
            Err(PlanError::AboveLimit {
                layer: 0,
                what: "altura"
            })
        );
        assert_eq!(
            check_publish(TrackKind::Screen, &tela(1920, 1080, 120, 8000), &full),
            Err(PlanError::AboveLimit { layer: 0, what: "fps" })
        );
    }

    #[test]
    fn altura_e_o_lado_menor() {
        let hd = limits(PlanId::Hd);
        // Janela em pé de 720 de largura: cabe no HD.
        assert_eq!(
            check_publish(TrackKind::Window, &[camada(0, 720, 1280, 30, 2500)], &hd),
            Ok(())
        );
        // Tela ultralarga em 1080p: cabe no Full HD.
        assert_eq!(
            check_publish(
                TrackKind::Screen,
                &[camada(0, 2520, 1080, 60, 8000)],
                &limits(PlanId::FullHd)
            ),
            Ok(())
        );
    }

    #[test]
    fn camada_escondida_acima_do_plano_tambem_e_recusada() {
        // App modificado não esconde uma camada grande atrás de uma camada 0 pequena.
        let escada = vec![camada(0, 1280, 720, 30, 2500), camada(1, 1920, 1080, 60, 8000)];
        assert_eq!(
            check_publish(TrackKind::Screen, &escada, &limits(PlanId::Hd)),
            Err(PlanError::AboveLimit {
                layer: 1,
                what: "altura"
            })
        );
        assert_eq!(PlanError::CannotPublish.field(), "camadas");
    }
}
