# Gateway Switch Project Documentation

Version: 1.12.0

This document is the single technical source of truth for Gateway Switch. It merges the former project architecture notes and the Codex Gateway notes into one maintained file.

## 0. AI Handoff Summary

If another AI receives this repository, start here:

- Product: macOS Tauri app that routes Claude Desktop, Claude Code, and Codex App to third-party model providers.
- Current version: `1.12.0`.
- Main frontend: `src/App.tsx` and `src/App.css`.
- Main backend: `src-tauri/src/*.rs`.
- Claude gateway: `src-tauri/src/gateway.rs`, local Anthropic Messages surface on `127.0.0.1:3456`.
- Codex gateway: `src-tauri/src/codex_gateway.rs`, local OpenAI Responses surface on `127.0.0.1:3457`.
- Runtime compatibility layer: `src-tauri/src/compatibility.rs`.
- Database and migrations: `src-tauri/src/database.rs`.
- Tauri command bridge: `src-tauri/src/commands.rs` and command registration in `src-tauri/src/lib.rs`.
- Bindings: Claude Desktop in `desktop_binding.rs`, Claude Code in `claude_code_binding.rs`, Codex in `codex_binding.rs`.
- Local data: `~/Library/Application Support/Gateway Switch/`.
- Build: `PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build` when `cargo` is not already on `PATH`.
- Latest verified tests: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (32 passed, 3 ignored), `PATH="$HOME/.cargo/bin:$PATH" cargo test -- --ignored --nocapture --test-threads=1` (3 passed on real Codex.app), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build` after the 1.8.1 codex++ repair hotfix.

The design intent is to make third-party models less likely to degrade Claude/Codex behavior by normalizing protocol shapes, repairing common tool-call failures, redacting secrets, exposing provider capability profiles, and adding safety/diagnostic gates around agent-like workflows.

## 1. Product Goal

Gateway Switch is a macOS desktop app for routing Claude Desktop, Claude Code, and Codex App traffic to third-party model providers.

The app solves two related but different protocol problems:

1. Claude Desktop expects Anthropic Messages API semantics and validates Claude model names. Gateway Switch exposes Claude-compatible aliases, rewrites requests to the real upstream model, first tries an Anthropic Messages upstream, and can automatically fall back to an OpenAI Chat Completions upstream when the provider does not support `/v1/messages`.
2. Claude Code can either use the local Claude Gateway or bind directly to an Anthropic-compatible provider endpoint through `~/.claude/settings.json`.
3. Codex App expects OpenAI Responses API semantics. Many third-party providers only support OpenAI Chat Completions. Gateway Switch exposes a local `/v1/responses` endpoint, converts Responses requests into Chat Completions requests, then converts sync and streaming Chat Completions responses back into Responses-shaped output.

The shared design goal is simple: providers share identity, auth header, auth scheme, and API key, but they do not share one universal Base URL. Provider URLs are split by protocol: OpenAI Base URL for Codex and Chat Completions fallback, Anthropic Base URL for Claude and Claude Code direct requests.

## 3. Version 1.12.0 Unified Diagnostics and Provider Presets Scope

Version 1.12.0 combines the planned diagnostics-center milestone with Provider Presets so Gateway Switch behaves like a cross-product operations console.

Main changes:

- A Unified Diagnostics Center aggregates Claude Desktop, Claude Code, Codex Gateway, Codex++, Providers, and Install / Runtime health into one report with section scores and action recommendations.
- Failed request snapshots are clustered by provider, surface, and status code, then mapped to compatibility suggestions such as `system_to_user`, `tool_to_user`, `strip_unsupported_params`, `codex_strip_reasoning`, rate-limit handling, or provider outage review.
- Built-in Provider Presets cover OpenRouter, Volcengine Ark DeepSeek, DeepSeek official, Moonshot Kimi, Qwen DashScope, Xiaomi MiMo, standard Anthropic-compatible providers, and generic OpenAI Chat providers.
- Applying a preset creates or updates provider URLs and writes a recommended compatibility policy while preserving existing API keys when no new key is supplied.
- The UI adds a System > Diagnostics page and a Provider Presets section under Providers.
- Regression tests cover failure recommendation mapping, preset safety defaults, diagnostics status derivation, version comparison, and safe install warnings.

## 4. Version 1.10.0 Unified Compatibility Diagnostics Scope

Version 1.10.0 completes the remaining compatibility roadmap by turning the v1.9.0 diagnostics layer into a configurable control plane shared by Claude, Claude Code, and Codex.

Main changes:

- Failed Claude gateway requests are stored as sanitized diagnostic snapshots with original payload JSON, converted upstream JSON, redaction summary, and likely-cause analysis.
- Provider Compatibility Policies persist nullable manual overrides for Claude and Codex strategy flags while inheriting automatic profiles when fields are unset.
- Claude Code can repair unsafe Direct Provider bindings by backing up settings and rebinding to Gateway Route.
- Codex Gateway consumes the same effective Provider Compatibility Profile and exposes route diagnostics for Responses fallback, reasoning cleanup, and strict tool-call behavior.
- Settings includes a GitHub Release update checker and a safe install plan that warns when the app runs from DMG or temporary paths.
- Automatic profiles now cover OpenRouter, Xiaomi MiMo, DeepSeek official, Moonshot Kimi, Qwen DashScope, Volcengine Ark, standard Anthropic, and OpenAI Chat fallback providers.
- Regression tests cover provider policy persistence, diagnostic snapshots, replay redaction, Codex diagnostics, version comparison, and safe install warnings.

## 5. Version 1.9.0 Compatibility Diagnostics Scope

Version 1.9.0 turns the Volcengine DeepSeek Gateway Route hotfix into a provider compatibility diagnostics layer.

Main changes:

- Claude routes expose Provider Compatibility Profiles such as `standard_anthropic`, `openai_chat_fallback`, and `volcengine_deepseek_coding`.
- Claude Code shows Route Diagnostics that explain Direct Provider safety, Gateway Route recommendations, and system/tool role conversion behavior.
- Payload Preview uses a fixed redacted sample request to show the converted upstream Chat payload without calling the upstream provider.
- Runtime Source Report warns when the app runs from `/Volumes`, `/tmp`, or another non-standard location instead of `/Applications`.
- Regression tests cover compatibility profile selection, route diagnostics, payload preview role conversion, and runtime source classification.

## 6. Version 1.8.8 Upstream Tweak Store Scope

Version 1.8.8 makes Gateway Switch a more faithful Codex++ upstream store console while keeping safe install boundaries.

Main changes:

- The Codex++ market page now includes an Upstream Tweak Store panel backed by `https://b-nnett.github.io/codex-plusplus/store/index.json`.
- Store entries show GitHub repo, approved commit, derived archive URL, installed status, and installed path.
- Legacy requested script names remain visible, but Gateway Switch labels missing exact upstream matches truthfully and maps usage-related items to `co.bennett.ui-improvements` when available.
- Store validation requires schema version 1, safe `owner/repo` values, manifest repo consistency, and full 40-character approved commit SHAs before any install URL is derived.
- Cold Start check/repair logic is split into `src-tauri/src/coldstart.rs`, with `commands.rs` retaining only the Tauri command bridge.

