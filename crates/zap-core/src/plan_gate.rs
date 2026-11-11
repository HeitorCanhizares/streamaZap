//! Plano da sessão Online aplicado no app (`docs/planos/fase3-app-planos.md` §4.3): cadeados das
//! qualidades e conferência antes de mandar ao servidor. Os números vêm só de `zap_proto::plan`
//! e das escadas do `zap-rate`. Puro: sem I/O, sem tokio.
//!
//! Este arquivo tem os tipos e as escadas; as conferências (`best_allowed`, `check_layers`,
//! `check_join`, `error_from_server`) ficam em `plan_check.rs` e são reexportadas aqui.
use serde::{Deserialize, Serialize};
use zap_proto::plan::{check_publish, limits, PlanId, PlanLimits};
use zap_proto::{LayerSpec, TrackKind};
use zap_rate::ladder::{camera_ladder, screen_ladder};

use crate::types::Preset;

pub use crate::plan_check::{best_allowed, check_join, check_layers, error_from_server};

/// As 4 qualidades, na ordem de exibição (e da menor para a maior).
pub const PRESETS: [Preset; 4] = [Preset::P720p30, Preset::P720p60, Preset::P1080p30, Preset::P1080p60];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanAction {
    /// Criar ou entrar em sala Online.
    #[serde(rename = "entrar")]
    Join,
    /// Publicar ou trocar a escada de uma trilha.
    #[serde(rename = "transmitir")]
    Publish,
}

/// Recusa por plano (da conferência local ou do `erro` do servidor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRefusal {
    pub plan: PlanId,
    pub action: PlanAction,
    /// `"tipo"` | `"camadas"` ao transmitir; `None` ao entrar.
    pub field: Option<String>,
    /// Texto em português para a interface.
    pub message: String,
}

/// Um preset acima do plano.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetLock {
    pub preset: Preset,
    /// Plano mais barato que libera (`hd` ou `fullhd`); `None` = nenhum plano libera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<PlanId>,
}

/// `Snapshot.quality_locks`: só existe com plano na sessão Online (conta).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct QualityLocks {
    /// O plano transmite alguma coisa (tela, janela, câmera ou só o som).
    pub can_publish: bool,
    /// Tela e janela, na ordem de `PRESETS`.
    pub screen: Vec<PresetLock>,
    pub camera: Vec<PresetLock>,
}

/// `Snapshot.plan_notice`: a última recusa. `seq` cresce a cada recusa (a interface mostra o aviso
/// uma vez por `seq`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlanNotice {
    pub seq: u64,
    pub plan: PlanId,
    pub action: PlanAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub message: String,
}

/// Textos (os mesmos da interface).
pub const MSG_NAO_TRANSMITE: &str = "Seu plano não permite transmitir no Online";
pub const MSG_QUALIDADE: &str = "Seu plano não permite essa qualidade";
pub const MSG_NAO_ENTRA: &str = "Seu plano não permite entrar em salas Online";

/// Escada usada para os cadeados: fonte 1920×1080 (16:9) e câmera a 60 fps.
/// Tela e janela: `screen_ladder(p, 1920, 1080)`; câmera: `camera_ladder(p, 1920, 1080, 60)`;
/// áudio: vazia.
pub fn nominal_ladder(kind: TrackKind, preset: Preset) -> Vec<LayerSpec> {
    match kind {
        TrackKind::Screen | TrackKind::Window => screen_ladder(preset.to_rate(), 1920, 1080),
        TrackKind::Camera => camera_ladder(preset.to_rate(), 1920, 1080, 60),
        TrackKind::Audio => Vec::new(),
    }
}

/// `kbps` de cada camada = `min(kbps, teto do tipo)` (tela/janela: `screen_kbps`; câmera:
/// `camera_kbps`). Áudio e plano sem `publish`: devolve igual. Nunca aumenta nada.
///
/// Motivo: a escada escala o kbps pela área quando a fonte é mais baixa que o preset; sem este
/// ajuste, o preset liberado pelo plano seria recusado por kbps.
pub fn fit_ladder(kind: TrackKind, mut ladder: Vec<LayerSpec>, l: &PlanLimits) -> Vec<LayerSpec> {
    let Some(publish) = l.publish else {
        return ladder;
    };
    let cap = match kind {
        TrackKind::Audio => return ladder,
        TrackKind::Screen | TrackKind::Window => publish.screen_kbps,
        TrackKind::Camera => publish.camera_kbps,
    };
    for layer in &mut ladder {
        layer.kbps = layer.kbps.min(cap);
    }
    ladder
}

/// `check_publish(kind, &fit_ladder(kind, nominal_ladder(kind, p), l), l).is_ok()`.
pub fn preset_allowed(kind: TrackKind, preset: Preset, l: &PlanLimits) -> bool {
    let ladder = fit_ladder(kind, nominal_ladder(kind, preset), l);
    check_publish(kind, &ladder, l).is_ok()
}

/// O primeiro de `[Hd, FullHd]` (pela tabela `zap_proto::plan::limits`) em que `preset_allowed`.
pub fn cheapest_plan_for(kind: TrackKind, preset: Preset) -> Option<PlanId> {
    [PlanId::Hd, PlanId::FullHd]
        .into_iter()
        .find(|&id| preset_allowed(kind, preset, &limits(id)))
}

