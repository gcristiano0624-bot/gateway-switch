use std::{fs, time::Instant};
use tauri::State;
use crate::{
    codex_binding, codex_gateway, database, desktop_binding, gateway, models::*, settings,
    state::{AppState, GatewayStatus},
};

#[tauri::command]
pub fn get_status(st: State<'_, AppState>) -> Result<AppStatus, String> {
    let gw = gateway::status(&st)?;
    let info = desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let profile = database::get_profile(&st.db_path)?;
    Ok(AppStatus {
        gateway_running: gw.running,
        gateway_port: profile.listen_port,
        binding_active: info.managed,
        provider_count: providers.len(),
        route_count: routes.len(),
    })
}

#[tauri::command]
pub fn get_settings(st: State<'_, AppState>) -> Result<AppSettings, String> {
    settings::load(&st.settings_path)
}

#[tauri::command]
pub fn save_settings(st: State<'_, AppState>, payload: AppSettings) -> Result<AppSettings, String> {
    settings::save(&st.settings_path, &payload)?;
    database::save_profile(&st.db_path, &GatewayProfile {
        listen_host: payload.listen_host.clone(),
        listen_port: payload.listen_port,
        auth_token: payload.auth_token.clone(),
    })?;
    settings::load(&st.settings_path)
}

#[tauri::command]
pub fn get_profile(st: State<'_, AppState>) -> Result<GatewayProfile, String> {
    database::get_profile(&st.db_path)
}

#[tauri::command]
pub fn list_providers(st: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    database::list_providers(&st.db_path)
}

#[tauri::command]
pub fn create_provider(st: State<'_, AppState>, payload: CreateProvider) -> Result<Vec<Provider>, String> {
    database::create_provider(&st.db_path, &payload)?;
    database::list_providers(&st.db_path)
}

#[tauri::command]
pub fn update_provider(st: State<'_, AppState>, payload: UpdateProvider) -> Result<Vec<Provider>, String> {
    database::update_provider(&st.db_path, &payload)?;
    database::list_providers(&st.db_path)
}

#[tauri::command]
pub fn delete_provider(st: State<'_, AppState>, id: String) -> Result<Vec<Provider>, String> {
    database::delete_provider(&st.db_path, &id)?;
    database::list_providers(&st.db_path)
}

#[tauri::command]
pub fn list_routes(st: State<'_, AppState>) -> Result<Vec<ModelRoute>, String> {
    database::list_routes(&st.db_path)
}

#[tauri::command]
pub fn create_route(st: State<'_, AppState>, payload: CreateModelRoute) -> Result<Vec<ModelRoute>, String> {
    database::create_route(&st.db_path, &payload)?;
    database::list_routes(&st.db_path)
}

#[tauri::command]
pub fn update_route(st: State<'_, AppState>, payload: UpdateModelRoute) -> Result<Vec<ModelRoute>, String> {
    database::update_route(&st.db_path, &payload)?;
    database::list_routes(&st.db_path)
}

#[tauri::command]
pub fn delete_route(st: State<'_, AppState>, id: String) -> Result<Vec<ModelRoute>, String> {
    database::delete_route(&st.db_path, &id)?;
    database::list_routes(&st.db_path)
}

#[tauri::command]
pub async fn start_gateway(st: State<'_, AppState>) -> Result<String, String> {
    gateway::start(&st)
}

#[tauri::command]
pub async fn stop_gateway(st: State<'_, AppState>) -> Result<String, String> {
    gateway::stop(&st)
}

#[tauri::command]
pub fn list_logs(st: State<'_, AppState>) -> Result<Vec<RequestLog>, String> {
    database::list_logs(&st.db_path, 200)
}

