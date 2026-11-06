//! Inicialização e saída do app.

use std::error::Error;
use std::sync::Arc;

use tauri::{App, AppHandle, Manager};
use tracing_subscriber::EnvFilter;
use zap_core::Core;

use crate::sink::TauriSink;
use crate::state::AppState;
use crate::video::VideoChannels;

/// Inicia o log (`tracing_subscriber` com `EnvFilter`, padrão `info`), resolve o diretório de
/// dados (`app.path().app_data_dir()`, criando-o), cria `VideoChannels` + `TauriSink`
/// (`crate::sink`), inicia `zap_core::Core::start(dir, sink)` e registra
/// `AppState { core: Arc<Core>, videos }` com `app.manage`. Erro do núcleo aborta o `setup`
/// com a mensagem do `CoreError`.
pub fn init(app: &mut App) -> Result<(), Box<dyn Error>> {
    // Log: erro ao instalar (já instalado) é ignorado de propósito.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init();

    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;

    let videos = Arc::new(VideoChannels::default());
    let sink = Arc::new(TauriSink {
        app: app.handle().clone(),
        videos: videos.clone(),
    });
    let core = Core::start(dir, sink).map_err(|e| Box::new(e) as Box<dyn Error>)?;
    app.manage(Arc::new(AppState {
        core: Arc::new(core),
        videos,
    }));
    app.manage(crate::update_state::UpdateState::default());
    crate::commands::update::anunciar_aplicada(app.handle());
    crate::commands::update::agendar_auto(app.handle());
    Ok(())
}

/// `RunEvent::Exit`: com o `AppState` gerenciado, `block_on(core.stop_share())` e
/// `block_on(core.leave_room())` (`tauri::async_runtime::block_on`). Sem `AppState`: não faz nada.
pub fn on_exit(app: &AppHandle) {
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        // Shutdown exige &mut, então só encerra compartilhamento e sala.
        tauri::async_runtime::block_on(async {
            st.core.stop_share().await;
            st.core.leave_room().await;
        });
    }
}
