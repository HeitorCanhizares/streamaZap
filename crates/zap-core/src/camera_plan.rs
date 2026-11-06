//! Plano da câmera: modo nativo escolhido + escada de camadas (puro, sem I/O).
//!
//! A captura da câmera (`zap_media::win_video`) pede à Media Foundation o modo mais próximo da
//! **camada 0 da escada** (maior largura, altura e fps), com `pick_camera_mode`. Por isso o plano
//! repete essa escolha: dois planos com o mesmo `mode` usam a mesma captura aberta, e só a escada
//! muda (`set_ladder`); modos diferentes pedem reiniciar a captura.

use zap_capture::CameraMode;
use zap_proto::LayerSpec;
use zap_rate::ladder::camera_ladder;

use crate::types::Preset;

/// Modo de captura previsto e a escada que a câmera vai publicar.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraPlan {
    /// Modo nativo que a captura vai abrir. `None`: câmera sintética ou câmera sem modos listados.
    pub mode: Option<CameraMode>,
    /// Escada de `zap_rate::ladder::camera_ladder`, limitada pelo modo real.
    pub ladder: Vec<LayerSpec>,
}

/// Largura, altura e fps desejados para a qualidade escolhida:
/// 720p30 -> (1280, 720, 30), 720p60 -> (1280, 720, 60), 1080p30 -> (1920, 1080, 30),
/// 1080p60 -> (1920, 1080, 60).
pub fn want_of(preset: Preset) -> (u32, u32, u32) {
    match preset {
        Preset::P720p30 => (1280, 720, 30),
        Preset::P720p60 => (1280, 720, 60),
        Preset::P1080p30 => (1920, 1080, 30),
        Preset::P1080p60 => (1920, 1080, 60),
    }
}

/// Plano para uma câmera real com os modos `modes` (de `zap_capture::camera_modes`).
///
/// 1. `want_of(preset)`; `pick_camera_mode(modes, w, h, fps)` escolhe o modo inicial;
/// 2. escada = `zap_rate::ladder::camera_ladder(preset.to_rate(), modo.width, modo.height, modo.fps)`;
/// 3. o `mode` final é o que a captura vai abrir: `pick_camera_mode` de novo, com a largura, a
///    altura e o fps máximos da escada (é o que `win_video` pede).
///
/// `modes` vazio: `mode = None` e escada `camera_ladder(preset.to_rate(), 1280, 720, 30)`
/// (o comportamento de antes).
pub fn plan_from_modes(modes: &[CameraMode], preset: Preset) -> CameraPlan {
    let (w, h, fps) = want_of(preset);
    let inicial = zap_capture::pick_camera_mode(modes, w, h, fps).and_then(|i| modes.get(i));
    let Some(inicial) = inicial else {
        return CameraPlan {
            mode: None,
            ladder: camera_ladder(preset.to_rate(), 1280, 720, 30),
        };
    };
    let ladder = camera_ladder(preset.to_rate(), inicial.width, inicial.height, inicial.fps);
    // Mesmo pedido que a captura faz: maiores largura, altura e fps da escada.
    let max_w = ladder.iter().map(|l| u32::from(l.width)).max().unwrap_or(w);
    let max_h = ladder.iter().map(|l| u32::from(l.height)).max().unwrap_or(h);
    let max_fps = ladder.iter().map(|l| u32::from(l.fps)).max().unwrap_or(fps);
    let final_ = zap_capture::pick_camera_mode(modes, max_w, max_h, max_fps)
        .and_then(|i| modes.get(i))
        .copied()
        .unwrap_or(*inicial);
    CameraPlan {
        mode: Some(final_),
        ladder,
    }
}

/// Plano para a câmera sintética `synthetic:LxA`: sem modo nativo (`mode = None`); escada
/// `camera_ladder(preset.to_rate(), width, height, fps)` com `fps` = o da qualidade escolhida
/// (`want_of`), já que a fonte sintética entrega o fps pedido.
pub fn plan_synthetic(width: u32, height: u32, preset: Preset) -> CameraPlan {
    let (_, _, fps) = want_of(preset);
    CameraPlan {
        mode: None,
        ladder: camera_ladder(preset.to_rate(), width, height, fps),
    }
}

