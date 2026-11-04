//! Código de convite `ZAP-XXXX`.
use crate::error::ProtoError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Código de convite `ZAP-XXXX` (alfabeto sem 0/O/1/I/L).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoomCode(String);

impl RoomCode {
    pub const ALPHABET: &'static [u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

    /// Cada byte `% 31` escolhe um caractere do alfabeto.
    pub fn from_entropy(bytes: [u8; 4]) -> Self {
        let mut s = String::with_capacity(8);
        s.push_str("ZAP-");
        for b in bytes {
            // `% 31` mantém o índice dentro do alfabeto (31 caracteres).
            s.push(char::from(Self::ALPHABET[usize::from(b) % Self::ALPHABET.len()]));
        }
        Self(s)
    }

    /// Aceita minúsculas, espaços nas pontas e sem o prefixo "ZAP-".
    pub fn parse(text: &str) -> Result<Self, ProtoError> {
        // Normaliza antes de validar: aceita "zap-7k2q", "  7K2Q " etc.
        let upper = text.trim().to_ascii_uppercase();
        let body = upper.strip_prefix("ZAP-").unwrap_or(&upper);
        let valid = body.len() == 4 && body.bytes().all(|b| Self::ALPHABET.contains(&b));
        if !valid {
            return Err(ProtoError::InvalidRoomCode(text.to_string()));
        }
        Ok(Self(format!("ZAP-{body}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RoomCode {
    type Error = ProtoError;
    fn try_from(s: String) -> Result<Self, ProtoError> {
        Self::parse(&s)
    }
}

impl From<RoomCode> for String {
    fn from(c: RoomCode) -> String {
        c.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_com_prefixo_minusculo() {
        let c = RoomCode::parse("zap-7k2q").expect("código válido");
        assert_eq!(c.as_str(), "ZAP-7K2Q");
    }

    #[test]
    fn parse_sem_prefixo() {
        assert_eq!(RoomCode::parse("7K2Q").expect("ok").as_str(), "ZAP-7K2Q");
        assert_eq!(RoomCode::parse("  7k2q ").expect("ok").as_str(), "ZAP-7K2Q");
    }

    #[test]
    fn letra_proibida() {
        for t in ["ZAP-7K2O", "ZAP-7K21", "ZAP-7K2L", "ZAP-7K2I", "ZAP-7K20"] {
            assert!(
                matches!(RoomCode::parse(t), Err(ProtoError::InvalidRoomCode(_))),
                "deveria recusar {t}"
            );
        }
    }

    #[test]
    fn tamanho_errado() {
        for t in ["ZAP-7K2", "ZAP-7K2QQ", ""] {
            assert!(
                matches!(RoomCode::parse(t), Err(ProtoError::InvalidRoomCode(_))),
                "deveria recusar {t:?}"
            );
        }
    }

    #[test]
    fn entropia() {
        assert_eq!(RoomCode::from_entropy([0, 1, 2, 3]).as_str(), "ZAP-2345");
        // 31 % 31 = 0 ('2'), 32 % 31 = 1 ('3'), 62 % 31 = 0 ('2'), 255 % 31 = 7 ('9')
        let c = RoomCode::from_entropy([31, 32, 62, 255]);
        assert_eq!(c.as_str(), "ZAP-2329");
        let back = RoomCode::parse(c.as_str()).expect("round-trip");
        assert_eq!(back, c);
    }

    #[test]
    fn serde() {
        let code = RoomCode::parse("zap-7k2q").expect("ok");
        assert_eq!(serde_json::to_string(&code).expect("ser"), "\"ZAP-7K2Q\"");
        let de: RoomCode = serde_json::from_str("\"zap-7k2q\"").expect("de");
        assert_eq!(de, code);
        assert!(serde_json::from_str::<RoomCode>("\"ZAP-7K2O\"").is_err());
    }
}
