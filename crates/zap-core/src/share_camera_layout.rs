//! Jeito da câmera (canto da tela ou solta) ao vivo. `impl ShareSession`.

use std::sync::Arc;

use zap_proto::CameraLayout;

use crate::engine::Shared;
use crate::share::ShareSession;
use crate::CoreError;

impl ShareSession {
    /// Manda `trilha.atualizar` só com `camera_layout` para a trilha da câmera
    /// (`room.update_track(id, None, None, Some(layout))`), sem republicar nem mexer nas camadas.
    /// Sem câmera ligada ou sem sala: `Ok` sem fazer nada (o valor já foi guardado nas
    /// configurações por `Core::set_camera_layout`). Tarefa core-07.
    pub async fn set_camera_layout(shared: &Arc<Shared>, layout: CameraLayout) -> Result<(), CoreError> {
        // Copia o id e solta o lock antes de falar com a sala.
        let track_id = {
            let sessao = shared.share.lock().await;
            match sessao.camera.as_ref() {
                Some(c) => c.track.id.clone(),
                None => return Ok(()),
            }
        };
        let Some(room) = shared.current_room() else {
            return Ok(());
        };
        room.update_track(&track_id, None, None, Some(layout)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Core, CoreOptions};
    use crate::testutil::TestSink;

    #[test]
    fn sem_camera_e_sem_sala_e_ok() {
        let d = std::env::temp_dir().join(format!("zapcore-camlayout-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        let core = Core::start_with(d.clone(), TestSink::new(), opts).expect("core");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let r = rt.block_on(ShareSession::set_camera_layout(&core.shared, CameraLayout::Free));
        assert!(r.is_ok());
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }
}
