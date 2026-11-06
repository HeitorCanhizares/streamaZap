//! Conexão QUIC `zap/4` com a ponte do servidor (docs/api/midia.md).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use zap_net::control::{ControlRx, ControlTx};
use zap_net::handshake::{connect_handshake, Hello, Welcome};
use zap_net::NetError;

use crate::engine::Shared;
use crate::CoreError;

/// Conexão já autenticada (depois do `bem_vindo`).
pub struct BridgeConnected {
    pub conn: quinn::Connection,
    pub ctrl: ControlTx,
    pub crx: ControlRx,
    pub welcome: Welcome,
}

/// Conecta na ponte e faz o aperto de mão:
/// - `address` = `"host:porta"` (ou `options.dev_bridge_addr` se definido); resolve com
///   `tokio::net::lookup_host` (primeiro endereço); nome do servidor TLS = parte `host`
///   (ou `"zap"` se o host for um IP);
/// - `zap_net::endpoint::client_endpoint()` + `zap_net::tls::client_config(
///   options.dev_cert_fp.as_deref())`; timeout de 10 s para conectar;
/// - `connect_handshake(&conn, Hello{app_version, install_id, name, color (das settings),
///   ticket: Some(ingresso), room_id: Some(room_id), preview: preview.then_some(true)}, None)`;
/// - o `Endpoint` precisa viver tanto quanto a conexão (guarde-o numa tarefa que espera
///   `conn.closed()`).
///
/// Erros: `NetError::Rejected(codigo)` → `CoreError::Server{code: codigo, message: <texto em
/// português; `nao_autenticado` → "Ingresso recusado pelo servidor", `protocolo_incompativel`
///   → "Atualize o StreamaZap para entrar neste servidor", outros → o próprio código>}`;
///   tempo esgotado/rede → `CoreError::Connect`.
pub async fn connect_bridge(
    shared: &Arc<Shared>,
    address: &str,
    ticket: &str,
    room_id: &str,
    preview: bool,
) -> Result<BridgeConnected, CoreError> {
    let efetivo = shared
        .options
        .dev_bridge_addr
        .clone()
        .unwrap_or_else(|| address.to_string());
    let (host, porta) = separa_endereco(&efetivo)?;
    let addr = resolve(host, porta).await?;
    // Para IP o certificado vale para o nome fixo "zap".
    let nome = if host.parse::<IpAddr>().is_ok() { "zap" } else { host };

    let endpoint = zap_net::endpoint::client_endpoint()?;
    let cfg = zap_net::tls::client_config(shared.options.dev_cert_fp.as_deref())?;
    let conectando = endpoint.connect_with(cfg, addr, nome).map_err(NetError::from)?;
    let conn = match tokio::time::timeout(Duration::from_secs(10), conectando).await {
        Err(_) => return Err(CoreError::Connect("tempo esgotado ao conectar na ponte".into())),
        Ok(r) => r.map_err(NetError::from)?,
    };

    // O endpoint precisa viver tanto quanto a conexão.
    let guarda = conn.clone();
    tokio::spawn(async move {
        let _ep = endpoint;
        guarda.closed().await;
    });

    let hello = {
        let s = shared.settings.read();
        Hello {
            app_version: env!("CARGO_PKG_VERSION").into(),
            install_id: s.install_id.clone(),
            name: s.name.clone(),
            color: s.color.clone(),
            ticket: Some(ticket.to_string()),
            room_id: Some(room_id.to_string()),
            preview: preview.then_some(true),
        }
    };
    match connect_handshake(&conn, hello, None).await {
        Ok((ctrl, crx, welcome)) => Ok(BridgeConnected {
            conn,
            ctrl,
            crx,
            welcome,
        }),
        Err(e) => {
            conn.close(0u32.into(), b"falha");
            Err(match e {
                NetError::Rejected(codigo) => map_refused(&codigo),
                NetError::Timeout => CoreError::Connect("tempo esgotado ao conectar na ponte".into()),
                outro => CoreError::Net(outro),
            })
        }
    }
}

/// Separa `host:porta`; porta inválida ou ausente vira `Invalid`.
fn separa_endereco(endereco: &str) -> Result<(&str, u16), CoreError> {
    let invalido = || CoreError::Invalid(format!("Endereço da ponte inválido: {endereco}"));
    let (host, porta) = endereco.trim().rsplit_once(':').ok_or_else(invalido)?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err(invalido());
    }
    let porta = porta.parse::<u16>().map_err(|_| invalido())?;
    Ok((host, porta))
}

