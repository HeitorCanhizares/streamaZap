//! Teste de aceite do aperto de mão, pela API pública de `zap_net`.
//!
//! Cada cenário sobe um servidor QUIC local com identidade própria e um cliente que fixa a
//! impressão digital dessa identidade (modo Local). Os asserts verificam o resultado de cada
//! lado: `Ok` com o `participant_id` esperado ou o `Err` do tipo esperado.

use std::net::SocketAddr;

use zap_net::{
    endpoint,
    handshake::{accept_handshake, connect_handshake, Hello, ServerAuth},
    tls::{self, Identity},
    NetError,
};
use zap_proto::{types::Access, ControlMsg, Room, RoomCode, RoomMode};

type Res = Result<(), Box<dyn std::error::Error>>;

/// Par de conexões QUIC locais. Os endpoints ficam vivos junto com as conexões: soltá-los fecha tudo.
struct Par {
    cli: quinn::Connection,
    srv: quinn::Connection,
    _eps: (quinn::Endpoint, quinn::Endpoint),
}

/// Sobe o servidor com `id` numa porta livre e conecta um cliente que fixa a impressão digital dela.
async fn par(id: &Identity) -> Result<Par, Box<dyn std::error::Error>> {
    let (servidor, porta) = endpoint::bind_server(id, 0).await?;
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
        code: Some(RoomCode::from_entropy([2, 3, 4, 5])),
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
async fn aberta() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let s = sala();
    let srv = accept_handshake(&p.srv, &ServerAuth::Open, &s, "p1".into());
    let cli = connect_handshake(&p.cli, hello(None), None);
    let (r, c) = tokio::join!(srv, cli);
    let (_stx, _srx, h) = r?;
    let (_ctx, _crx, w) = c?;
    assert_eq!(w.participant_id, "p1");
    assert_eq!(w.room, sala());
    assert_eq!(h.name, "Ana");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn senha_certa() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let auth = ServerAuth::Password("segredo".into());
    let s = sala();
    let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
    let cli = connect_handshake(&p.cli, hello(None), Some("segredo"));
    let (r, c) = tokio::join!(srv, cli);
    let (_stx, _srx, _h) = r?;
    let (_ctx, _crx, w) = c?;
    assert_eq!(w.participant_id, "p1");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn senha_errada() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let auth = ServerAuth::Password("segredo".into());
    let s = sala();
    let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
    let cli = connect_handshake(&p.cli, hello(None), Some("errada"));
    let (r, c) = tokio::join!(srv, cli);
    assert!(
        matches!(r, Err(NetError::Rejected(ref code)) if code == "senha_sala"),
        "servidor deveria recusar com senha_sala"
    );
    assert!(
        matches!(c, Err(NetError::Rejected(ref code)) if code == "senha_sala"),
        "cliente deveria receber senha_sala"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn senha_ausente() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let auth = ServerAuth::Password("segredo".into());
    let s = sala();
    let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
    // O cliente recebe o desafio, não tem senha e desiste; fechar a conexão libera o servidor,
    // que esperaria um `autenticar` que nunca vem.
    let cli = async {
        let r = connect_handshake(&p.cli, hello(None), None).await;
        p.cli.close(0u32.into(), b"sem senha");
        r
    };
    let (r, c) = tokio::join!(srv, cli);
    assert!(
        matches!(c, Err(NetError::Rejected(ref code)) if code == "senha_sala"),
        "cliente sem senha deveria receber senha_sala"
    );
    assert!(r.is_err(), "servidor não deveria aceitar sem senha");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn ingresso_valido() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let auth = ServerAuth::Ticket(Box::new(|t| t == "ok"));
    let s = sala();
    let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
    let cli = connect_handshake(&p.cli, hello(Some("ok")), None);
    let (r, c) = tokio::join!(srv, cli);
    let (_stx, _srx, h) = r?;
    let (_ctx, _crx, w) = c?;
    assert_eq!(w.participant_id, "p1");
    assert_eq!(h.ticket.as_deref(), Some("ok"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn ingresso_invalido() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    let auth = ServerAuth::Ticket(Box::new(|t| t == "ok"));
    let s = sala();
    let srv = accept_handshake(&p.srv, &auth, &s, "p1".into());
    let cli = connect_handshake(&p.cli, hello(Some("ruim")), None);
    let (r, c) = tokio::join!(srv, cli);
    assert!(
        matches!(r, Err(NetError::Rejected(ref code)) if code == "ingresso"),
        "servidor deveria recusar com ingresso"
    );
    assert!(
        matches!(c, Err(NetError::Rejected(ref code)) if code == "ingresso"),
        "cliente deveria receber ingresso"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn fingerprint_errada_falha_no_tls() -> Res {
    let id = Identity::generate("zap")?;
    let outra = Identity::generate("zap")?;
    let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
    let addr: SocketAddr = format!("127.0.0.1:{porta}").parse()?;
    let cliente = endpoint::client_endpoint()?;
    // O servidor aceita a tentativa para o handshake TLS andar; sem isso o cliente só desistiria
    // pelo timeout do QUIC, sem mostrar a falha de TLS.
    let aceita = tokio::spawn(async move {
        if let Some(inc) = servidor.accept().await {
            let _ = inc.await;
        }
    });
    // Fixa a impressão digital de outro certificado: o TLS recusa antes de qualquer `ola`.
    let conectando = cliente.connect_with(tls::client_config(Some(&outra.fingerprint()))?, addr, "zap")?;
    let r = conectando.await;
    assert!(r.is_err(), "conexão com impressão digital errada deveria falhar");
    aceita.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn apos_welcome_controle_funciona() -> Res {
    let id = Identity::generate("zap")?;
    let p = par(&id).await?;
    // Servidor: depois do aperto de mão, recebe o Ping e responde com Pong de mesmo ts_ms.
    let srv = async {
        let (stx, mut srx, _h) = accept_handshake(&p.srv, &ServerAuth::Open, &sala(), "p1".into()).await?;
        match srx.recv().await? {
            Some(ControlMsg::Ping { ts_ms }) => stx.send(&ControlMsg::Pong { ts_ms }).await?,
            outro => return Err(format!("servidor esperava ping, veio {outro:?}").into()),
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };
    // Cliente: depois do Welcome, manda Ping e espera o Pong pelo canal devolvido no aperto.
    let cli = async {
        let (ctx, mut crx, w) = connect_handshake(&p.cli, hello(None), None).await?;
        ctx.send(&ControlMsg::Ping { ts_ms: 42 }).await?;
        let pong = crx.recv().await?;
        Ok::<_, Box<dyn std::error::Error>>((w, pong))
    };
    let (r, c) = tokio::join!(srv, cli);
    r?;
    let (w, pong) = c?;
    assert_eq!(w.participant_id, "p1");
    assert_eq!(pong, Some(ControlMsg::Pong { ts_ms: 42 }));
    Ok(())
}
