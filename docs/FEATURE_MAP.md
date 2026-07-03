# Feature Map

**Version:** v1.19.0
**Last updated:** 2026-07-03

## Overview

Gateway Switch is a runtime compatibility gateway that bridges AI-native desktop apps (Claude Desktop, Claude Code, Codex App) with third-party model providers.

## Feature Coverage

### ✅ Core Gateway

| Feature | Claude Desktop | Claude Code | Codex App |
|---|---|---|---|
| Request forwarding | ✅ | ✅ | ✅ |
| Streaming SSE | ✅ | ✅ | ✅ |
| Sync responses | ✅ | ✅ | ✅ |
| Tool calls | ✅ | ✅ | ✅ |
| Multi-turn conversation | ✅ | ✅ | ✅ |

### ✅ Protocol Compatibility

| Feature | Status | Notes |
|---|---|---|
| Anthropic Messages → Anthropic upstream | ✅ | Native |
| Anthropic Messages → OpenAI Chat | ✅ | Auto fallback |
| OpenAI Responses → OpenAI Chat | ✅ | Codex gateway |
| SSE event remapping | ✅ | Both directions |
| Token usage tracking | ✅ | `include_usage` injection |

### ✅ Tool Call Reliability

| Feature | Status | Notes |
|---|---|---|
| JSON parameter repair | ✅ | Unquoted keys, trailing commas, single quotes |
| Pseudo tool-call detection | ✅ | Detects "I called the tool" patterns |
| Missing tool-call retry | ✅ | `tool_choice: "required"` retry |
| `finish_reason` tracking | ✅ | Length → incomplete |
| Stream timeout | ✅ | 120 seconds |
| LoopGuard (duplicate suppression) | ✅ | Shared across all gateways |

### ✅ Codex Gateway — v1.19.0 Improvements

| Improvement | Status | Module |
|---|---|---|
| #1: CodexToolContext + bidirectional tool restore | ✅ | `codex_tools.rs` |
| #2: Cross-request function_call history LRU | ✅ | `codex_history.rs` |
| #3: Reasoning panel restoration | ✅ | `codex_gateway.rs` |
| #4: Platform-aware reasoning translation | ✅ | `gateway_strategy.rs` |
| #5: vLLM/enterprise gateway compatibility | ✅ | `codex_gateway.rs` |

### ✅ Tool Type Support (Codex)

| Tool Type | Request (downgrade) | Response (restore) | Notes |
|---|---|---|---|
| `function` | ✅ passthrough | ✅ passthrough | No change |
| `custom` | ✅ → function | ✅ → custom_tool_call | Spec embedded in description |
| `tool_search` | ✅ → function | ✅ → tool_search_call | Spec embedded in description |
| `namespace` | ✅ → function | ✅ → function_call | Original name preserved |

### ✅ Reasoning Translation

| Platform | Parameter | Status |
|---|---|---|
| DeepSeek | `thinking: { type: "enabled" }` | ✅ |
| GLM / ZhiPu | `thinking: { type: "enabled" }` | ✅ |
| Qwen / DashScope | `thinking: { type: "enabled" }` | ✅ |
| Kimi / Moonshot | `enable_thinking: true` | ✅ |
| SiliconFlow | `enable_thinking: true` | ✅ |
| StepFun | `reasoning_split: true` | ✅ |
| OpenRouter | `reasoning: { effort: "..." }` | ✅ (max → xhigh) |
| Xiaomi MiMo | no injection | ✅ |
| Volcengine / 火山引擎 | no injection | ✅ |

### ✅ Provider Compatibility Profiles

| Provider | Anthropic | Chat Completions | Tool Calls | Reasoning |
|---|---|---|---|---|
| OpenRouter | ✅ | ✅ | ✅ | ✅ |
| DeepSeek official | ✅ | ✅ | ✅ | ✅ |
| Moonshot Kimi | ✅ | ✅ | ✅ | ✅ |
| Qwen / DashScope | ✅ | ✅ | ✅ | ✅ |
| Xiaomi MiMo | ✅ | ✅ | ✅ | ✅ (internal) |
| Volcengine Ark | ⚠️ | ✅ | ✅ | ⚠️ strict |
| Anthropic-compatible | ✅ | ✅ | ✅ | — |
| Generic OpenAI Chat | — | ✅ | ✅ | — |

### ✅ Security & Privacy

| Feature | Status |
|---|---|
| API key secret scrubbing in logs | ✅ |
| Local-only SQLite database | ✅ |
| Gateway binds to 127.0.0.1 only | ✅ |
| No telemetry | ✅ |
| No data collection | ✅ |

### ✅ Diagnostics

| Feature | Status |
|---|---|
| Request logging | ✅ |
| Failed-request snapshots | ✅ |
| Payload preview (sanitized) | ✅ |
| Loop guard diagnostics | ✅ |
| Provider health checks | ✅ |
| Cold start doctor | ✅ |

## Known Gaps / Limitations

| Gap | Severity | Workaround |
|---|---|---|
| Cross-request history is in-memory only | Medium | Restart resets cache; client retries with full payload |
| No Developer ID signing | Low | Right-click → Open on first launch |
| macOS only (no Windows/Linux) | Medium | — |
| `tool_search` downgrade loses advanced filters | Low | Basic query preserved |
| Volcengine Kimi tool name restrictions | Low | Names sanitized to `[a-zA-Z0-9_-]` |
| No per-provider rate limiting config | Low | Global throttle after 429 |

## Future Roadmap

See [NEXT_STEPS.md](./NEXT_STEPS.md) for detailed roadmap.