/// Resolve o nome preferindo IPv4 (o endpoint cliente escuta em `0.0.0.0`).
async fn resolve(host: &str, porta: u16) -> Result<SocketAddr, CoreError> {
    let todos: Vec<SocketAddr> = tokio::net::lookup_host((host, porta))
        .await
        .map_err(|e| CoreError::Connect(format!("não foi possível resolver {host}: {e}")))?
        .collect();
    todos
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| todos.first())
        .copied()
        .ok_or_else(|| CoreError::Connect(format!("não foi possível resolver {host}")))
}

/// Traduz o código de recusa da ponte em erro com mensagem em português.
pub(crate) fn map_refused(code: &str) -> CoreError {
    let message = match code {
        "nao_autenticado" => "Ingresso recusado pelo servidor",
        "protocolo_incompativel" => "Atualize o StreamaZap para entrar neste servidor",
        "sala_nao_encontrada" => "Sala não encontrada",
        "sala_cheia" => "A sala está cheia",
        "senha_sala" => "Senha da sala incorreta",
        outro => outro,
    };
    CoreError::Server {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;
    use zap_net::online::{CreateRoomRequest, JoinResponse, LoginRequest, ServerClient};
    use zap_proto::Access;
    use zap_test_server::{Config, Handle};

    async fn novo_core(fp: Option<String>, dev_addr: Option<String>) -> (Core, std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-bridge-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                dev_cert_fp: fp,
                dev_bridge_addr: dev_addr,
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap()
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    async fn entrar(srv: &Handle) -> JoinResponse {
        let cli = ServerClient::login(
            &srv.base_url,
            LoginRequest {
                name: "Teste".into(),
                color: "#3BF08A".into(),
                install_id: uuid::Uuid::new_v4().to_string(),
                protocol: zap_proto::PROTOCOL_VERSION,
                app_version: "teste".into(),
                server_password: None,
                account_token: None,
            },
        )
        .await
        .unwrap();
        let sala = cli
            .create_room(CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            })
            .await
            .unwrap();
        cli.join(&sala.code, None).await.unwrap()
    }

    fn endereco(j: &JoinResponse) -> String {
        j.media.bridge.as_ref().unwrap().address.clone()
    }

    #[test]
    fn map_refused_codigos() {
        let msg = |c: &str| match map_refused(c) {
            CoreError::Server { code, message } => {
                assert_eq!(code, c);
                message
            }
            outro => panic!("esperava Server, veio {outro:?}"),
        };
        assert_eq!(msg("nao_autenticado"), "Ingresso recusado pelo servidor");
        assert_eq!(
            msg("protocolo_incompativel"),
            "Atualize o StreamaZap para entrar neste servidor"
        );
        assert_eq!(msg("sala_nao_encontrada"), "Sala não encontrada");
        assert_eq!(msg("sala_cheia"), "A sala está cheia");
        assert_eq!(msg("senha_sala"), "Senha da sala incorreta");
        assert_eq!(msg("qualquer"), "qualquer");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn endereco_invalido_porta() {
        let (core, d) = novo_core(None, None).await;
        for ruim in ["host-sem-porta", "host:abc", ":80", "host:99999"] {
            let r = connect_bridge(&core.shared, ruim, "t", "r", false).await;
            assert!(matches!(r, Err(CoreError::Invalid(_))), "{ruim}");
        }
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn conecta_com_ingresso_valido() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.cert_fingerprint.clone()), None).await;
        let join = entrar(&srv).await;
        let c = connect_bridge(&core.shared, &endereco(&join), &join.ticket, &join.room.id, false)
            .await
            .unwrap();
        assert_eq!(c.welcome.participant_id, join.participant_id);
        c.conn.close(0u32.into(), b"fim");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ingresso_invalido_recusado() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.cert_fingerprint.clone()), None).await;
        let join = entrar(&srv).await;
        let r = connect_bridge(&core.shared, &endereco(&join), "ingresso-falso", &join.room.id, false).await;
        assert!(matches!(r, Err(CoreError::Server { ref code, .. }) if code == "nao_autenticado"));
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dev_bridge_addr_tem_prioridade() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some(srv.cert_fingerprint.clone()), Some(srv.quic_addr.to_string())).await;
        let join = entrar(&srv).await;
        let c = connect_bridge(&core.shared, "lixo:1", &join.ticket, &join.room.id, false)
            .await
            .unwrap();
        assert_eq!(c.welcome.participant_id, join.participant_id);
        c.conn.close(0u32.into(), b"fim");
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn certificado_errado_falha() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(Some("00".repeat(32)), None).await;
        let join = entrar(&srv).await;
        let r = connect_bridge(&core.shared, &endereco(&join), &join.ticket, &join.room.id, false).await;
        assert!(r.is_err());
        encerrar(core, d).await;
    }
}