/// A mesma captura serve aos dois planos? Verdadeiro quando `a.mode == b.mode`
/// (inclusive os dois `None`). Falso força reiniciar a captura da câmera.
pub fn same_capture(a: &CameraPlan, b: &CameraPlan) -> bool {
    a.mode == b.mode
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(width: u32, height: u32, fps: u32, type_index: u32) -> CameraMode {
        CameraMode {
            width,
            height,
            fps,
            type_index,
        }
    }

    fn modos() -> Vec<CameraMode> {
        vec![m(1280, 720, 30, 0), m(1920, 1080, 30, 1), m(1920, 1080, 60, 2)]
    }

    #[test]
    fn p720p30_escolhe_1280x720_30() {
        let p = plan_from_modes(&modos(), Preset::P720p30);
        assert_eq!(p.mode.map(|x| x.type_index), Some(0));
        assert_eq!(p.ladder, camera_ladder(Preset::P720p30.to_rate(), 1280, 720, 30));
    }

    #[test]
    fn p1080p60_escolhe_1920x1080_60() {
        let p = plan_from_modes(&modos(), Preset::P1080p60);
        assert_eq!(p.mode.map(|x| x.type_index), Some(2));
        assert_eq!(p.ladder, camera_ladder(Preset::P1080p60.to_rate(), 1920, 1080, 60));
    }

    #[test]
    fn p1080p30_escolhe_1920x1080_30() {
        let p = plan_from_modes(&modos(), Preset::P1080p30);
        assert_eq!(p.mode.map(|x| x.type_index), Some(1));
        assert_eq!(p.ladder.len(), 3);
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1920, 1080));
    }

    #[test]
    fn p720p60_com_camera_so_30() {
        let p = plan_from_modes(&[m(1280, 720, 30, 0)], Preset::P720p60);
        assert_eq!(p.mode.map(|x| x.type_index), Some(0));
        assert_eq!(p.ladder, camera_ladder(Preset::P720p60.to_rate(), 1280, 720, 30));
    }

    #[test]
    fn camera_so_720p30_pedindo_1080p60_sai_720p30() {
        let p = plan_from_modes(&[m(1280, 720, 30, 0)], Preset::P1080p60);
        assert_eq!(p.ladder, camera_ladder(Preset::P1080p60.to_rate(), 1280, 720, 30));
        assert_eq!(
            (p.ladder[0].width, p.ladder[0].height, p.ladder[0].fps),
            (1280, 720, 30)
        );
    }

    #[test]
    fn sem_modos() {
        for preset in [Preset::P720p30, Preset::P720p60, Preset::P1080p30, Preset::P1080p60] {
            let p = plan_from_modes(&[], preset);
            assert_eq!(p.mode, None);
            assert_eq!(p.ladder, camera_ladder(Preset::P720p30.to_rate(), 1280, 720, 30));
        }
    }

    #[test]
    fn synthetic_1080p30() {
        let p = plan_synthetic(1920, 1080, Preset::P1080p30);
        assert_eq!(p.mode, None);
        assert_eq!(p.ladder.len(), 3);
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1920, 1080));
    }

    #[test]
    fn synthetic_720p30_em_fonte_1920x1080() {
        let p = plan_synthetic(1920, 1080, Preset::P720p30);
        assert_eq!((p.ladder[0].width, p.ladder[0].height), (1280, 720));
    }

    #[test]
    fn same_capture_compara_modo() {
        let a = plan_from_modes(&modos(), Preset::P720p30);
        let b = plan_from_modes(&modos(), Preset::P720p30);
        let c = plan_from_modes(&modos(), Preset::P1080p60);
        assert!(same_capture(&a, &b));
        assert!(!same_capture(&a, &c));
        let s1 = plan_synthetic(1920, 1080, Preset::P720p30);
        let s2 = plan_synthetic(1280, 720, Preset::P1080p30);
        assert!(same_capture(&s1, &s2));
    }
}
