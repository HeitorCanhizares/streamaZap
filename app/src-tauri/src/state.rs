//! Estado gerenciado pelo Tauri (`app.manage`): o núcleo e os canais de vídeo.

use std::sync::Arc;

use zap_core::Core;

use crate::video::VideoChannels;

pub struct AppState {
    pub core: Arc<Core>,
    pub videos: Arc<VideoChannels>,
}
