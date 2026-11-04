//! Prévia na lista de salas: camada 2, sem áudio, ingresso de prévia (60 s, renovável).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use zap_net::control::{ControlRx, ControlTx};
use zap_net::media_rx::{spawn_receiver, MediaEvent};
use zap_net::online::ServerClient;
use zap_net::report::{spawn_reporter, ReportBuilder, ReportBuilderHandle};
use zap_proto::{ControlMsg, RoomCode, TileSize, TrackKind};

use crate::engine::Shared;
use crate::types::{TileId, VideoPacket};
use crate::CoreError;

/// Prévias abertas por tile.
#[derive(Default)]
pub struct PreviewRegistry {
    pub(crate) active: Mutex<HashMap<TileId, tokio::task::JoinHandle<()>>>,
}

/// Camada única da prévia.
const CAMADA_PREVIA: u8 = 2;
/// Validade do ingresso de prévia.
const VALIDADE_INGRESSO: Duration = Duration::from_secs(60);
/// Espera antes de tentar de novo depois de uma falha passageira.
const NOVA_TENTATIVA: Duration = Duration::from_secs(5);

/// Quando renovar o ingresso: 75% do prazo, no mínimo 5 s.
fn renew_in(expires_in: Duration) -> Duration {
    (expires_in * 3 / 4).max(Duration::from_secs(5))
}

/// Evento de mídia → pacote de vídeo; só camada 2 (áudio, perdas e outras camadas caem).
fn packet_from_frame(ev: &MediaEvent) -> Option<VideoPacket> {
    match ev {
        MediaEvent::Video { header, data, .. } if header.layer == CAMADA_PREVIA => Some(VideoPacket {
            track: header.track,
            layer: header.layer,
            keyframe: header.flags.is_keyframe(),
            width: header.width,
            height: header.height,
            seq: header.seq,
            capture_us: header.capture_us,
            data: data.clone(),
        }),
        _ => None,
    }
}

/// Conexão de prévia com as tarefas dela; ao sair de cena fecha tudo.
struct Link {
    conn: quinn::Connection,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.conn.close(0u32.into(), b"fim");
        for t in &self.tasks {
            t.abort();
        }
    }
}

/// Prazo para o `sala.estado` chegar na conexão de prévia.
const ESPERA_ESTADO: Duration = Duration::from_secs(5);

/// Lê o controle até o `sala.estado` (respondendo `ping`) e devolve a primeira trilha de vídeo.
/// O REST só diz os TIPOS compartilhados (`compartilhando`); o id da trilha vem daqui.
async fn trilha_principal(crx: &mut ControlRx, ctrl: &ControlTx) -> Result<String, CoreError> {
    let espera = async {
        loop {
            match crx.recv().await? {
                Some(ControlMsg::RoomState { tracks, .. }) => {
                    return tracks
                        .into_iter()
                        .find(|t| t.kind != TrackKind::Audio)
                        .map(|t| t.id)
                        .ok_or_else(|| CoreError::Invalid("Ninguém está transmitindo".into()));
                }
                Some(ControlMsg::Ping { ts_ms }) => ctrl.send(&ControlMsg::Pong { ts_ms }).await?,
                Some(_) => {}
                None => return Err(CoreError::Connect("ponte fechou a conexão de prévia".into())),
            }
        }
    };
    tokio::time::timeout(ESPERA_ESTADO, espera)
        .await
        .map_err(|_| CoreError::Connect("ponte não mandou o estado da sala".into()))?
}