## 5. Version 1.8.5 Recommended Scripts Scope

Version 1.8.5 adds a safe Recommended Scripts layer for the Codex++ script-market items the user explicitly wanted restored.

Main changes:

- The Codex++ market page now includes a Recommended Scripts panel for `Codex Context Used Meter`, `Hide Usage Alert`, `Codex Token Usage`, and `Codex List Pagebuster`.
- Backend status detection reports `codex_user_scripts` only when the installed runtime exposes native user-script markers and a known script storage directory exists.
- Install actions are safely gated: if storage is `unknown`, Gateway Switch returns a clear error and writes no files.
- The existing Tweak Store grid remains unchanged, so approved tweak installation continues to work separately.
- Regression tests cover both unknown storage and detected native script storage.

## 5. Version 1.8.4 UI Scope

Version 1.8.4 focuses on sidebar navigation clarity and adaptive desktop window layout.

Main changes:

- The left navigation is regrouped into Dashboard, Products, Features, General, and System.
- `Claude Code`, `MCP Sync`, and `Cold Start` sidebar labels use single-line text instead of manual `<br />` breaks.
- The sidebar width is fluid on desktop and collapses to an icon-only rail below the narrow-window breakpoint.
- Main content, cards, provider grids, forms, and tables add safer responsive constraints to avoid broken wrapping and horizontal overflow.
- The Tauri minimum window size is reduced to `760x560`.

## 6. Version 1.8.3 Hotfix Scope

Version 1.8.3 focuses on Codex++ watcher reliability after running Gateway Switch from a DMG.

Main changes:

- Native CLI shim and watcher generation avoid transient `/Volumes/Gateway Switch/...` executable paths and prefer `/Applications/Gateway Switch.app`.
- The launchd watcher command now matches Codex++ runtime health expectations: `CODEX_PLUSPLUS_WATCHER=1 codexplusplus update --watcher --quiet`.
- Staged Codex++ runtime files patch watcher health detection to accept `launchctl print gui/$UID/com.codexplusplus.watcher` on modern macOS.
- Watcher health logs are reset and written to `~/Library/Logs/codex-plusplus-watcher.log`, which is the path the Codex++ settings page reads.

## 7. Version 1.8.2 Hotfix Scope

Version 1.8.2 focuses on debuggability and safe page-enhancement fallback behavior.

Main changes:

- Native repair writes detailed compatibility diagnostics to `~/Library/Application Support/codex-plusplus/log/native-debug.log`.
- `native-install.log` remains concise and points users to the debug file when deeper detail is available.
- `app.asar.unpacked` diagnostics include required native file status, file sizes, `.node` binary count, and sample module paths.
- Node/npm diagnostics include raw GUI/launchd PATH, augmented PATH, and per-candidate path resolution.
- `codexPlusPlus.uiSafeMode` is now a first-class config key with default `false`. Setting it to `true` disables only `co.bennett.ui-improvements`.

## 8. Version 1.8.1 Hotfix Scope

Version 1.8.1 is a focused hotfix for Codex++ native repair stability and Claude Desktop route error handling.

Main changes:

- Native repair validates `Codex.app/Contents/Resources/app.asar.unpacked` before continuing. If required native modules such as `better_sqlite3.node` are missing, it restores them from codex++ backup directories and then re-signs the app.
- Repair logs now show unpacked artifact health, backup candidates, restore source, Node/npm resolution, and the PATH used by npm subprocesses.
- Node/npm discovery no longer depends only on the GUI or launchd PATH; it also searches `/usr/local/bin`, `/opt/homebrew/bin`, and standard system paths.
- The Codex++ enhancement UI includes a UI Safe Mode that disables only `co.bennett.ui-improvements`, keeping route, script market, history repair, watcher, and CLI features active.
- Claude Desktop route handling now avoids fallback loops for explicit upstream errors such as `413 Request too large`.

Verification for 1.8.1:

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test codex_pp::native_install_acceptance_tests::native_real_repair_smoke -- --ignored --nocapture --test-threads=1`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## 9. Version 1.8.0 Scope

Version 1.8.0 focuses on turning codex++ support from an external wrapper into a native Gateway Switch product capability with real-machine validation against `Codex.app`.

Main changes:

- Added a native Rust codex++ install / repair flow that handles source download, extract, source switching, runtime staging, app backup, app patch, signing, watcher installation, and rollback.
- Added streaming codex++ execution logs and install preflight checks in the desktop UI so users can inspect Node/npm readiness and live step progress before and during install.
- Added native local signing identity creation and reuse for `install-local`, including re-signing Mach-O binaries under `app.asar.unpacked` and the outer `Codex.app` bundle.
- Added native default tweak installation with GitHub release fallback behavior.
- Migrated CLI shim generation to `gateway-switch codexpp ...` so shell usage no longer depends on the upstream Node CLI entrypoint.
- Migrated the `launchd` watcher to the same native `gateway-switch codexpp repair` entrypoint.
- Added ignored real acceptance tests for install-local success, injected rollback, and repair recovery on a real `/Applications/Codex.app`.
- Versioned package metadata and release artifacts as `1.8.0`.

Verification for 1.8.0:

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`: passed, 32 passed and 3 ignored.
- `PATH="$HOME/.cargo/bin:$PATH" cargo test -- --ignored --nocapture --test-threads=1`: passed, 3 real-machine acceptance tests.
- `pnpm build`: passed.
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`: passed.

## 5. Version 1.7.2 Scope

Version 1.7.2 focuses on integrating the standalone MCP synchronization workflow into Gateway Switch as a native first-class module.

Main changes:

- Added a dedicated `MCP Sync` page after Codex in the sidebar.
- Added target status cards for Claude Desktop, Claude Code, and Codex, including config path, format, parse status, server count, writable state, and backup information.
- Added sync preview for merged MCP Servers, source badges, conflict resolution, completeness score, and secret-key masking.
- Added native Rust MCP sync logic in `src-tauri/src/mcp_sync.rs`, covering JSON/TOML extraction, name-based merge, backup creation, and write-back while preserving non-MCP fields.
- Added Tauri commands `get_mcp_sync_status`, `preview_mcp_sync`, and `run_mcp_sync`.
- Versioned package metadata and release artifacts as `1.7.2`.

Verification for 1.7.2:

- `pnpm build`: passed.
- `PATH="$HOME/.cargo/bin:$PATH" cargo test`: passed, 32 tests.
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`: passed.

