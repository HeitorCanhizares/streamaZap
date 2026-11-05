//! Ponte QUIC `zap/4` (`docs/api/midia.md`): aceita conexões, faz o aperto de mão com o
//! `ingresso`, lê relatórios e repassa mídia.
//!
//! Como no servidor de produção (`docs/api/mensagens.md`):
//! - o `ola` com ingresso vale como `sala.entrar`: logo depois do `bem_vindo` a conexão recebe
//!   `sala.estado` e, se não for prévia, o histórico do chat;
//! - uma conexão por participante: a nova substitui a antiga; quem cai tem `TOLERANCIA_QUEDA`
//!   para voltar antes de as trilhas dele saírem da sala (a conexão nova recebe de novo
//!   `camadas.pedidas` e `camada.atual`);
//! - as mensagens da mídia (`camadas.pedidas`, `keyframe.pedir`, `camada.atual`, `relatorio`)
//!   saem só por aqui, por um único escritor (`ParticipantData::quic_ctl`);
//! - pelo stream de controle o servidor de teste aceita `relatorio`, `ping`/`pong`, `assinar`,
//!   `cancelar` e `keyframe.pedir` (o resto vai pelo WebSocket; aqui vira `erro` `invalido`);
//! - o publicador recebe o `relatorio` de subida (chegadas e perdas, sem `banda_kbps` nem
//!   `estado`) a cada 100 ms em que houve chegada.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::{
    sync::mpsc,
    time::{interval, MissedTickBehavior},
};
use zap_net::{
    control::{ControlRx, ControlTx},
    handshake::HANDSHAKE_TIMEOUT,
    media_rx::read_frame,
    report::{ReportBuilder, ReportBuilderHandle, REPORT_INTERVAL},
    tls::Identity,
    NetError,
};
use zap_proto::{
    media::{decode_datagram, Datagram, VIDEO_HEADER_LEN},
    ControlMsg, Report,
};

use crate::{
    media::{forward_audio, forward_video},
    state::{Hub, Via},
    util::now_unix_ms,
    TestServerError,
};

/// Protocolo aceito no `ola`.
const PROTOCOLO: u32 = 4;
/// Espera antes de fechar uma conexão recusada, para o `recusado` chegar ao outro lado.
const ESPERA_RECUSA: Duration = Duration::from_millis(50);
/// Quanto a ponte espera a volta de quem caiu antes de tirar as trilhas dele da sala.
const TOLERANCIA_QUEDA: Duration = Duration::from_secs(5);

/// Quem passou pelo aperto de mão.
pub struct Admitted {
    pub tx: ControlTx,
    pub rx: ControlRx,
    pub participant_id: String,
    /// Sala do ingresso.
    pub room_id: String,
    /// Ingresso de prévia: só assina vídeo, não publica.
    pub preview: bool,
}

/// Abre o endpoint QUIC (porta livre) com `identity`, preenche `Hub::bridge_addr` e devolve
/// o endpoint, o endereço `127.0.0.1:<porta>` e a tarefa do laço de `accept`.
pub async fn start(
    hub: Arc<Hub>,
    identity: &Identity,
) -> Result<(quinn::Endpoint, SocketAddr, tokio::task::JoinHandle<()>), TestServerError> {
    // `bind_server` escuta em 0.0.0.0; os testes e o app usam o loopback.
    let (endpoint, porta) = zap_net::endpoint::bind_server(identity, 0).await?;
    let addr = SocketAddr::from(([127, 0, 0, 1], porta));
    // Um `Hub` só tem uma ponte: um segundo `start` não troca o endereço já anunciado.
    let _ = hub.bridge_addr.set(addr.to_string());

    let aceitador = endpoint.clone();
    let tarefa = tokio::spawn(async move {
        while let Some(incoming) = aceitador.accept().await {
            let hub = hub.clone();
            tokio::spawn(async move {
                match incoming.await {
                    Ok(conn) => serve_connection(hub, conn).await,
                    Err(e) => tracing::debug!("conexão QUIC não completou: {e}"),
                }
            });
        }
    });
    Ok((endpoint, addr, tarefa))
}