#[tauri::command]
pub fn get_desktop_info(st: State<'_, AppState>) -> Result<desktop_binding::DesktopInfo, String> {
    let _ = st;
    desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub fn apply_binding(st: State<'_, AppState>) -> Result<desktop_binding::DesktopInfo, String> {
    let profile = database::get_profile(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let models: Vec<String> = routes.into_iter().filter(|r| r.enabled).map(|r| r.claude_alias).collect();
    desktop_binding::apply(
        &dirs::home_dir().ok_or("no home")?,
        &desktop_binding::gateway_base_url(&profile.listen_host, profile.listen_port),
        "x-api-key", &profile.auth_token, &models,
    )
}

#[tauri::command]
pub fn restore_binding(st: State<'_, AppState>) -> Result<desktop_binding::DesktopInfo, String> {
    let _ = st;
    desktop_binding::restore(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub async fn check_gateway_health(st: State<'_, AppState>) -> Result<HealthStatus, String> {
    let profile = database::get_profile(&st.db_path)?;
    let url = format!("http://{}:{}/health", profile.listen_host, profile.listen_port);
    let start = Instant::now();
    match reqwest::get(&url).await {
        Ok(r) => Ok(HealthStatus {
            target: "gateway".into(),
            ok: r.status().is_success(),
            message: format!("HTTP {}", r.status()),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
        Err(e) => Ok(HealthStatus {
            target: "gateway".into(),
            ok: false,
            message: e.to_string(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
    }
}

#[tauri::command]
pub async fn check_provider_health(st: State<'_, AppState>, id: String) -> Result<HealthStatus, String> {
    let providers = database::list_providers(&st.db_path)?;
    let p = providers.into_iter().find(|p| p.id == id).ok_or("Provider not found")?;
    let client = reqwest::Client::new();
    let mut req = client.get(upstream_url(&p.base_url, "models"));
    req = req.header("content-type", "application/json");
    if let Some(key) = p.api_key.as_deref().filter(|s| !s.is_empty()) {
        let mut val = key.to_string();
        if let Some(scheme) = p.auth_scheme.as_deref().filter(|s| !s.is_empty()) {
            val = format!("{scheme} {val}");
        }
        req = req.header(&*p.auth_header, val);
    }
    let start = Instant::now();
    match req.send().await {
        Ok(r) => Ok(HealthStatus {
            target: id,
            ok: r.status().is_success(),
            message: format!("HTTP {}", r.status()),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
        Err(e) => Ok(HealthStatus {
            target: id,
            ok: false,
            message: e.to_string(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
    }
}

#[tauri::command]
pub fn export_config(st: State<'_, AppState>) -> Result<String, String> {
    let settings = settings::load(&st.settings_path)?;
    let profile = database::get_profile(&st.db_path)?;
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let bundle = serde_json::json!({ "settings": settings, "profile": profile, "providers": providers, "routes": routes });
    let path = st.backups_dir.join(format!("export-{}.json", chrono::Utc::now().timestamp_millis()));
    fs::write(&path, serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn import_config(st: State<'_, AppState>, file_path: String) -> Result<String, String> {
    let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    let bundle: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    if let Some(providers) = bundle.get("providers").and_then(|v| v.as_array()) {
        for p in providers {
            if let (Some(id), Some(name), Some(url)) = (
                p.get("id").and_then(|v| v.as_str()),
                p.get("name").and_then(|v| v.as_str()),
                p.get("base_url").and_then(|v| v.as_str()),
            ) {
                let existing = database::list_providers(&st.db_path).unwrap_or_default();
                if existing.iter().any(|e| e.id == id) {
                    let _ = database::update_provider(&st.db_path, &UpdateProvider {
                        id: id.into(), name: name.into(), base_url: url.into(),
                        auth_header: p.get("auth_header").and_then(|v| v.as_str()).unwrap_or("x-api-key").into(),
                        auth_scheme: p.get("auth_scheme").and_then(|v| v.as_str()).map(Into::into),
                        api_key: p.get("api_key").and_then(|v| v.as_str()).map(Into::into),
                        enabled: p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    });
                } else {
                    let _ = database::create_provider(&st.db_path, &CreateProvider {
                        id: id.into(), name: name.into(), base_url: url.into(),
                        auth_header: p.get("auth_header").and_then(|v| v.as_str()).unwrap_or("x-api-key").into(),
                        auth_scheme: p.get("auth_scheme").and_then(|v| v.as_str()).map(Into::into),
                        api_key: p.get("api_key").and_then(|v| v.as_str()).map(Into::into),
                    });
                }
            }
        }
    }

    if let Some(routes) = bundle.get("routes").and_then(|v| v.as_array()) {
        for r in routes {
            if let (Some(id), Some(alias), Some(display), Some(pid), Some(upstream)) = (
                r.get("id").and_then(|v| v.as_str()),
                r.get("claude_alias").and_then(|v| v.as_str()),
                r.get("display_name").and_then(|v| v.as_str()),
                r.get("provider_id").and_then(|v| v.as_str()),
                r.get("upstream_model").and_then(|v| v.as_str()),
            ) {
                let existing = database::list_routes(&st.db_path).unwrap_or_default();
                if existing.iter().any(|e| e.id == id) {
                    let _ = database::update_route(&st.db_path, &UpdateModelRoute {
                        id: id.into(), claude_alias: alias.into(), display_name: display.into(),
                        provider_id: pid.into(), upstream_model: upstream.into(),
                        enabled: r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    });
                } else {
                    let _ = database::create_route(&st.db_path, &CreateModelRoute {
                        id: id.into(), claude_alias: alias.into(), display_name: display.into(),
                        provider_id: pid.into(), upstream_model: upstream.into(),
                    });
                }
            }
        }
    }

    Ok(format!("Imported from {}", file_path))
}

// =====================================================
//  CODEX GATEWAY COMMANDS
// =====================================================

#[tauri::command]
pub async fn start_codex_gateway(st: State<'_, AppState>) -> Result<String, String> {
    codex_gateway::start(&st)
}

#[tauri::command]
pub async fn stop_codex_gateway(st: State<'_, AppState>) -> Result<String, String> {
    codex_gateway::stop(&st)
}

#[tauri::command]
pub fn get_codex_status(st: State<'_, AppState>) -> Result<GatewayStatus, String> {
    codex_gateway::status(&st)
}

#[tauri::command]
pub fn get_codex_profile(st: State<'_, AppState>) -> Result<GatewayProfile, String> {
    database::get_codex_profile(&st.db_path)
}

#[tauri::command]
pub fn save_codex_profile(st: State<'_, AppState>, payload: GatewayProfile) -> Result<GatewayProfile, String> {
    database::save_codex_profile(&st.db_path, &payload)?;
    database::get_codex_profile(&st.db_path)
}

#[tauri::command]
pub fn list_codex_routes(st: State<'_, AppState>) -> Result<Vec<CodexRoute>, String> {
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub fn create_codex_route(st: State<'_, AppState>, payload: CreateCodexRoute) -> Result<Vec<CodexRoute>, String> {
    database::create_codex_route(&st.db_path, &payload)?;
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub fn update_codex_route(st: State<'_, AppState>, payload: UpdateCodexRoute) -> Result<Vec<CodexRoute>, String> {
    database::update_codex_route(&st.db_path, &payload)?;
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub fn delete_codex_route(st: State<'_, AppState>, id: String) -> Result<Vec<CodexRoute>, String> {
    database::delete_codex_route(&st.db_path, &id)?;
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub async fn check_codex_health(st: State<'_, AppState>) -> Result<HealthStatus, String> {
    let profile = database::get_codex_profile(&st.db_path)?;
    let url = format!("http://{}:{}/health", profile.listen_host, profile.listen_port);
    let start = Instant::now();
    match reqwest::get(&url).await {
        Ok(r) => Ok(HealthStatus {
            target: "codex-gateway".into(),
            ok: r.status().is_success(),
            message: format!("HTTP {}", r.status()),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
        Err(e) => Ok(HealthStatus {
            target: "codex-gateway".into(),
            ok: false,
            message: e.to_string(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }),
    }
}

#[tauri::command]
pub fn get_codex_binding_info(st: State<'_, AppState>) -> Result<CodexBindingInfo, String> {
    let _ = st;
    codex_binding::inspect(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub fn apply_codex_binding(st: State<'_, AppState>, model: String) -> Result<CodexBindingInfo, String> {
    let profile = database::get_codex_profile(&st.db_path)?;
    codex_binding::apply(
        &dirs::home_dir().ok_or("no home")?,
        &format!("http://{}:{}/v1", profile.listen_host, profile.listen_port),
        &profile.auth_token,
        &model,
    )
}

#[tauri::command]
pub fn restore_codex_binding(st: State<'_, AppState>) -> Result<CodexBindingInfo, String> {
    let _ = st;
    codex_binding::restore(&dirs::home_dir().ok_or("no home")?)
}

// =====================================================
//  MODEL ALIASES COMMANDS
// =====================================================

#[tauri::command]
pub fn list_model_aliases(st: State<'_, AppState>, alias_type: String) -> Result<Vec<ModelAlias>, String> {
    database::list_model_aliases(&st.db_path, &alias_type)
}

#[tauri::command]
pub fn create_model_alias(st: State<'_, AppState>, payload: CreateModelAlias) -> Result<Vec<ModelAlias>, String> {
    database::create_model_alias(&st.db_path, &payload)?;
    database::list_model_aliases(&st.db_path, &payload.alias_type)
}

#[tauri::command]
pub fn delete_model_alias(st: State<'_, AppState>, id: String, alias_type: String) -> Result<Vec<ModelAlias>, String> {
    database::delete_model_alias(&st.db_path, &id)?;
    database::list_model_aliases(&st.db_path, &alias_type)
}

fn upstream_url(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{endpoint}")
    } else {
        format!("{base}/v1/{endpoint}")
    }
}