## 5. Version 1.7.1 Scope

Version 1.7.1 focuses on Claude Desktop developer-mode binding metadata, Claude/Codex page layout polish, visible health-check feedback, and Chinese localization completeness.

Main changes:

- Claude Desktop binding now writes each enabled route as `{ name, displayName, supports1m }`, using the route display name for `displayName` and enabling `supports1m` by default.
- The Claude page health-check action now renders the latest result inside the Claude Gateway status card and shows success/failure feedback.
- The Claude page layout now follows the requested sequence: gateway status plus binding status, route editor, Claude aliases, route cards plus exposed models, and route table.
- The Codex page layout now keeps gateway status plus real-model verification on the first row, and places Codex App binding plus context/reasoning notes on the second row.
- Chinese localization was completed for Claude Code runtime cards, Codex context and route-helper copy, and Cold Start Doctor diagnostic output.
- Temporary Codex stream-disconnect debug instrumentation and local debug files were removed from the formal release tree.
- Versioned package metadata and release artifacts as `1.7.1`.

Verification for 1.7.1:

- `pnpm build`: passed.
- `PATH="$HOME/.cargo/bin:$PATH" cargo test`: passed, 29 tests.
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`: passed.

## 5. Version 1.7.0 Scope

Version 1.7.0 adds the Cold Start Doctor, fixes the left navigation readability issue, introduces a bilingual Chinese/English interface, and finalizes Codex compatibility for large payloads and Volcengine Ark Coding Plan.

Main changes:

- Reworked the left sidebar from icon-only hover labels to permanent icon + text labels, matching the selected visual direction while keeping the compact rail layout.
- Fixed the Gateway Switch version label offset by removing the floating sidebar brand tooltip and moving the stable version indicator to the footer.
- Added bilingual UI support. Chinese is the default language, English remains available from Settings, and required technical terms such as Claude, Codex, Gateway, Provider, API Key, Base URL, MCP, Responses API, and Chat Completions stay in English for diagnostic clarity.
- Added a persistent `language` field to app settings with a backward-compatible default for existing installations.
- Added a new `Cold Start` tab that combines three phases: readiness overview, execution/repair log, and capability matrix.
- Added backend cold-start checks for Claude Desktop, Claude Code, Codex App, local gateway processes, health endpoints, provider inventory, route inventory, and third-party routing security risk.
- Added a safe repair command that can start stopped local gateways and apply backup-backed Claude Desktop / Codex bindings when routes are available.
- Added detailed `[coldstart]` Rust log printing for every major inspection, repair, health check, capability result, and report-generation node.
- Added Markdown cold-start report generation under the app backup directory for post-mortem review and support handoff.
- Added `coldstart/claude_coldstart_skill.md` and `coldstart/codex_coldstart.skill.md` as reference workflows for the checks represented in the UI.
- Merged the Codex large-request stability fix by raising the local Responses gateway body limit above Axum's default and adding regression coverage.
- Fixed OpenAI-compatible Base URL joining for providers whose base already ends with `/v2`, `/v3`, `/api/coding/v3`, or a full `/chat/completions` endpoint. This fixes Volcengine Ark Coding Plan 404 failures caused by incorrectly appending `/v1`.
- Normalized Codex Responses `developer` role messages into Chat Completions `system` messages, preserving compatibility with providers that only allow `system`, `assistant`, `user`, and `tool`.
- Versioned package metadata and release artifacts as `1.7.0`.

Verification for 1.7.0:

- `pnpm build`: passed.
- `cargo test`: passed, 28 tests.
- `CI=false pnpm tauri build --bundles app,dmg`: passed.

## 6. Version 1.6.4 Scope

Version 1.6.4 fixes Xiaomi MiMO Codex routing against the latest MiMO OpenAI-compatible Chat Completions behavior.

Main changes:

- Fixed Codex `502 Bad Gateway` / upstream `Param Incorrect` failures when routing through Xiaomi MiMO `mimo-v2.5` and `mimo-v2.5-pro`.
- Root cause: MiMO thinking mode is enabled by default for these models, and multi-turn tool workflows require historical `reasoning_content` to be passed back. Codex Responses traffic does not preserve that provider-specific field, so the upstream rejected follow-up tool turns.
- Gateway now applies Xiaomi/MiMO-specific Codex compatibility after Responses-to-Chat conversion by defaulting `thinking` to `{ "type": "disabled" }` for Xiaomi/MiMO routes only.
- The compatibility layer preserves explicit caller-provided `thinking` settings, so future callers can opt into MiMO thinking behavior deliberately.
- Xiaomi/MiMO Codex token limits now use `max_completion_tokens` instead of `max_tokens`, matching the current Xiaomi MiMO OpenAI API documentation.
- Added focused Rust tests covering MiMO thinking disablement, explicit thinking preservation, and token parameter renaming.
- Versioned package metadata and release artifacts as `1.6.4`.

Verification for 1.6.4:

- `pnpm build`: passed.
- `cargo test`: passed, 25 tests.
- `pnpm tauri build --bundles app`: passed.
- Manual verification: reinstalling the validation build restored Codex conversation through the Xiaomi MiMO route.

## 6. Version 1.6.3 Scope

Version 1.6.3 is the Claude Warm Native UI and icon refresh release.

Main changes:

- Replaced the visual system with a Claude-inspired warm native desktop style: white surfaces, warm paper backgrounds, ink text, oxblood accents, low-saturation semantic colors, and softer macOS-like rounded cards.
- Reworked the left navigation from a wide text sidebar into a compact icon rail. Hover tooltips keep labels discoverable while the main workspace gains horizontal room.
- Redesigned the in-app brand mark, App Icon, and tray/status icon around a white `Gateway Pin` symbol with a Claude oxblood route point. The symbol communicates routing Claude Desktop, Claude Code, and Codex traffic through Gateway Switch to upstream providers.
- Updated frontend typography to Geist, Fraunces, and Geist Mono to match the new editorial/native design direction.
- Versioned package metadata and release artifacts as `1.6.3`.

Verification for 1.6.3:

- `pnpm build`: passed.
- `cargo test`: passed.
- `pnpm tauri build`: passed.

## 7. Version 1.6.2 Scope

Version 1.6.2 fixes the Codex conversation stall problem when routing through third-party Chat Completions models.

Main changes:

- Fixed Codex conversation stall: when third-party models (DeepSeek, MiMo, Qwen, etc.) describe planned actions in text ("Let me read the file...") without emitting structured `tool_calls`, Codex treats the turn as complete and stops. The gateway now detects this pattern via `has_action_description()` and automatically retries the upstream request with `tool_choice: "required"` to force tool invocation.
- Added `extract_finish_reason()` to parse `finish_reason` from Chat Completions SSE streaming data. Truncated responses (`"length"`) now emit `status: "incomplete"` with `incomplete_details.reason: "max_output_tokens"` instead of misleading `"completed"`.
- Added 120-second stream timeout via `tokio::time::timeout` in the `process_chat_stream!` macro to prevent indefinite waits when upstream providers hang or stop sending data.
- Enhanced system prompt: when tools are present in the request, the gateway now injects a stronger prompt that explicitly lists available tool names and instructs the model to use `tool_calls` instead of describing actions in text.
- Stream errors now set `response.completed` status to `"failed"` instead of `"completed"`, and log `finish_reason` information for post-mortem diagnostics.
- Refactored the streaming handler into a `process_chat_stream!` macro that is reused for both the initial attempt and the retry with `tool_choice: "required"`.
- Request body is cloned before the first `.send()` so the retry can re-send without re-parsing.
- Versioned package metadata and release artifacts as `1.6.2`.

Verification for 1.6.2:

- `pnpm build`: passed.
- `cargo test`: passed, 23 tests.
- `cargo clippy`: no new warnings introduced by the streaming changes.

## 8. Version 1.6.1 Scope

Version 1.6.1 is the AI handoff and Codex agent reliability release.

Main changes:

- Browser/Vite preview now works outside Tauri. `src/App.tsx` detects whether Tauri internals exist and loads realistic mock data when they do not, preventing blank previews and repeated `invoke()` errors.
- Frontend polling was reduced from 3 seconds to 12 seconds and now pauses while `document.hidden` is true, lowering IPC and SQLite read pressure.
- Codex Responses-to-Chat conversion now adds a compatibility system note whenever tools are present. This nudges third-party Chat Completions models to emit structured `tool_calls` instead of saying they will inspect, analyze, run, or edit without actually calling a tool.
- Codex converted Chat requests default to `tool_choice: "auto"` when tools exist and the client did not specify a tool choice.
- Streaming Codex tool calls are closed before the final assistant message completion event. This avoids clients treating the natural-language assistant message as the end of turn before function calls are available.
- Streaming Codex function-call arguments are repaired with the shared JSON repair helper before final `response.function_call_arguments.done` and `response.output_item.done` events.
- Added targeted tests for the Codex tool-call guardrail and streaming argument repair.
- Added `CLAUDE.md` as a fast AI handoff note with the correct project folder, commands, and preview behavior.
- Versioned package metadata and release artifacts as `1.6.1`.

Verification for 1.6.1:

- `pnpm build`: passed.
- `cargo test`: passed, 23 tests.
- Browser preview at `http://localhost:1420/`: rendered Gateway Switch with mock XiaoMiMo/Codex data and no Tauri invoke error.

