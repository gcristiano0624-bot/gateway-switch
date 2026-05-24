<div align="center">

# Gateway Switch

**Runtime compatibility gateway for Claude Desktop, Claude Code, and Codex App**

> Gateway Switch is not just a model router. It is a **runtime compatibility layer** that sits between AI-native desktop applications and third-party model providers, bridging protocol gaps, repairing malformed tool calls, enforcing safety boundaries, and degrading gracefully when upstream providers misbehave.

[![Version](https://img.shields.io/badge/Version-1.7.2-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

English | [中文](./README.md)

</div>

---

## What is Gateway Switch?

Gateway Switch is a macOS desktop app that routes Claude Desktop, Claude Code, and Codex App model traffic to third-party model APIs.

It solves three related problems:

- **Claude Desktop** validates Claude model IDs. Gateway Switch exposes local Claude aliases such as `claude-sonnet-4-6`, maps them to real upstream models, and forwards traffic through a local Claude gateway. The upstream can be Anthropic Messages compatible or OpenAI Chat Completions compatible.
- **Claude Code** can use local Gateway Route mode for unified routing, or Direct Provider mode for third-party Anthropic-compatible endpoints.
- **Codex App** uses OpenAI Responses API, while many third-party providers only support Chat Completions. Gateway Switch exposes a local `/v1/responses` endpoint, converts Codex requests to `/v1/chat/completions`, then converts responses back into Responses format.

Providers are shared, but protocol URLs are separate. Codex uses the OpenAI Base URL. Claude and Claude Code prefer the Anthropic Base URL so one provider URL is not accidentally reused by incompatible clients.

---

## Architecture & Engineering

Gateway Switch is built in **Rust + React/Tauri** with ~2,700 lines of backend Rust code. Beyond simple request forwarding, it implements a comprehensive runtime compatibility layer that makes third-party models work reliably with AI-native clients that expect specific protocol semantics.

### Multi-Protocol Runtime Conversion

Gateway Switch bridges **three distinct API surfaces** with bidirectional conversion:

```
Claude Desktop ──→ Anthropic Messages API ──→ Gateway ──→ Upstream provider
                                                  ↓
                                    Anthropic Messages  (preferred)
                                    Chat Completions    (automatic fallback)

Claude Code ────→ Anthropic Messages API ──→ Gateway ──→ Anthropic Base URL

Codex App ──────→ OpenAI Responses API ──→ Gateway ──→ OpenAI Chat Completions
```

The Claude gateway performs **automatic protocol fallback**: it first tries the Anthropic Messages endpoint on the upstream provider. If the provider does not support `/v1/messages` (common with Chinese mainland providers like XiaoMiMo), the gateway transparently converts the request to OpenAI Chat Completions format, sends it to the provider's `/v1/chat/completions` endpoint, and converts the response back — all invisible to Claude Desktop.

The Codex gateway handles the **Responses API ↔ Chat Completions** conversion, including streaming SSE event remapping (`response.created`, `response.output_text.delta`, `response.function_call_arguments.delta`, `response.completed`), `instructions` → system message conversion, `function_call_output` → tool message conversion, and `max_output_tokens` → Chat Completions token mapping (using `max_completion_tokens` for Xiaomi/MiMO).

### Tool-Call Repair & Reliability Engine

Third-party models frequently emit malformed tool calls — unquoted JSON keys, trailing commas, single quotes, or tool arguments wrapped in prose. Gateway Switch includes a **multi-layer tool-call repair pipeline**:

1. **JSON argument repair** (`repair_json_object`): Extracts JSON objects from surrounding text, fixes unquoted keys, converts single quotes to double quotes, removes trailing commas. Applied to both sync and streaming tool-call arguments before forwarding to clients.

2. **Fake tool-call detection** (`detect_fake_tool_call`): Identifies text that *claims* a tool was called without an actual tool block — patterns like "I called the tool", "I read the file", "我已经调用...". The Claude gateway attaches `gateway_warning` to suspicious SSE text deltas.

3. **Missing tool-call retry** (`has_action_description` + `tool_choice: "required"`): When the Codex gateway detects that a model described planned actions in text ("Let me read the file...", "我来查看...") without emitting structured `tool_calls`, it automatically retries the upstream request with `tool_choice: "required"` to force tool invocation. This fixes the #1 cause of Codex conversation stalls with Chinese mainland models.

4. **`finish_reason` tracking**: The gateway parses `finish_reason` from upstream SSE streams. Truncated responses (`"length"`) are reported as `status: "incomplete"` instead of `"completed"`, so Codex knows when to request more output.

5. **Stream timeout enforcement**: A 120-second timeout on upstream stream reads prevents indefinite hangs when providers stop responding mid-stream.

### Secret Redaction Engine

Before any request metadata is written to the local SQLite log database, the **Secret Redaction Engine** scans error summaries and replaces sensitive patterns:

- OpenAI API keys (`sk-...`)
- Anthropic API keys
- GitHub tokens (`ghp_...`, `gho_...`)
- JWT-like strings
- AWS access keys
- PEM blocks
- Generic bearer tokens

This ensures that API keys or tokens accidentally included in upstream error messages never persist to disk.

### Safety Gates (MCP / Shell / Patch)

Gateway Switch includes pre-built safety infrastructure for agent-like execution workflows. These gates are implemented in `compatibility.rs` and exposed as Tauri commands, ready for future execution entry points:

- **MCP Path Safety** (`mcp_path_safety`): Blocks access to `.env`, `.ssh/`, private key files (`id_rsa`, `id_ed25519`), token/cookie-like paths, path traversal attempts, and absolute paths outside the workspace root.

- **Command Safety Gate** (`command_safety`): Blocks high-risk shell patterns — `rm -rf`, `sudo`, recursive `chmod`, `curl | bash`, global package installs, and direct system path mutations.

- **Patch Validator** (`validate_patch`): Validates unified diff patches for recognizable file headers, unsafe paths, missing hunks, and malformed `---`/`+++` headers. Includes a **Patch Repair Engine** that can fix common header drift by adding `a/`/`b/` prefixes.

- **Fake Action Detector** (`detect_fake_action`): Detects text claiming an action was performed ("I edited the file", "我已经修改了...") without actual execution evidence.

### Provider Capability Profiling

Gateway Switch automatically infers **provider capability profiles** from metadata:

| Capability | Detection |
|---|---|
| Messages API | Anthropic Base URL present |
| Chat Completions | Default (all providers) |
| Responses API | OpenAI-like provider ID |
| Tool Use | OpenAI / Qwen / Claude-like |
| Vision | OpenAI / Qwen / Claude-like |
| Reasoning | DeepSeek / Qwen / OpenAI |
| Streaming | Default (all providers) |
| JSON Stability | High (OpenAI/Claude) / Medium / Low |
| Tool-call Accuracy | High / Medium (Qwen) / Low |
| Max Context | 32K – 128K inferred from provider |

Claude and Codex `/health` endpoints expose these profiles, so external tools can inspect runtime readiness without opening the desktop UI.

### Context Compression & Agent Recovery

For long-running agent workflows:

- **Context Compression** (`compress_context`): Implements a sliding-window compression strategy with tool-state pinning. Recent messages and tool-related messages are preserved while older context is summarized.

- **Agent State Recovery** (`recover_agent_state`): Reconstructs a lightweight state object from a conversation — plan, files touched, commands run, errors seen, patches applied, and suggested next action. This reduces long-task drift when an agent resumes work after losing context.

### Compatibility Benchmark Suite

`benchmark_provider` grades providers across 8 dimensions:

| Dimension | Grade A | Grade B | Grade C |
|---|---|---|---|
| Chat | All providers | — | — |
| Tool Use | OpenAI/Claude + high accuracy | Tool-capable | Others |
| MCP | Tool + system prompt support | Tool-capable | Others |
| Artifacts | Tool + high JSON stability | Medium stability | Others |
| Long Context | 128K+ | 32K+ | <32K |
| Responses Compat | Native Responses API | Chat Completions | Others |
| Patch Quality | Tool + high JSON stability | Tool-capable | Others |
| Agent Recovery | 128K + tool support | 32K+ | Others |

### Diagnostics Export

`export_diagnostics` generates a comprehensive JSON bundle containing: runtime feature status, all provider capability profiles, benchmark results, provider configurations, route configurations, Codex route configurations, and recent request logs — everything needed to reproduce and debug an issue remotely.

---

## Version 1.7.2 Highlights

- **Added the MCP Sync module.** A dedicated `MCP Sync` page now checks, previews, and synchronizes MCP Servers across Claude Desktop, Claude Code, and Codex.
- **Added target status cards and sync preview.** The page shows config paths, formats, parse status, server counts, writability, conflict counts, merged servers, and execution logs.
- **Added native Rust synchronization.** The backend reads JSON/TOML configs directly, merges `mcpServers` / `mcp_servers`, creates backups before writes, and preserves non-MCP fields.
- **Protected secret display.** The UI shows only `env` / `headers` key names and never displays token or API key values.
- App version is now `1.7.2`.
- Latest verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (32 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## Version 1.7.1 Highlights

- **Enhanced Claude Desktop binding.** Route display names are written into Claude Desktop `displayName`, and exposed models default to `supports1m: true` for the newer 1M-context variant toggle.
- **Fixed Claude health-check feedback.** The Claude page now shows the latest health result inside the gateway status card and surfaces success or failure feedback.
- **Reordered Claude and Codex layouts.** Claude now uses the requested gateway/binding, route editor, aliases, route/exposed-models, and route-table order; Codex now places binding and context notes side by side on row 2.
- **Completed Chinese localization pass.** Claude Code runtime notes, Codex context and route-copy text, and Cold Start Doctor diagnostics no longer show large English-only sections in Chinese mode.
- **Removed debug leftovers.** Formal builds no longer include the temporary Codex stream-disconnect instrumentation or debug files.
- App version is now `1.7.1`.
- Latest verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (29 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## Version 1.7.0 Highlights

- **Added Cold Start Doctor.** New readiness and safe-repair workflow for Claude Desktop, Claude Code, Codex App, local gateways, providers, routes, and security risk checks.
- **Added bilingual UI support.** Chinese is the default language, English can be enabled from Settings, and diagnostic terms such as Claude, Codex, Gateway, Provider, Responses API, and Chat Completions remain in English.
- **Fixed Codex 413 large-request failures.** The local Responses gateway now has an explicit request body limit above Axum's default so multi-turn Codex payloads with history and tool outputs are accepted.
- **Fixed Volcengine Ark Coding Plan 404 routing.** OpenAI Base URLs ending in `/v2`, `/v3`, or `/api/coding/v3` are now treated as versioned bases, and complete `/chat/completions` endpoint URLs are preserved instead of receiving an extra `/v1` segment.
- **Fixed Volcengine `developer` role compatibility.** Codex Responses `developer` messages are normalized to Chat Completions `system` messages so providers that only accept `system`, `assistant`, `user`, and `tool` roles can handle Codex traffic.
- App version remains `1.7.0`.
- Latest verification: `pnpm build`, `cargo test` (28 passed), and `CI=false pnpm tauri build --bundles app,dmg`.

## Version 1.6.4 Highlights

- **Fixed Xiaomi MiMO Codex routing failures with 502 / `Param Incorrect`.** The latest Xiaomi MiMO OpenAI-compatible API enables thinking by default for `mimo-v2.5` / `mimo-v2.5-pro`; during multi-turn tool conversations, upstream requires historical `reasoning_content` to be replayed. Gateway now injects `thinking: {"type":"disabled"}` for Xiaomi/MiMO Codex conversion requests by default, preventing upstream rejection during Codex tool workflows.
- Xiaomi/MiMO Codex requests now map `max_output_tokens` to `max_completion_tokens`, matching the current MiMO OpenAI API contract instead of sending the less compatible `max_tokens` field.
- Explicit caller-provided `thinking` controls are preserved, and the compatibility default is scoped only to Xiaomi/MiMO routes so other providers are unaffected.
- Added focused Rust tests for Xiaomi/MiMO Codex compatibility, covering thinking disablement and token parameter renaming.
- App version is now `1.6.4`.
- Latest verification: `pnpm build`, `cargo test`, and `pnpm tauri build --bundles app`.

## Version 1.6.3 Highlights

- New **Claude Warm Native** UI with white surfaces, warm paper backgrounds, ink text, Claude oxblood accents, low-saturation status colors, and a lighter macOS-native utility feel.
- Reworked the left navigation into a compact icon rail that preserves Dashboard, Claude, Claude Code, Codex, Providers, Logs, and Settings while freeing more space for the main workbench.
- Redesigned the App Icon and status bar icon around a white `Gateway Pin` symbol with a Claude oxblood route point, representing multi-client routing through the local gateway to upstream providers.
- Updated the frontend design system to Geist / Fraunces / Geist Mono and refreshed cards, tables, forms, buttons, badges, and health indicators.
- App version is now `1.6.3`.
- Latest verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (29 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## Version 1.6.2 Highlights

- **Core fix: Codex conversation stall.** When third-party models (DeepSeek, MiMo, Qwen, etc.) describe actions in text ("Let me read the file...") instead of emitting structured `tool_calls`, Codex treats the turn as complete and stops. The gateway now detects this pattern and automatically retries the upstream request with `tool_choice: "required"` to force tool invocation.
- Added `finish_reason` detection: when upstream returns `finish_reason: "length"` (truncated output), `response.completed` now correctly sets `status: "incomplete"` instead of `"completed"`, letting Codex know the response was cut short.
- Added stream timeout (120 seconds): when the upstream provider hangs or sends no data for too long, the gateway no longer waits indefinitely — it disconnects and reports a timeout error.
- Enhanced system prompt: when tools are present, injects a stronger bilingual system prompt that lists available tool names and instructs the model to use `tool_calls` instead of describing actions in text.
- Stream errors no longer appear as normal completions: when the upstream stream breaks, `response.completed` sets `status: "failed"` and logs the `finish_reason` for diagnostics.
- App version is now `1.6.2`.
- Latest verification: `pnpm build` passed and `cargo test` passed with `23 passed`.

## Version 1.6.1 Highlights

- Fixed blank browser previews caused by missing Tauri `invoke()` internals. The Vite browser preview now loads mock data outside Tauri so AI agents and developers can inspect the UI.
- Reduced full-state polling from 3 seconds to 12 seconds and pause polling while the page is hidden.
- Improved Codex Gateway agent compatibility for third-party Chat Completions models: when Codex provides tools, converted requests now instruct the model to emit structured `tool_calls` and default to `tool_choice: "auto"`.
- Fixed streaming Responses event ordering for Codex tool calls so clients do not treat a completed assistant message as the end of the turn before tool calls are closed.
- Added JSON repair for streaming tool-call arguments, improving execution reliability for providers that emit loose JSON.
- Added `CLAUDE.md` with project directory, command, and preview notes for future AI handoffs.
- App version is now `1.6.1`.
- Latest verification: `pnpm build` passed and `cargo test` passed with `23 passed`.

## Version 1.6.0 Highlights

- Upgraded the backend from a forwarding layer into an Anthropic-compatible and Responses-compatible runtime compatibility layer.
- Added Provider Capability Profile and Codex Capability Profile for Chat, Tool Use, Vision, Reasoning, Long Context, Responses, and Patch readiness.
- Claude Gateway now repairs malformed tool-call JSON and can warn about fake tool-call text in Anthropic SSE output.
- Codex Gateway now supports string `input`, sync `function_call` output items, and streaming function-call argument events.
- Added Secret Redaction Engine for API keys, GitHub tokens, JWTs, PEM blocks, and log summaries.
- Added runtime safety and diagnostics: Command Safety Gate, MCP Path Safety, Patch Validator/Patch Repair, Fake Action Detector, Context Compression, Long Task State Tracker, Agent Recovery, Compatibility Benchmark, and Diagnostics Export.
- Fixed stream request-id logging so one request can be traced through provider, real upstream model, duration, and errors.
- Fixed provider persistence so `base_url` is no longer overwritten by `openai_base_url`.
- App version is now `1.6.0`.

---

## Features

### Dashboard

- View Claude Gateway and Codex Gateway status.
- View binding status and latest upstream call.
- Run Claude/Codex health checks.
- Refresh current state.

Dashboard does not start gateways or bind apps. Startup and binding live on the relevant product page.

### Claude

- Manage Claude model aliases.
- Create routes: `Claude Alias -> Provider -> Upstream Model`.
- Start/stop Claude Gateway.
- Bind or restore Claude Desktop.
- Supports Anthropic Messages streaming and non-streaming forwarding.
- Supports automatic adaptation for OpenAI Chat Completions upstreams, useful for providers that only expose `/v1/chat/completions`.

Default address:

```text
http://127.0.0.1:3456
```

### Claude Code

- Bind Claude Code independently from Claude Desktop.
- `Gateway Route`: writes the local Claude Gateway, useful for unified routing and Chat Completions fallback.
- `Direct Provider`: writes the provider's Anthropic Base URL, API key, and model name directly into Claude Code.
- Direct Provider is intended for providers that expose an Anthropic-compatible endpoint, such as XiaoMiMo with `https://.../anthropic`.

### Codex

- Manage Codex model names.
- Create routes: `Codex Model -> Provider -> Upstream Model`.
- Start/stop Codex Gateway.
- Bind or restore Codex App.
- Convert Responses API requests to Chat Completions.
- Convert Chat Completions responses back to Responses format.
- Verify the latest real upstream model directly on the Codex page.

Default address:

```text
http://127.0.0.1:3457
```

### Providers

- Manage shared third-party providers.
- Configure OpenAI Base URL, Anthropic Base URL, Auth Header, Auth Scheme, and API Key.
- Use built-in presets or custom providers.
- Claude, Claude Code, and Codex share provider identity and keys, but use protocol-specific Base URLs.

### Logs

- View request time, requested model, provider, real upstream model, status code, and duration.
- Use logs to verify which model was actually called.
- Error summaries are redacted before storage to avoid saving API keys or tokens.

### Runtime Compatibility

- Provider/Codex capability profiles and compatibility benchmarks.
- Tool-call JSON repair and fake tool/action detection.
- MCP path safety, command safety, patch validation, and patch repair.
- Context compression, long-task state recovery, and diagnostics export.
- These capabilities live mainly in `src-tauri/src/compatibility.rs` and are exposed through Tauri commands.

---

## Quick Start

### Claude Desktop Routing

1. Open `Providers` and add a provider. Use `/v1` or an equivalent Chat Completions URL for OpenAI Base URL; use `/anthropic` or an equivalent Messages URL for Anthropic Base URL.
2. Open `Claude` and add or select a Claude alias.
3. Create a route and enter the real upstream model name.
4. Start Claude Gateway from the `Claude` page.
5. Bind Claude Desktop from the `Claude` page.
6. Restart Claude Desktop and use the mapped Claude model.

### Claude Code Binding

1. Open `Providers` and confirm the target provider has an Anthropic Base URL.
2. Open `Claude Code`.
3. Select `Direct Provider`, choose the provider, and enter the real upstream model, such as `mimo-v2.5`.
4. Click `Bind Claude Code`.
5. Restart Claude Code or open a new session, then choose the bound model.

If the provider does not have an Anthropic Base URL, use `Gateway Route` so the local gateway can handle protocol conversion.

### Codex App Routing

1. Open `Providers` and add a provider that supports OpenAI Chat Completions.
2. Open `Codex` and add or select a Codex model, such as `gpt-5.5`.
3. Create a route and enter the real upstream model name.
4. Select the default Codex model.
5. Click `Start & Bind Codex App`.
6. Restart Codex App and start using it.

Binding writes:

```toml
model_provider = "gateway-switch"
model = "gpt-5.5"
preferred_auth_method = "apikey"

[model_providers.gateway-switch]
name = "Gateway Switch"
base_url = "http://127.0.0.1:3457/v1"
wire_api = "responses"
requires_openai_auth = false
experimental_bearer_token = "gateway-switch-token"
```

---

## Provider URLs And Auth

Recommended URL split:

```text
OpenAI Base URL: https://provider.example.com/v1
Anthropic Base URL: https://provider.example.com/anthropic
```

Codex only uses OpenAI Base URL. Claude Code Direct Provider only uses Anthropic Base URL.

Common setup:

```text
Auth Header: Authorization
Auth Scheme: Bearer
API Key: sk-...
```

For providers that require `x-api-key`:

```text
Auth Header: x-api-key
Auth Scheme:
API Key: your-key
```

When `Auth Scheme` is empty, Gateway Switch sends the raw API key in the configured header.

Note: `Local Gateway Auth` / `x-api-key` in the Claude Desktop binding is the auth method from **Claude Desktop to local Gateway Switch**. Provider auth such as `Authorization: Bearer ...` is the auth method from **Gateway Switch to the third-party model service**. They are separate links and do not need to match.

---

## Claude, Claude Code, And Codex Protocols

The products can share the same Provider/API key, but they should not share one protocol URL:

- Claude uses `http://127.0.0.1:3456/v1/messages` and presents an Anthropic Messages API surface.
- Claude Code Direct Provider uses the provider's Anthropic Base URL.
- Codex uses `http://127.0.0.1:3457/v1/responses` and presents an OpenAI Responses API surface.

When a Claude route points to a Chat Completions-only upstream, Gateway Switch first tries `/v1/messages`; if unsupported, it automatically falls back to `/v1/chat/completions`.

---

## Verify The Real Model

After sending a request from Claude or Codex:

1. Return to Gateway Switch.
2. Open the `Codex` page and check `Verify Real Model`.
3. Or open `Logs` for full history.

Important fields:

- `Requested Model`: the model requested by the client.
- `Provider`: the matched provider.
- `Real Upstream`: the actual model sent to the third-party API.

---

## Codex Reasoning Notes

Gateway Switch only converts protocol shape. It does not add or remove model reasoning capability.

If a third-party provider does not return reasoning fields through Chat Completions, Codex may only show final text. Fast replies are normal and depend on the upstream model, provider behavior, and prompt complexity.

---

## Download

Download the latest `.dmg` from [Releases](https://github.com/gcristiano0624-bot/gateway-switch/releases/).

Requirements:

- macOS 12+
- Claude Desktop or Codex App, depending on your use case

---

## Build From Source

Requirements:

- Node.js 18+
- pnpm 8+
- Rust 1.85+
- Xcode Command Line Tools

Commands:

```bash
pnpm install
pnpm build
cd src-tauri && cargo test
cd ..
pnpm tauri build
```

Artifacts:

```text
src-tauri/target/release/bundle/macos/Gateway Switch.app
src-tauri/target/release/bundle/dmg/Gateway Switch_1.7.2_aarch64.dmg
```

---

## Technical Documentation

Full architecture, protocol conversion, database schema, binding strategy, and release process:

[docs/project.md](./docs/project.md)

---

## Data Storage

Gateway Switch app data:

```text
~/Library/Application Support/Gateway Switch/
```

Claude Desktop config:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

Codex App config:

```text
~/.codex/config.toml
```

---

## Known Limits

- Claude Code Direct Provider requires an Anthropic Messages compatible upstream.
- Codex Gateway requires an OpenAI Chat Completions compatible upstream.
- Claude Gateway fallback requires OpenAI Chat Completions compatibility.
- Version 1.6.0 includes MCP/Shell/Patch safety gates, but the current app does not include a real MCP or shell executor. Future execution entry points should reuse these gates.
- Visible Codex reasoning depends on whether the upstream returns reasoning data.
- Codex conversation history across different login/provider states is controlled by Codex App, not Gateway Switch.
