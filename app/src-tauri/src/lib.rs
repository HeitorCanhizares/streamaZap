//! App Tauri do StreamaZap: liga o `zap-core` à interface (comandos, eventos e canais de vídeo).

mod commands;
#[cfg(test)]
mod conf_tests;
pub mod convert;
pub mod fwfix;
mod setup;
pub mod sink;
pub mod state;
pub mod update_agenda;
pub mod update_aplicada;
pub mod update_auto;
pub mod update_contexto;
pub mod update_instalar;
pub mod update_state;
pub mod updater;
pub mod video;

/// Sobe o app: plugins, estado (`Core` + `TauriSink`), comandos e janela.
pub fn run() {
    // Modo elevado `--corrigir-firewall`: sem janela, sem Tauri.
    let args: Vec<String> = std::env::args().collect();
    if fwfix::pedido(&args) {
        std::process::exit(fwfix::executar());
    }
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            commands::window::focus_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| setup::init(app))
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::snapshot_get,
            commands::diag::diagnostics_run,
            commands::diag::diagnostics_text,
            commands::diag::firewall_fix,
            commands::update::update_check,
            commands::update::update_download,
            commands::update::update_install_now,
            commands::update::update_install_on_exit,
            commands::update::update_postpone,
            commands::update::update_applied_take,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::sources::sources_list,
            commands::sources::source_thumbnail,
            commands::online::server_test,
            commands::online::room_preview,
            commands::online::room_preview_stop,
            commands::room::room_create,
            commands::room::room_join,
            commands::room::room_leave,
            commands::room::chat_send,
            commands::room::chat_typing,
            commands::room::music_add,
            commands::room::music_remove,
            commands::room::music_vote,
            commands::room::music_report,
            commands::share::share_start,
            commands::share::share_update,
            commands::share::share_stop,
            commands::share::camera_set,
            commands::share::camera_quality_set,
            commands::share::camera_layout_set,
            commands::share::camera_preview,
            commands::share::camera_preview_stop,
            commands::watch::watch,
            commands::watch::unwatch,
            commands::watch::set_volume,
            commands::watch::set_master_volume,
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
                commands::update::instalar_ao_sair(handle);
                setup::on_exit(handle);
            }
        }),
        Err(e) => {
            tracing::error!("erro ao montar o app streamaZap: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    /// Extrai os nomes de comando chamados pelo `api.ts`: `call('x')`, `call<T>('x')`, `invoke<T>('x')`.
    fn comandos_do_api(texto: &str) -> Vec<String> {
        let mut out = Vec::new();
        for chave in ["call", "invoke"] {
            let mut inicio = 0;
            while let Some(p) = texto[inicio..].find(chave) {
                let pos = inicio + p;
                inicio = pos + chave.len();
                // evita casar pedaço de outra palavra (ex.: `recall`)
                let antes_ok = texto[..pos]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                if !antes_ok {
                    continue;
                }
                let mut s = &texto[inicio..];
                if let Some(r) = s.strip_prefix('<') {
                    match r.find('>') {
                        Some(i) => s = &r[i + 1..],
                        None => continue,
                    }
                }
                let Some(r) = s.strip_prefix('(').and_then(|r| r.strip_prefix('\'')) else {
                    continue;
                };
                if let Some(fim) = r.find('\'') {
                    out.push(r[..fim].to_string());
                }
            }
        }
        out
    }

    #[test]
    fn comandos_do_api_ts_existem() {
        const API: &str = include_str!("../../src/bridge/api.ts");
        const LIB: &str = include_str!("lib.rs");
        let nomes = comandos_do_api(API);
        assert!(nomes.len() >= 17, "poucos comandos extraídos do api.ts: {nomes:?}");
        assert!(nomes.iter().any(|n| n == "source_thumbnail"));
        for nome in &nomes {
            let sufixo = format!("::{nome},");
            let registrado = LIB
                .lines()
                .map(str::trim)
                .any(|l| l.starts_with("commands::") && l.ends_with(&sufixo));
            assert!(
                registrado,
                "comando `{nome}` do api.ts sem `commands::<mod>::{nome}` no generate_handler!"
            );
        }
    }
}
