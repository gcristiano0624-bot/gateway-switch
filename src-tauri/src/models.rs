use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAlias {
    pub id: String,
    pub alias: String,
    pub alias_type: String,  // "claude" or "codex"
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start_gateway: true,
            auto_takeover_desktop: false,
            listen_host: "127.0.0.1".into(),
            listen_port: 3456,
            auth_token: "gateway-switch-token".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub gateway_running: bool,
    pub gateway_port: u16,
    pub binding_active: bool,
    pub provider_count: usize,
    pub route_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
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
    pub auth_header: String,
    pub auth_scheme: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
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

// ── Codex Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCodexRoute {
    pub id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub upstream_model: String,
    pub enabled: bool,
}
