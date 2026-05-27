use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAlias {
    pub id: String,
    pub alias: String,
    pub alias_type: String, // "claude" or "codex"
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModelAlias {
    pub alias: String,
    pub alias_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_start_gateway: bool,
    pub auto_takeover_desktop: bool,
    pub listen_host: String,
    pub listen_port: u16,
    pub auth_token: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "zh".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start_gateway: true,
            auto_takeover_desktop: false,
            listen_host: "127.0.0.1".into(),
            listen_port: 3456,
            auth_token: "gateway-switch-token".into(),
            language: default_language(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub gateway_running: bool,
    pub gateway_port: u16,
    pub gateway_error: Option<String>,
    pub binding_active: bool,
    pub provider_count: usize,
    pub route_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub openai_base_url: String,
    pub anthropic_base_url: Option<String>,
    pub auth_header: String,
    pub auth_scheme: Option<String>,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub openai_base_url: Option<String>,
    pub anthropic_base_url: Option<String>,
    pub auth_header: String,
    pub auth_scheme: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub openai_base_url: Option<String>,
    pub anthropic_base_url: Option<String>,
    pub auth_header: String,
    pub auth_scheme: Option<String>,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoute {
    pub id: String,
    pub claude_alias: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModelRoute {
    pub id: String,
    pub claude_alias: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelRoute {
    pub id: String,
    pub claude_alias: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayProfile {
    pub listen_host: String,
    pub listen_port: u16,
    pub auth_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexBindingInfo {
    pub config_path: String,
    pub config_exists: bool,
    pub managed: bool,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaudeCodeInfo {
    pub config_path: String,
    pub config_exists: bool,
    pub managed: bool,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub auth_env: Option<String>,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeBindPayload {
    pub mode: String,
    pub model: String,
    pub provider_id: Option<String>,
    pub upstream_model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestLog {
    pub request_id: String,
    pub claude_alias: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub status_code: Option<u16>,
    pub duration_ms: Option<u64>,
    pub is_stream: bool,
    pub error_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub target: String,
    pub ok: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartStep {
    pub id: String,
    pub label: String,
    pub target: String,
    pub status: String,
    pub detail: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartCapability {
    pub name: String,
    pub target: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartReport {
    pub generated_at: String,
    pub mode: String,
    pub verdict: String,
    pub claude_score: u8,
    pub codex_score: u8,
    pub overall_score: u8,
    pub biggest_risk: String,
    pub most_important_fix: String,
    pub report_path: Option<String>,
    pub auto_fixes_applied: Vec<String>,
    pub manual_fixes_required: Vec<String>,
    pub steps: Vec<ColdStartStep>,
    pub capabilities: Vec<ColdStartCapability>,
}

// ── Codex Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub tool_call_mode: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub tool_call_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub tool_call_mode: Option<String>,
    pub enabled: bool,
}
