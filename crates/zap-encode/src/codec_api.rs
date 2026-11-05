//! Configuração por `ICodecAPI`.
use crate::{logic, EncodeError, EncoderConfig};
use std::mem::ManuallyDrop;
use windows::core::GUID;
use windows::Win32::Foundation::{VARIANT_BOOL, VARIANT_TRUE};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_CBR, eAVEncCommonRateControlMode_LowDelayVBR, CODECAPI_AVEncCommonBufferSize,
    CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount,
    CODECAPI_AVEncMPVGOPSize, CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode, ICodecAPI,
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

/// Aplica: `AVEncCommonRateControlMode = CBR` (se recusar tenta `LowDelayVBR`, depois deixa o
/// padrão), `AVEncCommonMeanBitRate = kbps*1000`, `AVEncMPVGOPSize = gop_frames`,
/// `AVLowLatencyMode = true`, `AVEncMPVDefaultBPictureCount = 0`,
/// `AVEncCommonBufferSize = logic::buffer_size_bits(kbps)`. Cada opção recusada é logada uma vez
/// (`tracing::warn!`) e ignorada, exceto o bitrate: se recusado devolve erro.
pub(crate) fn apply_settings(api: &ICodecAPI, cfg: &EncoderConfig) -> Result<(), EncodeError> {
    // Controle de taxa: CBR, senão VBR de baixo atraso, senão o padrão do MFT.
    let (cbr, vbr) = (
        eAVEncCommonRateControlMode_CBR.0 as u32,
        eAVEncCommonRateControlMode_LowDelayVBR.0 as u32,
    );
    if let Err(e1) = set_u32(api, &CODECAPI_AVEncCommonRateControlMode, cbr) {
        if let Err(e2) = set_u32(api, &CODECAPI_AVEncCommonRateControlMode, vbr) {
            avisa("AVEncCommonRateControlMode", &e2);
            tracing::debug!(erro = %e1, "CBR recusado antes do LowDelayVBR");
        }
    }

    // Bitrate médio: obrigatório.
    set_bitrate(api, cfg.kbps)?;

    if let Err(e) = set_u32(api, &CODECAPI_AVEncMPVGOPSize, cfg.gop_frames) {
        avisa("AVEncMPVGOPSize", &e);
    }
    // AVLowLatencyMode é VT_BOOL na documentação.
    if let Err(e) = set_bool(api, &CODECAPI_AVLowLatencyMode, true) {
        avisa("AVLowLatencyMode", &e);
    }
    // Sem B-frames: cada quadro sai na ordem de entrada.
    if let Err(e) = set_u32(api, &CODECAPI_AVEncMPVDefaultBPictureCount, 0) {
        avisa("AVEncMPVDefaultBPictureCount", &e);
    }
    if let Err(e) = set_u32(api, &CODECAPI_AVEncCommonBufferSize, logic::buffer_size_bits(cfg.kbps)) {
        avisa("AVEncCommonBufferSize", &e);
    }
    Ok(())
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

/// Lê de volta `AVEncCommonRateControlMode` e `AVEncCommonMeanBitRate` (`ICodecAPI::GetValue`) e
/// loga com `tracing::info!`. Nunca falha; valor ilegível vira `None` no log.
pub(crate) fn log_readback(api: &ICodecAPI) {
    let modo = get_u32(api, &CODECAPI_AVEncCommonRateControlMode);
    let bitrate_bps = get_u32(api, &CODECAPI_AVEncCommonMeanBitRate);
    tracing::info!(modo = ?modo, bitrate_bps = ?bitrate_bps, "ICodecAPI readback");
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
        apply_settings(&api, &cfg).expect("apply_settings");
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
}
