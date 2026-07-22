use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn get_gateway_status(st: &crate::state::AppState) -> String {
    match crate::gateway::status(st) {
        Ok(s) if s.running => "Running".to_string(),
        _ => "Stopped".to_string(),
    }
}

fn get_codex_status(st: &crate::state::AppState) -> String {
    match crate::codex_gateway::status(st) {
        Ok(s) if s.running => "Running".to_string(),
        _ => "Stopped".to_string(),
    }
}

fn get_desktop_binding_status() -> String {
    match crate::desktop_binding::inspect(&dirs::home_dir().unwrap_or_default()) {
        Ok(info) if info.managed => "Managed".to_string(),
        _ => "Unmanaged".to_string(),
    }
}

fn get_codex_binding_status() -> String {
    match crate::codex_binding::inspect(&dirs::home_dir().unwrap_or_default()) {
        Ok(info) if info.managed => "Managed".to_string(),
        _ => "Unmanaged".to_string(),
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let st = app.state::<crate::state::AppState>();
    let gw_status = get_gateway_status(&st);
    let codex_status = get_codex_status(&st);
    let desktop_status = get_desktop_binding_status();
    let codex_bind_status = get_codex_binding_status();

    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let start_claude_item = MenuItem::with_id(app, "start_claude", format!("Start Claude Gateway ({})", gw_status), true, None::<&str>)?;
    let stop_claude_item = MenuItem::with_id(app, "stop_claude", "Stop Claude Gateway", true, None::<&str>)?;
    let bind_claude_item = MenuItem::with_id(app, "bind_claude", format!("Bind Claude Desktop ({})", desktop_status), true, None::<&str>)?;
    let restore_claude_item = MenuItem::with_id(app, "restore_claude", "Restore Claude Desktop", true, None::<&str>)?;

    let sep2 = PredefinedMenuItem::separator(app)?;

    let start_codex_item = MenuItem::with_id(app, "start_codex", format!("Start Codex Gateway ({})", codex_status), true, None::<&str>)?;
    let stop_codex_item = MenuItem::with_id(app, "stop_codex", "Stop Codex Gateway", true, None::<&str>)?;
    let bind_codex_item = MenuItem::with_id(app, "bind_codex", format!("Bind Codex App ({})", codex_bind_status), true, None::<&str>)?;
    let restore_codex_item = MenuItem::with_id(app, "restore_codex", "Restore Codex App", true, None::<&str>)?;

    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item, &sep1,
            &start_claude_item, &stop_claude_item, &bind_claude_item, &restore_claude_item,
            &sep2,
            &start_codex_item, &stop_codex_item, &bind_codex_item, &restore_codex_item,
            &sep3, &quit_item,
        ],
    )?;

    let icon = app.default_window_icon().cloned().ok_or_else(|| std::io::Error::other("no icon"))?;

    let _ = TrayIconBuilder::with_id("gateway-switch-tray")
        .icon(icon)
        .tooltip(&format!("Gateway Switch\nClaude: {}\nCodex: {}", gw_status, codex_status))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show(app),
            "start_claude" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::gateway::start(&st);
            }
            "stop_claude" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::gateway::stop(&st);
            }
            "bind_claude" => {
                let st = app.state::<crate::state::AppState>();
                if let Ok(p) = crate::database::get_profile(&st.db_path) {
                    if let Ok(routes) = crate::database::list_routes(&st.db_path) {
                        let models = crate::desktop_binding::model_configs_from_routes(&routes);
                        let _ = crate::desktop_binding::apply(
                            &dirs::home_dir().unwrap_or_default(),
                            &crate::desktop_binding::gateway_base_url(&p.listen_host, p.listen_port),
                            "x-api-key", &p.auth_token, &models,
                        );
                    }
                }
            }
            "restore_claude" => {
                let _ = crate::desktop_binding::restore(&dirs::home_dir().unwrap_or_default());
            }
            "start_codex" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::codex_gateway::start(&st);
            }
            "stop_codex" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::codex_gateway::stop(&st);
            }
            "bind_codex" => {
                let st = app.state::<crate::state::AppState>();
                if let Ok(p) = crate::database::get_codex_profile(&st.db_path) {
                    let _ = crate::codex_binding::apply(
                        &dirs::home_dir().unwrap_or_default(),
                        &format!("http://{}:{}/v1", p.listen_host, p.listen_port),
                        &p.auth_token, "claude-3.5-sonnet",
                    );
                }
            }
            "restore_codex" => {
                let _ = crate::codex_binding::restore(&dirs::home_dir().unwrap_or_default());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