/// Uma conexão: aperto de mão (`handshake`), registro, depois os laços concorrentes (controle,
/// vídeo, áudio; a prévia só tem o de controle). Ao fechar: se ainda é a conexão registrada do
/// participante, limpa o registro e, se ele não voltar em `TOLERANCIA_QUEDA`, chama
/// `drop_participant_tracks` na sala do ingresso.
pub async fn serve_connection(hub: Arc<Hub>, conn: quinn::Connection) {
    let admitido = match handshake(&hub, &conn).await {
        Ok(a) => a,
        Err(e) => {
            tracing::debug!("aperto de mão QUIC recusado: {e}");
            conn.close(0u32.into(), b"recusado");
            return;
        }
    };
    let Admitted {
        tx,
        rx,
        participant_id: who,
        room_id,
        preview,
    } = admitido;

    // Escritor único do stream de controle: o `bem_vindo` já foi escrito pelo aperto de mão, então
    // tudo o que entra neste canal sai depois dele, na ordem.
    let (saida, mut fila) = mpsc::unbounded_channel::<ControlMsg>();
    let escritor = tokio::spawn(async move {
        while let Some(msg) = fila.recv().await {
            if let Err(e) = tx.send(&msg).await {
                tracing::debug!("stream de controle QUIC fechou: {e}");
                break;
            }
        }
    });

    registrar(&hub, &who, &room_id, preview, &conn, &saida);

    let relatorios = (!preview).then(|| Arc::new(Mutex::new(ReportBuilder::new(false))));
    let relator = relatorios
        .clone()
        .map(|rb| tokio::spawn(relatorio_de_subida(hub.clone(), rb, saida.clone())));

    let controle = {
        let (hub, who, saida, conn) = (hub.clone(), who.clone(), saida.clone(), conn.clone());
        async move {
            control_loop(hub, who, rx, saida).await;
            // Sem controle não há conexão: fecha para os outros laços terminarem.
            conn.close(0u32.into(), b"controle encerrado");
        }
    };
    if preview {
        controle.await;
    } else {
        tokio::join!(
            controle,
            video_loop(hub.clone(), who.clone(), conn.clone(), relatorios.clone()),
            datagram_loop(hub.clone(), who.clone(), conn.clone(), relatorios),
        );
    }

    escritor.abort();
    if let Some(r) = relator {
        r.abort();
    }
    desregistrar(&hub, &who, &room_id, &conn);
}

/// Registra a conexão (substituindo a antiga), anuncia a sala por ela e faz o ticker repetir o que
/// pode ter sido descartado enquanto não havia conexão.
fn registrar(
    hub: &Hub,
    who: &str,
    room_id: &str,
    preview: bool,
    conn: &quinn::Connection,
    saida: &mpsc::UnboundedSender<ControlMsg>,
) {
    let antiga = {
        let mut inner = hub.inner.lock();
        let Some(p) = inner.participants.get_mut(who) else {
            return;
        };
        let antiga = p.quic.replace(conn.clone());
        p.quic_ctl = Some(saida.clone());
        p.quic_room = Some(room_id.to_string());
        p.quic_port = Some(conn.remote_address().port());
        // Um único `MediaSender` por conexão: o encaminhamento de mídia e a sonda dividem o mesmo
        // `Arc` (contadores e números de sequência não podem se duplicar).
        let sender = Arc::new(zap_net::media_tx::MediaSender::new(conn.clone(), hub.clock.clone()));
        p.media_tx = Some(sender.clone());
        // Sonda só para quem publica; a prévia não manda mídia.
        p.prober = if preview {
            None
        } else {
            Some(zap_net::probe::Prober::new(sender))
        };
        let num = p.num;

        if let Some(sala) = inner.rooms.get_mut(room_id) {
            for t in sala.tracks.iter_mut() {
                if t.participant_id == who {
                    t.relay.rerequest();
                }
                t.relay.reannounce(num);
            }
        }
        if let Some(sala) = inner.rooms.get(room_id) {
            let _ = saida.send(inner.room_state_msg(sala));
            if !preview {
                for e in &sala.chat {
                    let _ = saida.send(ControlMsg::ChatMessage {
                        id: e.id.clone(),
                        participant_id: e.participant_id.clone(),
                        name: e.name.clone(),
                        color: e.color.clone(),
                        text: e.text.clone(),
                        at: e.at.clone(),
                        client_id: None,
                    });
                }
            }
        }
        antiga
    };
    if let Some(antiga) = antiga.filter(|a| a.stable_id() != conn.stable_id()) {
        antiga.close(0u32.into(), b"substituida");
    }
}

