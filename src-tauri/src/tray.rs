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

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start", "Start Claude Gateway", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Claude Gateway", true, None::<&str>)?;
    let bind_item = MenuItem::with_id(app, "bind", "Bind Claude Desktop", true, None::<&str>)?;
    let restore_item = MenuItem::with_id(app, "restore", "Restore Desktop", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &start_item, &stop_item, &bind_item, &restore_item, &sep, &quit_item])?;
    let icon = app.default_window_icon().cloned()
        .ok_or_else(|| std::io::Error::other("no icon"))?;

    let _ = TrayIconBuilder::with_id("gateway-switch-tray")
        .icon(icon)
        .tooltip("Gateway Switch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show(app),
            "start" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::gateway::start(&st);
            }
            "stop" => {
                let st = app.state::<crate::state::AppState>();
                let _ = crate::gateway::stop(&st);
            }
            "bind" => {
                let st = app.state::<crate::state::AppState>();
                if let Ok(p) = crate::database::get_profile(&st.db_path) {
                    if let Ok(routes) = crate::database::list_routes(&st.db_path) {
                        let models: Vec<String> = routes.into_iter()
                            .filter(|r| r.enabled).map(|r| r.claude_alias).collect();
                        let _ = crate::desktop_binding::apply(
                            &dirs::home_dir().unwrap_or_default(),
                            &format!("http://{}:{}/v1/messages", p.listen_host, p.listen_port),
                            "x-api-key", &p.auth_token, &models,
                        );
                    }
                }
            }
            "restore" => {
                let _ = crate::desktop_binding::restore(&dirs::home_dir().unwrap_or_default());
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
