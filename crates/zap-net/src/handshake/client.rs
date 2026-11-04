//! Lado cliente do aperto de mão.

use zap_proto::{auth, ControlMsg};

use super::{Hello, Welcome, HANDSHAKE_TIMEOUT};
use crate::{
    control::{ControlRx, ControlTx},
    NetError,
};

/// Lado cliente: abre o stream bidi, manda `ola`, responde ao `desafio` se houver senha.
pub async fn connect_handshake(
    conn: &quinn::Connection,
    hello: Hello,
    password: Option<&str>,
) -> Result<(ControlTx, ControlRx, Welcome), NetError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, executa(conn, hello, password))
        .await
        .map_err(|_| NetError::Timeout)?
}

async fn executa(
    conn: &quinn::Connection,
    hello: Hello,
    password: Option<&str>,
) -> Result<(ControlTx, ControlRx, Welcome), NetError> {
    let (send, recv) = conn.open_bi().await?;
    let tx = ControlTx::new(send);
    let mut rx = ControlRx::new(recv);

    tx.send(&ControlMsg::Hello {
        protocol: zap_proto::PROTOCOL_VERSION,
        app_version: hello.app_version,
        install_id: hello.install_id,
        name: hello.name,
        color: hello.color,
        ticket: hello.ticket,
        room_id: hello.room_id,
        preview: hello.preview,
        token: None,
    })
    .await?;

    // Um único desafio é aceito: um segundo indica servidor com comportamento inesperado.
    let mut desafios = 0u8;
    loop {
        match rx.recv().await? {
            Some(ControlMsg::Challenge { nonce, .. }) => {
                desafios += 1;
                if desafios > 1 {
                    return Err(NetError::Unexpected("segundo desafio".into()));
                }
                let Some(pw) = password else {
                    return Err(NetError::Rejected("senha_sala".into()));
                };
                let mac = auth::password_hmac(pw, &auth::unb64(&nonce)?);
                tx.send(&ControlMsg::Authenticate { hmac: auth::b64(&mac) }).await?;
            }
            Some(ControlMsg::Welcome {
                participant_id,
                room,
                clock_us,
                media_secret,
            }) => {
                let welcome = Welcome {
                    participant_id,
                    room,
                    clock_us,
                    media_secret,
                };
                return Ok((tx, rx, welcome));
            }
            Some(ControlMsg::Refused { code, .. }) => return Err(NetError::Rejected(code)),
            None => return Err(NetError::Unexpected("fim do stream".into())),
            Some(outra) => return Err(NetError::Unexpected(format!("{outra:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::ServerAuth;
    use crate::{
        endpoint,
        handshake::accept_handshake,
        tls::{self, Identity},
    };
    use std::net::SocketAddr;
    use zap_proto::{types::Access, Room, RoomMode};

    struct Par {
        cli: quinn::Connection,
        srv: quinn::Connection,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn par() -> Result<Par, Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
        let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?;
        let inc = servidor.accept().await.ok_or("sem conexão")?;
        let srv = inc.await?;
        let cli = conectando.await?;
        Ok(Par {
            cli,
            srv,
            _eps: (servidor, cliente),
        })
    }

    fn sala() -> Room {
        Room {
            id: "r1".into(),
            code: None,
            name: "Sala".into(),
            owner_id: "p0".into(),
            access: Access::Open,
            mode: RoomMode::Local,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn hello(ticket: Option<&str>) -> Hello {
        Hello {
            app_version: "1.0".into(),
            install_id: "inst".into(),
            name: "Ana".into(),
            color: "#fff".into(),
            ticket: ticket.map(str::to_owned),
            room_id: None,
            preview: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn aberta_ok() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let s = sala();
        let srv = accept_handshake(&p.srv, &ServerAuth::Open, &s, "p1".into());
        let cli = connect_handshake(&p.cli, hello(None), None);
        let (r, c) = tokio::join!(srv, cli);
        let (_stx, _srx, h) = r?;
        let (_ctx, _crx, w) = c?;
        assert_eq!(w.participant_id, "p1");
        assert_eq!(w.room, sala());
        assert_eq!(h.name, "Ana");
        assert_eq!(h.color, "#fff");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn senha_certa_ok() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let auth = ServerAuth::Password("segredo".into());
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = connect_handshake(&p.cli, hello(None), Some("segredo"));
        let (r, c) = tokio::join!(srv, cli);
        r?;
        let (_ctx, _crx, w) = c?;
        assert_eq!(w.participant_id, "p1");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn senha_errada() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let auth = ServerAuth::Password("segredo".into());
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = connect_handshake(&p.cli, hello(None), Some("errada"));
        let (r, c) = tokio::join!(srv, cli);
        assert!(matches!(c, Err(NetError::Rejected(ref code)) if code == "senha_sala"));
        assert!(matches!(r, Err(NetError::Rejected(ref code)) if code == "senha_sala"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sem_senha_quando_exigida() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        // O servidor fica esperando `autenticar` que nunca vem; só o cliente é verificado aqui.
        let srv_conn = p.srv.clone();
        let _srv = tokio::spawn(async move {
            accept_handshake(&srv_conn, &ServerAuth::Password("segredo".into()), &sala(), "p1".into()).await
        });
        let c = connect_handshake(&p.cli, hello(None), None).await;
        assert!(matches!(c, Err(NetError::Rejected(ref code)) if code == "senha_sala"));
        // Fecha a conexão para o servidor parar de esperar o `autenticar`.
        p.cli.close(0u32.into(), b"fim");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_ok_e_invalido() -> Result<(), Box<dyn std::error::Error>> {
        let auth = ServerAuth::Ticket(Box::new(|t| t == "ok"));
        // válido
        let p = par().await?;
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = connect_handshake(&p.cli, hello(Some("ok")), None);
        let (r, c) = tokio::join!(srv, cli);
        r?;
        assert_eq!(c?.2.participant_id, "p1");
        // inválido
        let p = par().await?;
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = connect_handshake(&p.cli, hello(Some("ruim")), None);
        let (r, c) = tokio::join!(srv, cli);
        assert!(matches!(r, Err(NetError::Rejected(ref code)) if code == "ingresso"));
        assert!(matches!(c, Err(NetError::Rejected(ref code)) if code == "ingresso"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fingerprint_errada() -> Result<(), Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let outra = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
        let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        // Fixa a impressão digital de outro certificado: o TLS deve recusar antes do aperto de mão.
        let conectando = cliente.connect_with(tls::client_config(Some(&outra.fingerprint()))?, addr, "zap")?;
        let r = conectando.await;
        assert!(r.is_err());
        drop(servidor);
        Ok(())
    }
}
