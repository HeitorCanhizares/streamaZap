//! Lado servidor do aperto de mão.

use std::time::Duration;

use zap_proto::{auth, types::Access, ControlMsg, Room};

use super::{Hello, ServerAuth, HANDSHAKE_TIMEOUT};
use crate::{
    control::{ControlRx, ControlTx},
    NetError,
};

/// Tempo para o `recusado` sair antes de o chamador fechar a conexão.
const ESPERA_RECUSA: Duration = Duration::from_millis(50);

/// Lado servidor: aceita o stream bidi, lê `ola`, desafia/valida, responde `bem_vindo`
/// ou `recusado` + `NetError::Rejected`.
pub async fn accept_handshake(
    conn: &quinn::Connection,
    auth: &ServerAuth,
    room: &Room,
    participant_id: String,
) -> Result<(ControlTx, ControlRx, Hello), NetError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, executa(conn, auth, room, participant_id))
        .await
        .map_err(|_| NetError::Timeout)?
}

/// Envia `recusado`, dá um tempo para o dado sair e devolve o erro correspondente.
async fn recusa(tx: &ControlTx, code: &str, message: &str) -> NetError {
    let msg = ControlMsg::Refused {
        code: code.to_owned(),
        message: message.to_owned(),
    };
    if let Err(e) = tx.send(&msg).await {
        return e;
    }
    tokio::time::sleep(ESPERA_RECUSA).await;
    NetError::Rejected(code.to_owned())
}