/// Pede ingresso, conecta na ponte, assina a trilha principal e liga o receptor.
async fn open(shared: &Arc<Shared>, client: &ServerClient, code: &RoomCode, tile: TileId) -> Result<Link, CoreError> {
    let prev = client.preview(code).await?;
    let detalhes = client.room(code).await?;
    // Sem ninguém compartilhando nem vale conectar na ponte.
    let alguem = detalhes
        .participant_list
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|p| !p.sharing.is_empty());
    if !alguem {
        return Err(CoreError::Invalid("Ninguém está transmitindo".into()));
    }

    let ponte = super::bridge::connect_bridge(
        shared,
        &prev.media.bridge.address,
        &prev.ticket,
        &detalhes.room.id,
        true,
    )
    .await?;
    let super::bridge::BridgeConnected {
        conn, ctrl, mut crx, ..
    } = ponte;
    // Daqui em diante o `Link` fecha a conexão se algo falhar ou a tarefa for abortada.
    let mut link = Link {
        conn,
        tasks: Vec::new(),
    };
    let track_id = trilha_principal(&mut crx, &ctrl).await?;

    let report: ReportBuilderHandle = Arc::new(Mutex::new(ReportBuilder::new(true)));
    let (etx, mut erx) = tokio::sync::mpsc::channel(256);
    link.tasks.push(spawn_receiver(
        link.conn.clone(),
        shared.clock.clone(),
        etx,
        report.clone(),
    ));
    link.tasks
        .push(spawn_reporter(ctrl.clone(), report, shared.clock.clone()));
    let sh = shared.clone();
    link.tasks.push(tokio::spawn(async move {
        while let Some(ev) = erx.recv().await {
            if let Some(pacote) = packet_from_frame(&ev) {
                sh.ui.video(tile, pacote);
            }
        }
    }));
    link.tasks.push(tokio::spawn(responde_ping(crx, ctrl.clone())));

    ctrl.send(&ControlMsg::Subscribe {
        track_id,
        size: TileSize::Thumbnail,
        max_layer: Some(CAMADA_PREVIA),
        audio: Some(false),
    })
    .await?;
    Ok(link)
}

/// Lê o controle da ponte e responde `ping` com `pong`.
async fn responde_ping(mut crx: ControlRx, ctrl: ControlTx) {
    while let Ok(Some(msg)) = crx.recv().await {
        match msg {
            ControlMsg::Ping { ts_ms } => {
                let _ = ctrl.send(&ControlMsg::Pong { ts_ms }).await;
            }
            // Recusa de assinatura etc.: não há a quem avisar, mas fica no log.
            ControlMsg::Error { code, message, .. } => {
                tracing::debug!(%code, %message, "prévia: erro da ponte");
            }
            _ => {}
        }
    }
}

/// Laço de fundo: renova o ingresso e reconecta; termina se a sala acabou.
async fn run(shared: Arc<Shared>, client: ServerClient, code: RoomCode, tile: TileId, first: Link) {
    let mut link = first;
    loop {
        // Dorme até renovar ou até a conexão cair.
        tokio::select! {
            _ = tokio::time::sleep(renew_in(VALIDADE_INGRESSO)) => {}
            _ = link.conn.closed() => {}
        }
        loop {
            match open(&shared, &client, &code, tile).await {
                // A conexão antiga fecha ao ser substituída.
                Ok(novo) => {
                    link = novo;
                    break;
                }
                Err(CoreError::Server { code: ref c, .. }) if c == "sala_nao_encontrada" => return,
                Err(e) => {
                    tracing::debug!(erro = %e, "prévia: nova tentativa");
                    tokio::time::sleep(NOVA_TENTATIVA).await;
                }
            }
        }
    }
}

/// Abre a prévia da sala `code` no `tile` (escolhido pela interface; não colide com os tiles de
/// `watch`: a interface usa ids ≥ 1_000_000 para prévias):
/// 1. `client.preview(&code)` → ingresso + ponte; `client.room(&code)` → `RoomDetails` com
///    `participant_list` (sala sem ninguém em `sharing` → `Invalid("Ninguém está transmitindo")`);
/// 2. `bridge::connect_bridge(.., preview = true)`; a trilha principal = primeira trilha de
///    vídeo do `sala.estado` que a ponte manda logo após o `bem_vindo` (`sharing` só tem tipos);
/// 3. manda `Subscribe{track_id, size: Thumbnail, max_layer: Some(2), audio: Some(false)}` no
///    stream de controle QUIC da conexão de prévia;
/// 4. `zap_net::media_rx::spawn_receiver` na conexão; cada quadro de vídeo vira
///    `VideoPacket` entregue direto em `shared.ui.video(tile, packet)` (sem `playout`, sem
///    áudio; descarta o que não for camada 2);
/// 5. renova o ingresso a cada 45 s (`client.preview` de novo + reconecta se necessário) e
///    encerra sozinha se a sala acabar.
///
/// Já existe prévia no `tile` → substitui. A tarefa fica em `session.previews`.
pub async fn start(shared: &Arc<Shared>, code: RoomCode, tile: TileId) -> Result<(), CoreError> {
    let client = shared
        .online
        .client()
        .ok_or_else(|| CoreError::Invalid("Servidor não conectado".into()))?;
    let link = open(shared, &client, &code, tile).await?;
    let handle = tokio::spawn(run(shared.clone(), client, code, tile, link));
    if let Some(antiga) = shared.online.previews.active.lock().insert(tile, handle) {
        antiga.abort();
    }
    Ok(())
}

