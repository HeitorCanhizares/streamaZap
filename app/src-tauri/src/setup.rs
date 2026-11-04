//! Inicialização e saída do app.

use std::error::Error;

use tauri::{App, AppHandle};

/// Inicia o log (`tracing_subscriber` com `EnvFilter`, padrão `info`), resolve o diretório de
/// dados (`app.path().app_data_dir()`, criando-o), cria `VideoChannels` + `TauriSink`
/// (`crate::sink`), inicia `zap_core::Core::start(dir, sink)` e registra
/// `AppState { core: Arc<Core>, videos }` com `app.manage`. Erro do núcleo aborta o `setup`
/// com a mensagem do `CoreError`.
pub fn init(app: &mut App) -> Result<(), Box<dyn Error>> {
    let _ = app;
    todo!("tarefa tauri-05")
}

/// `RunEvent::Exit`: com o `AppState` gerenciado, `block_on(core.stop_share())` e
/// `block_on(core.leave_room())` (`tauri::async_runtime::block_on`). Sem `AppState`: não faz nada.
pub fn on_exit(app: &AppHandle) {
    let _ = app;
    todo!("tarefa tauri-05")
}
