# Gateway Switch v1.19.0 Release Notes

**Release date:** 2026-07-03
**Version:** 1.19.0
**Tag:** v1.19.0
**Platform:** macOS (aarch64 / Apple Silicon)
**Bundle size:** ~19 MB app / ~7.6 MB DMG

## Highlights

v1.19.0 is the "Codex Routing Improvements" release, implementing all 5 improvements from the cc-switch architecture analysis as a continuous upgrade from v1.16.0 through v1.19.0. This release substantially improves Codex App compatibility with third-party Chat Completions providers — especially reasoning models, MCP tool scenarios, and Volcengine/火山引擎 routes.

### What's New in v1.19.0

#### 1. CodexToolContext + Bidirectional Tool Restore (Improvement 1)

Full round-trip support for all 4 Codex tool types (`function`, `custom`, `tool_search`, `namespace`). Previously only `function` tools were forwarded; now all tool types are downgraded to `function` on the way out (lossless, with original spec embedded) and restored to their original type on the way back.

- **Request side**: `CodexToolContext::from_request()` parses all tool types; `downgrade_to_function()` embeds original spec as `__codex_<type>__:<json>` marker in description.
- **Response side**: `restore_response_item()` recognizes the tool by chat_name and emits the correct Codex output item type:
  - `custom` → `type: "custom_tool_call"` with `name` + `arguments`
  - `tool_search` → `type: "tool_search_call"` with `query`
  - `namespace` → `type: "function_call"` with original name preserved
  - `function` → unchanged (passthrough)
- Applied to both sync and streaming paths. Backward compatible.

#### 2. Cross-request function_call History (Improvement 2)

In-memory LRU cache of function_call output items keyed by response_id, so multi-round Codex CLI sessions that send `previous_response_id` + `function_call_output` items (without the full assistant `tool_calls`) have missing tool calls reconstructed before forwarding.

- Cache size: 512 responses (LRU eviction), per gateway instance (in-memory only, lost on restart).
- Safe default: when `previous_response_id` is unknown, request passes through unchanged.

#### 3. Platform-aware Reasoning Translation (Improvement 4)

Translates Codex `reasoning.effort` / `reasoning_effort` to each provider's native field:

| Platform | Native Parameter |
|---|---|
| DeepSeek / GLM / Qwen | `thinking: { type: "enabled" }` |
| Kimi (Moonshot) / SiliconFlow | `enable_thinking: true` |
| StepFun | `reasoning_split: true` |
| OpenRouter | `reasoning: { effort: "..." }` with `max` → `xhigh` |
| Xiaomi MiMo | no injection |
| **Volcengine / 火山引擎** | **no injection** (new in 1.19.0 patch) |

#### 4. Reasoning Panel Restoration (Improvement 3)

Instead of dropping `reasoning_content` entirely (v1.15.0 fix), now routes it to a proper Codex Responses `type: "reasoning"` output item. The Codex App reasoning panel now shows DeepSeek-R1, Kimi K2, and other reasoning models' chain-of-thought separately from the assistant answer.

- Non-streaming: emits `reasoning` item before `message` item.
- Streaming: emits `response.reasoning_summary_text.delta` SSE events.
- Supports `<think>...</think>` block parsing for models like DeepSeek R1 distill.

#### 5. vLLM / Enterprise Gateway Compatibility

- Strip dangling `tool_choice` and `parallel_tool_calls` when a request has no tools after policy application.
- Conditional `stream_options.include_usage` injection (skipped when `strip_unsupported_params` is set).

### Volcengine Compatibility Fixes

This release includes targeted fixes for 火山引擎 routes that were causing 502 Bad Gateway / InvalidParameter errors:

1. **Volcengine reasoning parameter detection**: `infer_codex_chat_reasoning_config` now recognizes Volcengine/火山引擎 providers and returns `None` — no reasoning parameters are injected.
2. **Conditional `include_usage`**: `apply_codex_post_policy_cleanup` gates `stream_options.include_usage` on `!route.strategy.strip_unsupported_params`.
3. **Strict tool name sanitization**: namespace/custom/tool_search tools get OpenAI-compatible function names (`[a-zA-Z0-9_-]` only), fixing 400 errors on Kimi/Volcengine when MCP namespace tools contain colons or dots.
4. **`force_when_tools_present` fix**: no longer forces `tool_choice = "required"`; instead uses a system prompt hint. Only `strict_execution` enforces `required`. This fixes the "endless processing" issue where models with MCP tools looped in tool-call mode without producing final answers.

## Installation

### System Requirements

- macOS 12.0 or later (Apple Silicon / aarch64)
- ~50 MB free disk space

### Install Steps

1. Download `Gateway Switch_1.19.0_aarch64.dmg` from the release assets.
2. Double-click to mount the DMG.
3. Drag `Gateway Switch.app` into `Applications`.
4. Launch from `/Applications/Gateway Switch.app`.
5. If Gatekeeper blocks the app, right-click → Open → Open.

### Important: Run from /Applications

Do not run the app directly from the DMG. The app detects when it's launched from a read-only volume and shows a warning. Always copy to `/Applications` first.

## Verification

- Rust unit tests: `cd src-tauri && cargo test --lib` → **116 passed, 3 ignored**
- Frontend build: `pnpm build`
- Tauri build: `pnpm tauri build`
- DMG size: ~7.6 MB
- App size: ~19 MB

## Known Limitations

- Cross-request function_call history is in-memory only; it's lost on app restart.
- `tool_search` downgrade uses `query` parameter only; advanced search filters are not preserved.
- Volcengine Kimi routes still have stricter tool name validation than DeepSeek or GLM routes.
- No Developer ID signing / notarization — Gatekeeper will show a warning on first launch.

## Upgrading from v1.15.x

1. Quit Gateway Switch.
2. Replace `/Applications/Gateway Switch.app` with the new version.
3. Launch — all configuration (providers, routes, logs) is stored in `~/Library/Application Support/Gateway Switch/` and will be preserved.

## Rollback

If you experience issues, the previous stable release is v1.15.0. Configuration is backward compatible.

## Feedback

Report issues at: https://github.com/gcristiano0624-bot/gateway-switch/issues
