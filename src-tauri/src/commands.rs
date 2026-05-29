use crate::{
    claude_code_binding, codex_binding, codex_gateway, codex_pp,
    coldstart::{run_coldstart_checks, RunMode},
    compatibility, database, desktop_binding, gateway, mcp_sync,
    models::*,
    settings,
    state::{AppState, GatewayStatus},
};
use serde::{Deserialize, Serialize};
use std::{fs, process::Command, time::Instant};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_status(st: State<'_, AppState>) -> Result<AppStatus, String> {
    let gw = gateway::status(&st)?;
    let info = desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let profile = database::get_profile(&st.db_path)?;
    let health = probe_gateway_health(&profile).await;
    let gateway_running = health.ok;
    let gateway_error = if gateway_running {
        None
    } else {
        gw.error.or(Some(health.message))
    };
    Ok(AppStatus {
        gateway_running,
        gateway_port: profile.listen_port,
        gateway_error,
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
    database::save_profile(
        &st.db_path,
        &GatewayProfile {
            listen_host: payload.listen_host.clone(),
            listen_port: payload.listen_port,
            auth_token: payload.auth_token.clone(),
        },
    )?;
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
pub fn list_provider_capabilities(
    st: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(database::list_providers(&st.db_path)?
        .iter()
        .map(compatibility::provider_capability_json)
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSourceReport {
    pub bundle_path: String,
    pub is_applications: bool,
    pub is_dmg_volume: bool,
    pub is_temp_volume: bool,
    pub severity: String,
    pub summary: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaudeCodeRepairReport {
    pub repaired: bool,
    pub before: ClaudeCodeInfo,
    pub after: ClaudeCodeInfo,
    pub backup_path: Option<String>,
    pub selected_model: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckReport {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub asset_names: Vec<String>,
    pub summary: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeInstallPlan {
    pub current_exe: String,
    pub is_applications: bool,
    pub is_dmg_volume: bool,
    pub is_temp_volume: bool,
    pub applications_app_exists: bool,
    pub release_artifacts_dir: Option<String>,
    pub steps: Vec<String>,
    pub warning: Option<String>,
}

#[tauri::command]
pub fn get_route_diagnostics(
    st: State<'_, AppState>,
) -> Result<Vec<gateway::RouteCompatibilityDiagnostic>, String> {
    gateway::route_diagnostics(&st.db_path)
}

#[tauri::command]
pub fn preview_route_payload(
    st: State<'_, AppState>,
    claude_alias: String,
) -> Result<gateway::RoutePayloadPreview, String> {
    gateway::preview_route_payload(&st.db_path, claude_alias)
}

#[tauri::command]
pub fn list_provider_policies(
    st: State<'_, AppState>,
) -> Result<Vec<ProviderCompatibilityPolicy>, String> {
    database::list_provider_policies(&st.db_path)
}

#[tauri::command]
pub fn upsert_provider_policy(
    st: State<'_, AppState>,
    payload: ProviderCompatibilityPolicy,
) -> Result<Vec<ProviderCompatibilityPolicy>, String> {
    let mut policy = payload;
    if policy.updated_by.trim().is_empty() {
        policy.updated_by = "user".into();
    }
    database::upsert_provider_policy(&st.db_path, &policy)?;
    database::list_provider_policies(&st.db_path)
}

#[tauri::command]
pub fn reset_provider_policy(
    st: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ProviderCompatibilityPolicy>, String> {
    database::reset_provider_policy(&st.db_path, &provider_id)?;
    database::list_provider_policies(&st.db_path)
}

#[tauri::command]
pub fn list_failed_request_diagnostics(
    st: State<'_, AppState>,
) -> Result<Vec<FailedRequestDiagnosticCandidate>, String> {
    database::list_failed_request_snapshots(&st.db_path, 100)
}

#[tauri::command]
pub fn replay_request_diagnostic(
    st: State<'_, AppState>,
    request_id: String,
) -> Result<gateway::RequestReplayReport, String> {
    gateway::replay_request_diagnostic(&st.db_path, request_id)
}

#[tauri::command]
pub fn get_codex_route_diagnostics(
    st: State<'_, AppState>,
) -> Result<Vec<codex_gateway::CodexRouteDiagnostic>, String> {
    codex_gateway::route_diagnostics(&st.db_path)
}

#[tauri::command]
pub fn get_runtime_source_report() -> RuntimeSourceReport {
    runtime_source_report_for_path(
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|e| format!("unknown: {e}")),
    )
}

#[tauri::command]
pub async fn check_app_update() -> Result<UpdateCheckReport, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let url = "https://api.github.com/repos/gcristiano0624-bot/gateway-switch/releases/latest";
    let resp = reqwest::Client::new()
        .get(url)
        .header("user-agent", "Gateway Switch")
        .send()
        .await;
    let Ok(resp) = resp else {
        return Ok(UpdateCheckReport {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            asset_names: Vec::new(),
            summary:
                "Could not reach GitHub Releases. Keep using the installed version and retry later."
                    .into(),
            error: Some(resp.err().map(|e| e.to_string()).unwrap_or_default()),
        });
    };
    if !resp.status().is_success() {
        let status = resp.status();
        return Ok(UpdateCheckReport {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            asset_names: Vec::new(),
            summary: format!("GitHub Releases returned HTTP {status}."),
            error: Some(format!("HTTP {status}")),
        });
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string());
    let release_url = body
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(String::from);
    let asset_names = body
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| asset.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let update_available = tag
        .as_deref()
        .map(|latest| version_is_newer(latest, &current_version))
        .unwrap_or(false);
    Ok(UpdateCheckReport {
        current_version,
        latest_version: tag.clone(),
        update_available,
        release_url,
        asset_names,
        summary: if update_available {
            format!("Gateway Switch v{} is available. Download the DMG from GitHub Release and install it manually.", tag.unwrap_or_default())
        } else {
            "Gateway Switch is up to date or no newer release was detected.".into()
        },
        error: None,
    })
}

#[tauri::command]
pub fn get_safe_install_plan() -> SafeInstallPlan {
    safe_install_plan_for_path(
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|e| format!("unknown: {e}")),
    )
}

#[tauri::command]
pub fn reveal_safe_install_locations() -> Result<String, String> {
    let _ = Command::new("open").arg("/Applications").status();
    if let Some(dir) = latest_release_artifacts_dir() {
        let _ = Command::new("open").arg(&dir).status();
    }
    Ok("Opened /Applications and the latest local release-artifacts folder when available.".into())
}

fn runtime_source_report_for_path(bundle_path: String) -> RuntimeSourceReport {
    let is_applications = bundle_path.starts_with("/Applications/");
    let is_dmg_volume = bundle_path.starts_with("/Volumes/");
    let is_temp_volume =
        bundle_path.starts_with("/tmp/") || bundle_path.starts_with("/private/tmp/");
    let severity = if is_dmg_volume || is_temp_volume {
        "warn"
    } else if is_applications {
        "ok"
    } else {
        "info"
    }
    .to_string();
    let (summary, recommendation) = if is_dmg_volume {
        (
            "Gateway Switch is running from a mounted disk image.".into(),
            "Copy Gateway Switch.app to /Applications before binding launchd watchers or repairing Codex++.".into(),
        )
    } else if is_temp_volume {
        (
            "Gateway Switch is running from a temporary path.".into(),
            "Install the app under /Applications to avoid poisoned watcher or shim paths.".into(),
        )
    } else if is_applications {
        (
            "Gateway Switch is running from /Applications.".into(),
            "Runtime source looks stable for launchd watchers and Codex++ repair actions.".into(),
        )
    } else {
        (
            "Gateway Switch is running from a non-standard location.".into(),
            "For release builds, /Applications is recommended.".into(),
        )
    };
    RuntimeSourceReport {
        bundle_path,
        is_applications,
        is_dmg_volume,
        is_temp_volume,
        severity,
        summary,
        recommendation,
    }
}

fn safe_install_plan_for_path(current_exe: String) -> SafeInstallPlan {
    let runtime = runtime_source_report_for_path(current_exe.clone());
    let applications_app_exists = std::path::Path::new("/Applications/Gateway Switch.app").exists();
    let release_artifacts_dir = latest_release_artifacts_dir();
    let mut steps = Vec::new();
    steps.push("Quit Gateway Switch before replacing the app bundle.".into());
    steps.push("Open the latest Gateway Switch DMG or local release artifact.".into());
    steps.push("Drag Gateway Switch.app into /Applications using Finder.".into());
    steps.push("Launch Gateway Switch from /Applications, not from the mounted DMG.".into());
    if applications_app_exists {
        steps.push("If Finder asks, choose Replace only after the app is closed.".into());
    }
    SafeInstallPlan {
        current_exe,
        is_applications: runtime.is_applications,
        is_dmg_volume: runtime.is_dmg_volume,
        is_temp_volume: runtime.is_temp_volume,
        applications_app_exists,
        release_artifacts_dir,
        steps,
        warning: (runtime.severity == "warn").then_some(runtime.recommendation),
    }
}

fn latest_release_artifacts_dir() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let dir = cwd.join("release-artifacts");
    let entries = fs::read_dir(dir).ok()?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();
    paths.pop().map(|path| path.display().to_string())
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |value: &str| {
        value
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let latest_parts = parse(latest);
    let current_parts = parse(current);
    for i in 0..latest_parts.len().max(current_parts.len()) {
        let l = *latest_parts.get(i).unwrap_or(&0);
        let c = *current_parts.get(i).unwrap_or(&0);
        if l != c {
            return l > c;
        }
    }
    false
}

#[tauri::command]
pub fn get_runtime_feature_report() -> compatibility::RuntimeFeatureReport {
    compatibility::runtime_feature_report()
}

#[tauri::command]
pub fn run_compatibility_benchmark(
    st: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(database::list_providers(&st.db_path)?
        .iter()
        .map(|p| {
            serde_json::json!({
                "provider": p.id,
                "anthropic": compatibility::benchmark_provider(p),
                "codex": compatibility::codex_capability_profile(p)
            })
        })
        .collect())
}

#[tauri::command]
pub fn validate_patch_payload(
    patch: String,
) -> Result<compatibility::PatchValidationResult, String> {
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
pub fn compress_context_payload(
    messages: Vec<serde_json::Value>,
    max_items: usize,
) -> serde_json::Value {
    compatibility::compress_context(&messages, max_items)
}

#[tauri::command]
pub fn recover_agent_state_payload(
    history: Vec<serde_json::Value>,
) -> compatibility::AgentTaskState {
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
    let path = st.backups_dir.join(format!(
        "diagnostics-{}.json",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::write(
        &path,
        serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn get_mcp_sync_status(st: State<'_, AppState>) -> Result<mcp_sync::McpSyncPreview, String> {
    let _ = st;
    mcp_sync::inspect(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub fn preview_mcp_sync(st: State<'_, AppState>) -> Result<mcp_sync::McpSyncPreview, String> {
    let _ = st;
    mcp_sync::preview(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub fn run_mcp_sync(st: State<'_, AppState>) -> Result<mcp_sync::McpSyncResult, String> {
    let _ = st;
    mcp_sync::sync(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub async fn get_coldstart_status(st: State<'_, AppState>) -> Result<ColdStartReport, String> {
    run_coldstart_checks(&st, RunMode::Check).await
}

#[tauri::command]
pub async fn run_coldstart_repair(st: State<'_, AppState>) -> Result<ColdStartReport, String> {
    run_coldstart_checks(&st, RunMode::Repair).await
}

#[tauri::command]
pub fn create_provider(
    st: State<'_, AppState>,
    payload: CreateProvider,
) -> Result<Vec<Provider>, String> {
    database::create_provider(&st.db_path, &payload)?;
    database::list_providers(&st.db_path)
}

#[tauri::command]
pub fn update_provider(
    st: State<'_, AppState>,
    payload: UpdateProvider,
) -> Result<Vec<Provider>, String> {
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
pub fn create_route(
    st: State<'_, AppState>,
    payload: CreateModelRoute,
) -> Result<Vec<ModelRoute>, String> {
    database::create_route(&st.db_path, &payload)?;
    database::list_routes(&st.db_path)
}

#[tauri::command]
pub fn update_route(
    st: State<'_, AppState>,
    payload: UpdateModelRoute,
) -> Result<Vec<ModelRoute>, String> {
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
        "x-api-key",
        &profile.auth_token,
        &models,
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
pub async fn apply_claude_code_binding(
    st: State<'_, AppState>,
    payload: ClaudeCodeBindPayload,
) -> Result<ClaudeCodeInfo, String> {
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
            let upstream_model = payload.upstream_model.as_deref().unwrap_or(&payload.model);
            if provider_requires_gateway_route_for_claude_code(
                &st.db_path,
                &provider,
                upstream_model,
            ) {
                return Err("This provider/model is not Anthropic-compatible for Claude Code Direct Provider mode. Use Gateway Route so Gateway Switch can convert system/tool roles for Volcengine DeepSeek.".into());
            }
            claude_code_binding::apply_provider(
                &dirs::home_dir().ok_or("no home")?,
                provider.anthropic_base_url.as_deref().ok_or("This provider does not have an Anthropic Base URL for Claude Code Direct Provider mode")?,
                &provider.auth_header,
                provider.auth_scheme.as_deref(),
                provider.api_key.as_deref().unwrap_or_default(),
                upstream_model,
            )
        }
        _ => Err("Unknown Claude Code binding mode".into()),
    }
}

fn provider_requires_gateway_route_for_claude_code(
    db: &std::path::PathBuf,
    provider: &crate::models::Provider,
    upstream_model: &str,
) -> bool {
    !gateway::effective_provider_compatibility_profile(db, provider, upstream_model)
        .direct_provider_safe
}

#[tauri::command]
pub fn restore_claude_code_binding(st: State<'_, AppState>) -> Result<ClaudeCodeInfo, String> {
    let _ = st;
    claude_code_binding::restore(&dirs::home_dir().ok_or("no home")?)
}

#[tauri::command]
pub async fn repair_claude_code_gateway_binding(
    st: State<'_, AppState>,
    model: String,
) -> Result<ClaudeCodeRepairReport, String> {
    let home = dirs::home_dir().ok_or("no home")?;
    let before = claude_code_binding::inspect(&home)?;
    let diagnostics = gateway::route_diagnostics(&st.db_path)?;
    let selected = diagnostics
        .iter()
        .find(|d| d.claude_alias == model)
        .or_else(|| {
            diagnostics
                .iter()
                .find(|d| d.strategy.gateway_route_recommended)
        })
        .or_else(|| diagnostics.first())
        .ok_or("No Claude route is configured for Claude Code repair")?;
    let mut warnings = selected.warnings.clone();
    if selected.strategy.direct_provider_safe {
        warnings.push("Selected route is already Direct Provider safe; Gateway Route repair is still allowed.".into());
    }
    let profile = database::get_profile(&st.db_path)?;
    let _ = gateway::start(&st);
    let after = claude_code_binding::apply_gateway(
        &home,
        &desktop_binding::gateway_base_url(&profile.listen_host, profile.listen_port),
        &profile.auth_token,
        &selected.claude_alias,
    )?;
    Ok(ClaudeCodeRepairReport {
        repaired: true,
        before,
        backup_path: after.backup_path.clone(),
        after,
        selected_model: selected.claude_alias.clone(),
        warnings,
    })
}

#[tauri::command]
pub async fn check_gateway_health(st: State<'_, AppState>) -> Result<HealthStatus, String> {
    let profile = database::get_profile(&st.db_path)?;
    Ok(probe_gateway_health(&profile).await)
}

async fn probe_gateway_health(profile: &GatewayProfile) -> HealthStatus {
    let url = format!(
        "http://{}:{}/health",
        profile.listen_host, profile.listen_port
    );
    let start = Instant::now();
    match reqwest::get(&url).await {
        Ok(r) => HealthStatus {
            target: "gateway".into(),
            ok: r.status().is_success(),
            message: format!("HTTP {}", r.status()),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
        Err(e) => HealthStatus {
            target: "gateway".into(),
            ok: false,
            message: e.to_string(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
    }
}

#[tauri::command]
pub async fn check_provider_health(
    st: State<'_, AppState>,
    id: String,
) -> Result<HealthStatus, String> {
    let providers = database::list_providers(&st.db_path)?;
    let p = providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Provider not found")?;
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
    let path = st.backups_dir.join(format!(
        "export-{}.json",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::write(
        &path,
        serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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
                    let _ = database::update_provider(
                        &st.db_path,
                        &UpdateProvider {
                            id: id.into(),
                            name: name.into(),
                            base_url: url.into(),
                            openai_base_url: p
                                .get("openai_base_url")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            anthropic_base_url: p
                                .get("anthropic_base_url")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            auth_header: p
                                .get("auth_header")
                                .and_then(|v| v.as_str())
                                .unwrap_or("x-api-key")
                                .into(),
                            auth_scheme: p
                                .get("auth_scheme")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            api_key: p.get("api_key").and_then(|v| v.as_str()).map(Into::into),
                            enabled: p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        },
                    );
                } else {
                    let _ = database::create_provider(
                        &st.db_path,
                        &CreateProvider {
                            id: id.into(),
                            name: name.into(),
                            base_url: url.into(),
                            openai_base_url: p
                                .get("openai_base_url")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            anthropic_base_url: p
                                .get("anthropic_base_url")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            auth_header: p
                                .get("auth_header")
                                .and_then(|v| v.as_str())
                                .unwrap_or("x-api-key")
                                .into(),
                            auth_scheme: p
                                .get("auth_scheme")
                                .and_then(|v| v.as_str())
                                .map(Into::into),
                            api_key: p.get("api_key").and_then(|v| v.as_str()).map(Into::into),
                        },
                    );
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
                    let _ = database::update_route(
                        &st.db_path,
                        &UpdateModelRoute {
                            id: id.into(),
                            claude_alias: alias.into(),
                            display_name: display.into(),
                            provider_id: pid.into(),
                            upstream_model: upstream.into(),
                            enabled: r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        },
                    );
                } else {
                    let _ = database::create_route(
                        &st.db_path,
                        &CreateModelRoute {
                            id: id.into(),
                            claude_alias: alias.into(),
                            display_name: display.into(),
                            provider_id: pid.into(),
                            upstream_model: upstream.into(),
                        },
                    );
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
pub fn save_codex_profile(
    st: State<'_, AppState>,
    payload: GatewayProfile,
) -> Result<GatewayProfile, String> {
    database::save_codex_profile(&st.db_path, &payload)?;
    database::get_codex_profile(&st.db_path)
}

#[tauri::command]
pub fn list_codex_routes(st: State<'_, AppState>) -> Result<Vec<CodexRoute>, String> {
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub fn create_codex_route(
    st: State<'_, AppState>,
    payload: CreateCodexRoute,
) -> Result<Vec<CodexRoute>, String> {
    database::create_codex_route(&st.db_path, &payload)?;
    database::list_codex_routes(&st.db_path)
}

#[tauri::command]
pub fn update_codex_route(
    st: State<'_, AppState>,
    payload: UpdateCodexRoute,
) -> Result<Vec<CodexRoute>, String> {
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
    let url = format!(
        "http://{}:{}/health",
        profile.listen_host, profile.listen_port
    );
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
pub fn apply_codex_binding(
    st: State<'_, AppState>,
    model: String,
) -> Result<CodexBindingInfo, String> {
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

#[tauri::command]
pub fn detect_codex_pp() -> Result<codex_pp::CodexPpInstall, String> {
    Ok(codex_pp::detect())
}

#[tauri::command]
pub fn list_codex_pp_tweaks() -> Result<Vec<codex_pp::CodexPpTweak>, String> {
    codex_pp::list_tweaks()
}

#[tauri::command]
pub fn set_codex_pp_tweak_enabled(
    id: String,
    enabled: bool,
) -> Result<Vec<codex_pp::CodexPpTweak>, String> {
    codex_pp::set_tweak_enabled(id, enabled)
}

#[tauri::command]
pub async fn fetch_codex_pp_store() -> Result<codex_pp::CodexPpStoreIndex, String> {
    codex_pp::fetch_store().await
}

#[tauri::command]
pub async fn install_codex_pp_tweak(
    repo: String,
    approved_commit_sha: String,
) -> Result<Vec<codex_pp::CodexPpTweak>, String> {
    codex_pp::install_from_store(repo, approved_commit_sha).await
}

#[tauri::command]
pub fn uninstall_codex_pp_tweak(id: String) -> Result<Vec<codex_pp::CodexPpTweak>, String> {
    codex_pp::uninstall_tweak(id)
}

#[tauri::command]
pub fn get_codex_pp_health() -> Result<codex_pp::CodexPpHealth, String> {
    Ok(codex_pp::health())
}

#[tauri::command]
pub fn get_codex_pp_preflight() -> Result<codex_pp::CodexPpPreflight, String> {
    Ok(codex_pp::preflight())
}

#[tauri::command]
pub fn get_codex_pp_recommended_scripts(
) -> Result<codex_pp::CodexPpRecommendedScriptsReport, String> {
    Ok(codex_pp::recommended_scripts_report())
}

#[tauri::command]
pub fn install_codex_pp_recommended_scripts(
) -> Result<codex_pp::CodexPpRecommendedScriptsReport, String> {
    codex_pp::install_recommended_scripts()
}

#[tauri::command]
pub fn run_codex_pp_cli(
    app: AppHandle,
    action: String,
    session_id: Option<String>,
) -> Result<codex_pp::CodexPpCliResult, String> {
    codex_pp::run_cli(app, action, session_id)
}

#[tauri::command]
pub fn open_codex_pp_path(kind: String) -> Result<String, String> {
    codex_pp::open_path(kind)
}

// =====================================================
//  MODEL ALIASES COMMANDS
// =====================================================

#[tauri::command]
pub fn list_model_aliases(
    st: State<'_, AppState>,
    alias_type: String,
) -> Result<Vec<ModelAlias>, String> {
    database::list_model_aliases(&st.db_path, &alias_type)
}

#[tauri::command]
pub fn create_model_alias(
    st: State<'_, AppState>,
    payload: CreateModelAlias,
) -> Result<Vec<ModelAlias>, String> {
    database::create_model_alias(&st.db_path, &payload)?;
    database::list_model_aliases(&st.db_path, &payload.alias_type)
}

#[tauri::command]
pub fn delete_model_alias(
    st: State<'_, AppState>,
    id: String,
    alias_type: String,
) -> Result<Vec<ModelAlias>, String> {
    database::delete_model_alias(&st.db_path, &id)?;
    database::list_model_aliases(&st.db_path, &alias_type)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_source_report_classifies_applications_path() {
        let report = runtime_source_report_for_path(
            "/Applications/Gateway Switch.app/Contents/MacOS/gateway-switch".into(),
        );
        assert_eq!(report.severity, "ok");
        assert!(report.is_applications);
        assert!(!report.is_dmg_volume);
        assert!(!report.is_temp_volume);
    }

    #[test]
    fn runtime_source_report_warns_for_dmg_volume() {
        let report = runtime_source_report_for_path(
            "/Volumes/Gateway Switch/Gateway Switch.app/Contents/MacOS/gateway-switch".into(),
        );
        assert_eq!(report.severity, "warn");
        assert!(report.is_dmg_volume);
        assert!(report.recommendation.contains("/Applications"));
    }

    #[test]
    fn runtime_source_report_warns_for_temp_path() {
        let report = runtime_source_report_for_path(
            "/private/tmp/Gateway Switch.app/Contents/MacOS/gateway-switch".into(),
        );
        assert_eq!(report.severity, "warn");
        assert!(report.is_temp_volume);
    }

    #[test]
    fn version_comparison_detects_newer_semver_tags() {
        assert!(version_is_newer("v1.10.0", "1.9.0"));
        assert!(version_is_newer("1.10.1", "1.10.0"));
        assert!(!version_is_newer("v1.10.0", "1.10.0"));
        assert!(!version_is_newer("1.9.9", "1.10.0"));
    }

    #[test]
    fn safe_install_plan_warns_for_dmg_runtime() {
        let plan = safe_install_plan_for_path(
            "/Volumes/Gateway Switch/Gateway Switch.app/Contents/MacOS/gateway-switch".into(),
        );
        assert!(plan.is_dmg_volume);
        assert!(!plan.is_applications);
        assert!(plan.warning.unwrap_or_default().contains("/Applications"));
        assert!(plan
            .steps
            .iter()
            .any(|step| step.contains("Drag Gateway Switch.app")));
    }
}