/// Se `conn` ainda é a conexão registrada de `who`, limpa o registro e, se ele não reconectar à
/// mesma sala em `TOLERANCIA_QUEDA`, tira as trilhas e as assinaturas dele (sem QUIC não há
/// mídia). Uma conexão já substituída não mexe em nada.
fn desregistrar(hub: &Arc<Hub>, who: &str, room_id: &str, conn: &quinn::Connection) {
    let era_a_atual = {
        let mut inner = hub.inner.lock();
        let Some(p) = inner.participants.get_mut(who) else {
            return;
        };
        let atual = p.quic.as_ref().is_some_and(|q| q.stable_id() == conn.stable_id());
        if atual {
            p.quic = None;
            p.quic_ctl = None;
            p.quic_room = None;
            p.prober = None;
            p.media_tx = None;
        }
        atual
    };
    if !era_a_atual {
        return;
    }
    // O app reconecta logo (`online::reconnect`); as trilhas ficam para o `registrar` da conexão
    // nova repetir `camadas.pedidas` e `camada.atual`.
    let (hub, who, room_id) = (hub.clone(), who.to_string(), room_id.to_string());
    tokio::spawn(async move {
        tokio::time::sleep(TOLERANCIA_QUEDA).await;
        let voltou = hub
            .inner
            .lock()
            .participants
            .get(&who)
            .is_some_and(|p| p.quic.is_some() && p.quic_room.as_deref() == Some(room_id.as_str()));
        if !voltou {
            hub.drop_participant_tracks(&who, &room_id);
        }
    });
}

/// Aperto de mão do servidor (próprio, porque a sala só é conhecida pelo ingresso, e
/// `zap_net::handshake::accept_handshake` exige a sala antes): `accept_bi`, lê `ola` (10 s) e
/// responde `bem_vindo {participante_id, sala, relogio_us}` ou `recusado`:
/// - ponte desligada → `proibido`;
/// - primeira mensagem não é `ola` → `invalido`;
/// - `protocolo != 4` → `protocolo_incompativel`;
/// - sem ingresso, ingresso desconhecido ou vencido → `nao_autenticado`;
/// - sala do ingresso não existe mais → `sala_nao_encontrada`;
/// - ingresso de membro de quem não é mais membro → `proibido`.
pub async fn handshake(hub: &Hub, conn: &quinn::Connection) -> Result<Admitted, NetError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, aperto(hub, conn))
        .await
        .map_err(|_| NetError::Timeout)?
}

async fn aperto(hub: &Hub, conn: &quinn::Connection) -> Result<Admitted, NetError> {
    let (send, recv) = conn.accept_bi().await?;
    let tx = ControlTx::new(send);
    let mut rx = ControlRx::new(recv);

    let Some(ControlMsg::Hello { protocol, ticket, .. }) = rx.recv().await? else {
        return recusar(tx, "invalido", "A primeira mensagem precisa ser ola").await;
    };
    if !hub.config.relay {
        return recusar(tx, "proibido", "Este servidor não faz ponte de mídia").await;
    }
    if protocol != PROTOCOLO {
        return recusar(
            tx,
            "protocolo_incompativel",
            "Atualize o StreamaZap: o servidor fala o protocolo 4",
        )
        .await;
    }

    let decisao = {
        let inner = hub.inner.lock();
        let agora = now_unix_ms();
        match ticket.as_deref().and_then(|t| inner.tickets.get(t)) {
            Some(t) if t.expires_at_ms > agora => match inner.rooms.get(&t.room_id) {
                None => Err(("sala_nao_encontrada", "A sala não existe mais")),
                Some(sala) if !t.preview && !sala.members.contains(&t.participant_id) => {
                    Err(("proibido", "Entre na sala de novo (POST …/entrar)"))
                }
                Some(sala) => Ok((
                    t.participant_id.clone(),
                    t.room_id.clone(),
                    t.preview,
                    inner.room_view(sala),
                )),
            },
            _ => Err(("nao_autenticado", "Ingresso inválido ou expirado")),
        }
    };
    let (participant_id, room_id, preview, room) = match decisao {
        Ok(d) => d,
        Err((codigo, mensagem)) => return recusar(tx, codigo, mensagem).await,
    };

    tx.send(&ControlMsg::Welcome {
        participant_id: participant_id.clone(),
        room,
        clock_us: Some(hub.clock.now_us()),
        media_secret: None,
    })
    .await?;
    Ok(Admitted {
        tx,
        rx,
        participant_id,
        room_id,
        preview,
    })
}

