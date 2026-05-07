mod commands;
mod database;
mod desktop_binding;
mod gateway;
mod models;
mod settings;
mod state;
mod tray;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = state::AppState::init().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .setup(|app| {
            tray::create(app.handle())?;

            let st = app.state::<state::AppState>();
            let settings = settings::load(&st.settings_path).unwrap_or_default();

            if settings.auto_start_gateway {
                let st_clone = st.inner().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let _ = gateway::start(&st_clone);
                });
            }

            if settings.auto_takeover_desktop {
                if let Ok(profile) = database::get_profile(&st.db_path) {
                    if let Ok(routes) = database::list_routes(&st.db_path) {
                        let models: Vec<String> = routes.into_iter()
                            .filter(|r| r.enabled).map(|r| r.claude_alias).collect();
                        let _ = desktop_binding::apply(
                            &dirs::home_dir().unwrap_or_default(),
                            &format!("http://{}:{}/v1/messages", profile.listen_host, profile.listen_port),
                            "x-api-key", &profile.auth_token, &models,
                        );
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::save_settings,
            commands::get_profile,
            commands::list_providers,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::list_routes,
            commands::create_route,
            commands::update_route,
            commands::delete_route,
            commands::start_gateway,
            commands::stop_gateway,
            commands::list_logs,
            commands::get_desktop_info,
            commands::apply_binding,
            commands::restore_binding,
            commands::check_gateway_health,
            commands::check_provider_health,
            commands::export_config,
            commands::import_config,
        ])
        .run(tauri::generate_context!())
        .expect("error running app");
}
