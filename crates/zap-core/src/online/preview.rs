//! Prévia na lista de salas: camada 2, sem áudio, ingresso de prévia (60 s, renovável).

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use zap_proto::RoomCode;

use crate::engine::Shared;
use crate::types::TileId;
use crate::CoreError;

/// Prévias abertas por tile.
#[derive(Default)]
pub struct PreviewRegistry {
    pub(crate) active: Mutex<HashMap<TileId, tokio::task::JoinHandle<()>>>,
}

/// Abre a prévia da sala `code` no `tile` (escolhido pela interface; não colide com os tiles de
/// `watch`: a interface usa ids ≥ 1_000_000 para prévias):
/// 1. `client.preview(&code)` → ingresso + ponte; `client.room(&code)` → `RoomDetails` com
///    `participant_list`; escolhe a trilha principal = primeiro id de `sharing` (sala sem
///    ninguém compartilhando → `Invalid("Ninguém está transmitindo")`);
/// 2. `bridge::connect_bridge(.., preview = true)`;
/// 3. manda `Subscribe{track_id, size: Thumbnail, max_layer: Some(2), audio: Some(false)}` no
///    stream de controle QUIC da conexão de prévia;
/// 4. `zap_net::media_rx::spawn_receiver` na conexão; cada quadro de vídeo vira
///    `VideoPacket` entregue direto em `shared.ui.video(tile, packet)` (sem `playout`, sem
///    áudio; descarta o que não for camada 2);
/// 5. renova o ingresso a cada 45 s (`client.preview` de novo + reconecta se necessário) e
///    encerra sozinha se a sala acabar.
/// Já existe prévia no `tile` → substitui. A tarefa fica em `session.previews`.
pub async fn start(shared: &Arc<Shared>, code: RoomCode, tile: TileId) -> Result<(), CoreError> {
    let _ = (shared, code, tile);
    todo!("tarefa core-online-14")
}

/// Para a prévia do `tile` (aborta a tarefa; a conexão QUIC fecha). Tile sem prévia: nada.
pub fn stop(shared: &Arc<Shared>, tile: TileId) {
    let _ = (shared, tile);
    todo!("tarefa core-online-14")
}
