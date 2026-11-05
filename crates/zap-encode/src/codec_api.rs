//! Configuração por `ICodecAPI`.
use crate::{logic, EncodeError, EncoderConfig};
use std::mem::ManuallyDrop;
use windows::core::GUID;
use windows::Win32::Foundation::{VARIANT_BOOL, VARIANT_TRUE};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_CBR, eAVEncCommonRateControlMode_LowDelayVBR,
    eAVEncCommonRateControlMode_PeakConstrainedVBR, CODECAPI_AVEncCommonBufferSize, CODECAPI_AVEncCommonMaxBitRate,
    CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonQualityVsSpeed, CODECAPI_AVEncCommonRateControlMode,
    CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize, CODECAPI_AVEncVideoForceKeyFrame,
    CODECAPI_AVEncVideoMaxQP, CODECAPI_AVEncVideoMinQP, CODECAPI_AVLowLatencyMode, ICodecAPI,
};
use windows::Win32::System::Variant::{VariantClear, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4};

/// Monta um `VARIANT` do tipo `VT_UI4` com o valor `v`. Não aloca nada (tipo escalar), então
/// não há o que liberar com `VariantClear`; os campos ficam em `ManuallyDrop` só por exigência
/// do binding.
fn make_u32_variant(v: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: v },
            }),
        },
    }
}

/// `VARIANT` do tipo `VT_BOOL` (usado por `AVLowLatencyMode`, que é booleano de verdade).
fn make_bool_variant(v: bool) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    boolVal: if v { VARIANT_TRUE } else { VARIANT_BOOL(0) },
                },
            }),
        },
    }
}

/// Envia um `VARIANT` para `ICodecAPI::SetValue`.
fn set_variant(api: &ICodecAPI, key: &GUID, variant: &VARIANT) -> windows::core::Result<()> {
    // SAFETY: `key` e `variant` são referências vivas durante a chamada; o VARIANT é escalar
    // (sem BSTR/interface), então o MFT não toma posse de memória nossa.
    unsafe { api.SetValue(key, variant) }
}

/// `SetValue` com `VT_UI4`.
fn set_u32(api: &ICodecAPI, key: &GUID, value: u32) -> windows::core::Result<()> {
    set_variant(api, key, &make_u32_variant(value))
}

/// `SetValue` com `VT_BOOL`.
fn set_bool(api: &ICodecAPI, key: &GUID, value: bool) -> windows::core::Result<()> {
    set_variant(api, key, &make_bool_variant(value))
}

/// Loga uma opção recusada (uma vez por chamada, pois cada opção é tentada uma só vez).
fn avisa(opcao: &'static str, e: &windows::core::Error) {
    tracing::warn!(opcao = opcao, erro = %e, "MFT recusou opção do ICodecAPI; seguindo com o padrão");
}

/// Opção do `ICodecAPI` que o MFT pode recusar (esqueleto m4c: tarefa encode-01 preenche).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opcao {
    RateControl,
    MeanBitRate,
    BufferSize,
    MaxBitRate,
    LowLatency,
    BPictures,
    Gop,
    QualityVsSpeed,
    MinQp,
    MaxQp,
}

/// Configuração de controle de taxa (plano m4c 5.7 c). `Padrao` = o que o hardware usa hoje;
/// `Sw1..Sw4` = as 4 configurações do MFT de software, testadas nesta ordem.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PerfilTaxa {
    Padrao,
    Sw1,
    Sw2,
    Sw3,
    Sw4,
}

/// Valores lidos de volta do MFT (`None` = ilegível ou não suportado).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CodecReadback {
    pub rate_control: Option<u32>,
    pub mean_bitrate: Option<u32>,
    pub buffer_size: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub low_latency: Option<bool>,
    pub b_pictures: Option<u32>,
    pub gop: Option<u32>,
    pub quality_vs_speed: Option<u32>,
    pub min_qp: Option<u32>,
    pub max_qp: Option<u32>,
}

