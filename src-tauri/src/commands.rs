use std::{fs, time::Instant};
use tauri::State;
use crate::{
    claude_code_binding, codex_binding, codex_gateway, compatibility, database, desktop_binding, gateway, models::*, settings,
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
pub fn list_provider_capabilities(st: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    Ok(database::list_providers(&st.db_path)?
        .iter()
        .map(compatibility::provider_capability_json)
        .collect())
}

#[tauri::command]
pub fn get_runtime_feature_report() -> compatibility::RuntimeFeatureReport {
    compatibility::runtime_feature_report()
}

#[tauri::command]
pub fn run_compatibility_benchmark(st: State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    Ok(database::list_providers(&st.db_path)?
        .iter()
        .map(|p| serde_json::json!({
            "provider": p.id,
            "anthropic": compatibility::benchmark_provider(p),
            "codex": compatibility::codex_capability_profile(p)
        }))
        .collect())
}

#[tauri::command]
pub fn validate_patch_payload(patch: String) -> Result<compatibility::PatchValidationResult, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(compatibility::validate_patch(&patch, &cwd))
}

#[tauri::command]
pub fn check_command_safety(command: String) -> compatibility::SafetyDecision {
    compatibility::command_safety(&command)
}

#[tauri::command]
pub fn check_mcp_path_safety(path: String) -> Result<compatibility::SafetyDecision, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(compatibility::mcp_path_safety(&path, &cwd))
}

#[tauri::command]
pub fn detect_fake_action_text(text: String) -> serde_json::Value {
    serde_json::json!({
        "fake_tool_call": compatibility::detect_fake_tool_call(&text),
        "fake_action": compatibility::detect_fake_action(&text)
    })
}

#[tauri::command]
pub fn compress_context_payload(messages: Vec<serde_json::Value>, max_items: usize) -> serde_json::Value {
    compatibility::compress_context(&messages, max_items)
}

#[tauri::command]
pub fn recover_agent_state_payload(history: Vec<serde_json::Value>) -> compatibility::AgentTaskState {
    compatibility::recover_agent_state(&history)
}