/// Manda `recusado {codigo, mensagem}`, espera um pouco para ele chegar e devolve o erro.
async fn recusar(tx: ControlTx, codigo: &str, mensagem: &str) -> Result<Admitted, NetError> {
    let _ = tx
        .send(&ControlMsg::Refused {
            code: codigo.to_string(),
            message: mensagem.to_string(),
        })
        .await;
    tokio::time::sleep(ESPERA_RECUSA).await;
    Err(NetError::Rejected(codigo.to_string()))
}

/// Laço do stream de controle: `relatorio` → para cada trilha que o participante assina,
/// `TrackRelay::on_report(num, banda_kbps, estado)`; `ping` → `pong`; `assinar`, `cancelar` e
/// `keyframe.pedir` → `Hub::handle_sub_message_via` (resposta pelo QUIC). Termina quando o stream
/// acaba. Nunca segura o `Mutex` da `Hub` durante um `await`.
pub async fn control_loop(hub: Arc<Hub>, who: String, mut rx: ControlRx, saida: mpsc::UnboundedSender<ControlMsg>) {
    loop {
        let msg = match rx.recv().await {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(e) => {
                tracing::debug!("controle QUIC de {who}: {e}");
                break;
            }
        };
        match msg {
            ControlMsg::Report(r) => relatorio_do_espectador(&hub, &who, &r),
            ControlMsg::Ping { ts_ms } => {
                let _ = saida.send(ControlMsg::Pong { ts_ms });
            }
            // Latência e rota direta ficam fora do servidor de teste.
            ControlMsg::Pong { .. }
            | ControlMsg::Stats { .. }
            | ControlMsg::P2pMyAddresses { .. }
            | ControlMsg::DirectOk { .. }
            | ControlMsg::DirectFailed { .. } => {}
            outra => {
                if !hub.handle_sub_message_via(&who, &outra, Via::Quic) {
                    let _ = saida.send(ControlMsg::Error {
                        code: "invalido".into(),
                        message: "Pelo QUIC o servidor de teste só aceita relatorio, ping e assinaturas; \
                                  use o WebSocket"
                            .into(),
                        reference: None,
                    });
                }
            }
        }
    }
}

/// `relatorio` de quem assiste: atualiza banda e estado dele em toda trilha que ele assina e
/// alimenta a sonda de banda da conexão dele (fora do lock da `Hub`).
fn relatorio_do_espectador(hub: &Hub, who: &str, r: &Report) {
    let prober = {
        let mut guard = hub.inner.lock();
        let inner = &mut *guard;
        let Some(num) = inner.participants.get(who).map(|p| p.num) else {
            return;
        };
        for sala in inner.rooms.values_mut() {
            for t in sala.tracks.iter_mut() {
                if t.relay.viewers.contains_key(&num) {
                    t.relay.on_report(num, r.bandwidth_kbps, r.state);
                }
            }
        }
        inner.participants.get(who).and_then(|p| p.prober.clone())
    };
    if let Some(pr) = prober {
        pr.on_report(hub.clock.now_us(), r);
    }
}