/// Lê de volta todas as opções de `CodecReadback` (`GetValue`); nunca falha.
pub(crate) fn readback(api: &ICodecAPI) -> CodecReadback {
    CodecReadback {
        rate_control: get_u32(api, &CODECAPI_AVEncCommonRateControlMode),
        mean_bitrate: get_u32(api, &CODECAPI_AVEncCommonMeanBitRate),
        buffer_size: get_u32(api, &CODECAPI_AVEncCommonBufferSize),
        max_bitrate: get_u32(api, &CODECAPI_AVEncCommonMaxBitRate),
        low_latency: get_bool(api, &CODECAPI_AVLowLatencyMode),
        b_pictures: get_u32(api, &CODECAPI_AVEncMPVDefaultBPictureCount),
        gop: get_u32(api, &CODECAPI_AVEncMPVGOPSize),
        quality_vs_speed: get_u32(api, &CODECAPI_AVEncCommonQualityVsSpeed),
        min_qp: get_u32(api, &CODECAPI_AVEncVideoMinQP),
        max_qp: get_u32(api, &CODECAPI_AVEncVideoMaxQP),
    }
}

/// Opções de cada perfil, na ordem em que são aplicadas.
fn opcoes_do_perfil(perfil: PerfilTaxa) -> &'static [Opcao] {
    use Opcao::*;
    match perfil {
        PerfilTaxa::Padrao | PerfilTaxa::Sw1 => &[RateControl, MeanBitRate, Gop, LowLatency, BPictures, BufferSize],
        PerfilTaxa::Sw2 | PerfilTaxa::Sw3 => &[
            RateControl,
            MeanBitRate,
            Gop,
            LowLatency,
            BPictures,
            BufferSize,
            QualityVsSpeed,
            MaxQp,
            MinQp,
        ],
        PerfilTaxa::Sw4 => &[
            RateControl,
            MaxBitRate,
            MeanBitRate,
            Gop,
            LowLatency,
            BPictures,
            BufferSize,
            QualityVsSpeed,
            MaxQp,
            MinQp,
        ],
    }
}

/// Nome da opção para os logs.
fn nome(op: Opcao) -> &'static str {
    match op {
        Opcao::RateControl => "AVEncCommonRateControlMode",
        Opcao::MeanBitRate => "AVEncCommonMeanBitRate",
        Opcao::BufferSize => "AVEncCommonBufferSize",
        Opcao::MaxBitRate => "AVEncCommonMaxBitRate",
        Opcao::LowLatency => "AVLowLatencyMode",
        Opcao::BPictures => "AVEncMPVDefaultBPictureCount",
        Opcao::Gop => "AVEncMPVGOPSize",
        Opcao::QualityVsSpeed => "AVEncCommonQualityVsSpeed",
        Opcao::MinQp => "AVEncVideoMinQP",
        Opcao::MaxQp => "AVEncVideoMaxQP",
    }
}