async fn executa(
    conn: &quinn::Connection,
    auth: &ServerAuth,
    room: &Room,
    participant_id: String,
) -> Result<(ControlTx, ControlRx, Hello), NetError> {
    let (send, recv) = conn.accept_bi().await?;
    let tx = ControlTx::new(send);
    let mut rx = ControlRx::new(recv);

    let hello = match rx.recv().await? {
        Some(ControlMsg::Hello {
            protocol,
            app_version,
            install_id,
            name,
            color,
            ticket,
            room_id,
            preview,
            ..
        }) => {
            if protocol != zap_proto::PROTOCOL_VERSION {
                return Err(recusa(&tx, "versao", "Versão do protocolo incompatível").await);
            }
            Hello {
                app_version,
                install_id,
                name,
                color,
                ticket,
                room_id,
                preview,
            }
        }
        outra => return Err(NetError::Unexpected(format!("esperava ola, veio {outra:?}"))),
    };

    match auth {
        ServerAuth::Open => {}
        ServerAuth::Password(senha) => {
            // Nonce novo por tentativa, para a resposta não poder ser reaproveitada.
            let nonce: [u8; 32] = rand::random();
            tx.send(&ControlMsg::Challenge {
                nonce: auth::b64(&nonce),
                access: Access::Password,
            })
            .await?;
            let ok = match rx.recv().await? {
                Some(ControlMsg::Authenticate { hmac }) => match auth::unb64(&hmac) {
                    Ok(mac) => auth::verify_password_hmac(senha, &nonce, &mac),
                    Err(_) => false,
                },
                _ => false,
            };
            if !ok {
                return Err(recusa(&tx, "senha_sala", "Senha incorreta").await);
            }
        }
        ServerAuth::Ticket(valida) => {
            let ok = hello.ticket.as_deref().is_some_and(valida);
            if !ok {
                return Err(recusa(&tx, "ingresso", "Ingresso inválido").await);
            }
        }
    }

    tx.send(&ControlMsg::Welcome {
        participant_id,
        room: room.clone(),
        clock_us: None,
    })
    .await?;
    Ok((tx, rx, hello))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        endpoint,
        tls::{self, Identity},
    };
    use std::net::SocketAddr;
    use zap_proto::RoomMode;

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

    fn ola(protocol: u32, ticket: Option<&str>) -> ControlMsg {
        ControlMsg::Hello {
            protocol,
            app_version: "1.0".into(),
            install_id: "inst".into(),
            name: "Ana".into(),
            color: "#fff".into(),
            ticket: ticket.map(str::to_owned),
            room_id: None,
            preview: None,
            token: None,
        }
    }

    /// Cliente cru: abre o stream e manda o `ola`. Devolve tx/rx do cliente.
    async fn cliente(c: &quinn::Connection, msg: ControlMsg) -> Result<(ControlTx, ControlRx), NetError> {
        let (s, r) = c.open_bi().await?;
        let tx = ControlTx::new(s);
        tx.send(&msg).await?;
        Ok((tx, ControlRx::new(r)))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn aberta_ok() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (_ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, None)).await?;
        let (_tx, _rx, h) = accept_handshake(&p.srv, &ServerAuth::Open, &sala(), "p1".into()).await?;
        assert_eq!(h.name, "Ana");
        match crx.recv().await? {
            Some(ControlMsg::Welcome {
                participant_id, room, ..
            }) => {
                assert_eq!(participant_id, "p1");
                assert_eq!(room, sala());
            }
            outro => return Err(format!("inesperado: {outro:?}").into()),
        }
        Ok(())
    }

    async fn desafio_e_resposta(
        ctx: &ControlTx,
        crx: &mut ControlRx,
        senha: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let nonce = match crx.recv().await? {
            Some(ControlMsg::Challenge { nonce, .. }) => auth::unb64(&nonce)?,
            outro => return Err(format!("esperava desafio: {outro:?}").into()),
        };
        let mac = auth::password_hmac(senha, &nonce);
        ctx.send(&ControlMsg::Authenticate { hmac: auth::b64(&mac) }).await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn senha_certa_ok() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, None)).await?;
        let auth = ServerAuth::Password("segredo".into());
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = desafio_e_resposta(&ctx, &mut crx, "segredo");
        let (r, c) = tokio::join!(srv, cli);
        c?;
        r?;
        assert!(matches!(crx.recv().await?, Some(ControlMsg::Welcome { .. })));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn senha_errada_recusa() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, None)).await?;
        let auth = ServerAuth::Password("segredo".into());
        let s = sala();
        let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
        let cli = desafio_e_resposta(&ctx, &mut crx, "errada");
        let (r, c) = tokio::join!(srv, cli);
        c?;
        assert!(matches!(r, Err(NetError::Rejected(ref c)) if c == "senha_sala"));
        match crx.recv().await? {
            Some(ControlMsg::Refused { code, .. }) => assert_eq!(code, "senha_sala"),
            outro => return Err(format!("inesperado: {outro:?}").into()),
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_valido_e_invalido() -> Result<(), Box<dyn std::error::Error>> {
        let auth = ServerAuth::Ticket(Box::new(|t| t == "ok"));
        // válido
        let p = par().await?;
        let (_ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, Some("ok"))).await?;
        accept_handshake(&p.srv, &auth, &sala(), "p1".into()).await?;
        assert!(matches!(crx.recv().await?, Some(ControlMsg::Welcome { .. })));
        // inválido
        let p = par().await?;
        let (_ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, Some("ruim"))).await?;
        let r = accept_handshake(&p.srv, &auth, &sala(), "p1".into()).await;
        assert!(matches!(r, Err(NetError::Rejected(ref c)) if c == "ingresso"));
        assert!(matches!(crx.recv().await?, Some(ControlMsg::Refused { .. })));
        // ausente
        let p = par().await?;
        let (_ctx, _crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION, None)).await?;
        let r = accept_handshake(&p.srv, &auth, &sala(), "p1".into()).await;
        assert!(matches!(r, Err(NetError::Rejected(ref c)) if c == "ingresso"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn protocolo_errado_recusa_versao() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let (_ctx, mut crx) = cliente(&p.cli, ola(zap_proto::PROTOCOL_VERSION + 1, None)).await?;
        let r = accept_handshake(&p.srv, &ServerAuth::Open, &sala(), "p1".into()).await;
        assert!(matches!(r, Err(NetError::Rejected(ref c)) if c == "versao"));
        match crx.recv().await? {
            Some(ControlMsg::Refused { code, .. }) => assert_eq!(code, "versao"),
            outro => return Err(format!("inesperado: {outro:?}").into()),
        }
        Ok(())
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn timeout_sem_stream() -> Result<(), Box<dyn std::error::Error>> {
        let p = par().await?;
        let r = accept_handshake(&p.srv, &ServerAuth::Open, &sala(), "p1".into()).await;
        assert!(matches!(r, Err(NetError::Timeout)));
        Ok(())
    }
}