/// Para a prévia do `tile` (aborta a tarefa; a conexão QUIC fecha). Tile sem prévia: nada.
pub fn stop(shared: &Arc<Shared>, tile: TileId) {
    if let Some(h) = shared.online.previews.active.lock().remove(&tile) {
        h.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;
    use bytes::Bytes;
    use zap_net::online::CreateRoomRequest;
    use zap_proto::media::{VideoFlags, VideoHeader};
    use zap_proto::Access;
    use zap_test_server::Config;

    fn video(layer: u8, key: bool) -> MediaEvent {
        MediaEvent::Video {
            header: VideoHeader {
                flags: VideoFlags(u8::from(key)),
                track: 7,
                layer,
                width: 320,
                height: 180,
                seq: 9,
                capture_us: 1234,
                send_us: 0,
                size: 3,
            },
            data: Bytes::from_static(&[1, 2, 3]),
            arrival_us: 0,
        }
    }

    #[test]
    fn conversao_so_camada_2() {
        let p = packet_from_frame(&video(2, true)).unwrap();
        assert_eq!(
            (p.track, p.layer, p.keyframe, p.seq, p.capture_us),
            (7, 2, true, 9, 1234)
        );
        assert_eq!((p.width, p.height), (320, 180));
        assert_eq!(&p.data[..], &[1, 2, 3]);
        assert!(packet_from_frame(&video(0, false)).is_none());
        assert!(packet_from_frame(&video(1, false)).is_none());
        assert!(packet_from_frame(&MediaEvent::Lost {
            track: 7,
            layer: 2,
            seq: 1
        })
        .is_none());
    }

    #[test]
    fn renovacao_75_por_cento_minimo_5s() {
        assert_eq!(renew_in(Duration::from_secs(60)), Duration::from_secs(45));
        assert_eq!(renew_in(Duration::from_secs(4)), Duration::from_secs(5));
        assert_eq!(renew_in(Duration::from_secs(0)), Duration::from_secs(5));
    }

    async fn novo_core(url: String) -> (Core, std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-prev-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                ..Default::default()
            };
            let core = Core::start_with(d.clone(), TestSink::new(), opts).unwrap();
            core.update_settings(|s| s.server.address = url);
            (core, d)
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_sem_previa_nao_faz_nada() {
        let (core, d) = novo_core(String::new()).await;
        stop(&core.shared, 1_000_000);
        assert!(core.shared.online.previews.active.lock().is_empty());
        encerrar(core, d).await;
    }

    async fn sala(core: &Core, access: Access, password: Option<&str>) -> RoomCode {
        let _rx = core.shared.online.connect(&core.shared).await.unwrap();
        let client = core.shared.online.client().unwrap();
        client
            .create_room(CreateRoomRequest {
                name: "Sala".into(),
                access,
                password: password.map(str::to_string),
            })
            .await
            .unwrap()
            .code
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn start_sala_sem_transmissao_invalido() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let code = sala(&core, Access::Open, None).await;
        let r = start(&core.shared, code, 1_000_001).await;
        assert!(
            matches!(r, Err(CoreError::Invalid(ref m)) if m == "Ninguém está transmitindo"),
            "{r:?}"
        );
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn start_sala_com_senha_proibido() {
        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone()).await;
        let code = sala(&core, Access::Password, Some("segredo")).await;
        let r = start(&core.shared, code, 1_000_002).await;
        assert!(
            matches!(r, Err(CoreError::Server { ref code, .. }) if code == "proibido"),
            "{r:?}"
        );
        encerrar(core, d).await;
    }
}
