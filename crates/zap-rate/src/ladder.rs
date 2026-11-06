//! Escada de camadas (`docs/api/balanceamento.md#1-escada-de-camadas`).
#![allow(unused_variables, dead_code)]

use zap_proto::LayerSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    P720p30,
    P720p60,
    P1080p30,
    P1080p60,
}

/// Um degrau da tabela nominal (16:9): camada, altura, fps e kbps.
#[derive(Clone, Copy)]
struct Rung {
    layer: u8,
    height: u64,
    fps: u8,
    kbps: u64,
}

const LADDER_1080P60: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 1080,
        fps: 60,
        kbps: 8000,
    },
    Rung {
        layer: 1,
        height: 720,
        fps: 30,
        kbps: 2500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 400,
    },
];
const LADDER_1080P30: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 1080,
        fps: 30,
        kbps: 5000,
    },
    Rung {
        layer: 1,
        height: 720,
        fps: 30,
        kbps: 2500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 400,
    },
];
const LADDER_720P60: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 720,
        fps: 60,
        kbps: 4000,
    },
    Rung {
        layer: 1,
        height: 540,
        fps: 30,
        kbps: 1200,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 400,
    },
];
const LADDER_720P30: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 720,
        fps: 30,
        kbps: 2500,
    },
    Rung {
        layer: 1,
        height: 540,
        fps: 30,
        kbps: 1200,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 400,
    },
];
/// Câmera (docs/api/balanceamento.md §1): a camada 1 só existe quando acrescenta algo.
const CAMERA_720P30: [Rung; 2] = [
    Rung {
        layer: 0,
        height: 720,
        fps: 30,
        kbps: 1500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 300,
    },
];
const CAMERA_720P60: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 720,
        fps: 60,
        kbps: 2500,
    },
    Rung {
        layer: 1,
        height: 720,
        fps: 30,
        kbps: 1500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 300,
    },
];
const CAMERA_1080P30: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 1080,
        fps: 30,
        kbps: 3000,
    },
    Rung {
        layer: 1,
        height: 720,
        fps: 30,
        kbps: 1500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 300,
    },
];
const CAMERA_1080P60: [Rung; 3] = [
    Rung {
        layer: 0,
        height: 1080,
        fps: 60,
        kbps: 4500,
    },
    Rung {
        layer: 1,
        height: 720,
        fps: 30,
        kbps: 1500,
    },
    Rung {
        layer: 2,
        height: 360,
        fps: 15,
        kbps: 300,
    },
];

fn camera_rungs(preset: Preset) -> &'static [Rung] {
    match preset {
        Preset::P1080p60 => &CAMERA_1080P60,
        Preset::P1080p30 => &CAMERA_1080P30,
        Preset::P720p60 => &CAMERA_720P60,
        Preset::P720p30 => &CAMERA_720P30,
    }
}

/// (altura, fps) nominais do preset.
fn preset_mode(preset: Preset) -> (u32, u32) {
    match preset {
        Preset::P720p30 => (720, 30),
        Preset::P720p60 => (720, 60),
        Preset::P1080p30 => (1080, 30),
        Preset::P1080p60 => (1080, 60),
    }
}

/// Preset efetivo da câmera: o maior que o modo real dela sustenta sem passar do pedido.
/// Tolerância de 1 fps (câmeras reais reportam 29/59). Sem candidato, cai para 720p30.
fn effective_camera_preset(preset: Preset, source_h: u32, source_fps: u32) -> Preset {
    let (req_h, req_fps) = preset_mode(preset);
    [Preset::P720p30, Preset::P720p60, Preset::P1080p30, Preset::P1080p60]
        .into_iter()
        .filter(|&c| {
            let (h, f) = preset_mode(c);
            h <= source_h && f <= source_fps.saturating_add(1) && h <= req_h && f <= req_fps
        })
        .max_by_key(|&c| preset_mode(c))
        .unwrap_or(Preset::P720p30)
}

fn preset_rungs(preset: Preset) -> &'static [Rung] {
    match preset {
        Preset::P1080p60 => &LADDER_1080P60,
        Preset::P1080p30 => &LADDER_1080P30,
        Preset::P720p60 => &LADDER_720P60,
        Preset::P720p30 => &LADDER_720P30,
    }
}

pub fn screen_ladder(preset: Preset, source_w: u32, source_h: u32) -> Vec<LayerSpec> {
    build_ladder(preset_rungs(preset), source_w, source_h, false)
}

/// Escada da câmera para a qualidade escolhida (`preset`) e o modo real da câmera
/// (`source_w` x `source_h` a `source_fps`). Não promete mais que o modo real da câmera.
pub fn camera_ladder(preset: Preset, source_w: u32, source_h: u32, source_fps: u32) -> Vec<LayerSpec> {
    let efetivo = effective_camera_preset(preset, source_h, source_fps);
    build_ladder(camera_rungs(efetivo), source_w, source_h, true)
}

