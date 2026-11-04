//! Lógica pura (sem Windows) do codificador.
use crate::{EncoderChoice, EncoderInfo};
use zap_gpu::Vendor;

/// µs do relógio do processo -> unidades de 100 ns do Media Foundation (`x * 10`, saturando).
pub fn us_to_hns(us: u64) -> i64 {
    // Saturando: um valor fora de i64 vira i64::MAX em vez de estourar.
    i64::try_from(us.saturating_mul(10)).unwrap_or(i64::MAX)
}

/// 100 ns -> µs (negativo vira 0).
pub fn hns_to_us(hns: i64) -> u64 {
    if hns < 0 {
        0
    } else {
        (hns / 10) as u64
    }
}

/// Duração de um quadro em unidades de 100 ns: `10_000_000 / fps` (fps 0 vira 1).
pub fn frame_duration_hns(fps: u32) -> i64 {
    10_000_000 / i64::from(fps.max(1))
}

/// Tamanho do buffer do codificador em bits: `kbps * 1000 * 0.25` (arredondado para baixo, >= 1).
pub fn buffer_size_bits(kbps: u32) -> u32 {
    // u64 para não estourar na multiplicação; depois satura em u32.
    let bits = u64::from(kbps) * 1000 / 4;
    u32::try_from(bits).unwrap_or(u32::MAX).max(1)
}