/// Aplica uma única opção com o valor que o perfil manda.
fn aplicar_um(api: &ICodecAPI, cfg: &EncoderConfig, perfil: PerfilTaxa, op: Opcao) -> windows::core::Result<()> {
    let bps = cfg.kbps.saturating_mul(1000);
    match op {
        Opcao::RateControl => {
            if perfil == PerfilTaxa::Sw4 {
                let pico = eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32;
                return set_u32(api, &CODECAPI_AVEncCommonRateControlMode, pico);
            }
            let (cbr, vbr) = (
                eAVEncCommonRateControlMode_CBR.0 as u32,
                eAVEncCommonRateControlMode_LowDelayVBR.0 as u32,
            );
            // CBR primeiro; se recusar, VBR de baixo atraso; se ambos recusam, vale o erro do segundo.
            match set_u32(api, &CODECAPI_AVEncCommonRateControlMode, cbr) {
                Ok(()) => Ok(()),
                Err(e1) => {
                    tracing::debug!(erro = %e1, "CBR recusado antes do LowDelayVBR");
                    set_u32(api, &CODECAPI_AVEncCommonRateControlMode, vbr)
                }
            }
        }
        Opcao::MeanBitRate => {
            let v = if perfil == PerfilTaxa::Sw4 {
                // 90% do teto, em u64 para não estourar.
                u32::try_from(u64::from(cfg.kbps) * 1000 * 9 / 10).unwrap_or(u32::MAX)
            } else {
                bps
            };
            set_u32(api, &CODECAPI_AVEncCommonMeanBitRate, v)
        }
        Opcao::MaxBitRate => set_u32(api, &CODECAPI_AVEncCommonMaxBitRate, bps),
        Opcao::BufferSize => {
            let v = if perfil == PerfilTaxa::Sw3 {
                bps / 8
            } else {
                logic::buffer_size_bits(cfg.kbps)
            };
            set_u32(api, &CODECAPI_AVEncCommonBufferSize, v)
        }
        Opcao::Gop => set_u32(api, &CODECAPI_AVEncMPVGOPSize, cfg.gop_frames),
        // AVLowLatencyMode é VT_BOOL na documentação.
        Opcao::LowLatency => set_bool(api, &CODECAPI_AVLowLatencyMode, true),
        // Sem B-frames: cada quadro sai na ordem de entrada.
        Opcao::BPictures => set_u32(api, &CODECAPI_AVEncMPVDefaultBPictureCount, 0),
        Opcao::QualityVsSpeed => set_u32(api, &CODECAPI_AVEncCommonQualityVsSpeed, 0),
        Opcao::MaxQp => set_u32(api, &CODECAPI_AVEncVideoMaxQP, 51),
        Opcao::MinQp => set_u32(api, &CODECAPI_AVEncVideoMinQP, 0),
    }
}

/// Reaplica só as opções em `recusadas` (depois do `SetInputType`); devolve as que ainda recusam.
#[allow(dead_code)]
pub(crate) fn reapply_settings(
    api: &ICodecAPI,
    cfg: &EncoderConfig,
    perfil: PerfilTaxa,
    recusadas: &[Opcao],
) -> Vec<Opcao> {
    let mut ainda = Vec::new();
    for &op in recusadas {
        if let Err(e) = aplicar_um(api, cfg, perfil, op) {
            tracing::debug!(opcao = nome(op), erro = %e, "opção ainda recusada na reaplicação");
            ainda.push(op);
        }
    }
    ainda
}

/// Aplica as opções do perfil em ordem (ver `opcoes_do_perfil`). Cada opção recusada é logada
/// (`tracing::warn!`) e entra no `Vec` devolvido (para `reapply_settings`), exceto o bitrate
/// médio: se recusado devolve erro.
pub(crate) fn apply_settings(
    api: &ICodecAPI,
    cfg: &EncoderConfig,
    perfil: PerfilTaxa,
) -> Result<Vec<Opcao>, EncodeError> {
    let mut recusadas = Vec::new();
    for &op in opcoes_do_perfil(perfil) {
        if let Err(e) = aplicar_um(api, cfg, perfil, op) {
            if op == Opcao::MeanBitRate {
                return Err(EncodeError::os(nome(op), &e));
            }
            avisa(nome(op), &e);
            recusadas.push(op);
        }
    }
    Ok(recusadas)
}

/// `AVEncCommonMeanBitRate = kbps*1000` em tempo de execução. `Err` se o MFT recusar.
pub(crate) fn set_bitrate(api: &ICodecAPI, kbps: u32) -> Result<(), EncodeError> {
    set_u32(api, &CODECAPI_AVEncCommonMeanBitRate, kbps.saturating_mul(1000))
        .map_err(|e| EncodeError::os("AVEncCommonMeanBitRate", &e))
}

