mod claude_code_binding;
mod codex_binding;
mod codex_gateway;
mod codex_pp;
mod coldstart;
mod commands;
mod compatibility;
mod database;
mod desktop_binding;
mod gateway;
mod gateway_diagnostics;
mod gateway_protocol;
mod gateway_strategy;
mod loop_guard;
mod mcp_sync;
mod models;
mod settings;
mod state;
mod tray;

use tauri::Manager;

pub fn try_run_cli_from_args(args: &[String]) -> Result<Option<i32>, String> {
    if args.get(1).map(String::as_str) != Some("codexpp") {
        return Ok(None);
    }
    let code = codex_pp::run_headless_cli(&args[2..])?;
    Ok(Some(code))
}

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
                        let models = desktop_binding::model_configs_from_routes(&routes);
                        let _ = desktop_binding::apply(
                            &dirs::home_dir().unwrap_or_default(),
                            &desktop_binding::gateway_base_url(
                                &profile.listen_host,
                                profile.listen_port,
                            ),
                            "x-api-key",
                            &profile.auth_token,
                            &models,
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
            commands::list_provider_capabilities,
            commands::get_route_diagnostics,
            commands::preview_route_payload,
            commands::list_provider_policies,
            commands::upsert_provider_policy,
            commands::reset_provider_policy,
            commands::list_failed_request_diagnostics,
            commands::replay_request_diagnostic,
            commands::get_codex_route_diagnostics,
            commands::get_runtime_source_report,
            commands::check_app_update,
            commands::get_safe_install_plan,
            commands::reveal_safe_install_locations,
            commands::get_runtime_dashboard,
            commands::get_app_workbench,
            commands::get_provider_console,
            commands::get_usage_insights,
            commands::preview_route_builder,
            commands::apply_route_builder,
            commands::preview_provider_wizard,
            commands::apply_provider_wizard,
            commands::get_unified_diagnostics,
            commands::export_unified_diagnostics_bundle,
            commands::list_provider_presets,
            commands::apply_provider_preset,
            commands::get_runtime_feature_report,
            commands::run_compatibility_benchmark,
            commands::validate_patch_payload,
            commands::check_command_safety,
            commands::check_mcp_path_safety,
            commands::detect_fake_action_text,
            commands::compress_context_payload,
            commands::recover_agent_state_payload,
            commands::export_diagnostics,
            commands::get_mcp_sync_status,
            commands::preview_mcp_sync,
            commands::run_mcp_sync,
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
            commands::get_claude_code_info,
            commands::apply_claude_code_binding,
            commands::restore_claude_code_binding,
            commands::repair_claude_code_gateway_binding,
            commands::check_gateway_health,
            commands::check_provider_health,
            commands::export_config,
            commands::import_config,
            commands::start_codex_gateway,
            commands::stop_codex_gateway,
            commands::get_codex_status,
            commands::get_codex_profile,
            commands::save_codex_profile,
            commands::list_codex_routes,
            commands::create_codex_route,
            commands::update_codex_route,
            commands::delete_codex_route,
            commands::check_codex_health,
            commands::get_codex_binding_info,
            commands::apply_codex_binding,
            commands::restore_codex_binding,
            commands::detect_codex_pp,
            commands::list_codex_pp_tweaks,
            commands::set_codex_pp_tweak_enabled,
            commands::fetch_codex_pp_store,
            commands::install_codex_pp_tweak,
            commands::uninstall_codex_pp_tweak,
            commands::get_codex_pp_health,
            commands::get_codex_pp_preflight,
            commands::get_codex_pp_recommended_scripts,
            commands::install_codex_pp_recommended_scripts,
            commands::run_codex_pp_cli,
            commands::open_codex_pp_path,
            commands::list_model_aliases,
            commands::create_model_alias,
            commands::delete_model_alias,
            commands::get_coldstart_status,
            commands::run_coldstart_repair,
        ])
        .run(tauri::generate_context!())
        .expect("error running app");
}