/// A cada 100 ms, o `relatorio` de subida ao publicador (o servidor só registra chegadas; o
/// estimador roda no publicador). Só sai quando houve chegada, sonda ou perda.
async fn relatorio_de_subida(hub: Arc<Hub>, rb: ReportBuilderHandle, saida: mpsc::UnboundedSender<ControlMsg>) {
    let mut periodo = interval(REPORT_INTERVAL);
    periodo.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        periodo.tick().await;
        let r = rb.lock().take(hub.clock.now_us());
        if r.arrivals.is_empty() && r.probes.is_empty() && r.lost.is_empty() {
            continue;
        }
        if saida.send(ControlMsg::Report(r)).is_err() {
            break;
        }
    }
}

/// Aceita streams uni da conexão (um por quadro): `zap_net::media_rx::read_frame` e depois
/// `Hub::plan_video` + `media::forward_video`. Cada chegada entra no relatório de subida.
pub async fn video_loop(hub: Arc<Hub>, who: String, conn: quinn::Connection, rb: Option<ReportBuilderHandle>) {
    loop {
        let recv = match conn.accept_uni().await {
            Ok(r) => r,
            Err(_) => break,
        };
        let (hub, who, rb) = (hub.clone(), who.clone(), rb.clone());
        tokio::spawn(async move {
            match read_frame(recv, &hub.clock).await {
                Ok((header, data, chegada_us)) => {
                    if let Some(rb) = &rb {
                        let bytes = u32::try_from(VIDEO_HEADER_LEN + data.len()).unwrap_or(u32::MAX);
                        rb.lock().on_video(&header, chegada_us, bytes);
                    }
                    let plano = hub.plan_video(&who, &header);
                    forward_video(&hub, plano.targets, header, data).await;
                }
                // Stream resetado pelo publicador (quadro atrasado) ou incompleto: descartado.
                Err(e) => tracing::debug!("quadro de {who} descartado: {e}"),
            }
        });
    }
}