/// `AVEncVideoForceKeyFrame = 1` (VT_UI4). Erro é só logado (devolve false).
pub(crate) fn force_keyframe(api: &ICodecAPI) -> bool {
    match set_u32(api, &CODECAPI_AVEncVideoForceKeyFrame, 1) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(opcao = "AVEncVideoForceKeyFrame", erro = %e, "não foi possível forçar keyframe");
            false
        }
    }
}

/// Lê de volta todas as opções (`readback`) e loga com `tracing::info!`. Nunca falha.
pub(crate) fn log_readback(api: &ICodecAPI) {
    let rb = readback(api);
    tracing::info!(?rb, "ICodecAPI readback");
}

/// `GetValue` esperando `VT_UI4`. Qualquer erro ou tipo diferente vira `None`.
fn get_u32(api: &ICodecAPI, key: &GUID) -> Option<u32> {
    // SAFETY: `key` é referência viva durante a chamada.
    let mut v = unsafe { api.GetValue(key) }.ok()?;
    // SAFETY: o campo `ulVal` só é lido depois de conferir `vt == VT_UI4`; o tipo é escalar.
    let r = unsafe {
        let interno = &v.Anonymous.Anonymous;
        if interno.vt == VT_UI4 {
            Some(interno.Anonymous.ulVal)
        } else {
            None
        }
    };
    // SAFETY: `v` é um VARIANT válido devolvido pelo MFT; limpar libera o que ele tiver alocado.
    let _ = unsafe { VariantClear(&mut v) };
    r
}

