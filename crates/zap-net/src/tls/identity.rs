//! Identidade autoassinada e impressão digital.

use sha2::{Digest, Sha256};

use crate::NetError;

pub struct Identity {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

impl Identity {
    /// Gera um certificado autoassinado para `name` (chave PKCS#8 em `key_der`).
    pub fn generate(name: &str) -> Result<Self, NetError> {
        // O certificado é autoassinado: a confiança vem da impressão digital fixada,
        // não de uma CA. Por isso o SAN é só o nome lógico do nó.
        let ck =
            rcgen::generate_simple_self_signed(vec![name.to_string()]).map_err(|e| NetError::Tls(e.to_string()))?;
        Ok(Self {
            cert_der: ck.cert.der().to_vec(),
            key_der: ck.signing_key.serialize_der(),
        })
    }

    /// SHA-256 hex minúsculo do certificado.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.cert_der)
    }
}

/// SHA-256 hex minúsculo (64 caracteres) do certificado DER.
pub fn fingerprint_of(cert_der: &[u8]) -> String {
    Sha256::digest(cert_der).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_identidade_valida() {
        let id = Identity::generate("zap").expect("geração de identidade");
        assert!(!id.cert_der.is_empty());
        assert!(!id.key_der.is_empty());
    }

    #[test]
    fn fingerprint_64_hex_minusculo() {
        let id = Identity::generate("zap").expect("geração de identidade");
        let fp = id.fingerprint();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    }

    #[test]
    fn fingerprint_estavel() {
        let id = Identity::generate("zap").expect("geração de identidade");
        assert_eq!(id.fingerprint(), id.fingerprint());
    }

    #[test]
    fn identidades_distintas() {
        let a = Identity::generate("zap").expect("geração de identidade");
        let b = Identity::generate("zap").expect("geração de identidade");
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn vetor_sha256() {
        assert_eq!(
            fingerprint_of(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
