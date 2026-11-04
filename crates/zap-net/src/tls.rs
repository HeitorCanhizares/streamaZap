//! Identidade TLS autoassinada, impressão digital e configs QUIC.

mod identity;
mod transport;
mod verify;

pub use identity::{fingerprint_of, Identity};
pub use transport::transport_config;
pub use verify::PinnedVerifier;

use crate::NetError;

use std::sync::Arc;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

fn tls_err(e: impl std::fmt::Display) -> NetError {
    NetError::Tls(e.to_string())
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// ALPN `zap/4`, `transport_config()`.
pub fn server_config(id: &Identity) -> Result<quinn::ServerConfig, NetError> {
    let cert = CertificateDer::from(id.cert_der.clone());
    let key = PrivateKeyDer::try_from(id.key_der.clone()).map_err(tls_err)?;
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(tls_err)?;
    cfg.alpn_protocols = vec![zap_proto::ALPN.to_vec()];
    let quic = QuicServerConfig::try_from(cfg).map_err(tls_err)?;
    let mut sc = quinn::ServerConfig::with_crypto(Arc::new(quic));
    sc.transport_config(Arc::new(transport_config()));
    Ok(sc)
}

/// `Some(fp)`: aceita só o certificado com essa impressão digital (modo Local).
/// `None`: raízes web (`webpki-roots`) (modo Online).
pub fn client_config(pinned: Option<&str>) -> Result<quinn::ClientConfig, NetError> {
    let builder = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?;
    let mut cfg = match pinned {
        Some(fp) => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinnedVerifier::new(fp)))
            .with_no_client_auth(),
        None => {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    };
    cfg.alpn_protocols = vec![zap_proto::ALPN.to_vec()];
    let quic = QuicClientConfig::try_from(cfg).map_err(tls_err)?;
    let mut cc = quinn::ClientConfig::new(Arc::new(quic));
    cc.transport_config(Arc::new(transport_config()));
    Ok(cc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_ok() {
        let id = Identity::generate("x").expect("id");
        assert!(server_config(&id).is_ok());
    }

    #[test]
    fn client_config_pinned_ok() {
        assert!(client_config(Some(&"0".repeat(64))).is_ok());
    }

    #[test]
    fn client_config_none_ok() {
        assert!(client_config(None).is_ok());
    }

    async fn conecta(fp: String) -> Result<(), Box<dyn std::error::Error>> {
        let id = Identity::generate("x")?;
        let real = id.fingerprint();
        let server = quinn::Endpoint::server(server_config(&id)?, "127.0.0.1:0".parse()?)?;
        let addr = server.local_addr()?;
        let aceita = tokio::spawn(async move {
            if let Some(inc) = server.accept().await {
                if let Ok(c) = inc.await {
                    // mantém a conexão viva até o cliente fechar
                    c.closed().await;
                    return true;
                }
            }
            false
        });
        let mut client = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
        let fp = if fp.is_empty() { real } else { fp };
        client.set_default_client_config(client_config(Some(&fp))?);
        let conn = client.connect(addr, "x")?.await?;
        conn.close(0u32.into(), b"ok");
        let _ = aceita.await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn e2e_pinned() {
        assert!(conecta(String::new()).await.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn e2e_fingerprint_errada() {
        assert!(conecta("0".repeat(64)).await.is_err());
    }
}