## 9. Version 1.6.0 Scope

Version 1.6.0 is the runtime compatibility release.

Main changes:

- Added `compatibility.rs`, a runtime compatibility layer for provider profiling, JSON repair, fake tool/action detection, safety gates, patch validation, context compression, agent recovery, benchmark reports, and diagnostics support.
- Added Provider Capability Profile and Codex Capability Profile.
- Claude and Codex health responses now expose provider capabilities.
- Claude tool-call conversion now repairs malformed JSON arguments when possible.
- Claude SSE rewriting can mark likely fake tool-call text with `gateway_warning`.
- Codex Responses conversion now handles string `input`, sync `function_call` output items, and streaming function-call argument events.
- Logs redact common secrets before writing `request_logs.error_summary`.
- Stream logs preserve the original request ID instead of creating a new one at stream completion.
- Provider create/update now preserves `base_url` instead of overwriting it with `openai_base_url`.
- Added Tauri commands for runtime features, compatibility benchmark, command/path safety, patch validation, fake action detection, context compression, agent recovery, and diagnostics export.
- Versioned package metadata and release artifacts as `1.6.0`.

Version 1.5.0 was the protocol-split and Claude Code release. Its core behavior remains: providers store separate `openai_base_url` and `anthropic_base_url`, Claude Code supports Gateway Route and Direct Provider modes, Claude Gateway falls back to Chat Completions when needed, and Codex Gateway uses OpenAI Base URL for Responses-to-Chat conversion.

## 9. High-Level Architecture

```text
Gateway Switch
├─ React UI
│  ├─ Dashboard: status, health, recent traffic
│  ├─ Claude: aliases, routes, Claude Desktop binding
│  ├─ Claude Code: Gateway Route and Direct Provider binding
│  ├─ Codex: models, routes, Codex App binding
│  ├─ Providers: shared upstream provider registry
│  ├─ Logs: request history and real upstream verification
│  └─ Settings: app settings, import/export
├─ Tauri Commands
│  ├─ Provider CRUD
│  ├─ Claude route CRUD
│  ├─ Codex route CRUD
│  ├─ Alias CRUD
│  ├─ Gateway lifecycle commands
│  ├─ Desktop binding commands
│  ├─ Health/log/settings commands
│  └─ Runtime compatibility commands
├─ Rust Gateways
│  ├─ Claude Gateway: :3456, Anthropic Messages surface with Chat Completions fallback
│  └─ Codex Gateway: :3457, OpenAI Responses compatible
├─ Runtime Compatibility Layer
│  ├─ Provider/Codex capability profiles
│  ├─ Tool-call repair and fake action detection
│  ├─ MCP path, shell command, and patch safety gates
│  ├─ Context compression and long-task state recovery
│  └─ Benchmarks and diagnostics
├─ SQLite
│  ├─ providers
│  ├─ model_routes
│  ├─ codex_routes
│  ├─ model_aliases
│  ├─ gateway_profile
│  ├─ codex_profile
│  └─ request_logs
└─ External Configs
   ├─ Claude Desktop configLibrary
   ├─ ~/.claude/settings.json
   └─ ~/.codex/config.toml
```

