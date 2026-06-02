use crate::{
    claude_code_binding, codex_binding, codex_gateway, codex_pp,
    coldstart::{run_coldstart_checks, RunMode},
    compatibility, database, desktop_binding, gateway, mcp_sync,
    models::*,
    settings,
    state::{AppState, GatewayStatus},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, process::Command, time::Instant};
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

#[derive(Debug, Clone, Serialize)]
pub struct AppWorkbenchSummary {
    pub app_id: String,
    pub label: String,
    pub managed: bool,
    pub gateway_running: bool,
    pub route_count: usize,
    pub provider_count: usize,
    pub active_model: Option<String>,
    pub recent_request_count: usize,
    pub recent_failure_count: usize,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDashboardReport {
    pub generated_at: String,
    pub overall_status: String,
    pub overall_score: u8,
    pub claude_gateway: HealthStatus,
    pub codex_gateway: HealthStatus,
    pub provider_count: usize,
    pub claude_route_count: usize,
    pub codex_route_count: usize,
    pub apps: Vec<AppWorkbenchSummary>,
    pub recent_failures: Vec<RequestLog>,
    pub recent_activity: Vec<RequestLog>,
    pub runtime_source: RuntimeSourceReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppWorkbenchReport {
    pub generated_at: String,
    pub app: AppWorkbenchSummary,
    pub desktop: Option<desktop_binding::DesktopInfo>,
    pub claude_code: Option<ClaudeCodeInfo>,
    pub codex_binding: Option<CodexBindingInfo>,
    pub claude_routes: Vec<ModelRoute>,
    pub codex_routes: Vec<CodexRoute>,
    pub providers: Vec<Provider>,
    pub recent_logs: Vec<RequestLog>,
    pub diagnostics: UnifiedDiagnosticsReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderConsoleItem {
    pub provider: Provider,
    pub supports_claude: bool,
    pub supports_codex: bool,
    pub linked_claude_routes: usize,
    pub linked_codex_routes: usize,
    pub recent_request_count: usize,
    pub recent_failure_count: usize,
    pub health_score: u8,
    pub policy_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderConsoleReport {
    pub generated_at: String,
    pub providers: Vec<ProviderConsoleItem>,
    pub presets: Vec<ProviderPreset>,
    pub policies: Vec<ProviderCompatibilityPolicy>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageProviderStat {
    pub provider_id: String,
    pub provider_name: String,
    pub request_count: usize,
    pub failure_count: usize,
    pub success_rate: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageStatusBucket {
    pub status: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageInsightsReport {
    pub generated_at: String,
    pub total_requests: usize,
    pub success_rate: u8,
    pub failure_count: usize,
    pub average_latency_ms: Option<u64>,
    pub p95_latency_ms: Option<u64>,
    pub provider_stats: Vec<UsageProviderStat>,
    pub status_buckets: Vec<UsageStatusBucket>,
    pub recent_logs: Vec<RequestLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBuilderPayload {
    pub target_app: String,
    pub route_id: String,
    pub visible_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub tool_call_mode: Option<String>,
    pub conflict_strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBuilderPreview {
    pub target_app: String,
    pub route_kind: String,
    pub route_id: String,
    pub visible_model: String,
    pub provider_id: String,
    pub provider_name: String,
    pub upstream_model: String,
    pub conflict: bool,
    pub conflict_detail: Option<String>,
    pub policy_tags: Vec<String>,
    pub warnings: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteBuilderApplyReport {
    pub preview: RouteBuilderPreview,
    pub claude_routes: Vec<ModelRoute>,
    pub codex_routes: Vec<CodexRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWizardPayload {
    pub preset_id: String,
    pub api_key: Option<String>,
    pub target_app: Option<String>,
    pub route_id: Option<String>,
    pub visible_model: Option<String>,
    pub display_name: Option<String>,
    pub upstream_model: Option<String>,
    pub apply_route: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWizardPreview {
    pub preset: ProviderPreset,
    pub provider_exists: bool,
    pub provider_has_key: bool,
    pub route_preview: Option<RouteBuilderPreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWizardApplyReport {
    pub provider: Provider,
    pub providers: Vec<Provider>,
    pub policies: Vec<ProviderCompatibilityPolicy>,
    pub route_report: Option<RouteBuilderApplyReport>,
}

fn request_failed(log: &RequestLog) -> bool {
    log.status_code.map(|code| code >= 400).unwrap_or(true)
}

fn route_provider_ids(routes: &[ModelRoute], codex_routes: &[CodexRoute]) -> Vec<String> {
    routes
        .iter()
        .map(|route| route.provider_id.clone())
        .chain(codex_routes.iter().map(|route| route.provider_id.clone()))
        .collect()
}

fn policy_tags(policy: Option<&ProviderCompatibilityPolicy>) -> Vec<String> {
    let Some(policy) = policy else {
        return Vec::new();
    };
    [
        (policy.system_to_user, "system_to_user"),
        (policy.tool_to_user, "tool_to_user"),
        (policy.disable_tools, "disable_tools"),
        (policy.strip_unsupported_params, "strip_params"),
        (policy.direct_provider_safe, "direct_safe"),
        (policy.gateway_route_recommended, "gateway_route"),
        (policy.codex_disable_responses, "codex_chat_fallback"),
        (policy.codex_strict_tool_calls, "strict_tools"),
        (policy.codex_strip_reasoning, "strip_reasoning"),
    ]
    .into_iter()
    .filter_map(|(enabled, label)| enabled.unwrap_or(false).then(|| label.to_string()))
    .collect()
}

fn app_logs<'a>(
    app_id: &str,
    routes: &[ModelRoute],
    codex_routes: &[CodexRoute],
    logs: &'a [RequestLog],
) -> Vec<&'a RequestLog> {
    logs.iter()
        .filter(|log| match app_id {
            "codex" => codex_routes
                .iter()
                .any(|route| route.codex_model == log.claude_alias),
            _ => routes
                .iter()
                .any(|route| route.claude_alias == log.claude_alias),
        })
        .collect()
}

fn app_summaries(
    desktop: &desktop_binding::DesktopInfo,
    claude_code: &ClaudeCodeInfo,
    codex_binding: &CodexBindingInfo,
    claude_gateway_running: bool,
    codex_gateway_running: bool,
    routes: &[ModelRoute],
    codex_routes: &[CodexRoute],
    providers: &[Provider],
    logs: &[RequestLog],
) -> Vec<AppWorkbenchSummary> {
    let claude_logs = app_logs("claude_desktop", routes, codex_routes, logs);
    let codex_logs = app_logs("codex", routes, codex_routes, logs);
    vec![
        AppWorkbenchSummary {
            app_id: "claude_desktop".into(),
            label: "Claude Desktop".into(),
            managed: desktop.managed,
            gateway_running: claude_gateway_running,
            route_count: routes.iter().filter(|route| route.enabled).count(),
            provider_count: providers.len(),
            active_model: desktop.models.first().cloned(),
            recent_request_count: claude_logs.len(),
            recent_failure_count: claude_logs.iter().filter(|log| request_failed(log)).count(),
            next_action: if !desktop.managed {
                "Bind Claude Desktop to Gateway Switch".into()
            } else if !claude_gateway_running {
                "Start or repair Claude Gateway".into()
            } else if routes.is_empty() {
                "Build a Claude route".into()
            } else {
                "Monitor recent Claude requests".into()
            },
        },
        AppWorkbenchSummary {
            app_id: "claude_code".into(),
            label: "Claude Code".into(),
            managed: claude_code.managed,
            gateway_running: claude_gateway_running,
            route_count: routes.iter().filter(|route| route.enabled).count(),
            provider_count: providers.len(),
            active_model: claude_code.model.clone(),
            recent_request_count: claude_logs.len(),
            recent_failure_count: claude_logs.iter().filter(|log| request_failed(log)).count(),
            next_action: if !claude_code.managed {
                "Bind Claude Code to a Gateway Route".into()
            } else if !claude_gateway_running {
                "Start or repair Claude Gateway".into()
            } else {
                "Review Claude Code route diagnostics".into()
            },
        },
        AppWorkbenchSummary {
            app_id: "codex".into(),
            label: "Codex".into(),
            managed: codex_binding.managed,
            gateway_running: codex_gateway_running,
            route_count: codex_routes.iter().filter(|route| route.enabled).count(),
            provider_count: providers.len(),
            active_model: codex_binding.model.clone(),
            recent_request_count: codex_logs.len(),
            recent_failure_count: codex_logs.iter().filter(|log| request_failed(log)).count(),
            next_action: if !codex_binding.managed {
                "Bind Codex to the local Responses gateway".into()
            } else if !codex_gateway_running {
                "Start or repair Codex Gateway".into()
            } else if codex_routes.is_empty() {
                "Build a Codex route".into()
            } else {
                "Monitor Codex usage and reliability".into()
            },
        },
    ]
}

fn validate_route_target(target_app: &str) -> Result<&'static str, String> {
    match target_app {
        "claude_desktop" => Ok("claude_desktop_alias"),
        "claude_code" => Ok("claude_code_gateway"),
        "codex" => Ok("codex_responses"),
        _ => Err(format!("Unsupported target_app: {target_app}")),
    }
}

fn route_builder_preview_from_payload(
    providers: &[Provider],
    routes: &[ModelRoute],
    codex_routes: &[CodexRoute],
    policies: &[ProviderCompatibilityPolicy],
    payload: &RouteBuilderPayload,
) -> Result<RouteBuilderPreview, String> {
    let route_kind = validate_route_target(&payload.target_app)?.to_string();
    let route_id = payload.route_id.trim();
    let visible_model = payload.visible_model.trim();
    let provider_id = payload.provider_id.trim();
    let upstream_model = payload.upstream_model.trim();
    if route_id.is_empty() {
        return Err("Route ID is required".into());
    }
    if visible_model.is_empty() {
        return Err("Visible model is required".into());
    }
    if provider_id.is_empty() {
        return Err("Provider is required".into());
    }
    if upstream_model.is_empty() {
        return Err("Upstream model is required".into());
    }
    let provider = providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| format!("Provider '{provider_id}' not found"))?;
    let conflict_detail = if payload.target_app == "codex" {
        codex_routes
            .iter()
            .find(|route| route.id == route_id || route.codex_model == visible_model)
            .map(|route| {
                format!(
                    "Codex route '{}' or model '{}' already exists",
                    route.id, route.codex_model
                )
            })
    } else {
        routes
            .iter()
            .find(|route| route.id == route_id || route.claude_alias == visible_model)
            .map(|route| {
                format!(
                    "Claude route '{}' or alias '{}' already exists",
                    route.id, route.claude_alias
                )
            })
    };
    let policy = policies
        .iter()
        .find(|policy| policy.provider_id == provider_id);
    let mut warnings = Vec::new();
    if payload.target_app == "claude_code" {
        warnings.push("Claude Code direct-provider risk is avoided by using Gateway Route.".into());
    }
    if payload.target_app == "codex"
        && policy
            .and_then(|policy| policy.codex_disable_responses)
            .unwrap_or(false)
    {
        warnings
            .push("This provider is likely to use Responses-to-Chat fallback for Codex.".into());
    }
    if !provider.enabled {
        warnings.push("Provider is disabled; enable it before using this route.".into());
    }
    let conflict = conflict_detail.is_some();
    Ok(RouteBuilderPreview {
        target_app: payload.target_app.clone(),
        route_kind,
        route_id: route_id.into(),
        visible_model: visible_model.into(),
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        upstream_model: upstream_model.into(),
        conflict,
        conflict_detail,
        policy_tags: policy_tags(policy),
        warnings,
        next_action: if conflict {
            "Choose update existing or change the route/model name.".into()
        } else {
            "Save route and run Quick Check.".into()
        },
    })
}

fn apply_route_builder_inner(
    st: &AppState,
    payload: &RouteBuilderPayload,
) -> Result<RouteBuilderApplyReport, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let policies = database::list_provider_policies(&st.db_path)?;
    let preview =
        route_builder_preview_from_payload(&providers, &routes, &codex_routes, &policies, payload)?;
    let update_existing = payload.conflict_strategy.as_deref() == Some("update");
    if preview.conflict && !update_existing {
        return Err(preview
            .conflict_detail
            .clone()
            .unwrap_or_else(|| "Route conflict detected".into()));
    }

    if payload.target_app == "codex" {
        let existing = codex_routes.iter().find(|route| {
            route.id == preview.route_id || route.codex_model == preview.visible_model
        });
        if let Some(existing) = existing {
            database::update_codex_route(
                &st.db_path,
                &UpdateCodexRoute {
                    id: existing.id.clone(),
                    codex_model: preview.visible_model.clone(),
                    display_name: payload.display_name.trim().to_string(),
                    provider_id: preview.provider_id.clone(),
                    upstream_model: preview.upstream_model.clone(),
                    tool_call_mode: payload.tool_call_mode.clone(),
                    enabled: true,
                },
            )?;
        } else {
            database::create_codex_route(
                &st.db_path,
                &CreateCodexRoute {
                    id: preview.route_id.clone(),
                    codex_model: preview.visible_model.clone(),
                    display_name: payload.display_name.trim().to_string(),
                    provider_id: preview.provider_id.clone(),
                    upstream_model: preview.upstream_model.clone(),
                    tool_call_mode: payload.tool_call_mode.clone(),
                },
            )?;
        }
    } else {
        let existing = routes.iter().find(|route| {
            route.id == preview.route_id || route.claude_alias == preview.visible_model
        });
        if let Some(existing) = existing {
            database::update_route(
                &st.db_path,
                &UpdateModelRoute {
                    id: existing.id.clone(),
                    claude_alias: preview.visible_model.clone(),
                    display_name: payload.display_name.trim().to_string(),
                    provider_id: preview.provider_id.clone(),
                    upstream_model: preview.upstream_model.clone(),
                    enabled: true,
                },
            )?;
        } else {
            database::create_route(
                &st.db_path,
                &CreateModelRoute {
                    id: preview.route_id.clone(),
                    claude_alias: preview.visible_model.clone(),
                    display_name: payload.display_name.trim().to_string(),
                    provider_id: preview.provider_id.clone(),
                    upstream_model: preview.upstream_model.clone(),
                },
            )?;
        }
    }

    Ok(RouteBuilderApplyReport {
        preview,
        claude_routes: database::list_routes(&st.db_path)?,
        codex_routes: database::list_codex_routes(&st.db_path)?,
    })
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

#[tauri::command]
pub async fn get_runtime_dashboard(
    st: State<'_, AppState>,
) -> Result<RuntimeDashboardReport, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 40)?;
    let desktop = desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let claude_code = claude_code_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let codex_binding = codex_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let profile = database::get_profile(&st.db_path)?;
    let codex_profile = database::get_codex_profile(&st.db_path)?;
    let claude_gateway = probe_gateway_health(&profile).await;
    let codex_gateway = probe_codex_gateway_health(&codex_profile).await;
    let recent_failures = logs
        .iter()
        .filter(|log| request_failed(log))
        .take(10)
        .cloned()
        .collect::<Vec<_>>();
    let provider_ids = route_provider_ids(&routes, &codex_routes);
    let active_provider_count = providers
        .iter()
        .filter(|provider| provider_ids.iter().any(|id| id == &provider.id))
        .count();
    let apps = app_summaries(
        &desktop,
        &claude_code,
        &codex_binding,
        claude_gateway.ok,
        codex_gateway.ok,
        &routes,
        &codex_routes,
        &providers,
        &logs,
    );
    let healthy_apps = apps
        .iter()
        .filter(|app| app.managed && app.gateway_running)
        .count();
    let mut overall_score = 30
        + (healthy_apps as u8 * 15)
        + if active_provider_count > 0 { 10 } else { 0 }
        + if recent_failures.is_empty() { 15 } else { 0 };
    overall_score = overall_score.min(100);
    let overall_status = if overall_score >= 85 {
        "healthy"
    } else if overall_score >= 65 {
        "attention"
    } else if overall_score >= 40 {
        "degraded"
    } else {
        "critical"
    }
    .to_string();

    Ok(RuntimeDashboardReport {
        generated_at: Utc::now().to_rfc3339(),
        overall_status,
        overall_score,
        claude_gateway,
        codex_gateway,
        provider_count: providers.len(),
        claude_route_count: routes.len(),
        codex_route_count: codex_routes.len(),
        apps,
        recent_failures,
        recent_activity: logs.into_iter().take(10).collect(),
        runtime_source: get_runtime_source_report(),
    })
}

#[tauri::command]
pub fn get_app_workbench(
    st: State<'_, AppState>,
    app_id: String,
) -> Result<AppWorkbenchReport, String> {
    if !matches!(app_id.as_str(), "claude_desktop" | "claude_code" | "codex") {
        return Err(format!("Unsupported app_id: {app_id}"));
    }
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 80)?;
    let desktop = desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let claude_code = claude_code_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let codex_binding = codex_binding::inspect(&dirs::home_dir().ok_or("no home")?)?;
    let gateway_running = gateway::status(&st)
        .map(|status| status.running)
        .unwrap_or(false);
    let codex_gateway_running = codex_gateway::status(&st)
        .map(|status| status.running)
        .unwrap_or(false);
    let apps = app_summaries(
        &desktop,
        &claude_code,
        &codex_binding,
        gateway_running,
        codex_gateway_running,
        &routes,
        &codex_routes,
        &providers,
        &logs,
    );
    let app = apps
        .into_iter()
        .find(|summary| summary.app_id == app_id)
        .ok_or_else(|| format!("Unsupported app_id: {app_id}"))?;
    let recent_logs = logs
        .into_iter()
        .filter(|log| match app_id.as_str() {
            "codex" => codex_routes
                .iter()
                .any(|route| route.codex_model == log.claude_alias),
            _ => routes
                .iter()
                .any(|route| route.claude_alias == log.claude_alias),
        })
        .take(30)
        .collect();

    Ok(AppWorkbenchReport {
        generated_at: Utc::now().to_rfc3339(),
        app,
        desktop: (app_id == "claude_desktop").then_some(desktop),
        claude_code: (app_id == "claude_code").then_some(claude_code),
        codex_binding: (app_id == "codex").then_some(codex_binding),
        claude_routes: routes,
        codex_routes,
        providers,
        recent_logs,
        diagnostics: unified_diagnostics_report(&st)?,
    })
}

#[tauri::command]
pub fn get_provider_console(st: State<'_, AppState>) -> Result<ProviderConsoleReport, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let policies = database::list_provider_policies(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 200)?;
    let items = providers
        .iter()
        .cloned()
        .map(|provider| {
            let linked_claude_routes = routes
                .iter()
                .filter(|route| route.provider_id == provider.id)
                .count();
            let linked_codex_routes = codex_routes
                .iter()
                .filter(|route| route.provider_id == provider.id)
                .count();
            let recent_request_count = logs
                .iter()
                .filter(|log| log.provider_id == provider.id)
                .count();
            let recent_failure_count = logs
                .iter()
                .filter(|log| log.provider_id == provider.id && request_failed(log))
                .count();
            let policy = policies
                .iter()
                .find(|policy| policy.provider_id == provider.id);
            let health_score = if !provider.enabled {
                0
            } else if recent_request_count == 0 {
                75
            } else {
                let success_count = recent_request_count.saturating_sub(recent_failure_count);
                ((success_count * 100) / recent_request_count) as u8
            };
            ProviderConsoleItem {
                supports_claude: provider.anthropic_base_url.is_some() || linked_claude_routes > 0,
                supports_codex: !provider.openai_base_url.trim().is_empty(),
                linked_claude_routes,
                linked_codex_routes,
                recent_request_count,
                recent_failure_count,
                health_score,
                policy_tags: policy_tags(policy),
                provider,
            }
        })
        .collect();

    Ok(ProviderConsoleReport {
        generated_at: Utc::now().to_rfc3339(),
        providers: items,
        presets: built_in_provider_presets(),
        policies,
    })
}

#[tauri::command]
pub fn get_usage_insights(st: State<'_, AppState>) -> Result<UsageInsightsReport, String> {
    let providers = database::list_providers(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 500)?;
    let total_requests = logs.len();
    let failure_count = logs.iter().filter(|log| request_failed(log)).count();
    let success_rate = if total_requests == 0 {
        0
    } else {
        (((total_requests - failure_count) * 100) / total_requests) as u8
    };
    let mut durations = logs
        .iter()
        .filter_map(|log| log.duration_ms)
        .collect::<Vec<_>>();
    durations.sort_unstable();
    let average_latency_ms = if durations.is_empty() {
        None
    } else {
        Some(durations.iter().sum::<u64>() / durations.len() as u64)
    };
    let p95_latency_ms = if durations.is_empty() {
        None
    } else {
        Some(durations[((durations.len() as f64 * 0.95).floor() as usize).min(durations.len() - 1)])
    };
    let provider_stats = providers
        .iter()
        .map(|provider| {
            let request_count = logs
                .iter()
                .filter(|log| log.provider_id == provider.id)
                .count();
            let failure_count = logs
                .iter()
                .filter(|log| log.provider_id == provider.id && request_failed(log))
                .count();
            let success_rate = if request_count == 0 {
                0
            } else {
                (((request_count - failure_count) * 100) / request_count) as u8
            };
            UsageProviderStat {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                request_count,
                failure_count,
                success_rate,
            }
        })
        .collect();
    let mut buckets = HashMap::<String, usize>::new();
    for log in &logs {
        let key = log
            .status_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "network".into());
        *buckets.entry(key).or_default() += 1;
    }
    let mut status_buckets = buckets
        .into_iter()
        .map(|(status, count)| UsageStatusBucket { status, count })
        .collect::<Vec<_>>();
    status_buckets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.status.cmp(&b.status)));

    Ok(UsageInsightsReport {
        generated_at: Utc::now().to_rfc3339(),
        total_requests,
        success_rate,
        failure_count,
        average_latency_ms,
        p95_latency_ms,
        provider_stats,
        status_buckets,
        recent_logs: logs.into_iter().take(80).collect(),
    })
}

#[tauri::command]
pub fn preview_route_builder(
    st: State<'_, AppState>,
    payload: RouteBuilderPayload,
) -> Result<RouteBuilderPreview, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let policies = database::list_provider_policies(&st.db_path)?;
    route_builder_preview_from_payload(&providers, &routes, &codex_routes, &policies, &payload)
}

#[tauri::command]
pub fn apply_route_builder(
    st: State<'_, AppState>,
    payload: RouteBuilderPayload,
) -> Result<RouteBuilderApplyReport, String> {
    apply_route_builder_inner(&st, &payload)
}

#[tauri::command]
pub fn preview_provider_wizard(
    st: State<'_, AppState>,
    payload: ProviderWizardPayload,
) -> Result<ProviderWizardPreview, String> {
    let preset = built_in_provider_presets()
        .into_iter()
        .find(|p| p.id == payload.preset_id)
        .ok_or_else(|| format!("Provider preset '{}' not found", payload.preset_id))?;
    let providers = database::list_providers(&st.db_path)?;
    let provider_exists = providers.iter().any(|provider| provider.id == preset.id);
    let provider_has_key = providers
        .iter()
        .find(|provider| provider.id == preset.id)
        .and_then(|provider| provider.api_key.as_deref())
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
        || payload
            .api_key
            .as_deref()
            .map(|key| !key.trim().is_empty())
            .unwrap_or(false);
    let route_preview = if let Some(target_app) = payload.target_app.as_deref() {
        let route_payload = RouteBuilderPayload {
            target_app: target_app.into(),
            route_id: payload
                .route_id
                .clone()
                .unwrap_or_else(|| format!("{}-{}", target_app.replace('_', "-"), preset.id)),
            visible_model: payload.visible_model.clone().unwrap_or_else(|| {
                if target_app == "codex" {
                    preset.recommended_codex_model.clone()
                } else {
                    preset.recommended_claude_alias.clone()
                }
            }),
            display_name: payload
                .display_name
                .clone()
                .unwrap_or_else(|| format!("{} via {}", target_app.replace('_', " "), preset.name)),
            provider_id: preset.id.clone(),
            upstream_model: payload
                .upstream_model
                .clone()
                .unwrap_or_else(|| preset.upstream_model_example.clone()),
            tool_call_mode: None,
            conflict_strategy: Some("update".into()),
        };
        let routes = database::list_routes(&st.db_path)?;
        let codex_routes = database::list_codex_routes(&st.db_path)?;
        let policies = database::list_provider_policies(&st.db_path)?;
        Some(route_builder_preview_from_payload(
            &providers,
            &routes,
            &codex_routes,
            &policies,
            &route_payload,
        )?)
    } else {
        None
    };
    let mut warnings = preset.warnings.clone();
    if !provider_has_key {
        warnings.push(
            "API key is missing. The provider can be saved, but health checks may fail.".into(),
        );
    }
    Ok(ProviderWizardPreview {
        preset,
        provider_exists,
        provider_has_key,
        route_preview,
        warnings,
    })
}

#[tauri::command]
pub fn apply_provider_wizard(
    st: State<'_, AppState>,
    payload: ProviderWizardPayload,
) -> Result<ProviderWizardApplyReport, String> {
    let preset = built_in_provider_presets()
        .into_iter()
        .find(|p| p.id == payload.preset_id)
        .ok_or_else(|| format!("Provider preset '{}' not found", payload.preset_id))?;
    let existing = database::list_providers(&st.db_path)?
        .into_iter()
        .find(|provider| provider.id == preset.id);
    let api_key = payload
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .map(|key| key.trim().to_string())
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|provider| provider.api_key.clone())
        });
    if let Some(existing) = existing {
        database::update_provider(
            &st.db_path,
            &UpdateProvider {
                id: preset.id.clone(),
                name: preset.name.clone(),
                base_url: preset.base_url.clone(),
                openai_base_url: Some(preset.openai_base_url.clone()),
                anthropic_base_url: preset.anthropic_base_url.clone(),
                auth_header: preset.auth_header.clone(),
                auth_scheme: preset.auth_scheme.clone(),
                api_key,
                enabled: existing.enabled,
            },
        )?;
    } else {
        database::create_provider(
            &st.db_path,
            &CreateProvider {
                id: preset.id.clone(),
                name: preset.name.clone(),
                base_url: preset.base_url.clone(),
                openai_base_url: Some(preset.openai_base_url.clone()),
                anthropic_base_url: preset.anthropic_base_url.clone(),
                auth_header: preset.auth_header.clone(),
                auth_scheme: preset.auth_scheme.clone(),
                api_key,
            },
        )?;
    }
    database::upsert_provider_policy(&st.db_path, &preset.recommended_policy)?;

    let route_report = if payload.apply_route.unwrap_or(false) {
        let target_app = payload
            .target_app
            .clone()
            .unwrap_or_else(|| "claude_desktop".into());
        let route_payload = RouteBuilderPayload {
            target_app: target_app.clone(),
            route_id: payload
                .route_id
                .clone()
                .unwrap_or_else(|| format!("{}-{}", target_app.replace('_', "-"), preset.id)),
            visible_model: payload.visible_model.clone().unwrap_or_else(|| {
                if target_app == "codex" {
                    preset.recommended_codex_model.clone()
                } else {
                    preset.recommended_claude_alias.clone()
                }
            }),
            display_name: payload
                .display_name
                .clone()
                .unwrap_or_else(|| format!("{} via {}", target_app.replace('_', " "), preset.name)),
            provider_id: preset.id.clone(),
            upstream_model: payload
                .upstream_model
                .clone()
                .unwrap_or_else(|| preset.upstream_model_example.clone()),
            tool_call_mode: None,
            conflict_strategy: Some("update".into()),
        };
        Some(apply_route_builder_inner(&st, &route_payload)?)
    } else {
        None
    };
    let providers = database::list_providers(&st.db_path)?;
    let provider = providers
        .iter()
        .find(|provider| provider.id == preset.id)
        .cloned()
        .ok_or_else(|| format!("Provider '{}' not found after apply", preset.id))?;
    Ok(ProviderWizardApplyReport {
        provider,
        providers,
        policies: database::list_provider_policies(&st.db_path)?,
        route_report,
    })
}