/// Presets travados com os limites `l` (os da sessão, vindos do servidor).
pub fn quality_locks(l: &PlanLimits) -> QualityLocks {
    let locks = |kind: TrackKind| -> Vec<PresetLock> {
        PRESETS
            .iter()
            .copied()
            .filter(|&p| !preset_allowed(kind, p, l))
            .map(|p| PresetLock {
                preset: p,
                needs: cheapest_plan_for(kind, p),
            })
            .collect()
    };
    QualityLocks {
        can_publish: l.publish.is_some(),
        screen: locks(TrackKind::Screen),
        camera: locks(TrackKind::Camera),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_proto::plan::PlanError;
    use zap_rate::ladder::Preset as RatePreset;

    fn lock(preset: Preset, needs: PlanId) -> PresetLock {
        PresetLock {
            preset,
            needs: Some(needs),
        }
    }

    #[test]
    fn quality_locks_tabela_dos_4_planos() {
        let todos = vec![
            lock(Preset::P720p30, PlanId::Hd),
            lock(Preset::P720p60, PlanId::Hd),
            lock(Preset::P1080p30, PlanId::FullHd),
            lock(Preset::P1080p60, PlanId::FullHd),
        ];
        let casos = [
            (PlanId::None, false, todos.clone()),
            (PlanId::Viewer, false, todos.clone()),
            (PlanId::Hd, true, todos[2..].to_vec()),
            (PlanId::FullHd, true, vec![]),
        ];
        for (id, can_publish, travados) in casos {
            let q = quality_locks(&limits(id));
            assert_eq!(q.can_publish, can_publish, "{id:?}");
            assert_eq!(q.screen, travados, "tela {id:?}");
            assert_eq!(q.camera, travados, "camera {id:?}");
        }
    }

    #[test]
    fn cheapest_plan_for_escolhe_o_mais_barato() {
        assert_eq!(cheapest_plan_for(TrackKind::Screen, Preset::P720p60), Some(PlanId::Hd));
        assert_eq!(
            cheapest_plan_for(TrackKind::Screen, Preset::P1080p30),
            Some(PlanId::FullHd)
        );
        assert_eq!(
            cheapest_plan_for(TrackKind::Camera, Preset::P1080p60),
            Some(PlanId::FullHd)
        );
        assert_eq!(cheapest_plan_for(TrackKind::Camera, Preset::P720p30), Some(PlanId::Hd));
    }

    #[test]
    fn fit_ladder_janela_larga() {
        let full = limits(PlanId::FullHd);
        let original = screen_ladder(RatePreset::P1080p60, 2500, 1000);
        assert_eq!(original[0].kbps, 9645);
        assert!(matches!(
            check_publish(TrackKind::Window, &original, &full),
            Err(PlanError::AboveLimit { what: "kbps", .. })
        ));
        let ajustada = fit_ladder(TrackKind::Window, original, &full);
        assert_eq!(
            (ajustada[0].width, ajustada[0].height, ajustada[0].fps),
            (2500, 1000, 60)
        );
        assert_eq!(ajustada[0].kbps, 8000);
        assert!(check_publish(TrackKind::Window, &ajustada, &full).is_ok());
    }

    #[test]
    fn fit_ladder_camera_e_casos_sem_efeito() {
        let full = limits(PlanId::FullHd);
        let camada = |layer, kbps| LayerSpec {
            layer,
            width: 1920,
            height: 1080,
            fps: 60,
            kbps,
        };
        let ajustada = fit_ladder(TrackKind::Camera, vec![camada(0, 5000), camada(1, 1000)], &full);
        assert_eq!(ajustada[0].kbps, 4500);
        assert_eq!(ajustada[1].kbps, 1000);

        assert!(fit_ladder(TrackKind::Audio, vec![], &full).is_empty());
        let escada = vec![camada(0, 5000)];
        assert_eq!(
            fit_ladder(TrackKind::Camera, escada.clone(), &limits(PlanId::Viewer)),
            escada
        );
        assert_eq!(fit_ladder(TrackKind::Audio, escada.clone(), &full), escada);

        for kind in [TrackKind::Screen, TrackKind::Window, TrackKind::Camera] {
            for p in PRESETS {
                for id in [PlanId::None, PlanId::Viewer, PlanId::Hd, PlanId::FullHd] {
                    let antes = nominal_ladder(kind, p);
                    let depois = fit_ladder(kind, antes.clone(), &limits(id));
                    assert_eq!(antes.len(), depois.len());
                    for (a, d) in antes.iter().zip(&depois) {
                        assert!(d.kbps <= a.kbps);
                        assert_eq!((a.width, a.height, a.fps), (d.width, d.height, d.fps));
                    }
                }
            }
        }
    }

    #[test]
    fn nominal_ladder_basico() {
        assert!(nominal_ladder(TrackKind::Audio, Preset::P720p30).is_empty());
        let l = nominal_ladder(TrackKind::Screen, Preset::P720p60);
        assert_eq!((l[0].width, l[0].height, l[0].fps, l[0].kbps), (1280, 720, 60, 4000));
    }
}