/// Lê datagramas: áudio (`decode_datagram`) → `audio_targets` + `forward_audio`; sondas entram
/// só no relatório de subida; datagramas inválidos são ignorados.
pub async fn datagram_loop(hub: Arc<Hub>, who: String, conn: quinn::Connection, rb: Option<ReportBuilderHandle>) {
    loop {
        let bytes = match conn.read_datagram().await {
            Ok(b) => b,
            Err(_) => break,
        };
        let chegada_us = hub.clock.now_us();
        let tamanho = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        match decode_datagram(&bytes) {
            Ok(Datagram::Audio(h, opus)) => {
                if let Some(rb) = &rb {
                    rb.lock().on_audio(&h, chegada_us, tamanho);
                }
                let alvos = hub.audio_targets(&who, h.track);
                forward_audio(&hub, &alvos, h, opus);
            }
            Ok(Datagram::Probe(h, _)) => {
                if let Some(rb) = &rb {
                    rb.lock().on_probe(&h, chegada_us, tamanho);
                }
            }
            Err(e) => tracing::debug!("datagrama inválido de {who}: {e}"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Config;
    use std::time::Instant;
    use zap_net::{
        handshake::{connect_handshake, Hello},
        online::{CreateRoomRequest, LoginRequest},
    };
    use zap_proto::{Access, DelayState, TileSize};

    /// Nome no certificado da ponte nos testes.
    const NOME: &str = "zap-test";

    fn entrar(hub: &Hub, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: format!("inst-{nome}"),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

    fn ola(ticket: &str) -> Hello {
        Hello {
            app_version: "3.0.0".into(),
            install_id: "inst".into(),
            name: "Teste".into(),
            color: "#3366ff".into(),
            ticket: Some(ticket.into()),
            room_id: None,
            preview: None,
        }
    }

    /// Cenário da ponte: sala aberta de Ana, Bia membro, ponte de pé.
    struct Ponte {
        hub: Arc<Hub>,
        addr: SocketAddr,
        fp: String,
        sala: String,
        ana: String,
        bia: String,
        /// Ingresso de Bia.
        ingresso: String,
        /// Mantém o endpoint vivo.
        _ep: quinn::Endpoint,
    }

    async fn ponte() -> Ponte {
        let hub = Arc::new(Hub::new(Config::default()));
        let ana = entrar(&hub, "Ana");
        let bia = entrar(&hub, "Bia");
        let sala = hub
            .create_room(
                &ana,
                CreateRoomRequest {
                    name: "Sala".into(),
                    access: Access::Open,
                    password: None,
                },
            )
            .unwrap();
        let ingresso = hub.join_room(&bia, &sala.code, None).unwrap().ticket;
        let identity = Identity::generate(NOME).unwrap();
        let fp = identity.fingerprint();
        let (ep, addr, _tarefa) = start(hub.clone(), &identity).await.unwrap();
        Ponte {
            hub,
            addr,
            fp,
            sala: sala.id,
            ana,
            bia,
            ingresso,
            _ep: ep,
        }
    }

    async fn conectar(addr: SocketAddr, fp: &str) -> quinn::Connection {
        let ep = zap_net::endpoint::client_endpoint().unwrap();
        ep.connect_with(zap_net::tls::client_config(Some(fp)).unwrap(), addr, NOME)
            .unwrap()
            .await
            .unwrap()
    }

    /// Espera `cond` ficar verdadeira (até 2 s).
    async fn ate(mut cond: impl FnMut() -> bool) -> bool {
        let fim = Instant::now() + Duration::from_secs(2);
        while Instant::now() < fim {
            if cond() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        cond()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_valido_recebe_bem_vindo_e_estado() {
        let p = ponte().await;
        let conn = conectar(p.addr, &p.fp).await;
        let (_tx, mut rx, welcome) = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert_eq!(welcome.participant_id, p.bia);
        assert_eq!(welcome.room.id, p.sala);
        assert!(welcome.clock_us.is_some());

        // O `ola` vale como `sala.entrar`: o estado da sala chega pelo próprio QUIC.
        let estado = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(estado, Some(ControlMsg::RoomState { .. })), "{estado:?}");
        assert!(ate(|| p.hub.inner.lock().participants[&p.bia].quic_ctl.is_some()).await);
        assert_eq!(
            p.hub.inner.lock().participants[&p.bia].quic_room.as_deref(),
            Some(p.sala.as_str())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_invalido() {
        let p = ponte().await;
        let conn = conectar(p.addr, &p.fp).await;
        let erro = connect_handshake(&conn, ola("xxx"), None).await.err().unwrap();
        assert!(
            matches!(&erro, NetError::Rejected(c) if c == "nao_autenticado"),
            "{erro:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ingresso_de_quem_saiu_e_recusado() {
        let p = ponte().await;
        let codigo = p.hub.list_rooms(&p.ana)[0].code.clone();
        p.hub.leave_room(&p.bia, &codigo).unwrap();
        let conn = conectar(p.addr, &p.fp).await;
        let erro = connect_handshake(&conn, ola(&p.ingresso), None).await.err().unwrap();
        // Sair apaga o ingresso: ele deixa de ser reconhecido.
        assert!(
            matches!(&erro, NetError::Rejected(c) if c == "nao_autenticado"),
            "{erro:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relatorio_chega_ao_relay() {
        let p = ponte().await;
        let num = {
            let mut inner = p.hub.inner.lock();
            let num = inner.participants[&p.bia].num;
            let camadas = vec![zap_proto::LayerSpec {
                layer: 0,
                width: 1280,
                height: 720,
                fps: 30,
                kbps: 2500,
            }];
            let mut relay = crate::relay::TrackRelay::new(camadas.clone());
            relay.subscribe(crate::relay::ViewerEntry {
                participant_id: p.bia.clone(),
                num,
                size: TileSize::Main,
                max_layer_req: None,
                audio: false,
                preview: false,
                bandwidth_kbps: 0,
                state: DelayState::Normal,
                current: 0,
                pending: None,
                announced: None,
            });
            inner
                .rooms
                .get_mut(&p.sala)
                .unwrap()
                .tracks
                .push(crate::state::TrackData {
                    id: "t1".into(),
                    number: 1,
                    participant_id: p.ana.clone(),
                    kind: zap_proto::TrackKind::Camera,
                    title: "câmera".into(),
                    layers: camadas,
                    audio: None,
                    relay,
                });
            num
        };
        let conn = conectar(p.addr, &p.fp).await;
        let (tx, _rx, _w) = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        tx.send(&ControlMsg::Report(Report {
            arrivals: vec![],
            probes: vec![],
            lost: vec![],
            loss: 0.0,
            bandwidth_kbps: Some(4321),
            state: Some(DelayState::Normal),
            buffer_ms: Some(100),
        }))
        .await
        .unwrap();
        assert!(
            ate(|| p.hub.inner.lock().rooms[&p.sala].tracks[0].relay.viewers[&num].bandwidth_kbps == 4321).await,
            "o relatório não chegou ao alocador"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fechar_limpa_quic() {
        let p = ponte().await;
        let conn = conectar(p.addr, &p.fp).await;
        let _c = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert!(ate(|| p.hub.inner.lock().participants[&p.bia].quic.is_some()).await);
        conn.close(0u32.into(), b"tchau");
        assert!(
            ate(|| {
                let inner = p.hub.inner.lock();
                let q = &inner.participants[&p.bia];
                q.quic.is_none() && q.quic_ctl.is_none() && q.quic_room.is_none()
            })
            .await
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conexao_nova_substitui_a_antiga() {
        let p = ponte().await;
        let antiga = conectar(p.addr, &p.fp).await;
        let _a = connect_handshake(&antiga, ola(&p.ingresso), None).await.unwrap();
        let nova = conectar(p.addr, &p.fp).await;
        let _n = connect_handshake(&nova, ola(&p.ingresso), None).await.unwrap();

        let fechou = tokio::time::timeout(Duration::from_secs(2), antiga.closed()).await;
        assert!(fechou.is_ok(), "a conexão antiga devia ser fechada");
        // A queda da antiga não apaga o registro da nova: a registrada continua aberta. (O
        // `stable_id` só vale num lado da conexão, então não dá para comparar com o do cliente.)
        tokio::time::sleep(Duration::from_millis(200)).await;
        let inner = p.hub.inner.lock();
        let q = &inner.participants[&p.bia];
        assert!(q.quic.as_ref().is_some_and(|c| c.close_reason().is_none()));
        assert!(q.quic_ctl.is_some());
        assert!(nova.close_reason().is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ponte_desligada_recusa() {
        let hub = Arc::new(Hub::new(Config {
            relay: false,
            ..Config::default()
        }));
        let identity = Identity::generate(NOME).unwrap();
        let fp = identity.fingerprint();
        let (_ep, addr, _t) = start(hub, &identity).await.unwrap();
        let conn = conectar(addr, &fp).await;
        let erro = connect_handshake(&conn, ola("qualquer"), None).await.err().unwrap();
        assert!(matches!(&erro, NetError::Rejected(c) if c == "proibido"), "{erro:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn registrar_cria_prober_e_desregistrar_tira() {
        let p = ponte().await;
        let tem_prober = |p: &Ponte| p.hub.inner.lock().participants[&p.bia].prober.is_some();

        // Espectador não-prévia: a sonda nasce ao registrar a conexão.
        let conn = conectar(p.addr, &p.fp).await;
        let _c = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert!(ate(|| tem_prober(&p)).await, "a sonda devia existir após registrar");

        // Fechar a conexão tira a sonda (até 3 s).
        conn.close(0u32.into(), b"tchau");
        let fim = Instant::now() + Duration::from_secs(3);
        loop {
            if !tem_prober(&p) {
                break;
            }
            assert!(Instant::now() < fim, "a sonda devia sair ao fechar a conexão");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Ingresso de prévia: registra, mas não publica, então não há sonda.
        let codigo = p.hub.list_rooms(&p.ana)[0].code.clone();
        let ingresso_previa = p.hub.preview_room(&p.bia, &codigo).unwrap().ticket;
        let conn2 = conectar(p.addr, &p.fp).await;
        let _c2 = connect_handshake(&conn2, ola(&ingresso_previa), None).await.unwrap();
        assert!(ate(|| p.hub.inner.lock().participants[&p.bia].quic_ctl.is_some()).await);
        assert!(!tem_prober(&p), "prévia não deve ter sonda");
    }

    /// Trilha 1 de Ana com Bia assistindo a camada 0, sem troca pendente.
    fn trilha_de_ana_com_bia(p: &Ponte) {
        let mut inner = p.hub.inner.lock();
        let num = inner.participants[&p.bia].num;
        let camadas = vec![zap_proto::LayerSpec {
            layer: 0,
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 2500,
        }];
        let mut relay = crate::relay::TrackRelay::new(camadas.clone());
        relay.subscribe(crate::relay::ViewerEntry {
            participant_id: p.bia.clone(),
            num,
            size: TileSize::Main,
            max_layer_req: None,
            audio: false,
            preview: false,
            bandwidth_kbps: 0,
            state: DelayState::Normal,
            current: 0,
            pending: None,
            announced: None,
        });
        inner
            .rooms
            .get_mut(&p.sala)
            .unwrap()
            .tracks
            .push(crate::state::TrackData {
                id: "t1".into(),
                number: 1,
                participant_id: p.ana.clone(),
                kind: zap_proto::TrackKind::Camera,
                title: "câmera".into(),
                layers: camadas,
                audio: None,
                relay,
            });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn registrar_cria_media_tx_e_desregistrar_tira() {
        let p = ponte().await;
        let tem = |p: &Ponte| p.hub.inner.lock().participants[&p.bia].media_tx.is_some();

        let conn = conectar(p.addr, &p.fp).await;
        let _c = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert!(ate(|| tem(&p)).await, "o sender devia existir após registrar");

        conn.close(0u32.into(), b"tchau");
        assert!(ate(|| !tem(&p)).await, "o sender devia sair ao fechar a conexão");

        // Prévia: não publica (sem sonda), mas também recebe mídia, então tem sender.
        let codigo = p.hub.list_rooms(&p.ana)[0].code.clone();
        let ingresso_previa = p.hub.preview_room(&p.bia, &codigo).unwrap().ticket;
        let conn2 = conectar(p.addr, &p.fp).await;
        let _c2 = connect_handshake(&conn2, ola(&ingresso_previa), None).await.unwrap();
        assert!(ate(|| tem(&p)).await, "a prévia também devia ter sender");
        assert!(p.hub.inner.lock().participants[&p.bia].prober.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn plan_video_reusa_o_sender_da_conexao() {
        let p = ponte().await;
        trilha_de_ana_com_bia(&p);
        let conn = conectar(p.addr, &p.fp).await;
        let _c = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert!(ate(|| p.hub.inner.lock().participants[&p.bia].media_tx.is_some()).await);

        let header = zap_proto::media::VideoHeader {
            flags: zap_proto::media::VideoFlags(0),
            track: 1,
            layer: 0,
            width: 1280,
            height: 720,
            seq: 1,
            capture_us: 0,
            send_us: 0,
            size: 0,
        };
        let plano1 = p.hub.plan_video(&p.ana, &header);
        let plano2 = p.hub.plan_video(&p.ana, &header);
        assert_eq!(plano1.targets.len(), 1);
        assert!(Arc::ptr_eq(&plano1.targets[0], &plano2.targets[0]));
        let do_bia = p.hub.inner.lock().participants[&p.bia].media_tx.clone().unwrap();
        assert!(Arc::ptr_eq(&plano1.targets[0], &do_bia));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relatorio_do_espectador_alimenta_o_prober() {
        let p = ponte().await;
        let conn = conectar(p.addr, &p.fp).await;
        let (tx, _rx, _w) = connect_handshake(&conn, ola(&p.ingresso), None).await.unwrap();
        assert!(ate(|| p.hub.inner.lock().participants[&p.bia].prober.is_some()).await);
        tx.send(&ControlMsg::Report(Report {
            arrivals: vec![],
            probes: vec![],
            lost: vec![],
            loss: 0.0,
            bandwidth_kbps: Some(1000),
            state: Some(DelayState::Normal),
            buffer_ms: Some(100),
        }))
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(p.hub.inner.lock().participants[&p.bia].prober.is_some());
        assert!(conn.close_reason().is_none());
    }
}
