use std::{fs, path::PathBuf, time::Instant};

use crate::{
    claude_code_binding, codex_binding, codex_gateway, compatibility, database, desktop_binding,
    gateway,
    models::{
        AppSettings, CodexBindingInfo, CodexRoute, ColdStartCapability, ColdStartReport,
        ColdStartStep, GatewayProfile, ModelRoute, Provider,
    },
    settings,
    state::{AppState, GatewayStatus},
};

const SECURITY_DETAIL: &str = "Third-party routing may expose prompts, file contents, tool results, and code to upstream providers; keep official providers as fallback for critical/private tasks";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Check,
    Repair,
}

impl RunMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Repair => "repair",
        }
    }

    fn allows_side_effects(self) -> bool {
        matches!(self, Self::Repair)
    }

    fn persists_report(self) -> bool {
        matches!(self, Self::Repair)
    }
}

pub async fn run_coldstart_checks(st: &AppState, mode: RunMode) -> Result<ColdStartReport, String> {
    let mut collector = ColdstartCollector::default();
    collector.debug(
        "system",
        &format!("Coldstart run started, mode={}", mode.as_str()),
    );
    collector.log(
        "environment",
        "Environment discovery",
        "system",
        "ok",
        "Loaded local app state, settings path, database path, and binding targets",
    );

    let mut ctx = ColdstartContext::load(st)?;
    collector.debug(
        "system",
        &format!(
            "Context loaded: providers={}, claude_routes={}, codex_routes={}, home={}",
            ctx.providers.len(),
            ctx.enabled_routes.len(),
            ctx.enabled_codex_routes.len(),
            ctx.home.display()
        ),
    );
    collector.log(
        "inventory",
        "Provider and route inventory",
        "gateway",
        "ok",
        &format!(
            "{} providers, {} Claude routes, {} Codex routes",
            ctx.providers.len(),
            ctx.enabled_routes.len(),
            ctx.enabled_codex_routes.len()
        ),
    );

    record_binding_capabilities(&ctx, &mut collector);
    inspect_claude_gateway(st, &mut ctx, &mut collector, mode).await?;
    inspect_codex_gateway(st, &mut ctx, &mut collector, mode).await?;
    record_inventory_capabilities(&ctx, &mut collector);
    record_security_and_settings_guidance(&ctx, &mut collector);

    collector.finish();
    collector.log(
        "report",
        "Generate coldstart report",
        "system",
        "ok",
        "Compiled UI report, safe-fix results, manual remediation list, and security notes",
    );

    let mut report = ReportAssembler::build(&ctx, mode, collector);
    if mode.persists_report() {
        let report_path = write_coldstart_report(st, &report)?;
        println!("[coldstart][system][debug] Persisted coldstart report to {report_path}");
        report.report_path = Some(report_path);
    }

    println!(
        "[coldstart][system][debug] Coldstart run completed, mode={}, overall_score={}, report_path={}",
        report.mode,
        report.overall_score,
        report.report_path.as_deref().unwrap_or("none")
    );
    Ok(report)
}

struct ColdstartContext {
    settings: AppSettings,
    claude_profile: GatewayProfile,
    codex_profile: GatewayProfile,
    providers: Vec<Provider>,
    enabled_routes: Vec<ModelRoute>,
    enabled_codex_routes: Vec<CodexRoute>,
    home: PathBuf,
    desktop_info: desktop_binding::DesktopInfo,
    claude_code_info: crate::models::ClaudeCodeInfo,
    codex_info: CodexBindingInfo,
}