#[tauri::command]
pub fn get_unified_diagnostics(
    st: State<'_, AppState>,
) -> Result<UnifiedDiagnosticsReport, String> {
    unified_diagnostics_report(&st)
}

#[tauri::command]
pub fn export_unified_diagnostics_bundle(st: State<'_, AppState>) -> Result<String, String> {
    let report = unified_diagnostics_report(&st)?;
    let path = st.backups_dir.join(format!(
        "unified-diagnostics-{}.json",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::write(
        &path,
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn list_provider_presets() -> Vec<ProviderPreset> {
    built_in_provider_presets()
}

#[tauri::command]
pub fn apply_provider_preset(
    st: State<'_, AppState>,
    payload: ApplyProviderPresetPayload,
) -> Result<Vec<Provider>, String> {
    let preset = built_in_provider_presets()
        .into_iter()
        .find(|p| p.id == payload.preset_id)
        .ok_or_else(|| format!("Provider preset '{}' not found", payload.preset_id))?;
    let existing = database::list_providers(&st.db_path)?
        .into_iter()
        .find(|p| p.id == preset.id);
    let api_key = payload
        .api_key
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .or_else(|| existing.as_ref().and_then(|p| p.api_key.clone()));

    if let Some(existing) = existing {
        database::update_provider(
            &st.db_path,
            &UpdateProvider {
                id: preset.id.clone(),
                name: preset.name.clone(),
                base_url: preset.base_url.clone(),
                openai_base_url: Some(preset.openai_base_url.clone()),
                anthropic_base_url: preset.anthropic_base_url.clone(),
                auth_header: preset.auth_header.clone(),
                auth_scheme: preset.auth_scheme.clone(),
                api_key,
                enabled: existing.enabled,
            },
        )?;
    } else {
        database::create_provider(
            &st.db_path,
            &CreateProvider {
                id: preset.id.clone(),
                name: preset.name.clone(),
                base_url: preset.base_url.clone(),
                openai_base_url: Some(preset.openai_base_url.clone()),
                anthropic_base_url: preset.anthropic_base_url.clone(),
                auth_header: preset.auth_header.clone(),
                auth_scheme: preset.auth_scheme.clone(),
                api_key,
            },
        )?;
    }
    database::upsert_provider_policy(&st.db_path, &preset.recommended_policy)?;
    database::list_providers(&st.db_path)
}

fn unified_diagnostics_report(
    st: &State<'_, AppState>,
) -> Result<UnifiedDiagnosticsReport, String> {
    let providers = database::list_providers(&st.db_path)?;
    let routes = database::list_routes(&st.db_path)?;
    let codex_routes = database::list_codex_routes(&st.db_path)?;
    let logs = database::list_logs(&st.db_path, 200)?;
    let failed = database::list_failed_request_snapshots(&st.db_path, 100)?;
    let route_diagnostics = gateway::route_diagnostics(&st.db_path).unwrap_or_default();
    let codex_diagnostics = codex_gateway::route_diagnostics(&st.db_path).unwrap_or_default();
    let runtime = runtime_source_report_for_path(
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|e| format!("unknown: {e}")),
    );
    let desktop = desktop_binding::inspect(&dirs::home_dir().ok_or("no home")?).ok();
    let claude_code = claude_code_binding::inspect(&dirs::home_dir().ok_or("no home")?).ok();
    let codex_status = codex_gateway::status(st).unwrap_or_default();
    let codex_binding = codex_binding::inspect(&dirs::home_dir().ok_or("no home")?).ok();
    let codex_pp_install = codex_pp::detect();
    let codex_pp_health = codex_pp::health();
    let failure_clusters = failure_clusters_from_snapshots(&failed);

    let sections = vec![
        claude_desktop_section(desktop.as_ref(), &routes, &route_diagnostics, &logs),
        claude_code_section(claude_code.as_ref(), &route_diagnostics),
        codex_gateway_section(
            &codex_status,
            codex_binding.as_ref(),
            &codex_routes,
            &codex_diagnostics,
        ),
        codex_pp_section(&codex_pp_install, &codex_pp_health),
        providers_section(
            &providers,
            &route_diagnostics,
            &codex_diagnostics,
            &failure_clusters,
        ),
        install_runtime_section(&runtime),
    ];
    let score = if sections.is_empty() {
        0
    } else {
        (sections.iter().map(|s| s.score as u16).sum::<u16>() / sections.len() as u16) as u8
    };
    let status = overall_status(&sections);
    Ok(UnifiedDiagnosticsReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        summary: format!(
            "{} sections checked, {} issue clusters found.",
            sections.len(),
            failure_clusters.len()
        ),
        status,
        score,
        sections,
        failure_clusters,
    })
}

fn metric(label: &str, value: impl Into<String>, status: &str) -> DiagnosticsMetric {
    DiagnosticsMetric {
        label: label.into(),
        value: value.into(),
        status: status.into(),
    }
}

fn action(id: &str, label: &str, target: &str, severity: &str, detail: &str) -> DiagnosticsAction {
    DiagnosticsAction {
        id: id.into(),
        label: label.into(),
        target: target.into(),
        severity: severity.into(),
        detail: detail.into(),
    }
}

fn section(
    id: &str,
    title: &str,
    status: &str,
    score: u8,
    summary: impl Into<String>,
    metrics: Vec<DiagnosticsMetric>,
    actions: Vec<DiagnosticsAction>,
) -> DiagnosticsSection {
    DiagnosticsSection {
        id: id.into(),
        title: title.into(),
        status: status.into(),
        score,
        summary: summary.into(),
        metrics,
        actions,
    }
}

fn claude_desktop_section(
    desktop: Option<&desktop_binding::DesktopInfo>,
    routes: &[ModelRoute],
    route_diagnostics: &[gateway::RouteCompatibilityDiagnostic],
    logs: &[RequestLog],
) -> DiagnosticsSection {
    let binding_ok = desktop.map(|d| d.managed).unwrap_or(false);
    let route_count = routes.iter().filter(|r| r.enabled).count();
    let recent_failures = logs
        .iter()
        .filter(|l| l.status_code.map(|c| c >= 400).unwrap_or(false))
        .count();
    let unsafe_routes = route_diagnostics
        .iter()
        .filter(|d| !d.strategy.direct_provider_safe && d.strategy.gateway_route_recommended)
        .count();
    let mut actions = Vec::new();
    if !binding_ok {
        actions.push(action(
            "apply_claude_binding",
            "Apply Claude Desktop binding",
            "claude",
            "attention",
            "Claude Desktop is not currently managed by Gateway Switch.",
        ));
    }
    if route_count == 0 {
        actions.push(action(
            "create_claude_route",
            "Create a Claude route",
            "claude",
            "critical",
            "No enabled Claude route is configured.",
        ));
    }
    if unsafe_routes > 0 {
        actions.push(action(
            "review_route_strategy",
            "Review route strategy",
            "claude",
            "attention",
            "Some routes require Gateway compatibility transformations.",
        ));
    }
    let score = score_from_issues(&[
        (!binding_ok, 25),
        (route_count == 0, 35),
        (recent_failures > 0, 15),
    ]);
    section(
        "claude_desktop",
        "Claude Desktop",
        status_from_score(score),
        score,
        if binding_ok {
            "Claude Desktop binding is present."
        } else {
            "Claude Desktop binding needs attention."
        },
        vec![
            metric(
                "Binding",
                if binding_ok { "managed" } else { "not managed" },
                if binding_ok { "healthy" } else { "attention" },
            ),
            metric(
                "Enabled routes",
                route_count.to_string(),
                if route_count > 0 {
                    "healthy"
                } else {
                    "critical"
                },
            ),
            metric(
                "Recent failures",
                recent_failures.to_string(),
                if recent_failures == 0 {
                    "healthy"
                } else {
                    "attention"
                },
            ),
        ],
        actions,
    )
}

fn claude_code_section(
    info: Option<&ClaudeCodeInfo>,
    route_diagnostics: &[gateway::RouteCompatibilityDiagnostic],
) -> DiagnosticsSection {
    let managed = info.map(|i| i.managed).unwrap_or(false);
    let unsafe_route_count = route_diagnostics
        .iter()
        .filter(|d| !d.strategy.direct_provider_safe)
        .count();
    let mut actions = Vec::new();
    if !managed {
        actions.push(action(
            "repair_claude_code_gateway",
            "Repair Claude Code to Gateway Route",
            "claudeCode",
            "attention",
            "Gateway Route is the safest default for providers with uncertain Anthropic compatibility.",
        ));
    }
    let score = score_from_issues(&[(!managed, 30), (unsafe_route_count > 0, 10)]);
    section(
        "claude_code",
        "Claude Code",
        status_from_score(score),
        score,
        if managed {
            "Claude Code is bound through Gateway Switch."
        } else {
            "Claude Code may be using an unmanaged or direct provider binding."
        },
        vec![
            metric(
                "Binding",
                if managed { "managed" } else { "not managed" },
                if managed { "healthy" } else { "attention" },
            ),
            metric(
                "Gateway-recommended routes",
                unsafe_route_count.to_string(),
                if unsafe_route_count == 0 {
                    "healthy"
                } else {
                    "attention"
                },
            ),
        ],
        actions,
    )
}

fn codex_gateway_section(
    status: &GatewayStatus,
    binding: Option<&CodexBindingInfo>,
    routes: &[CodexRoute],
    diagnostics: &[codex_gateway::CodexRouteDiagnostic],
) -> DiagnosticsSection {
    let running = status.running;
    let managed = binding.map(|b| b.managed).unwrap_or(false);
    let enabled_routes = routes.iter().filter(|r| r.enabled).count();
    let strict_routes = diagnostics
        .iter()
        .filter(|d| d.strategy.codex_strict_tool_calls)
        .count();
    let mut actions = Vec::new();
    if !running {
        actions.push(action(
            "start_codex_gateway",
            "Start Codex Gateway",
            "codex",
            "critical",
            "Codex routes require the local Responses gateway to be running.",
        ));
    }
    if !managed {
        actions.push(action(
            "apply_codex_binding",
            "Apply Codex binding",
            "codex",
            "attention",
            "Codex config is not currently managed by Gateway Switch.",
        ));
    }
    let score = score_from_issues(&[(!running, 35), (!managed, 20), (enabled_routes == 0, 25)]);
    section(
        "codex_gateway",
        "Codex Gateway",
        status_from_score(score),
        score,
        if running {
            "Codex Gateway is available."
        } else {
            "Codex Gateway is not running."
        },
        vec![
            metric(
                "Gateway",
                if running { "running" } else { "stopped" },
                if running { "healthy" } else { "critical" },
            ),
            metric(
                "Binding",
                if managed { "managed" } else { "not managed" },
                if managed { "healthy" } else { "attention" },
            ),
            metric(
                "Enabled routes",
                enabled_routes.to_string(),
                if enabled_routes > 0 {
                    "healthy"
                } else {
                    "critical"
                },
            ),
            metric("Strict tool routes", strict_routes.to_string(), "info"),
        ],
        actions,
    )
}

fn codex_pp_section(
    install: &codex_pp::CodexPpInstall,
    health: &codex_pp::CodexPpHealth,
) -> DiagnosticsSection {
    let installed = install.installed;
    let failed_checks = health
        .checks
        .iter()
        .filter(|check| check.status == "failed" || check.status == "error")
        .count();
    let review_checks = health
        .checks
        .iter()
        .filter(|check| check.status == "review")
        .count();
    let mut actions = Vec::new();
    if !installed {
        actions.push(action(
            "install_codex_pp",
            "Install Codex++",
            "codex",
            "attention",
            "Codex++ is not installed in the managed runtime.",
        ));
    }
    if failed_checks > 0 || review_checks > 0 {
        actions.push(action(
            "review_codex_pp_health",
            "Review Codex++ health",
            "codex",
            "attention",
            "One or more Codex++ checks need review.",
        ));
    }
    let score = score_from_issues(&[
        (!installed, 35),
        (failed_checks > 0, 25),
        (review_checks > 0, 10),
    ]);
    section(
        "codex_pp",
        "Codex++",
        status_from_score(score),
        score,
        if installed {
            "Codex++ runtime is detected."
        } else {
            "Codex++ runtime is not installed."
        },
        vec![
            metric(
                "Installed",
                installed.to_string(),
                if installed { "healthy" } else { "attention" },
            ),
            metric(
                "Failed checks",
                failed_checks.to_string(),
                if failed_checks == 0 {
                    "healthy"
                } else {
                    "critical"
                },
            ),
            metric(
                "Review checks",
                review_checks.to_string(),
                if review_checks == 0 {
                    "healthy"
                } else {
                    "attention"
                },
            ),
        ],
        actions,
    )
}

fn providers_section(
    providers: &[Provider],
    route_diagnostics: &[gateway::RouteCompatibilityDiagnostic],
    codex_diagnostics: &[codex_gateway::CodexRouteDiagnostic],
    failure_clusters: &[FailureCluster],
) -> DiagnosticsSection {
    let enabled = providers.iter().filter(|p| p.enabled).count();
    let no_key = providers
        .iter()
        .filter(|p| p.enabled && p.api_key.as_deref().unwrap_or_default().is_empty())
        .count();
    let gateway_recommended = route_diagnostics
        .iter()
        .filter(|d| d.strategy.gateway_route_recommended)
        .count()
        + codex_diagnostics
            .iter()
            .filter(|d| d.strategy.gateway_route_recommended)
            .count();
    let provider_failures = failure_clusters
        .iter()
        .filter(|c| c.provider_id.is_some())
        .count();
    let mut actions = Vec::new();
    if enabled == 0 {
        actions.push(action(
            "apply_provider_preset",
            "Apply a Provider Preset",
            "providers",
            "critical",
            "No enabled provider is configured.",
        ));
    }
    if no_key > 0 {
        actions.push(action(
            "fill_provider_keys",
            "Add provider API keys",
            "providers",
            "attention",
            "Some enabled providers are missing API keys.",
        ));
    }
    if provider_failures > 0 {
        actions.push(action(
            "review_failure_clusters",
            "Review provider failure clusters",
            "logs",
            "attention",
            "Recent failed requests indicate provider-specific issues.",
        ));
    }
    let score = score_from_issues(&[
        (enabled == 0, 35),
        (no_key > 0, 20),
        (provider_failures > 0, 15),
    ]);
    section(
        "providers",
        "Providers",
        status_from_score(score),
        score,
        format!("{enabled} enabled providers, {provider_failures} failure clusters."),
        vec![
            metric(
                "Enabled providers",
                enabled.to_string(),
                if enabled > 0 { "healthy" } else { "critical" },
            ),
            metric(
                "Missing API keys",
                no_key.to_string(),
                if no_key == 0 { "healthy" } else { "attention" },
            ),
            metric(
                "Gateway-recommended routes",
                gateway_recommended.to_string(),
                "info",
            ),
        ],
        actions,
    )
}

fn install_runtime_section(runtime: &RuntimeSourceReport) -> DiagnosticsSection {
    let stable = runtime.severity == "ok";
    let score = score_from_issues(&[(!stable, 25)]);
    let actions = if stable {
        Vec::new()
    } else {
        vec![action(
            "safe_install",
            "Install under /Applications",
            "settings",
            "attention",
            &runtime.recommendation,
        )]
    };
    section(
        "install_runtime",
        "Install / Runtime",
        status_from_score(score),
        score,
        runtime.summary.clone(),
        vec![
            metric("Source", runtime.bundle_path.clone(), &runtime.severity),
            metric(
                "Applications",
                runtime.is_applications.to_string(),
                if runtime.is_applications {
                    "healthy"
                } else {
                    "attention"
                },
            ),
            metric(
                "DMG",
                runtime.is_dmg_volume.to_string(),
                if runtime.is_dmg_volume {
                    "attention"
                } else {
                    "healthy"
                },
            ),
        ],
        actions,
    )
}

fn failure_clusters_from_snapshots(
    snapshots: &[FailedRequestDiagnosticCandidate],
) -> Vec<FailureCluster> {
    let mut clusters: HashMap<String, FailureCluster> = HashMap::new();
    for snapshot in snapshots {
        let key = format!(
            "{}|{}|{}",
            snapshot.provider_id.as_deref().unwrap_or("unknown"),
            snapshot.surface,
            snapshot
                .status_code
                .map(|v| v.to_string())
                .unwrap_or_else(|| "network".into())
        );
        let entry = clusters
            .entry(key.clone())
            .or_insert_with(|| FailureCluster {
                key: key.clone(),
                provider_id: snapshot.provider_id.clone(),
                surface: snapshot.surface.clone(),
                status_code: snapshot.status_code,
                count: 0,
                sample_error: snapshot.error_summary.clone(),
                recommendation: failure_recommendation(
                    snapshot.status_code,
                    snapshot.error_summary.as_deref(),
                ),
            });
        entry.count += 1;
        if entry.sample_error.is_none() {
            entry.sample_error = snapshot.error_summary.clone();
        }
    }
    let mut values = clusters.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| b.count.cmp(&a.count));
    values
}

fn failure_recommendation(status_code: Option<u16>, error_summary: Option<&str>) -> String {
    let lower = error_summary.unwrap_or_default().to_ascii_lowercase();
    if lower.contains("messages.role") || lower.contains("system") {
        "Enable system_to_user and prefer Gateway Route for this provider.".into()
    } else if lower.contains("tool") && lower.contains("role") {
        "Enable tool_to_user or disable_tools for this provider.".into()
    } else if lower.contains("reasoning") || lower.contains("thinking") {
        "Enable strip_unsupported_params and codex_strip_reasoning.".into()
    } else {
        match status_code {
            Some(400) => "Review provider protocol compatibility and payload shape.".into(),
            Some(413) => "Reduce attachment size or split the request.".into(),
            Some(429) => {
                "Provider rate limit or quota reached; retry later or switch provider.".into()
            }
            Some(500..=599) => {
                "Provider is unhealthy or the upstream gateway is returning server errors.".into()
            }
            _ => "Network or unknown failure; inspect the redacted replay preview.".into(),
        }
    }
}

fn score_from_issues(issues: &[(bool, u8)]) -> u8 {
    let penalty = issues
        .iter()
        .filter(|(active, _)| *active)
        .map(|(_, weight)| *weight as u16)
        .sum::<u16>();
    100u16.saturating_sub(penalty).min(100) as u8
}

fn status_from_score(score: u8) -> &'static str {
    match score {
        85..=100 => "healthy",
        65..=84 => "attention",
        40..=64 => "degraded",
        _ => "critical",
    }
}

