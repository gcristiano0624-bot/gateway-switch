use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    database,
    models::{Provider, ProviderCompatibilityPolicy},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodexThinkingParam {
    Thinking,
    EnableThinking,
    ReasoningSplit,
    ReasoningDotEffort,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodexEffortParam {
    None,
    Passthrough,
    ForceHigh,
}

#[derive(Debug, Clone)]
pub struct CodexChatReasoningConfig {
    pub thinking_param: CodexThinkingParam,
    pub effort_param: CodexEffortParam,
    pub max_effort_override: Option<&'static str>,
}

pub fn infer_codex_chat_reasoning_config(
    provider: &Provider,
    upstream_model: &str,
) -> CodexChatReasoningConfig {
    let key = format!(
        "{} {} {} {}",
        provider.id, provider.name, provider.base_url, upstream_model
    )
    .to_ascii_lowercase();

    if key.contains("volcengine")
        || key.contains("volc")
        || key.contains("ark.cn-")
        || key.contains("火山")
    {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::None,
            effort_param: CodexEffortParam::None,
            max_effort_override: None,
        };
    }

    if key.contains("openrouter") {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::ReasoningDotEffort,
            effort_param: CodexEffortParam::Passthrough,
            max_effort_override: Some("xhigh"),
        };
    }

    if key.contains("siliconflow") {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::EnableThinking,
            effort_param: CodexEffortParam::ForceHigh,
            max_effort_override: None,
        };
    }

    if key.contains("moonshot") || key.contains("kimi") {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::EnableThinking,
            effort_param: CodexEffortParam::ForceHigh,
            max_effort_override: None,
        };
    }

    if key.contains("stepfun") || key.contains("step") {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::ReasoningSplit,
            effort_param: CodexEffortParam::ForceHigh,
            max_effort_override: None,
        };
    }

    if key.contains("deepseek")
        || key.contains("glm")
        || key.contains("zhipu")
        || key.contains("qwen")
        || key.contains("dashscope")
        || key.contains("aliyun")
    {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::Thinking,
            effort_param: CodexEffortParam::ForceHigh,
            max_effort_override: None,
        };
    }

    if key.contains("xiaomi") || key.contains("mimo") || key.contains("xiaomimimo") {
        return CodexChatReasoningConfig {
            thinking_param: CodexThinkingParam::None,
            effort_param: CodexEffortParam::None,
            max_effort_override: None,
        };
    }

    CodexChatReasoningConfig {
        thinking_param: CodexThinkingParam::None,
        effort_param: CodexEffortParam::None,
        max_effort_override: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCompatibilityProfile {
    pub strategy_id: String,
    pub system_to_user: bool,
    pub tool_to_user: bool,
    pub disable_tools: bool,
    pub strip_unsupported_params: bool,
    pub direct_provider_safe: bool,
    pub gateway_route_recommended: bool,
    pub codex_disable_responses: bool,
    pub codex_strict_tool_calls: bool,
    pub codex_strip_reasoning: bool,
    pub summary: String,
}


pub(crate) fn should_force_chat_fallback(profile: &ProviderCompatibilityProfile) -> bool {
    !profile.direct_provider_safe
        && matches!(
            profile.strategy_id.as_str(),
            "volcengine_deepseek_coding"
                | "xiaomi_mimo_chat"
                | "deepseek_official_chat"
                | "moonshot_kimi_chat"
                | "qwen_dashscope_chat"
        )
}


pub fn provider_compatibility_profile(
    provider: &Provider,
    upstream_model: &str,
) -> ProviderCompatibilityProfile {
    let key = provider_profile_key(provider, upstream_model);
    let has_anthropic = provider
        .anthropic_base_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if is_volcengine_minimax_key(&key) {
        ProviderCompatibilityProfile {
            strategy_id: "volcengine_minimax_anthropic".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: has_anthropic,
            gateway_route_recommended: !has_anthropic,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: if has_anthropic {
                "Volcengine MiniMax has an Anthropic-compatible endpoint configured, but Claude thinking/reasoning parameters are stripped for compatibility.".into()
            } else {
                "Volcengine MiniMax without an Anthropic endpoint falls back through OpenAI Chat and strips unsupported thinking/reasoning parameters.".into()
            },
        }
    } else if is_volcengine_deepseek_key(&key) {
        ProviderCompatibilityProfile {
            strategy_id: "volcengine_deepseek_coding".into(),
            system_to_user: !has_anthropic,
            tool_to_user: !has_anthropic,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: has_anthropic,
            gateway_route_recommended: !has_anthropic,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: if has_anthropic {
                "Volcengine Ark DeepSeek has an Anthropic-compatible endpoint configured; Claude clients can use the Anthropic route while Codex keeps OpenAI compatibility.".into()
            } else {
                "Volcengine Ark DeepSeek without an Anthropic endpoint falls back through OpenAI Chat, strips unsupported params, and uses auto tool_choice to avoid validation errors.".into()
            },
        }
    } else if key.contains("openrouter") {
        ProviderCompatibilityProfile {
            strategy_id: "openrouter_anthropic_or_chat".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: has_anthropic,
            gateway_route_recommended: !has_anthropic,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: "OpenRouter is safest through Gateway Route unless an Anthropic-compatible endpoint is explicitly configured.".into(),
        }
    } else if key.contains("xiaomi") || key.contains("mimo") || key.contains("xiaomimimo.com") {
        ProviderCompatibilityProfile {
            strategy_id: "xiaomi_mimo_chat".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: has_anthropic,
            gateway_route_recommended: !has_anthropic,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: if has_anthropic {
                "Xiaomi MiMo has an Anthropic-compatible endpoint configured; Claude clients can use the Anthropic route while Codex keeps OpenAI compatibility.".into()
            } else {
                "Xiaomi MiMo without an Anthropic endpoint falls back through OpenAI Chat; strict Codex tool enforcement stays disabled to avoid tool-planning loops.".into()
            },
        }
    } else if key.contains("deepseek") {
        ProviderCompatibilityProfile {
            strategy_id: "deepseek_official_chat".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: has_anthropic,
            gateway_route_recommended: !has_anthropic,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: if has_anthropic {
                "DeepSeek has an Anthropic-compatible endpoint configured; Claude clients can use the Anthropic route while Codex keeps OpenAI compatibility.".into()
            } else {
                "DeepSeek without an Anthropic endpoint falls back through OpenAI Chat for Claude clients.".into()
            },
        }
    } else if key.contains("moonshot") || key.contains("kimi") {
        ProviderCompatibilityProfile {
            strategy_id: "moonshot_kimi_chat".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: false,
            gateway_route_recommended: true,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: false,
            summary: "Moonshot/Kimi is treated as OpenAI Chat-compatible; Gateway Route is recommended for Claude clients.".into(),
        }
    } else if key.contains("qwen") || key.contains("dashscope") || key.contains("aliyun") {
        ProviderCompatibilityProfile {
            strategy_id: "qwen_dashscope_chat".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: true,
            direct_provider_safe: false,
            gateway_route_recommended: true,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: true,
            summary: "Qwen/DashScope routes use OpenAI Chat compatibility; Gateway Route is recommended for Claude clients.".into(),
        }
    } else if has_anthropic {
        ProviderCompatibilityProfile {
            strategy_id: "standard_anthropic".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: false,
            direct_provider_safe: true,
            gateway_route_recommended: false,
            codex_disable_responses: false,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: false,
            summary: "Standard Anthropic-compatible route; Direct Provider may be used when the endpoint is truly Anthropic-compatible.".into(),
        }
    } else {
        ProviderCompatibilityProfile {
            strategy_id: "openai_chat_fallback".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: false,
            direct_provider_safe: false,
            gateway_route_recommended: true,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: false,
            summary: "OpenAI Chat fallback route; use Gateway Route for Claude and Claude Code."
                .into(),
        }
    }
}

pub fn effective_provider_compatibility_profile(
    db: &PathBuf,
    provider: &Provider,
    upstream_model: &str,
) -> ProviderCompatibilityProfile {
    let base = provider_compatibility_profile(provider, upstream_model);
    match database::get_provider_policy(db, &provider.id) {
        Ok(Some(policy)) => apply_provider_policy(base, &policy),
        _ => base,
    }
}

pub fn apply_provider_policy(
    mut base: ProviderCompatibilityProfile,
    policy: &ProviderCompatibilityPolicy,
) -> ProviderCompatibilityProfile {
    if let Some(v) = policy.system_to_user {
        base.system_to_user = v;
    }
    if let Some(v) = policy.tool_to_user {
        base.tool_to_user = v;
    }
    if let Some(v) = policy.disable_tools {
        base.disable_tools = v;
    }
    if let Some(v) = policy.strip_unsupported_params {
        base.strip_unsupported_params = v;
    }
    if let Some(v) = policy.direct_provider_safe {
        base.direct_provider_safe = v;
    }
    if let Some(v) = policy.gateway_route_recommended {
        base.gateway_route_recommended = v;
    }
    if let Some(v) = policy.codex_disable_responses {
        base.codex_disable_responses = v;
    }
    if let Some(v) = policy.codex_strict_tool_calls {
        base.codex_strict_tool_calls = v;
    }
    if let Some(v) = policy.codex_strip_reasoning {
        base.codex_strip_reasoning = v;
    }
    if policy.system_to_user.is_some()
        || policy.tool_to_user.is_some()
        || policy.disable_tools.is_some()
        || policy.strip_unsupported_params.is_some()
        || policy.direct_provider_safe.is_some()
        || policy.gateway_route_recommended.is_some()
        || policy.codex_disable_responses.is_some()
        || policy.codex_strict_tool_calls.is_some()
        || policy.codex_strip_reasoning.is_some()
    {
        base.summary = format!(
            "{} Manual provider policy overrides are active.",
            base.summary
        );
    }
    base
}

fn provider_profile_key(provider: &Provider, upstream_model: &str) -> String {
    format!(
        "{} {} {} {} {} {}",
        provider.id,
        provider.name,
        provider.base_url,
        provider.openai_base_url,
        provider.anthropic_base_url.as_deref().unwrap_or_default(),
        upstream_model
    )
    .to_ascii_lowercase()
}

fn is_volcengine_deepseek_key(key: &str) -> bool {
    (key.contains("volc") || key.contains("ark.cn-") || key.contains("火山"))
        && key.contains("deepseek")
}

fn is_volcengine_minimax_key(key: &str) -> bool {
    (key.contains("volc") || key.contains("ark.cn-") || key.contains("火山"))
        && key.contains("minimax")
}
