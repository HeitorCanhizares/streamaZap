//! `PROPERTYKEY` do Voice Capture DSP (`MFPKEY_WMAAECMA_*`), que o crate `windows` 0.62 não traz.
//! Os GUIDs vêm de `wmcodecdsp.h` do Windows SDK instalado (10.0.26100.0). No header, o `pid` de
//! cada chave é `PID_FIRST_USABLE + n`, com `PID_FIRST_USABLE = 2` (`propkeydef.h`).

#![allow(dead_code)]

use windows::core::GUID;
use windows::Win32::Foundation::PROPERTYKEY;

/// `fmtid` comum a todas as chaves `MFPKEY_WMAAECMA_*` (`wmcodecdsp.h`).
const FMTID_WMAAECMA: GUID = GUID::from_values(
    0x6f52c567,
    0x0360,
    0x4bd2,
    [0x96, 0x17, 0xcc, 0xbf, 0x14, 0x21, 0xc9, 0x39],
);

/// Primeiro `pid` utilizável (`PID_FIRST_USABLE`, `propkeydef.h`).
const PID_FIRST_USABLE: u32 = 2;

const fn chave(n: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: FMTID_WMAAECMA,
        pid: PID_FIRST_USABLE + n,
    }
}

/// `wmcodecdsp.h` linha 3292: `PID_FIRST_USABLE + 0`.
pub const MFPKEY_WMAAECMA_SYSTEM_MODE: PROPERTYKEY = chave(0);
/// `wmcodecdsp.h` linha 3293: `PID_FIRST_USABLE + 1`.
pub const MFPKEY_WMAAECMA_DMO_SOURCE_MODE: PROPERTYKEY = chave(1);
/// `wmcodecdsp.h` linha 3294: `PID_FIRST_USABLE + 2`.
pub const MFPKEY_WMAAECMA_DEVICE_INDEXES: PROPERTYKEY = chave(2);
/// `wmcodecdsp.h` linha 3295: `PID_FIRST_USABLE + 3`.
pub const MFPKEY_WMAAECMA_FEATURE_MODE: PROPERTYKEY = chave(3);
/// `wmcodecdsp.h` linha 3297: `PID_FIRST_USABLE + 5`.
pub const MFPKEY_WMAAECMA_FEATR_ECHO_LENGTH: PROPERTYKEY = chave(5);
/// `wmcodecdsp.h` linha 3298: `PID_FIRST_USABLE + 6`.
pub const MFPKEY_WMAAECMA_FEATR_NS: PROPERTYKEY = chave(6);
/// `wmcodecdsp.h` linha 3299: `PID_FIRST_USABLE + 7`.
pub const MFPKEY_WMAAECMA_FEATR_AGC: PROPERTYKEY = chave(7);
/// `wmcodecdsp.h` linha 3300: `PID_FIRST_USABLE + 8`.
pub const MFPKEY_WMAAECMA_FEATR_AES: PROPERTYKEY = chave(8);
/// `wmcodecdsp.h` linha 3304: `PID_FIRST_USABLE + 12`.
pub const MFPKEY_WMAAECMA_RETRIEVE_TS_STATS: PROPERTYKEY = chave(12);

/// `AEC_SYSTEM_MODE::SINGLE_CHANNEL_AEC` (`wmcodecdsp.h` linha 3796): um microfone com AEC.
pub const AEC_SYSTEM_MODE_SINGLE_CHANNEL_AEC: i32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    fn todas() -> Vec<(&'static str, PROPERTYKEY)> {
        vec![
            ("SYSTEM_MODE", MFPKEY_WMAAECMA_SYSTEM_MODE),
            ("DMO_SOURCE_MODE", MFPKEY_WMAAECMA_DMO_SOURCE_MODE),
            ("DEVICE_INDEXES", MFPKEY_WMAAECMA_DEVICE_INDEXES),
            ("FEATURE_MODE", MFPKEY_WMAAECMA_FEATURE_MODE),
            ("FEATR_ECHO_LENGTH", MFPKEY_WMAAECMA_FEATR_ECHO_LENGTH),
            ("FEATR_NS", MFPKEY_WMAAECMA_FEATR_NS),
            ("FEATR_AGC", MFPKEY_WMAAECMA_FEATR_AGC),
            ("FEATR_AES", MFPKEY_WMAAECMA_FEATR_AES),
            ("RETRIEVE_TS_STATS", MFPKEY_WMAAECMA_RETRIEVE_TS_STATS),
        ]
    }

    #[test]
    fn chaves_sao_distintas() {
        let t = todas();
        for (i, (na, a)) in t.iter().enumerate() {
            for (nb, b) in t.iter().skip(i + 1) {
                assert!(!(a.fmtid == b.fmtid && a.pid == b.pid), "{na} e {nb} são iguais");
            }
        }
    }

    #[test]
    fn pids_conferem_com_header() {
        assert_eq!(MFPKEY_WMAAECMA_SYSTEM_MODE.pid, 2);
        assert_eq!(MFPKEY_WMAAECMA_DMO_SOURCE_MODE.pid, 2 + 1);
        assert_eq!(MFPKEY_WMAAECMA_DEVICE_INDEXES.pid, 2 + 2);
        assert_eq!(MFPKEY_WMAAECMA_FEATURE_MODE.pid, 2 + 3);
        assert_eq!(MFPKEY_WMAAECMA_FEATR_ECHO_LENGTH.pid, 2 + 5);
        assert_eq!(MFPKEY_WMAAECMA_FEATR_NS.pid, 2 + 6);
        assert_eq!(MFPKEY_WMAAECMA_FEATR_AGC.pid, 2 + 7);
        assert_eq!(MFPKEY_WMAAECMA_FEATR_AES.pid, 2 + 8);
        assert_eq!(MFPKEY_WMAAECMA_RETRIEVE_TS_STATS.pid, 2 + 12);
        assert_eq!(AEC_SYSTEM_MODE_SINGLE_CHANNEL_AEC, 0);
    }

    #[test]
    fn fmtid_comum() {
        // No header, todas as chaves WMAAECMA usam o mesmo GUID 6f52c567-0360-4bd2-9617-ccbf1421c939.
        let esperado = GUID::from_values(
            0x6f52c567,
            0x0360,
            0x4bd2,
            [0x96, 0x17, 0xcc, 0xbf, 0x14, 0x21, 0xc9, 0x39],
        );
        for (nome, k) in todas() {
            assert_eq!(k.fmtid, esperado, "{nome}");
        }
    }
}
