//! Ponte QUIC `zap/4` (`docs/api/midia.md`): aceita conexões, faz o aperto de mão com o
//! `ingresso`, lê relatórios e repassa mídia.

use std::{net::SocketAddr, sync::Arc};

use zap_net::{
    control::{ControlRx, ControlTx},
    tls::Identity,
};

use crate::{state::Hub, TestServerError};

/// Abre o endpoint QUIC em `127.0.0.1:0` com `identity`, preenche `Hub::bridge_addr` e devolve
/// o endpoint, o endereço real e a tarefa do laço de `accept`.
pub async fn start(
    hub: Arc<Hub>,
    identity: &Identity,
) -> Result<(quinn::Endpoint, SocketAddr, tokio::task::JoinHandle<()>), TestServerError> {
    let _ = (hub, identity);
    todo!("tarefa zt-16")
}

/// Uma conexão: aperto de mão (`handshake`), depois três laços concorrentes (control, vídeo,
/// áudio). Ao fechar: limpa `ParticipantData::quic` e chama `drop_participant_tracks` se o
/// participante não tem mais conexão.
pub async fn serve_connection(hub: Arc<Hub>, conn: quinn::Connection) {
    let _ = (hub, conn);
    todo!("tarefa zt-16")
}

/// Aperto de mão do servidor (própria, porque a sala só é conhecida pelo ingresso, e
/// `zap_net::handshake::accept_handshake` exige a sala antes): `accept_bi`, lê `ola` (10 s);
/// `protocolo != 4` → `recusado` `protocolo_incompativel`; sem ingresso ou ingresso
/// desconhecido/expirado → `recusado` `nao_autenticado`; ingresso válido → registra a conexão em
/// `ParticipantData::quic` (e `quic_port`), responde `bem_vindo {participante_id, sala,
/// relogio_us}` e devolve `(tx, rx, participant_id, preview)`.
pub async fn handshake(
    hub: &Hub,
    conn: &quinn::Connection,
) -> Result<(ControlTx, ControlRx, String, bool), zap_net::NetError> {
    let _ = (hub, conn);
    todo!("tarefa zt-16")
}

/// Laço do stream de controle: `relatorio` → para cada trilha que o participante assina,
/// `TrackRelay::on_report(num, banda_kbps, estado)`; `ping` → `pong`; `keyframe.pedir` →
/// `Hub::handle_track_message`. Termina quando o stream acaba.
pub async fn control_loop(hub: Arc<Hub>, who: String, mut rx: ControlRx, tx: ControlTx) {
    let _ = (hub, who, &mut rx, tx);
    todo!("tarefa zt-16")
}

/// Aceita streams uni da conexão (um por quadro): `zap_net::media_rx::read_frame` e depois
/// `Hub::plan_video` + `media::forward_video`. Prévia (ingresso `preview`) não publica: ignora.
pub async fn video_loop(hub: Arc<Hub>, who: String, conn: quinn::Connection) {
    let _ = (hub, who, conn);
    todo!("tarefa zt-16")
}

/// Lê datagramas: áudio (`decode_datagram`) → `audio_targets` + `forward_audio`; sondas e
/// datagramas inválidos são ignorados.
pub async fn datagram_loop(hub: Arc<Hub>, who: String, conn: quinn::Connection) {
    let _ = (hub, who, conn);
    todo!("tarefa zt-16")
}
