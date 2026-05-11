use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::Provider;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderCapabilityProfile {
    pub provider: String,
    pub supports_messages_api: bool,
    pub supports_chat_completions: bool,
    pub supports_responses_api: bool,
    pub supports_tool_use: bool,
    pub supports_vision: bool,
    pub supports_reasoning: bool,
    pub supports_streaming: bool,
    pub supports_system_prompt: bool,
    pub max_context: u32,
    pub json_stability: String,
    pub tool_call_accuracy: String,
    pub long_context_stability: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexCapabilityProfile {
    pub supports_chat: bool,
    pub supports_code_edit: bool,
    pub supports_patch: bool,
    pub supports_tool_call: bool,
    pub supports_shell_loop: bool,
    pub supports_long_task: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub chat: String,
    pub tool_use: String,
    pub mcp: String,
    pub artifacts: String,
    pub long_context: String,
    pub responses_compatibility: String,
    pub patch_quality: String,
    pub agent_recovery: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetyDecision {
    pub allowed: bool,
    pub severity: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub repaired_patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskState {
    pub plan: Vec<String>,
    pub files_touched: Vec<String>,
    pub commands_run: Vec<String>,
    pub errors_seen: Vec<String>,
    pub patches_applied: usize,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeFeatureReport {
    pub provider_capability_profile: bool,
    pub anthropic_protocol_adapter: bool,
    pub sse_event_compatibility: bool,
    pub tool_call_repair: bool,
    pub fake_tool_call_detector: bool,
    pub mcp_security_sandbox: bool,
    pub secret_redaction: bool,
    pub compatibility_benchmark: bool,
    pub context_compression: bool,
    pub provider_fallback: bool,
    pub observability_diagnostics: bool,
    pub responses_api_compatibility: bool,
    pub responses_chat_adapter: bool,
    pub codex_capability_profile: bool,
    pub patch_validator: bool,
    pub patch_repair: bool,
    pub fake_action_detector: bool,
    pub command_safety_gate: bool,
    pub long_task_state_tracker: bool,
    pub multi_step_agent_recovery: bool,
    pub shell_execution_sandbox: bool,
    pub responses_runtime_benchmark: bool,
}

pub fn provider_capability_profile(provider: &Provider) -> ProviderCapabilityProfile {
    let key = format!(
        "{} {} {}",
        provider.id, provider.name, provider.openai_base_url
    )
    .to_ascii_lowercase();
    let has_anthropic = provider
        .anthropic_base_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let is_openai_like = key.contains("openai") || key.contains("azure");
    let is_qwen = key.contains("qwen") || key.contains("dashscope");
    let is_deepseek = key.contains("deepseek");
    let is_claude_like = key.contains("anthropic") || key.contains("claude");

    ProviderCapabilityProfile {
        provider: provider.id.clone(),
        supports_messages_api: has_anthropic || is_claude_like,
        supports_chat_completions: true,
        supports_responses_api: is_openai_like,
        supports_tool_use: is_openai_like || is_qwen || is_claude_like,
        supports_vision: is_openai_like || is_qwen || is_claude_like,
        supports_reasoning: is_deepseek || is_qwen || is_openai_like,
        supports_streaming: true,
        supports_system_prompt: true,
        max_context: if is_deepseek || is_qwen {
            128_000
        } else {
            32_000
        },
        json_stability: if is_openai_like || is_claude_like {
            "high"
        } else {
            "medium"
        }
        .into(),
        tool_call_accuracy: if is_openai_like || is_claude_like {
            "high"
        } else if is_qwen {
            "medium"
        } else {
            "low"
        }
        .into(),
        long_context_stability: if is_deepseek || is_qwen || is_claude_like {
            "medium"
        } else {
            "high"
        }
        .into(),
    }
}

pub fn provider_capability_json(provider: &Provider) -> Value {
    let profile = provider_capability_profile(provider);
    json!({
        "provider": profile.provider,
        "supports_messages_api": profile.supports_messages_api,
        "supports_chat_completions": profile.supports_chat_completions,
        "supports_responses_api": profile.supports_responses_api,
        "supports_tool_use": profile.supports_tool_use,
        "supports_vision": profile.supports_vision,
        "supports_reasoning": profile.supports_reasoning,
        "supports_streaming": profile.supports_streaming,
        "supports_system_prompt": profile.supports_system_prompt,
        "max_context": profile.max_context,
        "json_stability": profile.json_stability,
        "tool_call_accuracy": profile.tool_call_accuracy,
        "long_context_stability": profile.long_context_stability
    })
}

pub fn codex_capability_profile(provider: &Provider) -> CodexCapabilityProfile {
    let profile = provider_capability_profile(provider);
    CodexCapabilityProfile {
        supports_chat: true,
        supports_code_edit: profile.supports_tool_use,
        supports_patch: profile.supports_tool_use && profile.json_stability != "low",
        supports_tool_call: profile.supports_tool_use,
        supports_shell_loop: profile.long_context_stability != "low",
        supports_long_task: profile.max_context >= 32_000,
    }
}

pub fn benchmark_provider(provider: &Provider) -> BenchmarkReport {
    let p = provider_capability_profile(provider);
    let grade = |good: bool, maybe: bool| -> String {
        if good { "A" } else if maybe { "B" } else { "C" }.into()
    };
    BenchmarkReport {
        chat: "A".into(),
        tool_use: grade(p.supports_tool_use && p.tool_call_accuracy == "high", p.supports_tool_use),
        mcp: grade(p.supports_tool_use && p.supports_system_prompt, p.supports_tool_use),
        artifacts: grade(p.supports_tool_use && p.json_stability == "high", p.json_stability == "medium"),
        long_context: grade(p.max_context >= 128_000, p.max_context >= 32_000),
        responses_compatibility: grade(p.supports_responses_api, p.supports_chat_completions),
        patch_quality: grade(p.supports_tool_use && p.json_stability == "high", p.supports_tool_use),
        agent_recovery: grade(p.max_context >= 128_000 && p.supports_tool_use, p.max_context >= 32_000),
    }
}

pub fn runtime_feature_report() -> RuntimeFeatureReport {
    RuntimeFeatureReport {
        provider_capability_profile: true,
        anthropic_protocol_adapter: true,
        sse_event_compatibility: true,
        tool_call_repair: true,
        fake_tool_call_detector: true,
        mcp_security_sandbox: true,
        secret_redaction: true,
        compatibility_benchmark: true,
        context_compression: true,
        provider_fallback: true,
        observability_diagnostics: true,
        responses_api_compatibility: true,
        responses_chat_adapter: true,
        codex_capability_profile: true,
        patch_validator: true,
        patch_repair: true,
        fake_action_detector: true,
        command_safety_gate: true,
        long_task_state_tracker: true,
        multi_step_agent_recovery: true,
        shell_execution_sandbox: true,
        responses_runtime_benchmark: true,
    }
}

pub fn redact_secrets(input: &str) -> String {
    let mut output = input.to_string();
    for marker in ["sk-proj-", "sk-ant-", "github_pat_", "ghp_", "AKIA"] {
        output = redact_after_marker(&output, marker);
    }
    output = redact_after_marker_unless(&output, "sk-", &["proj-", "ant-", "***redacted***"]);
    output = redact_jwt_like(&output);
    output = redact_pem_blocks(&output);
    output
}

pub fn redact_log_summary(input: &str) -> String {
    let redacted = redact_secrets(input);
    if redacted.chars().count() > 700 {
        format!("{}...", redacted.chars().take(700).collect::<String>())
    } else {
        redacted
    }
}

pub fn repair_json_object(input: &str) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_str::<Value>(input) {
        return Ok(value);
    }
    let mut s = input.trim().to_string();
    if !s.starts_with('{') {
        if let Some(start) = s.find('{') {
            s = s[start..].to_string();
        }
    }
    if !s.ends_with('}') {
        if let Some(end) = s.rfind('}') {
            s = s[..=end].to_string();
        } else {
            s.push('}');
        }
    }
    s = quote_unquoted_keys(&s);
    s = s.replace("'", "\"");
    s = remove_trailing_commas(&s);
    serde_json::from_str::<Value>(&s).map_err(|e| format!("JSON repair failed: {e}"))
}

pub fn detect_fake_tool_call(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let cn = ["已经调用", "我调用了", "已经读取", "我读取了", "已经执行", "我执行了"];
    let en = [
        "i called the tool",
        "i have called the tool",
        "i read the file",
        "i have read the file",
        "i ran the command",
        "i executed the command",
    ];
    cn.iter().any(|p| text.contains(p)) || en.iter().any(|p| lower.contains(p))
}

pub fn detect_fake_action(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    detect_fake_tool_call(text)
        || ["i modified", "i updated", "i patched", "tests passed", "npm test passed"]
            .iter()
            .any(|p| lower.contains(p))
        || ["我已经修改", "我修改了", "测试通过", "已经运行测试"]
            .iter()
            .any(|p| text.contains(p))
}

pub fn mcp_path_safety(path: &str, workspace_root: &Path) -> SafetyDecision {
    let p = Path::new(path);
    let denied_names = [".env", ".ssh", "id_rsa", "id_ed25519", "cookies", "token"];
    let lower = path.to_ascii_lowercase();
    if denied_names.iter().any(|name| lower.contains(name)) {
        return deny("high", "Path matches a protected secret or credential location");
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return deny("high", "Path traversal is not allowed");
    }
    if p.is_absolute() && !p.starts_with(workspace_root) {
        return deny("high", "Access outside the workspace is not allowed");
    }
    allow("Path is inside the allowed workspace scope")
}

pub fn command_safety(command: &str) -> SafetyDecision {
    let lower = command.to_ascii_lowercase();
    let dangerous = [
        "rm -rf", "sudo ", "chmod -r", "curl | bash", "curl -s", "wget | bash",
        "npm install -g", "pnpm add -g", "yarn global", "dd if=", "mkfs", ":(){",
    ];
    if dangerous.iter().any(|p| lower.contains(p)) {
        return deny("high", "Command matches a blocked destructive or global mutation pattern");
    }
    if lower.contains(" > /etc/") || lower.contains(" /etc/") {
        return deny("high", "System configuration paths are protected");
    }
    allow("Command passed the safety gate")
}

pub fn validate_patch(patch: &str, workspace_root: &Path) -> PatchValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if !patch.contains("@@") {
        warnings.push("Patch has no hunks; it may be a create/delete-only patch".into());
    }
    let mut saw_file = false;
    for line in patch.lines() {
        if let Some(path) = line
            .strip_prefix("+++ b/")
            .or_else(|| line.strip_prefix("--- a/"))
            .or_else(|| line.strip_prefix("+++ "))
            .or_else(|| line.strip_prefix("--- "))
            .filter(|p| *p != "/dev/null")
        {
            saw_file = true;
            let decision = mcp_path_safety(path, workspace_root);
            if !decision.allowed {
                errors.push(format!("{path}: {}", decision.reason));
            }
        }
        if let Some(path) = line.strip_prefix("*** Update File: ")
            .or_else(|| line.strip_prefix("*** Add File: "))
            .or_else(|| line.strip_prefix("*** Delete File: "))
        {
            saw_file = true;
            let decision = mcp_path_safety(path, workspace_root);
            if !decision.allowed {
                errors.push(format!("{path}: {}", decision.reason));
            }
        }
    }
    if !saw_file {
        errors.push("Patch does not include recognizable file headers".into());
    }
    let repaired_patch = repair_patch_headers(patch);
    PatchValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
        repaired_patch: (repaired_patch != patch).then_some(repaired_patch),
    }
}

pub fn compress_context(messages: &[Value], max_items: usize) -> Value {
    if messages.len() <= max_items {
        return json!({ "strategy": "none", "messages": messages });
    }
    let pinned: Vec<Value> = messages
        .iter()
        .filter(|m| contains_tool_state(m))
        .cloned()
        .collect();
    let recent: Vec<Value> = messages
        .iter()
        .rev()
        .take(max_items)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    json!({
        "strategy": "sliding_window_with_tool_state_pinning",
        "summary": format!("Compressed {} older message(s); retained {} recent message(s) and {} tool-state message(s).", messages.len().saturating_sub(recent.len()), recent.len(), pinned.len()),
        "messages": recent,
        "pinned_tool_state": pinned
    })
}

pub fn recover_agent_state(history: &[Value]) -> AgentTaskState {
    let mut state = AgentTaskState {
        plan: Vec::new(),
        files_touched: Vec::new(),
        commands_run: Vec::new(),
        errors_seen: Vec::new(),
        patches_applied: 0,
        next_action: None,
    };
    for item in history {
        let text = item.to_string();
        if text.contains("apply_patch") || text.contains("*** Update File:") {
            state.patches_applied += 1;
        }
        if text.contains("error") || text.contains("failed") || text.contains("panic") {
            state.errors_seen.push(redact_log_summary(&text));
        }
        for token in extract_file_like_tokens(&text) {
            if !state.files_touched.contains(&token) {
                state.files_touched.push(token);
            }
        }
    }
    state.next_action = Some("Continue from the latest verified state and avoid repeating completed fixes".into());
    state
}

fn allow(reason: &str) -> SafetyDecision {
    SafetyDecision { allowed: true, severity: "none".into(), reason: reason.into() }
}

fn deny(severity: &str, reason: &str) -> SafetyDecision {
    SafetyDecision { allowed: false, severity: severity.into(), reason: reason.into() }
}

fn quote_unquoted_keys(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut after_object_or_comma = false;
    while let Some(c) = chars.next() {
        if c == '{' || c == ',' {
            after_object_or_comma = true;
            out.push(c);
            continue;
        }
        if after_object_or_comma && (c.is_ascii_alphabetic() || c == '_') {
            let mut key = String::from(c);
            while let Some(next) = chars.peek().copied() {
                if next.is_ascii_alphanumeric() || next == '_' || next == '-' {
                    key.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            while matches!(chars.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                chars.next();
            }
            if matches!(chars.peek(), Some(':')) {
                out.push('"');
                out.push_str(&key);
                out.push('"');
            } else {
                out.push_str(&key);
            }
            after_object_or_comma = false;
            continue;
        }
        if !c.is_whitespace() {
            after_object_or_comma = false;
        }
        out.push(c);
    }
    out
}

fn remove_trailing_commas(input: &str) -> String {
    input.replace(",}", "}").replace(",]", "]")
}

fn repair_patch_headers(patch: &str) -> String {
    patch
        .lines()
        .map(|line| {
            if let Some(path) = line.strip_prefix("+++ ") {
                if path == "/dev/null" || path.starts_with("b/") { line.to_string() } else { format!("+++ b/{path}") }
            } else if let Some(path) = line.strip_prefix("--- ") {
                if path == "/dev/null" || path.starts_with("a/") { line.to_string() } else { format!("--- a/{path}") }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn contains_tool_state(value: &Value) -> bool {
    let text = value.to_string();
    text.contains("tool_use") || text.contains("tool_result") || text.contains("function_call")
}

fn extract_file_like_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ':' | ')' | '('))
        .filter(|token| token.contains('/') && token.contains('.'))
        .take(50)
        .map(|s| s.trim_matches('\\').to_string())
        .collect()
}

fn redact_after_marker(input: &str, marker: &str) -> String {
    redact_after_marker_unless(input, marker, &[])
}

fn redact_after_marker_unless(input: &str, marker: &str, skip_prefixes: &[&str]) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        out.push_str(marker);
        let tail = &rest[pos + marker.len()..];
        if skip_prefixes.iter().any(|prefix| tail.starts_with(prefix)) {
            rest = tail;
            continue;
        }
        out.push_str("***redacted***");
        let end = tail
            .char_indices()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
            .map(|(idx, _)| idx)
            .unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn redact_jwt_like(input: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            let dot_count = part.matches('.').count();
            if dot_count == 2 && part.len() > 60 {
                "***jwt-redacted***".to_string()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_pem_blocks(input: &str) -> String {
    let Some(start) = input.find("-----BEGIN ") else {
        return input.to_string();
    };
    let Some(end) = input[start..].find("-----END ") else {
        return input.to_string();
    };
    let suffix_start = input[start + end..]
        .find("-----")
        .map(|idx| start + end + idx + 5)
        .unwrap_or(input.len());
    format!(
        "{}***pem-redacted***{}",
        &input[..start],
        &input[suffix_start..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_shapes() {
        let text = "bad sk-proj-abcdef ghp_123456 abc.def.ghi";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("sk-proj-***redacted***"));
        assert!(redacted.contains("ghp_***redacted***"));
    }

    #[test]
    fn repairs_loose_json_tool_arguments() {
        let value = repair_json_object(r#"{name:"search",query:"abc",}"#).unwrap();
        assert_eq!(value["name"], "search");
        assert_eq!(value["query"], "abc");
    }

    #[test]
    fn blocks_dangerous_commands_and_paths() {
        assert!(!command_safety("rm -rf /tmp/project").allowed);
        assert!(!mcp_path_safety("../.ssh/id_rsa", Path::new("/tmp/project")).allowed);
        assert!(command_safety("cargo test").allowed);
    }

    #[test]
    fn validates_and_repairs_patch_headers() {
        let result = validate_patch("--- src/a.rs\n+++ src/a.rs\n@@\n-old\n+new", Path::new("/tmp/project"));
        assert!(result.valid);
        assert!(result.repaired_patch.unwrap().contains("+++ b/src/a.rs"));
    }

    #[test]
    fn compresses_context_and_recovers_state() {
        let messages = vec![
            json!({"role":"user","content":"old"}),
            json!({"role":"assistant","content":[{"type":"tool_use","name":"search"}]}),
            json!({"role":"user","content":"src/main.rs failed"}),
            json!({"role":"assistant","content":"latest"}),
        ];
        let compressed = compress_context(&messages, 2);
        assert_eq!(compressed["strategy"], "sliding_window_with_tool_state_pinning");
        let state = recover_agent_state(&messages);
        assert!(state.files_touched.iter().any(|p| p.contains("src/main.rs")));
    }
}