impl ColdstartContext {
    fn load(st: &AppState) -> Result<Self, String> {
        let settings = settings::load(&st.settings_path)?;
        let claude_profile = database::get_profile(&st.db_path)?;
        let codex_profile = database::get_codex_profile(&st.db_path)?;
        let providers = database::list_providers(&st.db_path)?;
        let routes = database::list_routes(&st.db_path)?;
        let codex_routes = database::list_codex_routes(&st.db_path)?;
        let enabled_routes = routes.into_iter().filter(|route| route.enabled).collect();
        let enabled_codex_routes = codex_routes
            .into_iter()
            .filter(|route| route.enabled)
            .collect();
        let home = dirs::home_dir().ok_or("no home")?;
        let desktop_info = desktop_binding::inspect(&home)?;
        let claude_code_info = claude_code_binding::inspect(&home)?;
        let codex_info = codex_binding::inspect(&home)?;

        Ok(Self {
            settings,
            claude_profile,
            codex_profile,
            providers,
            enabled_routes,
            enabled_codex_routes,
            home,
            desktop_info,
            claude_code_info,
            codex_info,
        })
    }
}

#[derive(Default)]
struct ColdstartCollector {
    steps: Vec<ColdStartStep>,
    capabilities: Vec<ColdStartCapability>,
    auto_fixes_applied: Vec<String>,
    manual_fixes_required: Vec<String>,
}