fn overall_status(sections: &[DiagnosticsSection]) -> String {
    if sections.iter().any(|s| s.status == "critical") {
        "critical"
    } else if sections.iter().any(|s| s.status == "degraded") {
        "degraded"
    } else if sections.iter().any(|s| s.status == "attention") {
        "attention"
    } else {
        "healthy"
    }
    .into()
}

fn built_in_provider_presets() -> Vec<ProviderPreset> {
    vec![
        provider_preset(
            "openrouter",
            "OpenRouter",
            "Multi-provider OpenAI-compatible router with optional Anthropic-style model IDs.",
            "https://openrouter.ai/api/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "claude-sonnet-openrouter",
            "codex-openrouter",
            "anthropic/claude-sonnet-4",
            preset_policy("openrouter", None, None, None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(false)),
            vec!["Prefer Gateway Route unless the specific OpenRouter model is known Anthropic-compatible.".into()],
        ),
        provider_preset(
            "volcengine",
            "Volcengine Ark DeepSeek",
            "Volcengine Ark DeepSeek coding endpoints usually accept only user/assistant chat roles.",
            "https://ark.cn-beijing.volces.com/api/v3",
            None,
            "Authorization",
            Some("Bearer"),
            "deepseek-v4-pro",
            "codex-volcengine",
            "deepseek-v4-pro",
            preset_policy("volcengine", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(true)),
            vec!["Do not use Claude Code Direct Provider for this preset; use Gateway Route.".into()],
        ),
        provider_preset(
            "deepseek",
            "DeepSeek Official",
            "DeepSeek official OpenAI-compatible chat endpoint.",
            "https://api.deepseek.com/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "deepseek-chat",
            "codex-deepseek",
            "deepseek-chat",
            preset_policy("deepseek", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(true)),
            vec!["Reasoning parameters may need stripping for non-reasoner models.".into()],
        ),
        provider_preset(
            "moonshot",
            "Moonshot Kimi",
            "Moonshot OpenAI-compatible chat endpoint for Kimi models.",
            "https://api.moonshot.cn/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "kimi-k2",
            "codex-kimi",
            "kimi-k2-0711-preview",
            preset_policy("moonshot", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(true)),
            vec!["Use Gateway Route for Claude clients; Direct Provider is not assumed Anthropic-compatible.".into()],
        ),
        provider_preset(
            "qwen",
            "Qwen DashScope",
            "Alibaba DashScope OpenAI-compatible endpoint for Qwen models.",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "qwen-coder",
            "codex-qwen",
            "qwen3-coder-plus",
            preset_policy("qwen", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(true)),
            vec!["Tool-call quality varies by model; keep Codex strict tool mode available.".into()],
        ),
        provider_preset(
            "xiaomi",
            "Xiaomi MiMo",
            "Xiaomi MiMo OpenAI-compatible endpoint; often best through Chat fallback.",
            "https://token-plan-sgp.xiaomimimo.com/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "mimo-chat",
            "codex-mimo",
            "mimo-v2.5",
            preset_policy("xiaomi", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(true), Some(false), Some(true)),
            vec!["Singapore endpoint latency may be higher from mainland China networks.".into()],
        ),
        provider_preset(
            "anthropic",
            "Standard Anthropic-Compatible",
            "Generic provider that genuinely supports Anthropic Messages API.",
            "https://api.anthropic.com/v1",
            Some("https://api.anthropic.com/v1"),
            "x-api-key",
            None,
            "claude-sonnet",
            "codex-anthropic",
            "claude-sonnet-4-5",
            preset_policy("anthropic", Some(false), Some(false), Some(false), Some(false), Some(true), Some(false), Some(false), Some(false), Some(false)),
            vec!["Only use this preset for endpoints that truly implement Anthropic Messages API.".into()],
        ),
        provider_preset(
            "openai-chat",
            "OpenAI-Compatible Chat",
            "Generic OpenAI Chat Completions provider.",
            "https://api.openai.com/v1",
            None,
            "Authorization",
            Some("Bearer"),
            "openai-chat",
            "codex-openai-chat",
            "gpt-4o",
            preset_policy("openai-chat", Some(true), Some(true), None, Some(true), Some(false), Some(true), Some(false), Some(false), Some(false)),
            vec!["Direct Provider for Claude Code is not safe unless an Anthropic Base URL exists.".into()],
        ),
    ]
}