## 10. Technology Stack

| Layer | Technology | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Small macOS app bundle, Rust backend |
| Frontend | React 19 + TypeScript | Single app component with page functions |
| Build | Vite + pnpm | Frontend build and Tauri packaging |
| Backend | Rust 2021 | Type-safe async service layer |
| HTTP server | axum | Local gateways and health endpoints |
| HTTP client | reqwest | JSON and streaming upstream requests |
| Async runtime | tokio | Gateway lifecycle and streaming |
| Database | SQLite via rusqlite | Local persistent configuration |
| Serialization | serde / serde_json | Tauri IPC and request transformation |
| Time/IDs | chrono / uuid | Logs, backups, response IDs |

## 11. Source Layout

| Path | Responsibility |
| --- | --- |
| `src/App.tsx` | Main React UI, page rendering, Tauri command calls |
| `src/App.css` | App styling, dashboard, binding, route, alias, and log layouts |
| `src-tauri/src/lib.rs` | Tauri builder, command registration, startup hooks |
| `src-tauri/src/main.rs` | Native app entry point |
| `src-tauri/src/state.rs` | App state, runtime gateway handles, data paths |
| `src-tauri/src/models.rs` | Shared data models for providers, routes, logs, settings |
| `src-tauri/src/database.rs` | SQLite initialization and CRUD |
| `src-tauri/src/compatibility.rs` | 1.6.0 runtime compatibility layer: capability profiles, safety gates, repair, diagnostics |
| `src-tauri/src/gateway.rs` | Claude/Anthropic-compatible gateway with Chat Completions fallback |
| `src-tauri/src/codex_gateway.rs` | Codex Responses-compatible gateway and conversion layer |
| `src-tauri/src/desktop_binding.rs` | Claude Desktop config read/apply/restore |
| `src-tauri/src/claude_code_binding.rs` | Claude Code settings read/apply/restore |
| `src-tauri/src/codex_binding.rs` | Codex config read/apply/restore |
| `src-tauri/src/commands.rs` | Tauri IPC command implementations |
| `src-tauri/src/settings.rs` | `settings.json` load/save |
| `src-tauri/src/tray.rs` | macOS tray menu |
| `docs/project.md` | Complete technical documentation |

## 12. Data Storage

App data is stored under:

```text
~/Library/Application Support/Gateway Switch/
```

Important files:

- `gateway.db`: SQLite database containing providers, routes, aliases, profiles, and request logs.
- `settings.json`: app-level settings such as auto-start and Claude listen port.
- `backups/`: exported config backups.

Claude Desktop config is managed under:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

Codex App config is managed at:

```text
~/.codex/config.toml
```

Claude Code settings are managed at:

```text
~/.claude/settings.json
```

Codex backups are written to:

```text
~/.codex/gateway-switch-backups/
```

## 13. Database Schema

### `providers`

Stores reusable upstream provider definitions.

Fields:

- `id`: stable provider ID.
- `name`: display name.
- `base_url`: legacy compatibility field preserved for older config/import paths. Since 1.6.0 it is no longer overwritten by `openai_base_url`.
- `openai_base_url`: OpenAI-compatible provider URL, used for Codex and Chat Completions fallback.
- `anthropic_base_url`: Anthropic-compatible provider URL, used for Claude and Claude Code direct requests.
- `auth_header`: usually `Authorization` or `x-api-key`.
- `auth_scheme`: usually `Bearer`, or empty for raw key headers.
- `api_key`: stored locally.
- `enabled`: provider availability.
- `created_at`: creation timestamp.

### `model_routes`

Claude routing table.

Fields:

- `id`: route ID.
- `claude_alias`: model name exposed to Claude Desktop.
- `display_name`: user-visible label.
- `provider_id`: linked provider.
- `upstream_model`: real model sent to the provider.
- `enabled`: whether the route is active.
- `created_at`: creation timestamp.

### `codex_routes`

Codex routing table.

Fields:

- `id`: route ID.
- `codex_model`: model name requested by Codex App.
- `display_name`: user-visible label.
- `provider_id`: linked provider.
- `upstream_model`: real model sent to the provider.
- `enabled`: whether the route is active.
- `created_at`: creation timestamp.

### `model_aliases`

Editable model alias registry.

Fields:

- `id`: generated UUID.
- `alias`: model alias string.
- `alias_type`: `claude` or `codex`.
- `created_at`: creation timestamp.

### `gateway_profile` and `codex_profile`

Store listen host, listen port, and local auth token for each product gateway.

Defaults:

- Claude Gateway: `127.0.0.1:3456`
- Codex Gateway: `127.0.0.1:3457`
- Token: `gateway-switch-token`

### `request_logs`

Stores recent request traces.

Fields:

- `request_id`: generated request ID.
- `claude_alias`: historical field name now used as requested model for both Claude and Codex.
- `provider_id`: selected provider.
- `upstream_model`: real upstream model.
- `status_code`: upstream/local response status.
- `duration_ms`: measured request duration.
- `is_stream`: whether the request was streaming.
- `error_summary`: upstream or conversion error text.
- `created_at`: timestamp.

Logs are the primary way to verify which model was actually called.

Since 1.6.0, error summaries are passed through the Secret Redaction Engine before insertion. Common OpenAI/Anthropic keys, GitHub tokens, JWT-like strings, AWS access keys, and PEM blocks are replaced with redacted placeholders.

## 14. Claude Gateway

### Endpoint Surface

Claude Gateway listens on the configured Claude profile, default:

```text
http://127.0.0.1:3456
```

Endpoints:

- `GET /health`
- `GET /v1/models`
- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

### Request Flow

```text
Claude Desktop
-> POST /v1/messages model=claude-sonnet-4-6
-> Gateway Switch validates auth
-> Gateway Switch resolves model_routes by claude_alias
-> Gateway Switch rewrites request model to upstream_model
-> Gateway Switch posts to provider /v1/messages
-> If /v1/messages is unsupported or returns a non-Messages response, Gateway Switch falls back to provider /v1/chat/completions
-> Gateway Switch rewrites or converts response model fields back to claude_alias
-> Claude Desktop receives a Claude-compatible response
```

### Chat Completions Fallback

Some providers are OpenAI-compatible but do not implement Anthropic Messages. For those providers, Claude Gateway uses a conservative fallback path:

```text
Claude Desktop /v1/messages
-> Gateway Switch tries Provider /v1/messages
-> Provider returns unsupported status or non-Anthropic response
-> Gateway Switch converts the Claude Messages request to Chat Completions
-> Gateway Switch calls Provider /v1/chat/completions
-> Gateway Switch converts the Chat Completions response back to Claude Messages shape
-> Claude Desktop
```

This keeps Anthropic-compatible providers working as before while allowing providers such as XiaoMiMo to be used from Claude Desktop through their OpenAI-compatible endpoint when needed.

### Auth

Claude Gateway accepts:

- `x-api-key: <gateway-token>`
- `Authorization: Bearer <gateway-token>`

The app writes `x-api-key` into Claude Desktop binding by default. This is local gateway auth between Claude Desktop and Gateway Switch. It is separate from Provider auth, such as `Authorization: Bearer <provider-key>`, which Gateway Switch uses when calling the upstream provider.

### Provider URL Handling

The gateway appends the required endpoint to the protocol-specific provider URL. It also avoids double-appending `/v1`.

Examples:

- `https://api.example.com` + `messages` becomes `https://api.example.com/v1/messages`
- `https://api.example.com/v1` + `messages` becomes `https://api.example.com/v1/messages`

Resolution rules:

- Claude Gateway Anthropic requests use `anthropic_base_url` when present.
- Claude Gateway Chat Completions fallback uses `openai_base_url`.
- Codex Gateway always uses `openai_base_url`.
- Claude Code Direct Provider requires `anthropic_base_url`.

### Streaming

Claude streaming uses Anthropic SSE events. For Anthropic-compatible upstreams, Gateway Switch reads the upstream byte stream, splits lines, parses `data:` JSON when present, recursively rewrites `model` fields, and passes non-JSON SSE lines through unchanged.

For Chat Completions fallback upstreams, Gateway Switch converts Chat Completions SSE deltas into Anthropic Messages SSE events: `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, and `message_stop`.

## 15. Claude Desktop Binding

Claude Desktop binding reads and writes:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

The active config is determined through `_meta.json` and its `appliedId`.

Binding writes fields such as:

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:3456",
  "inferenceGatewayApiKey": "gateway-switch-token",
  "inferenceGatewayAuthScheme": "x-api-key",
  "inferenceModels": [
    { "name": "claude-sonnet-4-6" }
  ],
  "managedBy": "Gateway Switch"
}
```

Restore uses the latest backup created before Gateway Switch took over.

## 16. Claude Code Binding

Claude Code binding reads and writes:

```text
~/.claude/settings.json
```

Gateway Route mode writes the local Claude Gateway:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:3456",
    "ANTHROPIC_AUTH_TOKEN": "gateway-switch-token",
    "ANTHROPIC_MODEL": "claude-sonnet-4-6"
  },
  "gatewaySwitchClaudeCode": {
    "managedBy": "Gateway Switch",
    "mode": "gateway"
  }
}
```

Direct Provider mode writes the selected provider's Anthropic Base URL:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://token-plan-sgp.xiaomimimo.com/anthropic",
    "ANTHROPIC_AUTH_TOKEN": "<provider-api-key>",
    "ANTHROPIC_MODEL": "mimo-v2.5"
  },
  "gatewaySwitchClaudeCode": {
    "managedBy": "Gateway Switch",
    "mode": "direct-provider"
  }
}
```

Direct Provider is intentionally strict. If the selected provider has no Anthropic Base URL, the app refuses to bind. This prevents Claude Code from sending Anthropic requests to an OpenAI `/v1` endpoint, which can surface as model-not-found errors inside Claude Code.

Restore uses the latest backup under:

```text
~/.claude/gateway-switch-backups/
```

## 17. Codex Gateway

### Endpoint Surface

Codex Gateway listens on the configured Codex profile, default:

```text
http://127.0.0.1:3457
```

Endpoints:

- `GET /health`
- `GET /v1/models`
- `POST /v1/responses`

### Why This Gateway Exists

Codex App talks to OpenAI-compatible Responses API. A large number of third-party providers only expose Chat Completions. Gateway Switch bridges that gap:

```text
Codex App /v1/responses
-> Gateway Switch
-> Provider /v1/chat/completions
-> Gateway Switch converts response back to Responses shape
-> Codex App
```

### Codex Route Model

Each Codex route maps:

```text
Codex Model -> Provider -> Real Upstream Model
```

Example:

```text
gpt-5.5 -> XiaoMiMo -> xiaomi-real-model-name
```

`Codex Model` must match what Codex requests. `Upstream Model` must match the real model ID expected by the third-party API.

If no model name disguise is needed, both fields can be identical.

### Responses Request Conversion

Gateway Switch converts:

- `instructions` to a system message.
- If `tools` are present, a short compatibility system message is added to tell weak Chat Completions models to emit structured `tool_calls` for real file/command/edit work instead of only narrating intent.
- `input` message items to Chat Completions messages.
- `function_call_output` to tool messages.
- `function_call` to assistant `tool_calls`.
- `max_output_tokens` to the provider-compatible Chat Completions token limit field; Xiaomi/MiMO routes use `max_completion_tokens`.
- `temperature`, `top_p`, and `tool_choice` are passed through when present.
- `tools` of type `function` are converted to Chat Completions function tools.
- If tools exist and the client did not set `tool_choice`, Gateway Switch sends `tool_choice: "auto"`.

### Sync Response Conversion

For non-stream responses, Gateway Switch converts:

- `choices[0].message.content` to `output[0].content[0].text`.
- `usage.prompt_tokens` to `usage.input_tokens`.
- `usage.completion_tokens` to `usage.output_tokens`.
- `usage.total_tokens` is preserved or derived.
- `model` is rewritten to the Codex-requested model.

The Responses output includes fields required by Codex, including `status`, `output`, and detailed token usage.

### Streaming Response Conversion

For streaming, Gateway Switch emits Responses-compatible SSE events:

- `response.created`
- `response.output_item.added`
- `response.content_part.added`
- `response.output_text.delta`
- `response.output_text.done`
- `response.content_part.done`
- `response.output_item.done`
- `response.function_call_arguments.delta`
- `response.function_call_arguments.done`
- `response.completed`

Since 1.6.1, streaming function-call items are finalized before the final assistant message item. This matters for Codex-style clients because some clients stop the turn as soon as they see the assistant message complete. Function-call arguments are also repaired with `compatibility::repair_json_object` before final `done` events when possible.

Provider delta variants supported:

- `choices[0].delta.content`
- `choices[0].delta.reasoning_content`
- `choices[0].delta.reasoning`
- `choices[0].delta.text`
- content arrays with `text` or `content`

The gateway estimates usage for streaming when the provider does not send final usage data.

### Error Handling

Upstream non-2xx responses become `502 Bad Gateway` responses with a compact upstream error message. This made provider configuration problems visible in Codex instead of producing silent disconnects.

Common upstream errors:

- 401/403: wrong API key, auth header, or auth scheme.
- 404: wrong provider Base URL or unsupported endpoint.
- 429: provider quota or rate limit.
- 5xx: provider outage.

## 18. Runtime Compatibility Layer

Runtime compatibility code lives in:

```text
src-tauri/src/compatibility.rs
```

This module is intentionally independent from the UI. It contains reusable logic that can be used by the current gateways and by future MCP/shell/agent execution entry points.

### Provider Capability Profile

`provider_capability_profile` and `provider_capability_json` infer provider behavior from provider metadata:

- Messages API support
- Chat Completions support
- Responses support
- Tool Use support
- Vision support
- Reasoning support
- Streaming support
- System prompt support
- Estimated max context
- JSON stability
- Tool-call accuracy
- Long-context stability

Claude and Codex `/health` include these capability profiles so other tools can inspect current runtime readiness without opening the UI.

### Codex Capability Profile

`codex_capability_profile` maps provider behavior into Codex-oriented capability flags:

- Chat
- Code Edit
- Patch
- Tool Call
- Shell Loop
- Long Task

This is a backend capability model. The current UI does not yet render a full capability matrix, but Tauri commands expose the data.

### Tool Call Repair

`repair_json_object` repairs common malformed tool-call argument payloads:

- extracts the JSON object from surrounding prose
- quotes unquoted object keys
- converts single quotes to double quotes
- removes trailing commas

Claude tool-call conversion uses this repair layer when converting Chat Completions `tool_calls` into Anthropic `tool_use` blocks. Codex sync tool-call conversion also normalizes function-call arguments through this layer.

### Fake Tool/Fake Action Detection

`detect_fake_tool_call` and `detect_fake_action` detect text such as:

- "I called the tool"
- "I read the file"
- "I ran the command"
- "我已经调用..."
- "我已经修改..."

Claude SSE rewrite can attach `gateway_warning` when a text delta looks like fake tool-use text without an actual tool block. The app also exposes a Tauri command for checking arbitrary text.

### MCP Path Safety

`mcp_path_safety` blocks paths that should not be exposed to future MCP/file execution flows:

- `.env`
- `.ssh`
- private key names such as `id_rsa` and `id_ed25519`
- token/cookie-like paths
- path traversal
- absolute paths outside the workspace root

### Command Safety Gate

`command_safety` blocks high-risk shell patterns:

- `rm -rf`
- `sudo`
- recursive chmod
- `curl | bash` and similar install pipes
- global package installs
- direct system path mutation

The current app does not include a shell executor. This gate exists so future shell execution paths do not start from a blank safety model.

### Patch Validator And Repair

`validate_patch` checks patch text for:

- recognizable file headers
- unsafe paths
- missing hunks
- malformed `---` / `+++` headers

`repair_patch_headers` can repair common diff header drift by adding `a/` and `b/` prefixes.

### Context Compression And Agent Recovery

`compress_context` implements a sliding-window compression strategy with tool-state pinning. It preserves recent messages and tool-related messages while summarizing older context.

`recover_agent_state` reconstructs a lightweight state object:

- plan
- files touched
- commands run
- errors seen
- patches applied
- next action

This is meant to reduce long task drift when an agent resumes work after losing context.

### Benchmark And Diagnostics

`benchmark_provider` returns coarse grades for:

- Chat
- Tool Use
- MCP
- Artifacts
- Long Context
- Responses Compatibility
- Patch Quality
- Agent Recovery

`export_diagnostics` writes a JSON bundle to the app backups directory. The bundle includes runtime feature status, provider capabilities, benchmark output, providers, routes, Codex routes, and recent logs.

## 19. Codex App Binding

Binding writes to:

```text
~/.codex/config.toml
```

Gateway Switch writes:

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

Important fields:

- `model_provider = "gateway-switch"` makes Codex use the local provider.
- `model` selects the default Codex model exposed by Gateway Switch.
- `preferred_auth_method = "apikey"` makes Codex prefer API-key mode over OAuth for this local provider.
- `wire_api = "responses"` tells Codex to use the Responses API surface.
- `requires_openai_auth = false` prevents OpenAI OAuth from being required for this provider.
- `experimental_bearer_token` lets Codex App launched from Finder carry the local gateway token without terminal environment variables.

Restore returns to the latest unmanaged backup. If no clean backup exists, Gateway Switch removes its managed `gateway-switch` config block while preserving unrelated Codex config sections.

## 20. Reasoning Behavior With Third-Party Models

Gateway Switch does not add or remove model reasoning ability. It only converts protocol shape.

If the provider exposes reasoning through a Chat Completions delta field such as `reasoning_content`, Gateway Switch can forward it as output text. If the provider does not expose reasoning data, Codex will only see final text.

Fast responses are normal when:

- The upstream model is fast.
- The prompt is simple.
- The provider returns only final text.
- The model does not expose visible reasoning over Chat Completions.

## 21. Verifying The Real Model

The recommended verification path is:

1. Open the Codex page.
2. Send a message from Codex App.
3. Check the `Verify Real Model` card.
4. For detailed history, open Logs.

The important log fields are:

- `Requested Model`: what Claude or Codex requested.
- `Provider`: which provider route was selected.
- `Real Upstream`: the actual model ID sent to the third-party API.
- `Status`: whether the upstream call succeeded.
- `Duration`: response time.

## 22. Project History And Context Limits

Gateway Switch preserves local config sections such as `[projects...]` when binding or restoring Codex. This preserves project trust and local config as much as possible.

Codex App conversation history, account state, and provider-specific cloud state are controlled by Codex itself. Switching between OpenAI login and a local Gateway provider can show different conversation lists. Gateway Switch cannot force different Codex account/provider states to share one conversation database unless Codex App exposes that capability.

## 23. Frontend UX Model

Navigation is grouped as:

- Dashboard: status only, refresh, health checks, recent traffic.
- Products: Claude, Claude Code, and Codex product-specific setup.
- Shared: Providers, reused by both products.
- System: Logs and Settings.

Dashboard intentionally does not perform binding or gateway startup. Startup and binding live on the product page they affect:

- Claude page: Claude route setup and Claude Desktop binding.
- Claude Code page: Claude Code Gateway Route or Direct Provider binding.
- Codex page: Codex route setup and Codex App binding.

This prevents confusion between the Claude gateway and Codex gateway.

## 24. Input Focus Bug Fix

The app originally rendered page functions as nested React components, which changed component identity on every parent render. That caused input fields to remount after typing one character, losing focus.

The fix is to call internal page functions directly in the content switch instead of rendering them as nested component tags. This keeps input elements stable during state updates.

## 25. Tauri IPC Commands

Provider and route commands:

- `list_providers`
- `create_provider`
- `update_provider`
- `delete_provider`
- `list_routes`
- `create_route`
- `update_route`
- `delete_route`
- `list_codex_routes`
- `create_codex_route`
- `update_codex_route`
- `delete_codex_route`
- `list_model_aliases`
- `create_model_alias`
- `delete_model_alias`

Lifecycle commands:

- `start_gateway`
- `stop_gateway`
- `start_codex_gateway`
- `stop_codex_gateway`
- `get_status`
- `get_codex_status`

Binding commands:

- `get_desktop_info`
- `apply_binding`
- `restore_binding`
- `get_claude_code_info`
- `apply_claude_code_binding`
- `restore_claude_code_binding`
- `get_codex_binding_info`
- `apply_codex_binding`
- `restore_codex_binding`

Health and settings commands:

- `check_gateway_health`
- `check_codex_health`
- `check_provider_health`
- `get_settings`
- `save_settings`
- `export_config`
- `import_config`
- `list_logs`

Runtime compatibility commands:

- `list_provider_capabilities`
- `get_runtime_feature_report`
- `run_compatibility_benchmark`
- `validate_patch_payload`
- `check_command_safety`
- `check_mcp_path_safety`
- `detect_fake_action_text`
- `compress_context_payload`
- `recover_agent_state_payload`
- `export_diagnostics`

## 26. Build And Release

Development:

```bash
pnpm install
pnpm tauri dev
```

Frontend build:

```bash
pnpm build
```

Rust tests:

```bash
cd src-tauri
cargo test
```

Release build:

```bash
CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build
```

macOS artifacts:

```text
src-tauri/target/release/bundle/macos/Gateway Switch.app
src-tauri/target/release/bundle/dmg/Gateway Switch_1.10.0_aarch64.dmg
```

## 27. Release Checklist

- Frontend build passes.
- Rust tests pass. Latest 1.8.2 verification: standard Rust tests passed, UI Safe Mode regression passed, native real repair smoke passed, frontend build passed, and Tauri DMG packaging passed.
- Claude Gateway health check passes.
- Codex Gateway health check passes.
- Claude route can rewrite model request and response fields.
- Codex route can convert Responses to Chat Completions and back.
- Codex route can convert Chat Completions tool calls to Responses `function_call` items.
- Codex streaming response completes with `response.completed`.
- Runtime compatibility tests pass for JSON repair, secret redaction, safety gates, patch repair, context compression, and agent state recovery.
- Logs show requested model, provider, and real upstream model.
- Logs redact sensitive error summaries.
- Claude Desktop binding creates a backup and can restore.
- Claude Code Gateway Route binding creates a backup and can restore.
- Claude Code Direct Provider binding uses Anthropic Base URL.
- Codex binding creates a backup and can restore.
- DMG version matches `package.json`, `Cargo.toml`, and `tauri.conf.json`.

## 28. PRD Coverage Map

The 1.6.0 implementation covers the vNext PRD as follows:

| PRD item | Status in 1.6.0 | Main implementation |
| --- | --- | --- |
| Provider Capability Profile | Implemented | `compatibility.rs`, `/health`, `list_provider_capabilities` |
| Anthropic Protocol Adapter | Implemented for current gateway surface | `gateway.rs` |
| SSE Event Compatibility | Implemented for Claude and Codex stream conversion | `gateway.rs`, `codex_gateway.rs` |
| Tool Call Repair Layer | Implemented for JSON argument repair | `compatibility.rs`, gateway conversions |
| Fake Tool Call Detector | Implemented as detection/warning layer | `compatibility.rs`, `rewrite_sse` |
| MCP Security Sandbox | Implemented as path safety gate | `compatibility.rs`, Tauri command |
| Secret Redaction Engine | Implemented for logs/diagnostics | `compatibility.rs`, `database.rs` |
| Compatibility Benchmark Suite | Implemented as provider benchmark report | `compatibility.rs`, Tauri command |
| Context Compression Layer | Implemented as sliding window with tool-state pinning | `compatibility.rs`, Tauri command |
| Provider Fallback Engine | Implemented for Claude Messages to Chat Completions fallback | `gateway.rs` |
| Observability & Diagnostics | Implemented logs plus diagnostic JSON export | `database.rs`, `commands.rs` |
| Responses API Compatibility Layer | Implemented for current Codex gateway surface | `codex_gateway.rs` |
| Responses ↔ ChatCompletions Adapter | Implemented sync and streaming conversion | `codex_gateway.rs` |
| Codex Capability Profile | Implemented backend profile | `compatibility.rs` |
| Patch Validator | Implemented backend validator | `compatibility.rs`, Tauri command |
| Patch Repair Engine | Implemented diff header repair | `compatibility.rs` |
| Fake Action Detector | Implemented backend detector | `compatibility.rs`, Tauri command |
| Command Safety Gate | Implemented backend gate | `compatibility.rs`, Tauri command |
| Long Task State Tracker | Implemented state reconstruction helper | `compatibility.rs`, Tauri command |
| Multi-Step Agent Recovery | Implemented recovery anchor helper | `compatibility.rs` |
| Shell Execution Sandbox | Implemented safety gate, no shell executor exists yet | `compatibility.rs` |
| Responses Runtime Benchmark | Implemented as benchmark report | `compatibility.rs` |

## 29. Known Limitations

- Claude Code Direct Provider requires an Anthropic Messages-compatible upstream.
- Codex Gateway requires a Chat Completions-compatible upstream.
- Claude Gateway fallback requires a Chat Completions-compatible upstream.
- Codex visible reasoning depends on what the upstream model/provider returns.
- Gateway Switch cannot merge Codex cloud/account conversation history across provider states.
- API keys are stored locally for convenience; the app is designed for local personal use.
- 1.6.0 includes MCP, shell, and patch safety gates, but the current app does not include a real MCP executor or shell executor. Future execution entry points should reuse these gates before touching the filesystem or process environment.
