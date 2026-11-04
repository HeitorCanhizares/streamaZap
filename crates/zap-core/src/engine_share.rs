//! `impl Core`: compartilhamento (delegam a `ShareSession`, saltando para o runtime do núcleo,
//! e depois atualizam `UiState.share` + `notifier.touch()`).

use crate::engine::Core;
use crate::types::*;
use crate::CoreError;

impl Core {
    pub async fn start_share(&self, req: ShareRequest) -> Result<(), CoreError> {
        todo!("tarefa core-41")
    }
    pub async fn update_share(&self, req: ShareRequest) -> Result<(), CoreError> {
        todo!("tarefa core-41")
    }
    pub async fn stop_share(&self) {
        todo!("tarefa core-41")
    }
    pub async fn set_camera(&self, on: bool) -> Result<(), CoreError> {
        todo!("tarefa core-41")
    }
}