#[tauri::command]
pub fn export_diagnostics(st: State<'_, AppState>) -> Result<String, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 500)?;
    let bundle = serde_json::json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "runtime_features": compatibility::runtime_feature_report(),
        "provider_capabilities": providers.iter().map(compatibility::provider_capability_json).collect::<Vec<_>>(),
        "benchmarks": providers.iter().map(|p| serde_json::json!({"provider": p.id, "benchmark": compatibility::benchmark_provider(p)})).collect::<Vec<_>>(),
        "providers": providers,
        "routes": routes,
        "codex_routes": codex_routes,
        "logs": logs
    });
    let path = st.backups_dir.join(format!("diagnostics-{}.json", chrono::Utc::now().timestamp_millis()));
    fs::write(&path, serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn get_coldstart_status(st: State<'_, AppState>) -> Result<ColdStartReport, String> {
    run_coldstart_checks(&st, false).await
}

#[tauri::command]
pub async fn run_coldstart_repair(st: State<'_, AppState>) -> Result<ColdStartReport, String> {
    run_coldstart_checks(&st, true).await
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
    let models = desktop_binding::model_configs_from_routes(&routes);
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
pub fn get_claude_code_info(st: State<'_, AppState>) -> Result<ClaudeCodeInfo, String> {
    let _ = st;
    claude_code_binding::inspect(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub async fn apply_claude_code_binding(st: State<'_, AppState>, payload: ClaudeCodeBindPayload) -> Result<ClaudeCodeInfo, String> {
    match payload.mode.as_str() {
        "gateway" => {
            let profile = database::get_profile(&st.db_path)?;
            let _ = gateway::start(&st);
            claude_code_binding::apply_gateway(
                &dirs::home_dir().ok_or("no home")?,
                &desktop_binding::gateway_base_url(&profile.listen_host, profile.listen_port),
                &profile.auth_token,
                &payload.model,
            )
        }
        "provider" => {
            let provider_id = payload.provider_id.as_deref().ok_or("Choose a provider")?;
            let provider = database::list_providers(&st.db_path)?
                .into_iter()
                .find(|p| p.id == provider_id)
                .ok_or_else(|| format!("Provider '{provider_id}' not found"))?;
            claude_code_binding::apply_provider(
                &dirs::home_dir().ok_or("no home")?,
                provider.anthropic_base_url.as_deref().ok_or("This provider does not have an Anthropic Base URL for Claude Code Direct Provider mode")?,
                &provider.auth_header,
                provider.auth_scheme.as_deref(),
                provider.api_key.as_deref().unwrap_or_default(),
                payload.upstream_model.as_deref().unwrap_or(&payload.model),
            )
        }
        _ => Err("Unknown Claude Code binding mode".into()),
    }
}

#[tauri::command]
pub fn restore_claude_code_binding(st: State<'_, AppState>) -> Result<ClaudeCodeInfo, String> {
    let _ = st;
    claude_code_binding::restore(&dirs::home_dir().ok_or("no home")?)
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
    let mut req = client.get(upstream_url(&p.openai_base_url, "models"));
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
                        openai_base_url: p.get("openai_base_url").and_then(|v| v.as_str()).map(Into::into),
                        anthropic_base_url: p.get("anthropic_base_url").and_then(|v| v.as_str()).map(Into::into),
                        auth_header: p.get("auth_header").and_then(|v| v.as_str()).unwrap_or("x-api-key").into(),
                        auth_scheme: p.get("auth_scheme").and_then(|v| v.as_str()).map(Into::into),
                        api_key: p.get("api_key").and_then(|v| v.as_str()).map(Into::into),
                        enabled: p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    });
                } else {
                    let _ = database::create_provider(&st.db_path, &CreateProvider {
                        id: id.into(), name: name.into(), base_url: url.into(),
                        openai_base_url: p.get("openai_base_url").and_then(|v| v.as_str()).map(Into::into),
                        anthropic_base_url: p.get("anthropic_base_url").and_then(|v| v.as_str()).map(Into::into),
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

async fn run_coldstart_checks(st: &AppState, apply_fixes: bool) -> Result<ColdStartReport, String> {
    let mode = if apply_fixes { "repair" } else { "check" }.to_string();
    let mut steps: Vec<ColdStartStep> = Vec::new();
    let mut capabilities: Vec<ColdStartCapability> = Vec::new();
    let mut auto_fixes_applied: Vec<String> = Vec::new();
    let mut manual_fixes_required: Vec<String> = Vec::new();

    cold_log(&mut steps, "environment", "Environment discovery", "system", "ok", "Loaded local app state, settings path, database path, and binding targets");

    let settings = settings::load(&st.settings_path)?;
    let profile = database::get_profile(&st.db_path)?;
    let codex_profile = database::get_codex_profile(&st.db_path)?;
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let enabled_routes: Vec<ModelRoute> = routes.iter().filter(|r| r.enabled).cloned().collect();
    let enabled_codex_routes: Vec<CodexRoute> = codex_routes.iter().filter(|r| r.enabled).cloned().collect();
    cold_log(
        &mut steps,
        "inventory",
        "Provider and route inventory",
        "gateway",
        "ok",
        &format!("{} providers, {} Claude routes, {} Codex routes", providers.len(), enabled_routes.len(), enabled_codex_routes.len()),
    );

    let home = dirs::home_dir().ok_or("no home")?;
    let mut desktop = desktop_binding::inspect(&home)?;
    let claude_code = claude_code_binding::inspect(&home)?;
    let mut codex_info = codex_binding::inspect(&home)?;

    capability(&mut capabilities, "Claude Desktop config", "Claude", desktop_status(desktop.managed), &binding_detail(&desktop.config_path, desktop.managed, desktop.base_url.as_deref()));
    capability(&mut capabilities, "Claude Code config", "Claude Code", managed_status(claude_code.managed), &binding_detail(&claude_code.config_path, claude_code.managed, claude_code.base_url.as_deref()));
    capability(&mut capabilities, "Codex config", "Codex", managed_status(codex_info.managed), &binding_detail(&codex_info.config_path, codex_info.managed, codex_info.base_url.as_deref()));

    let mut claude_gateway_status = gateway::status(st)?;
    if !claude_gateway_status.running && apply_fixes && !enabled_routes.is_empty() {
        cold_log(&mut steps, "claude_gateway_start", "Start Claude Gateway", "Claude", "running", "Gateway was stopped; attempting safe start before Desktop validation");
        match gateway::start(st) {
            Ok(msg) => {
                auto_fixes_applied.push(format!("Claude Gateway start: {msg}"));
                cold_log(&mut steps, "claude_gateway_start_done", "Claude Gateway start result", "Claude", "fixed", &msg);
            }
            Err(e) => {
                manual_fixes_required.push(format!("Claude Gateway failed to start: {e}"));
                cold_log(&mut steps, "claude_gateway_start_failed", "Claude Gateway start failed", "Claude", "error", &e);
            }
        }
        claude_gateway_status = gateway::status(st)?;
    }
    capability(
        &mut capabilities,
        "Claude Gateway process",
        "Claude",
        if claude_gateway_status.running { "ok" } else { "warn" },
        &format!("status={}, error={}", claude_gateway_status.status, claude_gateway_status.error.as_deref().unwrap_or("none")),
    );

    if apply_fixes && !desktop.managed && !enabled_routes.is_empty() {
        cold_log(&mut steps, "desktop_apply", "Apply Claude Desktop binding", "Claude", "running", "Desktop is not managed by Gateway Switch; creating backup and applying current enabled routes");
        let models = desktop_binding::model_configs_from_routes(&enabled_routes);
        match desktop_binding::apply(
            &home,
            &desktop_binding::gateway_base_url(&profile.listen_host, profile.listen_port),
            "x-api-key",
            &profile.auth_token,
            &models,
        ) {
            Ok(info) => {
                desktop = info;
                auto_fixes_applied.push("Applied Claude Desktop Gateway Switch binding with backup".into());
                cold_log(&mut steps, "desktop_apply_done", "Claude Desktop binding applied", "Claude", "fixed", "Desktop config now points to local Claude Gateway");
            }
            Err(e) => {
                manual_fixes_required.push(format!("Claude Desktop binding failed: {e}"));
                cold_log(&mut steps, "desktop_apply_failed", "Claude Desktop binding failed", "Claude", "error", &e);
            }
        }
    } else if !desktop.managed {
        cold_log(&mut steps, "desktop_unmanaged", "Claude Desktop binding check", "Claude", "warn", "Desktop is not managed by Gateway Switch; run repair to apply a safe backup-backed binding");
    }

    let claude_health = local_health(&profile.listen_host, profile.listen_port).await;
    capability(&mut capabilities, "Claude health endpoint", "Claude", health_status(&claude_health), &claude_health);
    cold_log(&mut steps, "claude_health", "Claude Gateway health check", "Claude", health_status(&claude_health), &claude_health);

    let mut codex_gateway_status = codex_gateway::status(st)?;
    if !codex_gateway_status.running && apply_fixes && (codex_info.managed || !enabled_codex_routes.is_empty()) {
        cold_log(&mut steps, "codex_gateway_start", "Start Codex Gateway", "Codex", "running", "Codex Gateway was stopped; attempting safe start before config validation");
        match codex_gateway::start(st) {
            Ok(msg) => {
                auto_fixes_applied.push(format!("Codex Gateway start: {msg}"));
                cold_log(&mut steps, "codex_gateway_start_done", "Codex Gateway start result", "Codex", "fixed", &msg);
            }
            Err(e) => {
                manual_fixes_required.push(format!("Codex Gateway failed to start: {e}"));
                cold_log(&mut steps, "codex_gateway_start_failed", "Codex Gateway start failed", "Codex", "error", &e);
            }
        }
        codex_gateway_status = codex_gateway::status(st)?;
    }
    capability(
        &mut capabilities,
        "Codex Gateway process",
        "Codex",
        if codex_gateway_status.running { "ok" } else { "warn" },
        &format!("status={}, error={}", codex_gateway_status.status, codex_gateway_status.error.as_deref().unwrap_or("none")),
    );

    if apply_fixes && !codex_info.managed {
        if let Some(route) = enabled_codex_routes.first() {
            cold_log(&mut steps, "codex_apply", "Apply Codex binding", "Codex", "running", "Codex is not managed by Gateway Switch; creating backup and applying current default route");
            match codex_binding::apply(
                &home,
                &format!("http://{}:{}/v1", codex_profile.listen_host, codex_profile.listen_port),
                &codex_profile.auth_token,
                &route.codex_model,
            ) {
                Ok(info) => {
                    codex_info = info;
                    auto_fixes_applied.push(format!("Applied Codex Gateway Switch binding for model {}", route.codex_model));
                    cold_log(&mut steps, "codex_apply_done", "Codex binding applied", "Codex", "fixed", "Codex config now points to local Responses Gateway");
                }
                Err(e) => {
                    manual_fixes_required.push(format!("Codex binding failed: {e}"));
                    cold_log(&mut steps, "codex_apply_failed", "Codex binding failed", "Codex", "error", &e);
                }
            }
        } else {
            manual_fixes_required.push("Create at least one enabled Codex route before automatic Codex binding".into());
            cold_log(&mut steps, "codex_no_route", "Codex binding skipped", "Codex", "warn", "No enabled Codex route is available");
        }
    } else if !codex_info.managed {
        cold_log(&mut steps, "codex_unmanaged", "Codex binding check", "Codex", "warn", "Codex is not managed by Gateway Switch; run repair to apply a backup-backed binding");
    }

    let codex_health = local_health(&codex_profile.listen_host, codex_profile.listen_port).await;
    capability(&mut capabilities, "Codex health endpoint", "Codex", health_status(&codex_health), &codex_health);
    cold_log(&mut steps, "codex_health", "Codex Gateway health check", "Codex", health_status(&codex_health), &codex_health);

    let enabled_providers = providers.iter().filter(|p| p.enabled).count();
    capability(&mut capabilities, "Provider inventory", "Provider", if enabled_providers > 0 { "ok" } else { "error" }, &format!("{enabled_providers} enabled providers"));
    capability(&mut capabilities, "Claude route inventory", "Claude", if enabled_routes.is_empty() { "warn" } else { "ok" }, &format!("{} enabled Claude routes", enabled_routes.len()));
    capability(&mut capabilities, "Codex route inventory", "Codex", if enabled_codex_routes.is_empty() { "warn" } else { "ok" }, &format!("{} enabled Codex routes", enabled_codex_routes.len()));

    let security_detail = "Third-party routing may expose prompts, file contents, tool results, and code to upstream providers; keep official providers as fallback for critical/private tasks";
    capability(&mut capabilities, "Third-party routing security", "Security", "warn", security_detail);
    manual_fixes_required.push("Review provider privacy policy and avoid sending sensitive repositories to untrusted third-party models".into());

    if !settings.auto_start_gateway {
        manual_fixes_required.push("Enable Auto Start Gateway if Claude Desktop should work immediately after app launch".into());
    }
    if !settings.auto_takeover_desktop && desktop.managed {
        manual_fixes_required.push("Enable Auto Takeover Desktop if Gateway Switch should re-assert Claude Desktop binding on every launch".into());
    }
    manual_fixes_required.sort();
    manual_fixes_required.dedup();

    cold_log(&mut steps, "report", "Generate coldstart report", "system", "ok", "Compiled UI report, safe-fix results, manual remediation list, and security notes");

    let claude_score = score_for(&capabilities, "Claude");
    let codex_score = score_for(&capabilities, "Codex");
    let overall_score = score_overall(&capabilities);
    let verdict = if overall_score >= 85 {
        "ready as daily gateway environment"
    } else if overall_score >= 70 {
        "usable but needs targeted fixes"
    } else {
        "not ready for unattended daily use"
    }.to_string();
    let biggest_risk = security_detail.to_string();
    let most_important_fix = if !codex_info.managed {
        "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint"
    } else if !desktop.managed {
        "Bind Claude Desktop to Gateway Switch and verify the local /v1/messages health endpoint"
    } else {
        "Prove MCP/GitHub readiness inside Claude Desktop and Codex with real tool calls"
    }.to_string();

    let mut report = ColdStartReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode,
        verdict,
        claude_score,
        codex_score,
        overall_score,
        biggest_risk,
        most_important_fix,
        report_path: None,
        auto_fixes_applied,
        manual_fixes_required,
        steps,
        capabilities,
    };

    if apply_fixes {
        let report_path = write_coldstart_report(st, &report)?;
        report.report_path = Some(report_path);
    }

    Ok(report)
}

fn cold_log(steps: &mut Vec<ColdStartStep>, id: &str, label: &str, target: &str, status: &str, detail: &str) {
    println!("[coldstart][{target}][{status}] {label}: {detail}");
    steps.push(ColdStartStep {
        id: id.into(),
        label: label.into(),
        target: target.into(),
        status: status.into(),
        detail: compatibility::redact_log_summary(detail),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
}

fn capability(items: &mut Vec<ColdStartCapability>, name: &str, target: &str, status: &str, detail: &str) {
    println!("[coldstart][capability][{target}][{status}] {name}: {detail}");
    items.push(ColdStartCapability {
        name: name.into(),
        target: target.into(),
        status: status.into(),
        detail: compatibility::redact_log_summary(detail),
    });
}

fn managed_status(managed: bool) -> &'static str {
    if managed { "ok" } else { "warn" }
}

fn desktop_status(managed: bool) -> &'static str {
    if managed { "ok" } else { "warn" }
}

fn binding_detail(path: &str, managed: bool, base_url: Option<&str>) -> String {
    format!(
        "path={}, managed={}, base_url={}",
        path,
        managed,
        base_url.unwrap_or("not configured")
    )
}

async fn local_health(host: &str, port: u16) -> String {
    let url = format!("http://{host}:{port}/health");
    let start = Instant::now();
    match reqwest::get(&url).await {
        Ok(resp) => format!("{} in {}ms ({url})", resp.status(), start.elapsed().as_millis()),
        Err(e) => format!("unreachable: {e} ({url})"),
    }
}

fn health_status(message: &str) -> &'static str {
    if message.starts_with("200") {
        "ok"
    } else {
        "warn"
    }
}

fn score_for(capabilities: &[ColdStartCapability], target: &str) -> u8 {
    let filtered: Vec<&ColdStartCapability> = capabilities.iter().filter(|c| c.target == target).collect();
    score_items(&filtered)
}

fn score_overall(capabilities: &[ColdStartCapability]) -> u8 {
    let refs: Vec<&ColdStartCapability> = capabilities.iter().collect();
    score_items(&refs)
}

fn score_items(items: &[&ColdStartCapability]) -> u8 {
    if items.is_empty() {
        return 0;
    }
    let points: usize = items.iter().map(|c| match c.status.as_str() {
        "ok" | "fixed" => 100,
        "warn" | "running" => 55,
        "error" => 0,
        _ => 40,
    }).sum();
    (points / items.len()).min(100) as u8
}

fn write_coldstart_report(st: &AppState, report: &ColdStartReport) -> Result<String, String> {
    let dir = st.backups_dir.join("coldstart");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("coldstart-report-{}.md", chrono::Utc::now().timestamp_millis()));
    fs::write(&path, render_coldstart_markdown(report)).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

fn render_coldstart_markdown(report: &ColdStartReport) -> String {
    let mut out = String::new();
    out.push_str("# Gateway Switch Cold Start Report\n\n");
    out.push_str(&format!("- Generated: {}\n", report.generated_at));
    out.push_str(&format!("- Mode: {}\n", report.mode));
    out.push_str(&format!("- Verdict: {}\n", report.verdict));
    out.push_str(&format!("- Overall score: {}%\n", report.overall_score));
    out.push_str(&format!("- Claude score: {}%\n", report.claude_score));
    out.push_str(&format!("- Codex score: {}%\n", report.codex_score));
    out.push_str(&format!("- Biggest risk: {}\n", report.biggest_risk));
    out.push_str(&format!("- Most important fix: {}\n\n", report.most_important_fix));

    out.push_str("## Auto Fixes Applied\n\n");
    if report.auto_fixes_applied.is_empty() {
        out.push_str("- None\n");
    } else {
        for item in &report.auto_fixes_applied {
            out.push_str(&format!("- {}\n", item));
        }
    }

    out.push_str("\n## Manual Fixes Required\n\n");
    if report.manual_fixes_required.is_empty() {
        out.push_str("- None\n");
    } else {
        for item in &report.manual_fixes_required {
            out.push_str(&format!("- {}\n", item));
        }
    }

    out.push_str("\n## Capability Matrix\n\n");
    out.push_str("| target | capability | status | detail |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for c in &report.capabilities {
        out.push_str(&format!("| {} | {} | {} | {} |\n", c.target, c.name, c.status, c.detail.replace('|', "\\|")));
    }

    out.push_str("\n## Execution Log\n\n");
    out.push_str("| time | target | status | step | detail |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for step in &report.steps {
        out.push_str(&format!("| {} | {} | {} | {} | {} |\n", step.timestamp, step.target, step.status, step.label, step.detail.replace('|', "\\|")));
    }

    out.push_str("\n## Security Notes\n\n");
    out.push_str("- Reports are generated with Gateway Switch redaction helpers.\n");
    out.push_str("- Do not paste provider tokens, cookies, bearer headers, or private keys into support tickets.\n");
    out.push_str("- Keep official Claude/OpenAI providers available as fallback for critical private tasks.\n");
    out
}

fn upstream_url(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');
    if base.ends_with(endpoint) {
        base.to_string()
    } else if base.ends_with("/v1") || base.ends_with("/v2") || base.ends_with("/v3") {
        format!("{base}/{endpoint}")
    } else {
        format!("{base}/v1/{endpoint}")
    }
}