/// `GetValue` esperando `VT_BOOL` (aceita também `VT_UI4`, != 0). Outro caso vira `None`.
fn get_bool(api: &ICodecAPI, key: &GUID) -> Option<bool> {
    // SAFETY: `key` é referência viva durante a chamada.
    let mut v = unsafe { api.GetValue(key) }.ok()?;
    // SAFETY: cada campo só é lido depois de conferir `vt`; os tipos são escalares.
    let r = unsafe {
        let interno = &v.Anonymous.Anonymous;
        if interno.vt == VT_BOOL {
            Some(interno.Anonymous.boolVal.0 != 0)
        } else if interno.vt == VT_UI4 {
            Some(interno.Anonymous.ulVal != 0)
        } else {
            None
        }
    };
    // SAFETY: `v` é um VARIANT válido devolvido pelo MFT.
    let _ = unsafe { VariantClear(&mut v) };
    r
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{enumerate, mf::MfGuard, EncoderChoice};
    use windows::core::Interface;
    use windows::Win32::Media::MediaFoundation::IMFTransform;

    #[test]
    fn variant_ui4_tem_valor() {
        let v = make_u32_variant(0xDEAD_BEEF);
        // SAFETY: acabamos de montar o variant com o campo `ulVal` ativo e `vt = VT_UI4`.
        unsafe {
            let interno = &v.Anonymous.Anonymous;
            assert_eq!(interno.vt, VT_UI4);
            assert_eq!(interno.Anonymous.ulVal, 0xDEAD_BEEF);
        }
    }

    #[test]
    fn apply_settings_no_mft_de_software() {
        let Ok(_mf) = MfGuard::acquire() else {
            eprintln!("pulando: MFStartup indisponível");
            return;
        };
        let candidatos = match enumerate::enumerate(false) {
            Ok(c) if !c.is_empty() => c,
            _ => {
                eprintln!("pulando: nenhum MFT de software");
                return;
            }
        };
        // SAFETY: ativador válido vindo da enumeração.
        let mft: IMFTransform = unsafe { candidatos[0].activate.ActivateObject() }.expect("ativar MFT");
        let api = mft.cast::<ICodecAPI>().expect("ICodecAPI");
        let cfg = EncoderConfig {
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 2000,
            gop_frames: 300,
            choice: EncoderChoice::Auto,
        };
        apply_settings(&api, &cfg, PerfilTaxa::Padrao).expect("apply_settings");
    }

    #[test]
    fn log_readback_nao_entra_em_panico() {
        let Ok(_mf) = MfGuard::acquire() else {
            eprintln!("pulando: MFStartup indisponível");
            return;
        };
        let candidatos = match enumerate::enumerate(false) {
            Ok(c) if !c.is_empty() => c,
            _ => {
                eprintln!("pulando: nenhum MFT de software");
                return;
            }
        };
        // SAFETY: ativador válido vindo da enumeração.
        let mft: IMFTransform = unsafe { candidatos[0].activate.ActivateObject() }.expect("ativar MFT");
        let api = mft.cast::<ICodecAPI>().expect("ICodecAPI");
        log_readback(&api);
    }

    /// Ativa o primeiro MFT de software; `None` se indisponível.
    fn mft_software() -> Option<(MfGuard, IMFTransform, ICodecAPI, EncoderConfig)> {
        let mf = MfGuard::acquire().ok()?;
        let candidatos = enumerate::enumerate(false).ok().filter(|c| !c.is_empty())?;
        // SAFETY: ativador válido vindo da enumeração.
        let mft: IMFTransform = unsafe { candidatos[0].activate.ActivateObject() }.ok()?;
        let api = mft.cast::<ICodecAPI>().ok()?;
        let cfg = EncoderConfig {
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 2000,
            gop_frames: 300,
            choice: EncoderChoice::Auto,
        };
        Some((mf, mft, api, cfg))
    }

    #[test]
    fn readback_default_e_todo_none() {
        let rb = CodecReadback::default();
        assert_eq!(rb.rate_control, None);
        assert_eq!(rb.mean_bitrate, None);
        assert_eq!(rb.buffer_size, None);
        assert_eq!(rb.max_bitrate, None);
        assert_eq!(rb.low_latency, None);
        assert_eq!(rb.b_pictures, None);
        assert_eq!(rb.gop, None);
        assert_eq!(rb.quality_vs_speed, None);
        assert_eq!(rb.min_qp, None);
        assert_eq!(rb.max_qp, None);
    }

    #[test]
    fn readback_do_software_le_o_que_foi_aplicado() {
        let Some((_mf, mft, api, cfg)) = mft_software() else {
            eprintln!("pulando: sem MFT de software");
            return;
        };
        let saida = crate::media_types::make_output_type(&cfg).expect("tipo de saída");
        // SAFETY: tipo de mídia válido recém-criado.
        unsafe { mft.SetOutputType(0, &saida, 0) }.expect("SetOutputType");
        let entrada = crate::media_types::make_input_type(&cfg).expect("tipo de entrada");
        // SAFETY: idem.
        unsafe { mft.SetInputType(0, &entrada, 0) }.expect("SetInputType");
        let recusadas = apply_settings(&api, &cfg, PerfilTaxa::Sw2).expect("apply_settings");
        let rb = readback(&api);
        eprintln!("recusadas={recusadas:?} readback={rb:?}");
        assert_eq!(rb.mean_bitrate, Some(cfg.kbps * 1000));
    }

    #[test]
    fn apply_settings_devolve_recusadas() {
        let Some((_mf, _mft, api, cfg)) = mft_software() else {
            eprintln!("pulando: sem MFT de software");
            return;
        };
        for perfil in [
            PerfilTaxa::Padrao,
            PerfilTaxa::Sw1,
            PerfilTaxa::Sw2,
            PerfilTaxa::Sw3,
            PerfilTaxa::Sw4,
        ] {
            let r = apply_settings(&api, &cfg, perfil);
            eprintln!("{perfil:?}: {r:?}");
        }
    }

    #[test]
    fn reapply_vazio_nao_faz_nada() {
        let Some((_mf, _mft, api, cfg)) = mft_software() else {
            eprintln!("pulando: sem MFT de software");
            return;
        };
        assert!(reapply_settings(&api, &cfg, PerfilTaxa::Sw2, &[]).is_empty());
    }
}
