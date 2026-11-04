//! App Tauri do StreamaZap: liga o `zap-core` à interface (comandos, eventos e canais de vídeo).

// ESQUELETO: removido na tarefa tauri-13 (fechamento).
#![allow(unused_variables, dead_code)]

mod commands;
pub mod convert;
mod setup;
pub mod sink;
pub mod state;
pub mod video;

/// Sobe o app: plugins, estado (`Core` + `TauriSink`), comandos e janela.
pub fn run() {
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            commands::window::focus_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| setup::init(app))
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::diagnostics_run,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::sources::sources_list,
            commands::sources::source_thumbnail,
            commands::room::room_create,
            commands::room::room_join,
            commands::room::room_leave,
            commands::room::chat_send,
            commands::room::chat_typing,
            commands::share::share_start,
            commands::share::share_update,
            commands::share::share_stop,
            commands::share::camera_set,
            commands::watch::watch,
            commands::watch::unwatch,
            commands::watch::set_volume,
            commands::watch::set_smoothing,
            commands::window::window_minimize,
            commands::window::window_toggle_maximize,
            commands::window::window_close,
            commands::window::window_set_fullscreen,
        ])
        .build(tauri::generate_context!());
    match built {
        Ok(app) => app.run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                setup::on_exit(handle);
            }
        }),
        Err(e) => {
            tracing::error!("erro ao montar o app streamaZap: {e}");
            std::process::exit(1);
        }
    }
}
