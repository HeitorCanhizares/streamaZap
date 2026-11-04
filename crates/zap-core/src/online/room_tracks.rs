//! Trilhas da sala Online: publicar, atualizar, remover (pelo WebSocket).

use zap_proto::{LayerSpec, Track};

use super::room::OnlineRoom;
use crate::publish::PublishedTrack;
use crate::room::TrackSpec;
use crate::CoreError;

impl OnlineRoom {
    /// `TrackPublish{local_id, kind, title, layers, audio, address: None}` pelo WebSocket;
    /// espera `TrackPublished` com o mesmo `local_id` (entregue por `room_events` via `pending`)
    /// por 5 s (`Connect("servidor não respondeu")` senão; `Error` do servidor com `ref`/código
    /// → `CoreError::Server`). Devolve a `Track` com `id`/`number` do servidor (já no modelo).
    pub async fn publish_track(&self, spec: TrackSpec) -> Result<Track, CoreError> {
        let _ = spec;
        todo!("tarefa core-online-09")
    }
    /// `TrackUpdate{track_id, title, layers}`; o modelo muda quando chega `TrackUpdated`.
    pub async fn update_track(
        &self,
        track_id: &str,
        title: Option<String>,
        layers: Option<Vec<LayerSpec>>,
    ) -> Result<(), CoreError> {
        let _ = (track_id, title, layers);
        todo!("tarefa core-online-09")
    }
    /// `TrackRemove{track_id}`, `publish.detach(track_id)` e tira do modelo na hora.
    pub async fn remove_track(&self, track_id: &str) -> Result<(), CoreError> {
        let _ = track_id;
        todo!("tarefa core-online-09")
    }
    /// Chamado por `share.rs` quando o publicador local da trilha já existe: liga-o à ponte
    /// (`publish.attach`) para o servidor poder pedir camadas. Sem ponte conectada, guarda e
    /// liga quando ela conectar (`reconnect`).
    pub fn on_track_started(&self, track: &Track, published: PublishedTrack) {
        let _ = (track, published);
        todo!("tarefa core-online-09")
    }
}