impl ColdstartCollector {
    fn log(&mut self, id: &str, label: &str, target: &str, status: &str, detail: &str) {
        println!("[coldstart][{target}][{status}] {label}: {detail}");
        self.steps.push(ColdStartStep {
            id: id.into(),
            label: label.into(),
            target: target.into(),
            status: status.into(),
            detail: compatibility::redact_log_summary(detail),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    fn capability(&mut self, name: &str, target: &str, status: &str, detail: &str) {
        println!("[coldstart][capability][{target}][{status}] {name}: {detail}");
        self.capabilities.push(ColdStartCapability {
            name: name.into(),
            target: target.into(),
            status: status.into(),
            detail: compatibility::redact_log_summary(detail),
        });
    }

    fn debug(&self, target: &str, detail: &str) {
        println!(
            "[coldstart][{target}][debug] {}",
            compatibility::redact_log_summary(detail)
        );
    }

    fn auto_fix(&mut self, detail: impl Into<String>) {
        self.auto_fixes_applied.push(detail.into());
    }

    fn manual_fix(&mut self, detail: impl Into<String>) {
        self.manual_fixes_required.push(detail.into());
    }

    fn finish(&mut self) {
        self.manual_fixes_required.sort();
        self.manual_fixes_required.dedup();
    }
}

struct GatewaySpec {
    target: &'static str,
    gateway_name: &'static str,
    process_capability: &'static str,
    start_id: &'static str,
    start_done_id: &'static str,
    start_failed_id: &'static str,
    start_label: &'static str,
    start_result_label: &'static str,
    start_failed_label: &'static str,
    start_detail: &'static str,
    status: fn(&AppState) -> Result<GatewayStatus, String>,
    start: fn(&AppState) -> Result<String, String>,
}

async fn inspect_claude_gateway(
    st: &AppState,
    ctx: &mut ColdstartContext,
    collector: &mut ColdstartCollector,
    mode: RunMode,
) -> Result<(), String> {
    let spec = GatewaySpec {
        target: "Claude",
        gateway_name: "Claude Gateway",
        process_capability: "Claude Gateway process",
        start_id: "claude_gateway_start",
        start_done_id: "claude_gateway_start_done",
        start_failed_id: "claude_gateway_start_failed",
        start_label: "Start Claude Gateway",
        start_result_label: "Claude Gateway start result",
        start_failed_label: "Claude Gateway start failed",
        start_detail: "Gateway was stopped; attempting safe start before Desktop validation",
        status: gateway::status,
        start: gateway::start,
    };
    inspect_gateway_process(st, collector, mode, &spec, !ctx.enabled_routes.is_empty())?;
    repair_claude_binding(ctx, collector, mode);
    record_health(
        collector,
        "claude_health",
        "Claude Gateway health check",
        "Claude health endpoint",
        "Claude",
        &ctx.claude_profile,
    )
    .await;
    Ok(())
}

async fn inspect_codex_gateway(
    st: &AppState,
    ctx: &mut ColdstartContext,
    collector: &mut ColdstartCollector,
    mode: RunMode,
) -> Result<(), String> {
    let spec = GatewaySpec {
        target: "Codex",
        gateway_name: "Codex Gateway",
        process_capability: "Codex Gateway process",
        start_id: "codex_gateway_start",
        start_done_id: "codex_gateway_start_done",
        start_failed_id: "codex_gateway_start_failed",
        start_label: "Start Codex Gateway",
        start_result_label: "Codex Gateway start result",
        start_failed_label: "Codex Gateway start failed",
        start_detail: "Codex Gateway was stopped; attempting safe start before config validation",
        status: codex_gateway::status,
        start: codex_gateway::start,
    };
    inspect_gateway_process(
        st,
        collector,
        mode,
        &spec,
        ctx.codex_info.managed || !ctx.enabled_codex_routes.is_empty(),
    )?;
    repair_codex_binding(ctx, collector, mode);
    record_health(
        collector,
        "codex_health",
        "Codex Gateway health check",
        "Codex health endpoint",
        "Codex",
        &ctx.codex_profile,
    )
    .await;
    Ok(())
}

fn inspect_gateway_process(
    st: &AppState,
    collector: &mut ColdstartCollector,
    mode: RunMode,
    spec: &GatewaySpec,
    can_start: bool,
) -> Result<GatewayStatus, String> {
    let mut status = (spec.status)(st)?;
    collector.debug(
        spec.target,
        &format!(
            "{} status before repair: running={}, status={}, error={}, can_start={}, mode={}",
            spec.gateway_name,
            status.running,
            status.status,
            status.error.as_deref().unwrap_or("none"),
            can_start,
            mode.as_str()
        ),
    );
    if !status.running && mode.allows_side_effects() && can_start {
        collector.log(
            spec.start_id,
            spec.start_label,
            spec.target,
            "running",
            spec.start_detail,
        );
        match (spec.start)(st) {
            Ok(msg) => {
                collector.auto_fix(format!("{} start: {msg}", spec.gateway_name));
                collector.log(
                    spec.start_done_id,
                    spec.start_result_label,
                    spec.target,
                    "fixed",
                    &msg,
                );
            }
            Err(e) => {
                collector.manual_fix(format!("{} failed to start: {e}", spec.gateway_name));
                collector.log(
                    spec.start_failed_id,
                    spec.start_failed_label,
                    spec.target,
                    "error",
                    &e,
                );
            }
        }
        status = (spec.status)(st)?;
        collector.debug(
            spec.target,
            &format!(
                "{} status after start attempt: running={}, status={}, error={}",
                spec.gateway_name,
                status.running,
                status.status,
                status.error.as_deref().unwrap_or("none")
            ),
        );
    } else if !status.running {
        collector.debug(
            spec.target,
            &format!(
                "{} start skipped: allows_side_effects={}, can_start={}",
                spec.gateway_name,
                mode.allows_side_effects(),
                can_start
            ),
        );
    } else {
        collector.debug(
            spec.target,
            &format!("{} already running; start skipped", spec.gateway_name),
        );
    }

    collector.capability(
        spec.process_capability,
        spec.target,
        if status.running { "ok" } else { "warn" },
        &format!(
            "status={}, error={}",
            status.status,
            status.error.as_deref().unwrap_or("none")
        ),
    );
    Ok(status)
}

fn record_binding_capabilities(ctx: &ColdstartContext, collector: &mut ColdstartCollector) {
    collector.capability(
        "Claude Desktop config",
        "Claude",
        managed_status(ctx.desktop_info.managed),
        &binding_detail(
            &ctx.desktop_info.config_path,
            ctx.desktop_info.managed,
            ctx.desktop_info.base_url.as_deref(),
        ),
    );
    collector.capability(
        "Claude Code config",
        "Claude Code",
        managed_status(ctx.claude_code_info.managed),
        &binding_detail(
            &ctx.claude_code_info.config_path,
            ctx.claude_code_info.managed,
            ctx.claude_code_info.base_url.as_deref(),
        ),
    );
    collector.capability(
        "Codex config",
        "Codex",
        managed_status(ctx.codex_info.managed),
        &binding_detail(
            &ctx.codex_info.config_path,
            ctx.codex_info.managed,
            ctx.codex_info.base_url.as_deref(),
        ),
    );
}

fn repair_claude_binding(
    ctx: &mut ColdstartContext,
    collector: &mut ColdstartCollector,
    mode: RunMode,
) {
    collector.debug(
        "Claude",
        &format!(
            "Claude Desktop binding decision: managed={}, enabled_routes={}, mode={}",
            ctx.desktop_info.managed,
            ctx.enabled_routes.len(),
            mode.as_str()
        ),
    );
    if mode.allows_side_effects() && !ctx.desktop_info.managed && !ctx.enabled_routes.is_empty() {
        collector.log(
            "desktop_apply",
            "Apply Claude Desktop binding",
            "Claude",
            "running",
            "Desktop is not managed by Gateway Switch; creating backup and applying current enabled routes",
        );
        let models = desktop_binding::model_configs_from_routes(&ctx.enabled_routes);
        match desktop_binding::apply(
            &ctx.home,
            &desktop_binding::gateway_base_url(
                &ctx.claude_profile.listen_host,
                ctx.claude_profile.listen_port,
            ),
            "x-api-key",
            &ctx.claude_profile.auth_token,
            &models,
        ) {
            Ok(info) => {
                ctx.desktop_info = info;
                collector.auto_fix("Applied Claude Desktop Gateway Switch binding with backup");
                collector.log(
                    "desktop_apply_done",
                    "Claude Desktop binding applied",
                    "Claude",
                    "fixed",
                    "Desktop config now points to local Claude Gateway",
                );
            }
            Err(e) => {
                collector.manual_fix(format!("Claude Desktop binding failed: {e}"));
                collector.log(
                    "desktop_apply_failed",
                    "Claude Desktop binding failed",
                    "Claude",
                    "error",
                    &e,
                );
            }
        }
    } else if !ctx.desktop_info.managed && ctx.enabled_routes.is_empty() {
        collector.debug(
            "Claude",
            "Claude Desktop binding repair skipped: no enabled Claude routes",
        );
        collector.log(
            "desktop_unmanaged",
            "Claude Desktop binding check",
            "Claude",
            "warn",
            "Desktop is not managed by Gateway Switch; create at least one enabled Claude route before repair",
        );
    } else if !ctx.desktop_info.managed {
        collector.log(
            "desktop_unmanaged",
            "Claude Desktop binding check",
            "Claude",
            "warn",
            "Desktop is not managed by Gateway Switch; run repair to apply a safe backup-backed binding",
        );
    }
}

fn repair_codex_binding(
    ctx: &mut ColdstartContext,
    collector: &mut ColdstartCollector,
    mode: RunMode,
) {
    collector.debug(
        "Codex",
        &format!(
            "Codex binding decision: managed={}, enabled_routes={}, mode={}",
            ctx.codex_info.managed,
            ctx.enabled_codex_routes.len(),
            mode.as_str()
        ),
    );
    if mode.allows_side_effects() && !ctx.codex_info.managed {
        if let Some(route) = ctx.enabled_codex_routes.first() {
            collector.log(
                "codex_apply",
                "Apply Codex binding",
                "Codex",
                "running",
                "Codex is not managed by Gateway Switch; creating backup and applying current default route",
            );
            match codex_binding::apply(
                &ctx.home,
                &format!(
                    "http://{}:{}/v1",
                    ctx.codex_profile.listen_host, ctx.codex_profile.listen_port
                ),
                &ctx.codex_profile.auth_token,
                &route.codex_model,
            ) {
                Ok(info) => {
                    ctx.codex_info = info;
                    collector.auto_fix(format!(
                        "Applied Codex Gateway Switch binding for model {}",
                        route.codex_model
                    ));
                    collector.log(
                        "codex_apply_done",
                        "Codex binding applied",
                        "Codex",
                        "fixed",
                        "Codex config now points to local Responses Gateway",
                    );
                }
                Err(e) => {
                    collector.manual_fix(format!("Codex binding failed: {e}"));
                    collector.log(
                        "codex_apply_failed",
                        "Codex binding failed",
                        "Codex",
                        "error",
                        &e,
                    );
                }
            }
        } else {
            collector.manual_fix(
                "Create at least one enabled Codex route before automatic Codex binding",
            );
            collector.log(
                "codex_no_route",
                "Codex binding skipped",
                "Codex",
                "warn",
                "No enabled Codex route is available",
            );
        }
    } else if !ctx.codex_info.managed {
        collector.log(
            "codex_unmanaged",
            "Codex binding check",
            "Codex",
            "warn",
            "Codex is not managed by Gateway Switch; run repair to apply a backup-backed binding",
        );
    }
}

async fn record_health(
    collector: &mut ColdstartCollector,
    step_id: &str,
    step_label: &str,
    capability_name: &str,
    target: &str,
    profile: &GatewayProfile,
) {
    collector.debug(
        target,
        &format!(
            "Probing health endpoint: http://{}:{}/health",
            profile.listen_host, profile.listen_port
        ),
    );
    let health = local_health(&profile.listen_host, profile.listen_port).await;
    collector.debug(target, &format!("Health probe result: {health}"));
    collector.capability(capability_name, target, health_status(&health), &health);
    collector.log(step_id, step_label, target, health_status(&health), &health);
}

fn record_inventory_capabilities(ctx: &ColdstartContext, collector: &mut ColdstartCollector) {
    let enabled_providers = ctx
        .providers
        .iter()
        .filter(|provider| provider.enabled)
        .count();
    collector.debug(
        "Provider",
        &format!(
            "Inventory summary: enabled_providers={}, enabled_claude_routes={}, enabled_codex_routes={}",
            enabled_providers,
            ctx.enabled_routes.len(),
            ctx.enabled_codex_routes.len()
        ),
    );
    collector.capability(
        "Provider inventory",
        "Provider",
        if enabled_providers > 0 { "ok" } else { "error" },
        &format!("{enabled_providers} enabled providers"),
    );
    collector.capability(
        "Claude route inventory",
        "Claude",
        if ctx.enabled_routes.is_empty() {
            "warn"
        } else {
            "ok"
        },
        &format!("{} enabled Claude routes", ctx.enabled_routes.len()),
    );
    collector.capability(
        "Codex route inventory",
        "Codex",
        if ctx.enabled_codex_routes.is_empty() {
            "warn"
        } else {
            "ok"
        },
        &format!("{} enabled Codex routes", ctx.enabled_codex_routes.len()),
    );
}

fn record_security_and_settings_guidance(
    ctx: &ColdstartContext,
    collector: &mut ColdstartCollector,
) {
    collector.debug(
        "Security",
        &format!(
            "Settings guidance: auto_start_gateway={}, auto_takeover_desktop={}, desktop_managed={}",
            ctx.settings.auto_start_gateway,
            ctx.settings.auto_takeover_desktop,
            ctx.desktop_info.managed
        ),
    );
    collector.capability(
        "Third-party routing security",
        "Security",
        "warn",
        SECURITY_DETAIL,
    );
    collector.manual_fix("Review provider privacy policy and avoid sending sensitive repositories to untrusted third-party models");

    if !ctx.settings.auto_start_gateway {
        collector.manual_fix(
            "Enable Auto Start Gateway if Claude Desktop should work immediately after app launch",
        );
    }
    if !ctx.settings.auto_takeover_desktop && ctx.desktop_info.managed {
        collector.manual_fix("Enable Auto Takeover Desktop if Gateway Switch should re-assert Claude Desktop binding on every launch");
    }
}

struct ReportAssembler;

impl ReportAssembler {
    fn build(
        ctx: &ColdstartContext,
        mode: RunMode,
        collector: ColdstartCollector,
    ) -> ColdStartReport {
        let claude_score = score_for(&collector.capabilities, "Claude");
        let codex_score = score_for(&collector.capabilities, "Codex");
        let overall_score = score_overall(&collector.capabilities);
        let verdict = if overall_score >= 85 {
            "ready as daily gateway environment"
        } else if overall_score >= 70 {
            "usable but needs targeted fixes"
        } else {
            "not ready for unattended daily use"
        }
        .to_string();
        let most_important_fix = if !ctx.codex_info.managed {
            "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint"
        } else if !ctx.desktop_info.managed {
            "Bind Claude Desktop to Gateway Switch and verify the local /v1/messages health endpoint"
        } else {
            "Prove MCP/GitHub readiness inside Claude Desktop and Codex with real tool calls"
        }
        .to_string();

        println!(
            "[coldstart][system][debug] Report assembled: mode={}, claude_score={}, codex_score={}, overall_score={}, verdict={}",
            mode.as_str(),
            claude_score,
            codex_score,
            overall_score,
            verdict
        );

        ColdStartReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            mode: mode.as_str().into(),
            verdict,
            claude_score,
            codex_score,
            overall_score,
            biggest_risk: SECURITY_DETAIL.into(),
            most_important_fix,
            report_path: None,
            auto_fixes_applied: collector.auto_fixes_applied,
            manual_fixes_required: collector.manual_fixes_required,
            steps: collector.steps,
            capabilities: collector.capabilities,
        }
    }
}

fn managed_status(managed: bool) -> &'static str {
    if managed {
        "ok"
    } else {
        "warn"
    }
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
        Ok(resp) => format!(
            "{} in {}ms ({url})",
            resp.status(),
            start.elapsed().as_millis()
        ),
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
    let filtered: Vec<&ColdStartCapability> = capabilities
        .iter()
        .filter(|capability| capability.target == target)
        .collect();
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
    let points: usize = items
        .iter()
        .map(|capability| match capability.status.as_str() {
            "ok" | "fixed" => 100,
            "warn" | "running" => 55,
            "error" => 0,
            _ => 40,
        })
        .sum();
    (points / items.len()).min(100) as u8
}

fn write_coldstart_report(st: &AppState, report: &ColdStartReport) -> Result<String, String> {
    let dir = st.backups_dir.join("coldstart");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "coldstart-report-{}.md",
        chrono::Utc::now().timestamp_millis()
    ));
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
    out.push_str(&format!(
        "- Most important fix: {}\n\n",
        report.most_important_fix
    ));

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
    for capability in &report.capabilities {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            capability.target,
            capability.name,
            capability.status,
            capability.detail.replace('|', "\\|")
        ));
    }

    out.push_str("\n## Execution Log\n\n");
    out.push_str("| time | target | status | step | detail |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for step in &report.steps {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            step.timestamp,
            step.target,
            step.status,
            step.label,
            step.detail.replace('|', "\\|")
        ));
    }

    out.push_str("\n## Security Notes\n\n");
    out.push_str("- Reports are generated with Gateway Switch redaction helpers.\n");
    out.push_str("- Do not paste provider tokens, cookies, bearer headers, or private keys into support tickets.\n");
    out.push_str("- Keep official Claude/OpenAI providers available as fallback for critical private tasks.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ClaudeCodeInfo;

    fn capability(target: &str, status: &str) -> ColdStartCapability {
        ColdStartCapability {
            name: format!("{target} capability"),
            target: target.into(),
            status: status.into(),
            detail: "test detail".into(),
        }
    }

    fn test_context(desktop_managed: bool, codex_managed: bool) -> ColdstartContext {
        ColdstartContext {
            settings: AppSettings::default(),
            claude_profile: GatewayProfile {
                listen_host: "127.0.0.1".into(),
                listen_port: 3456,
                auth_token: "claude-token".into(),
            },
            codex_profile: GatewayProfile {
                listen_host: "127.0.0.1".into(),
                listen_port: 3457,
                auth_token: "codex-token".into(),
            },
            providers: Vec::new(),
            enabled_routes: Vec::new(),
            enabled_codex_routes: Vec::new(),
            home: PathBuf::new(),
            desktop_info: desktop_binding::DesktopInfo {
                config_path: "/tmp/claude.json".into(),
                config_exists: true,
                managed: desktop_managed,
                base_url: Some("http://127.0.0.1:3456".into()),
                auth_scheme: Some("x-api-key".into()),
                models: Vec::new(),
                backup_path: None,
            },
            claude_code_info: ClaudeCodeInfo {
                config_path: "/tmp/claude-code.json".into(),
                config_exists: true,
                managed: true,
                base_url: Some("http://127.0.0.1:3456".into()),
                model: None,
                auth_env: None,
                backup_path: None,
            },
            codex_info: CodexBindingInfo {
                config_path: "/tmp/codex.toml".into(),
                config_exists: true,
                managed: codex_managed,
                model_provider: Some("gateway-switch".into()),
                model: Some("gpt-5-codex".into()),
                base_url: Some("http://127.0.0.1:3457/v1".into()),
                backup_path: None,
            },
        }
    }

    #[test]
    fn run_mode_makes_side_effect_policy_explicit() {
        assert_eq!(RunMode::Check.as_str(), "check");
        assert!(!RunMode::Check.allows_side_effects());
        assert!(!RunMode::Check.persists_report());

        assert_eq!(RunMode::Repair.as_str(), "repair");
        assert!(RunMode::Repair.allows_side_effects());
        assert!(RunMode::Repair.persists_report());
    }

    #[test]
    fn score_for_uses_existing_status_weights() {
        let capabilities = vec![
            capability("Claude", "ok"),
            capability("Claude", "warn"),
            capability("Claude", "error"),
            capability("Codex", "fixed"),
        ];

        assert_eq!(score_for(&capabilities, "Claude"), 51);
        assert_eq!(score_for(&capabilities, "Codex"), 100);
        assert_eq!(score_for(&capabilities, "Missing"), 0);
        assert_eq!(score_overall(&capabilities), 63);
    }

    #[test]
    fn collector_finish_sorts_and_deduplicates_manual_fixes() {
        let mut collector = ColdstartCollector::default();
        collector.manual_fix("z fix");
        collector.manual_fix("a fix");
        collector.manual_fix("z fix");

        collector.finish();

        assert_eq!(collector.manual_fixes_required, vec!["a fix", "z fix"]);
    }

    #[test]
    fn report_assembler_prioritizes_codex_binding_fix() {
        let ctx = test_context(false, false);
        let mut collector = ColdstartCollector::default();
        collector.capabilities = vec![capability("Claude", "ok"), capability("Codex", "warn")];

        let report = ReportAssembler::build(&ctx, RunMode::Check, collector);

        assert_eq!(report.mode, "check");
        assert_eq!(report.claude_score, 100);
        assert_eq!(report.codex_score, 55);
        assert_eq!(report.overall_score, 77);
        assert_eq!(report.verdict, "usable but needs targeted fixes");
        assert_eq!(
            report.most_important_fix,
            "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint"
        );
        assert!(report.report_path.is_none());
    }

    #[test]
    fn report_assembler_falls_back_to_desktop_binding_fix_after_codex() {
        let ctx = test_context(false, true);
        let collector = ColdstartCollector::default();

        let report = ReportAssembler::build(&ctx, RunMode::Repair, collector);

        assert_eq!(report.mode, "repair");
        assert_eq!(
            report.most_important_fix,
            "Bind Claude Desktop to Gateway Switch and verify the local /v1/messages health endpoint"
        );
    }
}