fn provider_preset(
    id: &str,
    name: &str,
    description: &str,
    openai_base_url: &str,
    anthropic_base_url: Option<&str>,
    auth_header: &str,
    auth_scheme: Option<&str>,
    recommended_claude_alias: &str,
    recommended_codex_model: &str,
    upstream_model_example: &str,
    recommended_policy: ProviderCompatibilityPolicy,
    warnings: Vec<String>,
) -> ProviderPreset {
    ProviderPreset {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        base_url: openai_base_url.into(),
        openai_base_url: openai_base_url.into(),
        anthropic_base_url: anthropic_base_url.map(Into::into),
        auth_header: auth_header.into(),
        auth_scheme: auth_scheme.map(Into::into),
        recommended_claude_alias: recommended_claude_alias.into(),
        recommended_codex_model: recommended_codex_model.into(),
        upstream_model_example: upstream_model_example.into(),
        recommended_policy,
        warnings,
    }
}

#[allow(clippy::too_many_arguments)]
fn preset_policy(
    provider_id: &str,
    system_to_user: Option<bool>,
    tool_to_user: Option<bool>,
    disable_tools: Option<bool>,
    strip_unsupported_params: Option<bool>,
    direct_provider_safe: Option<bool>,
    gateway_route_recommended: Option<bool>,
    codex_disable_responses: Option<bool>,
    codex_strict_tool_calls: Option<bool>,
    codex_strip_reasoning: Option<bool>,
) -> ProviderCompatibilityPolicy {
    ProviderCompatibilityPolicy {
        provider_id: provider_id.into(),
        system_to_user,
        tool_to_user,
        disable_tools,
        strip_unsupported_params,
        direct_provider_safe,
        gateway_route_recommended,
        codex_disable_responses,
        codex_strict_tool_calls,
        codex_strip_reasoning,
        notes: Some("Applied from built-in provider preset".into()),
        updated_by: "preset".into(),
        updated_at: None,
    }
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
            ) && payload.force_direct_provider != Some(true)
            {
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
            message: format!("Gateway is not running or not reachable at {url}: {e}"),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
    }
}

