//! Criação de endpoints QUIC.

use std::net::SocketAddr;

use crate::{
    tls::{self, Identity},
    NetError,
};

/// Quantas portas além da preferida tentamos antes de desistir.
const TENTATIVAS_EXTRAS: u16 = 20;

/// Tenta `preferred_port`, depois +1 … +20. Devolve o endpoint e a porta usada.
///
/// Só pula porta em `AddrInUse`; outro erro de io é devolvido na hora. Porta 0 pedida
/// deixa o SO escolher, então não há iteração e a porta devolvida é a real.
pub async fn bind_server(id: &Identity, preferred_port: u16) -> Result<(quinn::Endpoint, u16), NetError> {
    if preferred_port == 0 {
        return bind_uma(id, 0);
    }
    for offset in 0..=TENTATIVAS_EXTRAS {
        // Para antes de estourar u16::MAX em vez de dar a volta para portas baixas.
        let Some(porta) = preferred_port.checked_add(offset) else {
            break;
        };
        match bind_uma(id, porta) {
            Ok(ok) => return Ok(ok),
            Err(NetError::Io(e)) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(e),
        }
    }
    Err(NetError::NoFreePort(preferred_port))
}

/// Uma tentativa de bind em `0.0.0.0:porta`; devolve a porta que o SO realmente deu.
fn bind_uma(id: &Identity, porta: u16) -> Result<(quinn::Endpoint, u16), NetError> {
    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    let endpoint = quinn::Endpoint::server(tls::server_config(id)?, addr)?;
    let real = endpoint.local_addr()?.port();
    Ok((endpoint, real))
}

/// Endpoint cliente em `0.0.0.0:0`.
///
/// NÃO define config padrão: quem conecta passa a config com `connect_with`:
/// `endpoint.connect_with(crate::tls::client_config(..)?, addr, nome)`.
pub fn client_endpoint() -> Result<quinn::Endpoint, NetError> {
    Ok(quinn::Endpoint::client(SocketAddr::from(([0, 0, 0, 0], 0)))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[tokio::test(flavor = "multi_thread")]
    async fn porta_zero() {
        let id = Identity::generate("zap").expect("identidade");
        let (_ep, porta) = bind_server(&id, 0).await.expect("bind");
        assert_ne!(porta, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ocupada_pula() {
        // Segura uma porta UDP ocupada durante o teste.
        let ocupado = UdpSocket::bind("0.0.0.0:0").expect("socket");
        let p = ocupado.local_addr().expect("addr").port();

        let id = Identity::generate("zap").expect("identidade");
        let (_ep, porta) = bind_server(&id, p).await.expect("bind");
        assert!(
            (p + 1..=p + 20).contains(&porta),
            "porta {porta} fora de {}..={}",
            p + 1,
            p + 20
        );
        drop(ocupado);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn client_endpoint_ok() {
        assert!(client_endpoint().is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn e2e_conecta() -> Result<(), Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = bind_server(&id, 0).await?;
        let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;

        let aceita = tokio::spawn(async move {
            if let Some(inc) = servidor.accept().await {
                if let Ok(c) = inc.await {
                    // mantém a conexão viva até o cliente fechar
                    c.closed().await;
                    return true;
                }
            }
            false
        });

        let cliente = client_endpoint()?;
        let conn = cliente
            .connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?
            .await?;
        conn.close(0u32.into(), b"ok");
        assert!(aceita.await?);
        Ok(())
    }
}
