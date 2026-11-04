//! HMAC de senha do modo Local e base64.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::ProtoError;

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256(chave = senha UTF-8, msg = nonce).
#[allow(clippy::expect_used)]
pub fn password_hmac(password: &str, nonce: &[u8]) -> [u8; 32] {
    // HMAC aceita chave de qualquer tamanho (chaves longas são hasheadas internamente),
    // então `new_from_slice` nunca falha aqui: o `expect` é inalcançável.
    let mut mac =
        HmacSha256::new_from_slice(password.as_bytes()).expect("HMAC-SHA256 aceita chave de qualquer tamanho");
    mac.update(nonce);
    mac.finalize().into_bytes().into()
}

/// Comparação em tempo constante.
pub fn verify_password_hmac(password: &str, nonce: &[u8], mac: &[u8]) -> bool {
    // MAC de tamanho errado nunca é válido; checa antes para não depender do detalhe da crate.
    if mac.len() != 32 {
        return false;
    }
    #[allow(clippy::expect_used)]
    let mut hmac =
        HmacSha256::new_from_slice(password.as_bytes()).expect("HMAC-SHA256 aceita chave de qualquer tamanho");
    hmac.update(nonce);
    // `verify_slice` compara em tempo constante (não usar `==` aqui).
    hmac.verify_slice(mac).is_ok()
}

/// Base64 padrão com padding.
pub fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn unb64(text: &str) -> Result<Vec<u8>, ProtoError> {
    STANDARD.decode(text).map_err(|_| ProtoError::Base64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn vetor_fixo() {
        // Gerado com: python3 -c "import hmac,hashlib; print(hmac.new(b'senha', bytes(range(32)), hashlib.sha256).hexdigest())"
        let esperado = "1d99d238a1c44d7b59c01959dda413acf4931752ec669ca1a39a0c24e1bda2eb";
        let nonce: Vec<u8> = (0..32).collect();
        assert_eq!(hex(&password_hmac("senha", &nonce)), esperado);
    }

    #[test]
    fn verifica_certo() {
        let nonce: Vec<u8> = (0..32).collect();
        let mac = password_hmac("senha", &nonce);
        assert!(verify_password_hmac("senha", &nonce, &mac));
    }

    #[test]
    fn verifica_errado() {
        let nonce: Vec<u8> = (0..32).collect();
        let outro_nonce: Vec<u8> = (1..33).collect();
        let mac = password_hmac("senha", &nonce);

        // senha errada
        assert!(!verify_password_hmac("outra", &nonce, &mac));
        // nonce diferente
        assert!(!verify_password_hmac("senha", &outro_nonce, &mac));
        // mac truncado (31 bytes)
        assert!(!verify_password_hmac("senha", &nonce, &mac[..31]));
        // mac vazio
        assert!(!verify_password_hmac("senha", &nonce, &[]));
    }

    #[test]
    fn b64_roundtrip() {
        for bytes in [vec![0u8, 1, 2, 253, 254, 255], Vec::new()] {
            let texto = b64(&bytes);
            assert_eq!(unb64(&texto).ok(), Some(bytes));
        }
    }

    #[test]
    fn b64_conhecido() {
        assert_eq!(b64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn unb64_invalido() {
        assert!(matches!(unb64("@@@"), Err(ProtoError::Base64)));
    }
}
