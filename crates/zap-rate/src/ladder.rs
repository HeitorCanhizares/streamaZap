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
/// Câmera: só camadas 0 e 2 (sem camada 1).
const LADDER_CAMERA: [Rung; 2] = [
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

fn preset_rungs(preset: Preset) -> &'static [Rung] {
    match preset {
        Preset::P1080p60 => &LADDER_1080P60,
        Preset::P1080p30 => &LADDER_1080P30,
        Preset::P720p60 => &LADDER_720P60,
        Preset::P720p30 => &LADDER_720P30,
    }
}

pub fn screen_ladder(preset: Preset, source_w: u32, source_h: u32) -> Vec<LayerSpec> {
    build_ladder(preset_rungs(preset), source_w, source_h)
}

pub fn camera_ladder(source_w: u32, source_h: u32) -> Vec<LayerSpec> {
    build_ladder(&LADDER_CAMERA, source_w, source_h)
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

fn build_ladder(rungs: &[Rung], source_w: u32, source_h: u32) -> Vec<LayerSpec> {
    // Fonte vazia ou de 1 px não tem escada útil (uma altura 0 não faz sentido).
    if source_w < 2 || source_h < 2 {
        return Vec::new();
    }
    let sw = u64::from(source_w);
    let sh = u64::from(source_h);

    // Passo 1: (largura, altura, kbps) de cada degrau. Vai de baixo para cima, porque o piso
    // de kbps de um degrau é o kbps final do degrau logo abaixo (índice maior).
    let mut dims = vec![(0u64, 0u64, 0u64); rungs.len()];
    let mut piso = 0u64;
    for (i, r) in rungs.iter().enumerate().rev() {
        let nom_h = r.height;
        let nom_w = nom_h * 16 / 9;
        let (w, h, kbps) = if nom_h <= sh {
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
        dims[i] = (w, h, kbps);
        piso = kbps;
    }

    // Passo 2: remove degraus (menos o 0) com altura >= à do último mantido. Eles não
    // acrescentam nada ao espectador, então não valem a banda.
    let mut out = Vec::with_capacity(rungs.len());
    let mut ultima_altura = 0u64;
    for (i, r) in rungs.iter().enumerate() {
        let (w, h, kbps) = dims[i];
        if i > 0 && h >= ultima_altura {
            continue;
        }
        ultima_altura = h;
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
        let c = camera_ladder(1920, 1080);
        assert_eq!(c, vec![s(0, 1280, 720, 30, 1500), s(2, 640, 360, 15, 300)]);
        assert_eq!(c.iter().map(|l| l.layer).collect::<Vec<_>>(), vec![0, 2]);
    }

    #[test]
    fn fonte_zero_vazio() {
        assert!(screen_ladder(Preset::P720p30, 0, 720).is_empty());
        assert!(screen_ladder(Preset::P720p30, 1280, 0).is_empty());
        assert!(camera_ladder(0, 0).is_empty());
        assert!(camera_ladder(1920, 0).is_empty());
    }

    proptest! {
        #[test]
        fn escada_pares_ordenada_sem_ampliar(
            w in 2u32..=7680,
            h in 2u32..=4320,
            preset in prop::sample::select(vec![
                Preset::P720p30,
                Preset::P720p60,
                Preset::P1080p30,
                Preset::P1080p60,
            ]),
        ) {
            for ladder in [screen_ladder(preset, w, h), camera_ladder(w, h)] {
                prop_assert_eq!(ladder.first().map(|l| l.layer), Some(0));
                for (i, l) in ladder.iter().enumerate() {
                    prop_assert_eq!(l.width % 2, 0);
                    prop_assert_eq!(l.height % 2, 0);
                    // Altura nunca passa da fonte. Largura pode passar 1 px por causa do par.
                    prop_assert!(u32::from(l.height) <= h);
                    prop_assert!(u32::from(l.width) <= w + 1);
                    if i > 0 {
                        prop_assert!(l.layer > ladder[i - 1].layer);
                        prop_assert!(l.height < ladder[i - 1].height);
                    }
                }
            }
        }
    }
}
