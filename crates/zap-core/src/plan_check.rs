//! Conferências do plano (parte de `plan_gate`, reexportada de lá): `best_allowed`,
//! `check_layers`, `check_join` e `error_from_server`. Puro: sem I/O.
use zap_proto::plan::{check_publish, PlanError, PlanId, PlanInfo, PlanLimits};
use zap_proto::{LayerSpec, TrackKind};

use crate::plan_gate::{
    preset_allowed, PlanAction, PlanRefusal, MSG_NAO_ENTRA, MSG_NAO_TRANSMITE, MSG_QUALIDADE, PRESETS,
};
use crate::types::Preset;

/// O maior preset permitido com altura e fps <= os do pedido (ordem: altura, depois fps). Nenhum
/// permitido: devolve o próprio pedido (a conferência recusa depois).
pub fn best_allowed(kind: TrackKind, preset: Preset, l: &PlanLimits) -> Preset {
    let (h, f) = preset.height_fps();
    PRESETS
        .iter()
        .copied()
        .filter(|p| {
            let (ph, pf) = p.height_fps();
            ph <= h && pf <= f
        })
        .filter(|&p| preset_allowed(kind, p, l))
        .max_by_key(|p| p.height_fps())
        .unwrap_or(preset)
}

/// `check_publish` → `PlanRefusal { action: Publish }`: `CannotPublish` → field `"tipo"` e
/// `MSG_NAO_TRANSMITE`; `AboveLimit` → field `"camadas"` e `MSG_QUALIDADE`.
pub fn check_layers(kind: TrackKind, layers: &[LayerSpec], plan: &PlanInfo) -> Result<(), PlanRefusal> {
    check_publish(kind, layers, &plan.limits).map_err(|e| PlanRefusal {
        plan: plan.id,
        action: PlanAction::Publish,
        field: Some(e.field().into()),
        message: match e {
            PlanError::CannotPublish => MSG_NAO_TRANSMITE,
            PlanError::AboveLimit { .. } => MSG_QUALIDADE,
        }
        .into(),
    })
}

/// `can_join` falso → `PlanRefusal { action: Join, field: None, message: MSG_NAO_ENTRA }`.
pub fn check_join(plan: &PlanInfo) -> Result<(), PlanRefusal> {
    if plan.limits.can_join {
        return Ok(());
    }
    Err(PlanRefusal {
        plan: plan.id,
        action: PlanAction::Join,
        field: None,
        message: MSG_NAO_ENTRA.into(),
    })
}

/// Erro de pedido vindo do servidor (`erro` do WebSocket): `plano_insuficiente` →
/// `CoreError::Plan` (plano do `erro`, senão `fallback`, senão `PlanId::None`; a mensagem do
/// servidor); qualquer outro código → `CoreError::Server { code, message }`.
pub fn error_from_server(
    code: &str,
    message: &str,
    plan: Option<PlanId>,
    field: Option<String>,
    fallback: Option<PlanId>,
    action: PlanAction,
) -> crate::CoreError {
    if code == "plano_insuficiente" {
        return crate::CoreError::Plan(PlanRefusal {
            plan: plan.or(fallback).unwrap_or_default(),
            action,
            field,
            message: message.to_string(),
        });
    }
    crate::CoreError::Server {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_gate::nominal_ladder;

    #[test]
    fn best_allowed_exemplos() {
        let hd = zap_proto::plan::limits(PlanId::Hd);
        assert_eq!(best_allowed(TrackKind::Camera, Preset::P1080p60, &hd), Preset::P720p60);
        assert_eq!(best_allowed(TrackKind::Camera, Preset::P1080p30, &hd), Preset::P720p30);
        assert_eq!(best_allowed(TrackKind::Camera, Preset::P720p30, &hd), Preset::P720p30);
        let v = zap_proto::plan::limits(PlanId::Viewer);
        assert_eq!(best_allowed(TrackKind::Camera, Preset::P1080p60, &v), Preset::P1080p60);
    }

    #[test]
    fn conferencias_de_camadas_e_entrada() {
        let e = check_layers(TrackKind::Audio, &[], &PlanInfo::of(PlanId::Viewer)).unwrap_err();
        assert_eq!(e.field.as_deref(), Some("tipo"));
        assert_eq!(e.message, MSG_NAO_TRANSMITE);
        assert_eq!(e.plan, PlanId::Viewer);
        assert_eq!(e.action, PlanAction::Publish);

        let hd = PlanInfo::of(PlanId::Hd);
        let e = check_layers(
            TrackKind::Screen,
            &nominal_ladder(TrackKind::Screen, Preset::P1080p30),
            &hd,
        )
        .unwrap_err();
        assert_eq!(e.field.as_deref(), Some("camadas"));
        assert_eq!(e.message, MSG_QUALIDADE);
        assert!(check_layers(
            TrackKind::Screen,
            &nominal_ladder(TrackKind::Screen, Preset::P720p60),
            &hd
        )
        .is_ok());

        let e = check_join(&PlanInfo::of(PlanId::None)).unwrap_err();
        assert_eq!(e.message, MSG_NAO_ENTRA);
        assert_eq!(e.field, None);
        assert_eq!(e.action, PlanAction::Join);
        assert!(check_join(&PlanInfo::of(PlanId::Viewer)).is_ok());
    }

    #[test]
    fn erro_do_servidor() {
        let publish = PlanAction::Publish;
        match error_from_server(
            "plano_insuficiente",
            "m",
            Some(PlanId::Hd),
            Some("camadas".into()),
            None,
            publish,
        ) {
            crate::CoreError::Plan(r) => {
                assert_eq!(r.plan, PlanId::Hd);
                assert_eq!(r.field.as_deref(), Some("camadas"));
                assert_eq!(r.message, "m");
                assert_eq!(r.action, publish);
            }
            o => panic!("{o:?}"),
        }
        let plano = |p, f| match error_from_server("plano_insuficiente", "m", p, None, f, publish) {
            crate::CoreError::Plan(r) => r.plan,
            o => panic!("{o:?}"),
        };
        assert_eq!(plano(None, Some(PlanId::FullHd)), PlanId::FullHd);
        assert_eq!(plano(None, None), PlanId::None);
        match error_from_server("invalido", "x", None, None, None, publish) {
            crate::CoreError::Server { code, message } => {
                assert_eq!((code.as_str(), message.as_str()), ("invalido", "x"))
            }
            o => panic!("{o:?}"),
        }
        let e = error_from_server("plano_insuficiente", "m", None, None, None, publish);
        assert_eq!(e.code(), "plano_insuficiente");
        assert_eq!(e.to_string(), "m");
    }
}
