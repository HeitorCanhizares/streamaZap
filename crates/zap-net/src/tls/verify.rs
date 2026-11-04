//! Verificador de certificado por impressão digital (modo Local).

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;

/// Aceita só o certificado cuja impressão digital (SHA-256 hex) é `fingerprint`.
#[derive(Debug)]
pub struct PinnedVerifier {
    pub(crate) fingerprint: String,
}

impl PinnedVerifier {
    pub fn new(fingerprint: &str) -> Self {
        Self {
            fingerprint: fingerprint.to_ascii_lowercase(),
        }
    }
}

// No modo Local o certificado é autoassinado e trocado fora de banda (pareamento);
// fixar a impressão digital já autentica o par, e a assinatura TLS segue verificada.
impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let atual = crate::tls::fingerprint_of(end_entity.as_ref());
        let a = atual.as_bytes();
        let b = self.fingerprint.as_bytes();
        // Comparação em tempo constante simples.
        let mut dif = (a.len() ^ b.len()) as u8;
        for (x, y) in a.iter().zip(b.iter()) {
            dif |= x ^ y;
        }
        if dif == 0 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::Identity;

    fn verifica(v: &PinnedVerifier, id: &Identity) -> Result<ServerCertVerified, rustls::Error> {
        v.verify_server_cert(
            &CertificateDer::from(id.cert_der.clone()),
            &[],
            &ServerName::try_from("a").unwrap(),
            &[],
            UnixTime::now(),
        )
    }

    #[test]
    fn aceita_mesma_impressao() {
        let id = Identity::generate("a").unwrap();
        assert!(verifica(&PinnedVerifier::new(&id.fingerprint()), &id).is_ok());
    }

    #[test]
    fn rejeita_outra_impressao() {
        let id = Identity::generate("a").unwrap();
        let outra = Identity::generate("b").unwrap();
        assert!(verifica(&PinnedVerifier::new(&outra.fingerprint()), &id).is_err());
    }

    #[test]
    fn aceita_maiuscula() {
        let id = Identity::generate("a").unwrap();
        let fp = id.fingerprint().to_uppercase();
        assert!(verifica(&PinnedVerifier::new(&fp), &id).is_ok());
    }

    #[test]
    fn supported_verify_schemes_nao_vazio() {
        assert!(!PinnedVerifier::new("x").supported_verify_schemes().is_empty());
    }
}