/// Bytes esperados por quadro não-chave: `kbps * 1000 / 8 / fps` (fps 0 vira 1).
pub fn expected_frame_bytes(kbps: u32, fps: u32) -> u32 {
    let bytes = u64::from(kbps) * 1000 / 8 / u64::from(fps.max(1));
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

/// Fabricante a partir do atributo `MFT_ENUM_HARDWARE_VENDOR_ID` ("VEN_10DE", "ven_8086"...) ou
/// de um nome. Hex após "VEN_" (qualquer caixa) vira `Vendor::from_pci_id`. Sem "VEN_": procura
/// "nvidia", "amd"/"advanced micro", "intel" no nome (sem diferenciar caixa); senão `Other(0)`.
pub fn vendor_from_mft_string(s: &str) -> Vendor {
    // to_ascii_lowercase mantém o tamanho em bytes, então as posições batem com `s`.
    let minusculas = s.to_ascii_lowercase();

    if let Some(pos) = minusculas.find("ven_") {
        let hex: String = minusculas[pos + 4..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .take(4)
            .collect();
        if let Ok(valor) = u32::from_str_radix(&hex, 16) {
            return Vendor::from_pci_id(valor);
        }
        // Hex inválido ou ausente: cai na busca pelo nome.
    }

    if minusculas.contains("nvidia") {
        Vendor::Nvidia
    } else if minusculas.contains("advanced micro") || minusculas.contains("amd") {
        Vendor::Amd
    } else if minusculas.contains("intel") {
        Vendor::Intel
    } else {
        Vendor::Other(0)
    }
}

/// Valida a configuração: largura/altura > 0 e pares, fps 1..=240, kbps > 0, gop_frames > 0.
/// Erro: `EncodeError::InvalidConfig` com a razão.
pub fn validate_config(cfg: &crate::EncoderConfig) -> Result<(), crate::EncodeError> {
    use crate::EncodeError;

    let invalida = |motivo: &str| Err(EncodeError::InvalidConfig(motivo.to_string()));

    if cfg.width == 0 || cfg.height == 0 {
        return invalida("largura e altura devem ser maiores que zero");
    }
    // H.264 com subamostragem 4:2:0 exige dimensões pares.
    if cfg.width % 2 != 0 || cfg.height % 2 != 0 {
        return invalida("largura e altura devem ser pares");
    }
    if !(1..=240).contains(&cfg.fps) {
        return invalida("fps deve estar entre 1 e 240");
    }
    if cfg.kbps == 0 {
        return invalida("taxa de bits (kbps) deve ser maior que zero");
    }
    if cfg.gop_frames == 0 {
        return invalida("quadros por GOP devem ser maiores que zero");
    }
    Ok(())
}

/// Ordem de tentativa dos codificadores (índices de `found`) para uma escolha.
/// `Auto`: hardware do `gpu_vendor` -> outros hardware -> software. `Hardware`: só hardware
/// (mesma ordem). `Software`: só software. A ordem original é mantida dentro de cada grupo.
pub fn order_candidates(found: &[EncoderInfo], choice: EncoderChoice, gpu_vendor: Vendor) -> Vec<usize> {
    let mesmo_fabricante = |i: &usize| found[*i].hardware && found[*i].vendor == gpu_vendor;
    let outro_hardware = |i: &usize| found[*i].hardware && found[*i].vendor != gpu_vendor;
    let software = |i: &usize| !found[*i].hardware;

    let indices: Vec<usize> = (0..found.len()).collect();
    let hw_do_gpu: Vec<usize> = indices.iter().copied().filter(mesmo_fabricante).collect();
    let hw_outros: Vec<usize> = indices.iter().copied().filter(outro_hardware).collect();
    let sw: Vec<usize> = indices.iter().copied().filter(software).collect();

    match choice {
        EncoderChoice::Auto => {
            let mut ordem = hw_do_gpu;
            ordem.extend(hw_outros);
            ordem.extend(sw);
            ordem
        }
        EncoderChoice::Hardware => {
            let mut ordem = hw_do_gpu;
            ordem.extend(hw_outros);
            ordem
        }
        EncoderChoice::Software => sw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EncodeError, EncoderConfig};

    fn info(nome: &str, vendor: Vendor, hardware: bool) -> EncoderInfo {
        EncoderInfo {
            name: nome.to_string(),
            vendor,
            hardware,
        }
    }

    fn cfg_base() -> EncoderConfig {
        EncoderConfig {
            width: 1920,
            height: 1080,
            fps: 60,
            kbps: 6000,
            gop_frames: 600,
            choice: EncoderChoice::Auto,
        }
    }

    #[test]
    fn conversao_us_hns_ida_e_volta() {
        assert_eq!(us_to_hns(1_000_000), 10_000_000);
        assert_eq!(hns_to_us(10_000_000), 1_000_000);
        // Saturação: u64::MAX * 10 não cabe em i64.
        assert_eq!(us_to_hns(u64::MAX), i64::MAX);
    }

    #[test]
    fn hns_negativo_vira_zero() {
        assert_eq!(hns_to_us(-1), 0);
        assert_eq!(hns_to_us(i64::MIN), 0);
    }

    #[test]
    fn frame_duration_60fps() {
        assert_eq!(frame_duration_hns(60), 166_666);
        // fps zero vira 1.
        assert_eq!(frame_duration_hns(0), 10_000_000);
    }

    #[test]
    fn buffer_size_1000kbps_e_250000() {
        assert_eq!(buffer_size_bits(1000), 250_000);
        // Mínimo 1.
        assert_eq!(buffer_size_bits(0), 1);
        // Não estoura com valores grandes.
        assert_eq!(buffer_size_bits(u32::MAX), u32::MAX);
    }

    #[test]
    fn expected_frame_bytes_4000kbps_60fps() {
        assert_eq!(expected_frame_bytes(4000, 60), 8333);
        // fps zero vira 1.
        assert_eq!(expected_frame_bytes(8, 0), 1000);
    }

    #[test]
    fn vendor_ven_10de_nvidia_caixa_mista() {
        assert_eq!(vendor_from_mft_string("ven_10DE"), Vendor::Nvidia);
        assert_eq!(vendor_from_mft_string("VEN_8086"), Vendor::Intel);
        assert_eq!(vendor_from_mft_string("VEN_1002"), Vendor::Amd);
    }

    #[test]
    fn vendor_por_nome() {
        assert_eq!(vendor_from_mft_string("NVIDIA H.264 Encoder MFT"), Vendor::Nvidia);
        assert_eq!(vendor_from_mft_string("Advanced Micro Devices H.264"), Vendor::Amd);
        assert_eq!(vendor_from_mft_string("Intel Quick Sync Video H.264"), Vendor::Intel);
    }

    #[test]
    fn vendor_desconhecido_other0() {
        assert_eq!(vendor_from_mft_string("Encoder desconhecido"), Vendor::Other(0));
        assert_eq!(vendor_from_mft_string(""), Vendor::Other(0));
        // VEN_ com hex inválido cai na busca por nome.
        assert_eq!(vendor_from_mft_string("VEN_XYZ Intel"), Vendor::Intel);
    }

    #[test]
    fn validate_config_casos_ok() {
        assert!(validate_config(&cfg_base()).is_ok());
        let mut c = cfg_base();
        c.fps = 1;
        assert!(validate_config(&c).is_ok());
        c.fps = 240;
        assert!(validate_config(&c).is_ok());
    }

    #[test]
    fn validate_config_erros() {
        let mut c = cfg_base();
        c.width = 0;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.height = 0;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.width = 1921;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.height = 1081;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.fps = 0;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.fps = 241;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.kbps = 0;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));

        let mut c = cfg_base();
        c.gop_frames = 0;
        assert!(matches!(validate_config(&c), Err(EncodeError::InvalidConfig(_))));
    }

    #[test]
    fn order_candidates_mista() {
        // Índices: 0 sw, 1 Nvidia hw, 2 Intel hw, 3 sw, 4 Amd hw, 5 Intel hw.
        let found = vec![
            info("sw0", Vendor::Microsoft, false),
            info("nv", Vendor::Nvidia, true),
            info("intel1", Vendor::Intel, true),
            info("sw3", Vendor::Microsoft, false),
            info("amd", Vendor::Amd, true),
            info("intel5", Vendor::Intel, true),
        ];
        let gpu = Vendor::Intel;

        assert_eq!(
            order_candidates(&found, EncoderChoice::Auto, gpu),
            vec![2, 5, 1, 4, 0, 3]
        );
        assert_eq!(order_candidates(&found, EncoderChoice::Hardware, gpu), vec![2, 5, 1, 4]);
        assert_eq!(order_candidates(&found, EncoderChoice::Software, gpu), vec![0, 3]);
    }

    #[test]
    fn order_candidates_lista_vazia() {
        let vazia: Vec<EncoderInfo> = Vec::new();
        assert!(order_candidates(&vazia, EncoderChoice::Auto, Vendor::Nvidia).is_empty());
        assert!(order_candidates(&vazia, EncoderChoice::Hardware, Vendor::Nvidia).is_empty());
        assert!(order_candidates(&vazia, EncoderChoice::Software, Vendor::Nvidia).is_empty());
    }
}