async fn probe_codex_gateway_health(profile: &GatewayProfile) -> HealthStatus {
    let url = format!(
        "http://{}:{}/health",
        profile.listen_host, profile.listen_port
    );
    let start = Instant::now();
    match reqwest::get(&url).await {
        Ok(r) => HealthStatus {
            target: "codex-gateway".into(),
            ok: r.status().is_success(),
            message: format!("HTTP {}", r.status()),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
        Err(e) => HealthStatus {
            target: "codex-gateway".into(),
            ok: false,
            message: format!("Codex Gateway is not running or not reachable at {url}: {e}"),
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

    #[test]
    fn failure_clusters_map_role_errors_to_strategy_recommendations() {
        let snapshots = vec![FailedRequestDiagnosticCandidate {
            request_id: "r1".into(),
            surface: "claude_messages".into(),
            claude_alias: Some("claude".into()),
            provider_id: Some("volcengine".into()),
            upstream_model: Some("deepseek".into()),
            status_code: Some(400),
            error_summary: Some("messages.role system is not valid".into()),
            redaction_summary: "none".into(),
            created_at: None,
        }];

        let clusters = failure_clusters_from_snapshots(&snapshots);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, 1);
        assert!(clusters[0].recommendation.contains("system_to_user"));
    }

    #[test]
    fn provider_presets_include_safe_defaults_for_volcengine() {
        let presets = built_in_provider_presets();
        let volcengine = presets.iter().find(|p| p.id == "volcengine").unwrap();
        assert_eq!(volcengine.recommended_policy.system_to_user, Some(true));
        assert_eq!(volcengine.recommended_policy.tool_to_user, Some(true));
        assert_eq!(
            volcengine.recommended_policy.direct_provider_safe,
            Some(false)
        );
        assert!(volcengine
            .warnings
            .iter()
            .any(|warning| warning.contains("Gateway Route")));
    }

    #[test]
    fn status_and_score_helpers_prioritize_critical_sections() {
        let sections = vec![
            section("ok", "OK", "healthy", 100, "ok", vec![], vec![]),
            section("bad", "Bad", "critical", 20, "bad", vec![], vec![]),
        ];
        assert_eq!(status_from_score(90), "healthy");
        assert_eq!(status_from_score(45), "degraded");
        assert_eq!(overall_status(&sections), "critical");
        assert_eq!(score_from_issues(&[(true, 25), (false, 50)]), 75);
    }
}
