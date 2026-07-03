# PRD: Codex Routing Improvements (v1.16.0 → v1.19.0)

Date: 2026-06-30
Author: Claude (analysis of cc-switch project)
Repository: https://github.com/gcristiano0624-bot/gateway-switch
Reference version: v1.15.0 (this PRD proposes v1.16.0–v1.19.0)

## Background

Gateway Switch v1.15.0 ships the v1.13.3-style Codex routing: the user binds `gateway-switch` in `~/.codex/config.toml`, selects an OpenAI-named model (e.g. `gpt-5.1`) in the Codex App, and the local gateway rewrites the request to a Chinese upstream (e.g. DeepSeek-V4-Pro via Volcengine) using the `codex_routes` SQLite table.

We studied [cc-switch](https://github.com/farion1231/cc-switch)'s Codex proxy implementation (commit at `/tmp/cc-switch` in our analysis environment) to identify gaps in our `codex_gateway.rs`. The 5 improvements below are the result. They are ordered by **risk/benefit ratio** — improvements 5 and 3 are pure additions with near-zero regression risk; improvements 1 and 2 are structural refactors that build on top of 5 and 3.

## Goals

1. Eliminate classes of upstream 400/503 errors that hit vLLM-style and reasoning-model providers.
2. Restore Codex App's reasoning panel for reasoning models (DeepSeek-R1, Kimi K2 thinking, etc.) without re-leaking chain-of-thought into the chat UI.
3. Restore multi-round function call reliability for Chinese chat-completions providers.
4. Preserve all v1.15.0 behavior — no new user-facing config, no breaking change to `codex_routes` table, no breaking change to `~/.codex/config.toml` schema.

## Non-Goals

- Switching to a "passthrough to OpenAI via ChatGPT OAuth" model (cc-switch's path). We keep our v1.13.3-style `experimental_bearer_token` approach.
- Adding a new `codex_routes` table column. All improvements operate on the existing schema.
- Touching Claude Gateway / Anthropic Messages paths.
- Removing `v1.14.2`'s skipped experimental `custom_models` block. v1.15.0 already excludes it.

## User Stories

### US-1: vLLM / enterprise gateway compatibility
**As a** user with a strict vLLM-hosted Chinese model (e.g. an internal Qwen proxy)
**When** Codex App sends a request with no tools but a leftover `tool_choice` field
**I want** the gateway to strip `tool_choice` (and `parallel_tool_calls`) before forwarding
**So that** the upstream returns 200 instead of `400 tool_choice not allowed without tools`

### US-2: Streaming usage tracking
**As a** user monitoring per-provider API costs
**When** Codex App uses a streaming request
**I want** the gateway to inject `stream_options.include_usage = true`
**So that** `usage.prompt_tokens`, `usage.completion_tokens`, and (for some providers) `cache_read_input_tokens` are populated in `request_logs`, not zero

### US-3: Reasoning panel restoration
**As a** user with a reasoning model route (e.g. `gpt-5.2 → Kimi-K2.7-Code` in thinking mode)
**When** the upstream returns `reasoning_content` in the chat completion delta
**I want** the gateway to emit a Codex Responses `reasoning` output item alongside the `message` item
**So that** the Codex App reasoning panel shows the model's chain-of-thought separately from the answer

### US-4: Platform-aware reasoning control
**As a** user with a DeepSeek / StepFun / Kimi / GLM / Qwen / mimo / OpenRouter / SiliconFlow route
**When** Codex App sends `reasoning: { effort: "high" }`
**I want** the gateway to translate it to the platform's native field (e.g. DeepSeek `thinking: { type: "enabled" }`, OpenRouter `reasoning: { effort: "high" }`)
**So that** the upstream does not return `400 reasoning_effort: Invalid option`

### US-5: Multi-round function call reliability
**As a** user running a Codex CLI session that uses tools across multiple turns
**When** the second turn sends `previous_response_id` + a streamlined `function_call_output` (no full `tool_calls` field)
**I want** the gateway to reconstruct the full assistant `tool_calls` from a local LRU cache
**So that** Chat-Completions providers (DeepSeek, Kimi) accept the multi-round request instead of rejecting the incomplete tool call

### US-6: Custom tool support
**As a** user with Codex `apply_patch` or other `custom` tool definitions
**When** the Codex App sends a `custom` or `namespace` tool spec
**I want** the gateway to forward it as a function tool with the original definition embedded in `description` (lossless downgrade), and on the response side restore it to the original `type`
**So that** `apply_patch` invocations round-trip correctly

## Priority Matrix

| # | Title | Risk | Benefit | Effort | Release |
|---|---|---|---|---|---|
| **5** | tool_choice cleanup + include_usage | **Very low** | Medium | 1-2h | v1.16.0 |
| **3** | reasoning split into Responses API `reasoning` item | Low | Medium | 3-5h | v1.16.0 |
| **4** | Platform-aware reasoning translation | Medium | **High** | 1-2 days | v1.17.0 |
| **2** | Cross-request function_call history | Low | **High** | 1-2 days | v1.18.0 |
| **1** | CodexToolContext + bidirectional restore | Medium | High | 2-3 days | v1.19.0 |

## Acceptance Criteria

### v1.16.0
- AC-5.1: A request body that has `tools: []` and `tool_choice: "auto"` from the client has both `tool_choice` and `parallel_tool_calls` removed before forwarding. Verified by unit test on `convert_request` with `has_tools == false` and original `tool_choice` set.
- AC-5.2: A streaming chat-completions request to the gateway has `stream_options: { include_usage: true }` injected after `convert_request`. Verified by unit test inspecting the body sent to the upstream client.
- AC-5.3: A non-streaming chat-completions request does **not** have `stream_options` injected. Verified by unit test.
- AC-3.1: When upstream returns `delta.reasoning_content = "thinking..."`, the gateway emits a Codex `response.reasoning_summary_text.delta` SSE event followed by `.done`. The subsequent `message` content does not contain the reasoning text. Verified by unit test on the streaming `sse` block.
- AC-3.2: When upstream returns `message.reasoning_content` in non-streaming mode, the gateway emits a `type: "reasoning"` output item with the text, plus a `type: "message"` item with the answer. Verified by unit test on `convert_sync_response`.
- AC-3.3: When upstream returns only `content` (no `reasoning_content`), the response is identical to v1.15.0 (no `reasoning` item). Verified by regression test.
- AC-3.4: When the upstream response text starts with `<think>...</think>\n`, the text inside the think block goes into the `reasoning` item and the rest into the `message` content. Verified by unit test.
- **All v1.15.0 tests still pass.**

### v1.17.0
- AC-4.1: A request to a DeepSeek route with `reasoning: { effort: "high" }` produces an upstream body with `thinking: { type: "enabled" }`. Verified by unit test on `apply_codex_reasoning_translation`.
- AC-4.2: A request to a Kimi route produces `enable_thinking: true`. Verified by unit test.
- AC-4.3: A request to an OpenRouter route with `effort: "max"` produces `effort: "xhigh"`. Verified by unit test.
- AC-4.4: A request to an OpenRouter route with `effort: "none"` produces `{ "reasoning": { "effort": "none" } }` (no other reasoning field). Verified by unit test asserting the field is the only one set.
- AC-4.5: A request to a Xiaomi MiMo route does not inject any reasoning control (MiMo handles this differently). Verified by unit test asserting the field is absent.

### v1.18.0
- AC-2.1: After receiving a chat completion with `tool_calls`, the gateway stores the function_call by `call_id`. On a follow-up request with the same `call_id` in a `function_call_output` block, the gateway reconstructs the missing `name` and `arguments` from the cache. Verified by integration test with a mock upstream.
- AC-2.2: The cache evicts the oldest entry when it exceeds 512 responses. Verified by unit test.
- AC-2.3: When two responses share the same `call_id`, the gateway rejects the request (returns 400) to avoid ambiguity. Verified by unit test.
- AC-2.4: The cache is per-gateway-instance (in-memory `Arc<RwLock<...>>`); restarts lose it. Documented in code comments.

### v1.19.0
- AC-1.1: A request with `tools: [{ type: "custom", name: "apply_patch", ... }]` is forwarded as `tools: [{ type: "function", name: "apply_patch", description: "<embedded original spec>", parameters: { type: "object", properties: { input: { type: "string" } } } }]`. The chat_name mapping is recorded.
- AC-1.2: On the response side, a `function` tool_call with name `apply_patch` is restored to `type: "custom_tool_call"` with the original spec re-applied.
- AC-1.3: A request with `tools: [{ type: "tool_search" }]` is similarly downgraded to a function tool.
- AC-1.4: `namespace` tools with `server_name:namespace_name` are preserved across the round-trip.

## Out of Scope (deferred to a future PRD)

- OAuth session bucket unification (cc-switch's `inject_codex_unified_session_bucket` feature) — requires touching `auth.json` and may break user's existing Codex session history.
- Rate-limit retry and backoff middleware (cc-switch has `load_balance` / `failover_queue`).
- Multi-account ChatGPT OAuth rotation.
- Cross-platform support (Windows .msi / Linux .AppImage).

## Reference

The 5 improvements are derived from a deep read of `cc-switch` (commit referenced in `/tmp/cc-switch`):

- `src-tauri/src/proxy/providers/codex.rs` (965 lines) — platform-aware provider logic
- `src-tauri/src/proxy/transform_codex_chat.rs` (3298 lines) — Responses ↔ Chat Completions conversion
- `src-tauri/src/proxy/providers/codex_chat_history.rs` (864 lines) — LRU function_call cache
- `src-tauri/src/services/codex_oauth_models.rs` (193 lines) — OAuth model list fetcher (not in scope)

See the companion `docs/tech-codex-routing-improvements.md` for the implementation blueprint with file:line references.