/// Arredonda `num / den` para o inteiro mais próximo (metade para cima).
fn round_div(num: u64, den: u64) -> u64 {
    (num * 2 + den) / (den * 2)
}

/// Arredonda para o par mais próximo: `round(x/2)*2`.
fn round_even(x: u64) -> u64 {
    (x + 1) & !1
}

/// Converte para u16 sem pânico. Valores acima do limite saturam no maior par de u16.
fn to_u16(x: u64) -> u16 {
    u16::try_from(x).unwrap_or(u16::MAX - 1)
}

/// `igual_se_fps_cai`: a câmera aceita degrau com a mesma altura do anterior quando o fps cai
/// (ex.: 720p60 e 720p30). A tela nunca aceita.
fn build_ladder(rungs: &[Rung], source_w: u32, source_h: u32, igual_se_fps_cai: bool) -> Vec<LayerSpec> {
    // Fonte vazia ou de 1 px não tem escada útil (uma altura 0 não faz sentido).
    if source_w < 2 || source_h < 2 {
        return Vec::new();
    }
    let sw = u64::from(source_w);
    let sh = u64::from(source_h);

    // Passo 1: (largura, altura, kbps) de cada degrau. Vai de baixo para cima, porque o piso
    // de kbps de um degrau é o kbps final do degrau logo abaixo (índice maior).
    let mut dims = vec![(0u64, 0u64, 0u64, false); rungs.len()];
    let mut piso = 0u64;
    for (i, r) in rungs.iter().enumerate().rev() {
        let nom_h = r.height;
        let nom_w = nom_h * 16 / 9;
        let clampado = nom_h > sh;
        let (w, h, kbps) = if !clampado {
            // Cabe na fonte: altura da tabela, largura pela proporção da fonte.
            (round_even(round_div(nom_h * sw, sh)), nom_h, r.kbps)
        } else {
            // Não amplia: altura cai para a fonte (par), largura é a da fonte (par).
            // kbps escala pela área em relação ao 16:9 nominal, sem ficar abaixo do degrau de baixo.
            let h = sh & !1;
            let w = round_even(sw);
            let escala = r.kbps * (w * h) / (nom_w * nom_h);
            (w, h, escala.max(piso))
        };
        dims[i] = (w, h, kbps, clampado);
        piso = kbps;
    }

    // Passo 2: remove degraus (menos o 0) com altura >= à do último mantido (salvo a exceção da câmera). Eles não
    // acrescentam nada ao espectador, então não valem a banda.
    let mut out = Vec::with_capacity(rungs.len());
    let mut ultima_altura = 0u64;
    let mut ultimo_fps = 0u8;
    for (i, r) in rungs.iter().enumerate() {
        let (w, h, kbps, clampado) = dims[i];
        let igual_ok = igual_se_fps_cai && !clampado && h == ultima_altura && r.fps < ultimo_fps;
        if i > 0 && h >= ultima_altura && !igual_ok {
            continue;
        }
        ultima_altura = h;
        ultimo_fps = r.fps;
        out.push(LayerSpec {
            layer: r.layer,
            width: to_u16(w),
            height: to_u16(h),
            fps: r.fps,
            kbps: u32::try_from(kbps).unwrap_or(u32::MAX),
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn s(layer: u8, width: u16, height: u16, fps: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer,
            width,
            height,
            fps,
            kbps,
        }
    }

    #[test]
    fn p1080p60_fonte_1080() {
        assert_eq!(
            screen_ladder(Preset::P1080p60, 1920, 1080),
            vec![
                s(0, 1920, 1080, 60, 8000),
                s(1, 1280, 720, 30, 2500),
                s(2, 640, 360, 15, 400)
            ]
        );
    }

    #[test]
    fn p1080p30_fonte_1080() {
        assert_eq!(
            screen_ladder(Preset::P1080p30, 1920, 1080),
            vec![
                s(0, 1920, 1080, 30, 5000),
                s(1, 1280, 720, 30, 2500),
                s(2, 640, 360, 15, 400)
            ]
        );
    }

    #[test]
    fn fonte_ultrawide_3440x1440() {
        assert_eq!(
            screen_ladder(Preset::P1080p30, 3440, 1440),
            vec![
                s(0, 2580, 1080, 30, 5000),
                s(1, 1720, 720, 30, 2500),
                s(2, 860, 360, 15, 400)
            ]
        );
    }

    #[test]
    fn fonte_720p_com_p1080p60_clampa_e_remove_camada_1() {
        // L0 cai para 720p com kbps pela área: 8000 * 1280*720 / 1920*1080 = 3555 (truncado).
        // Igual à L1 em altura, então a L1 sai.
        assert_eq!(
            screen_ladder(Preset::P1080p60, 1280, 720),
            vec![s(0, 1280, 720, 60, 3555), s(2, 640, 360, 15, 400)]
        );
    }

    #[test]
    fn fonte_menor_que_escada_remove_iguais() {
        // Fonte 1280x540 com P720p60: L0 vira 540p (kbps 4000*0.75=3000), L1 (540p) sai.
        assert_eq!(
            screen_ladder(Preset::P720p60, 1280, 540),
            vec![s(0, 1280, 540, 60, 3000), s(2, 854, 360, 15, 400)]
        );
    }

    #[test]
    fn camera_tem_camadas_0_e_2() {
        let c = camera_ladder(Preset::P720p30, 1920, 1080, 30);
        assert_eq!(c, vec![s(0, 1280, 720, 30, 1500), s(2, 640, 360, 15, 300)]);
        assert_eq!(c.iter().map(|l| l.layer).collect::<Vec<_>>(), vec![0, 2]);
    }

    #[test]
    fn camera_720p60_1280x720_60() {
        assert_eq!(
            camera_ladder(Preset::P720p60, 1280, 720, 60),
            vec![
                s(0, 1280, 720, 60, 2500),
                s(1, 1280, 720, 30, 1500),
                s(2, 640, 360, 15, 300)
            ]
        );
    }

    #[test]
    fn camera_1080p30_fonte_1920x1080_30() {
        assert_eq!(
            camera_ladder(Preset::P1080p30, 1920, 1080, 30),
            vec![
                s(0, 1920, 1080, 30, 3000),
                s(1, 1280, 720, 30, 1500),
                s(2, 640, 360, 15, 300)
            ]
        );
    }

    #[test]
    fn camera_1080p60_fonte_1920x1080_60() {
        assert_eq!(
            camera_ladder(Preset::P1080p60, 1920, 1080, 60),
            vec![
                s(0, 1920, 1080, 60, 4500),
                s(1, 1280, 720, 30, 1500),
                s(2, 640, 360, 15, 300)
            ]
        );
    }

    #[test]
    fn camera_1080p60_pedido_com_camera_720p30() {
        assert_eq!(
            camera_ladder(Preset::P1080p60, 1280, 720, 30),
            camera_ladder(Preset::P720p30, 1280, 720, 30)
        );
        assert_eq!(
            camera_ladder(Preset::P1080p60, 1280, 720, 30),
            vec![s(0, 1280, 720, 30, 1500), s(2, 640, 360, 15, 300)]
        );
    }

    #[test]
    fn camera_1080p60_com_720p60_e_1080p30() {
        assert_eq!(
            camera_ladder(Preset::P1080p60, 1280, 720, 60),
            camera_ladder(Preset::P720p60, 1280, 720, 60)
        );
        assert_eq!(
            camera_ladder(Preset::P1080p60, 1920, 1080, 30),
            camera_ladder(Preset::P1080p30, 1920, 1080, 30)
        );
    }

    #[test]
    fn camera_tolera_29_fps() {
        assert_eq!(
            camera_ladder(Preset::P720p60, 1280, 720, 59),
            camera_ladder(Preset::P720p60, 1280, 720, 60)
        );
    }

    #[test]
    fn camera_480p_pedindo_1080p60() {
        let c = camera_ladder(Preset::P1080p60, 640, 480, 30);
        assert_eq!(c.first().map(|l| l.layer), Some(0));
        assert_eq!(c[0].width % 2, 0);
        assert!(c[0].height <= 480);
        assert!(c.windows(2).all(|p| p[1].height < p[0].height));
    }

    #[test]
    fn fonte_zero_vazio() {
        assert!(screen_ladder(Preset::P720p30, 0, 720).is_empty());
        assert!(screen_ladder(Preset::P720p30, 1280, 0).is_empty());
        assert!(camera_ladder(Preset::P720p30, 0, 0, 30).is_empty());
        assert!(camera_ladder(Preset::P720p30, 1920, 0, 30).is_empty());
    }

    proptest! {
        #[test]
        fn escada_pares_ordenada_sem_ampliar(
            w in 2u32..=7680,
            h in 2u32..=4320,
            fps in 1u32..=120,
            preset in prop::sample::select(vec![
                Preset::P720p30,
                Preset::P720p60,
                Preset::P1080p30,
                Preset::P1080p60,
            ]),
        ) {
            for (camera, ladder) in [
                (false, screen_ladder(preset, w, h)),
                (true, camera_ladder(preset, w, h, fps)),
            ] {
                prop_assert_eq!(ladder.first().map(|l| l.layer), Some(0));
                for (i, l) in ladder.iter().enumerate() {
                    prop_assert_eq!(l.width % 2, 0);
                    prop_assert_eq!(l.height % 2, 0);
                    // Altura nunca passa da fonte. Largura pode passar 1 px por causa do par.
                    prop_assert!(u32::from(l.height) <= h);
                    prop_assert!(u32::from(l.width) <= w + 1);
                    if i > 0 {
                        prop_assert!(l.layer > ladder[i - 1].layer);
                        let ant = &ladder[i - 1];
                        // Câmera aceita mesma altura se o fps cai.
                        prop_assert!(
                            l.height < ant.height
                                || (camera && l.height == ant.height && l.fps < ant.fps)
                        );
                    }
                }
            }
        }
    }
}
