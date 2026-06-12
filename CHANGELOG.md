# Changelog

This file tracks user-visible Gateway Switch changes so future AI agents can quickly understand release history. For deeper architecture context, read `docs/project.md`.

## 1.14.0 - 2026-06-12

- Refactored the Claude Gateway runtime into focused backend modules while preserving the existing `/v1/messages`, `/v1/models`, route diagnostics, payload preview, and replay command surfaces.
- Moved Provider Compatibility Profiles and manual provider policy application into `gateway_strategy.rs`, keeping `gateway::ProviderCompatibilityProfile` and related functions re-exported for Codex Gateway compatibility.
- Moved Anthropic/OpenAI Chat request conversion, response conversion, tool-call parsing, stream delta extraction, and token estimation into `gateway_protocol.rs`.
- Moved failure snapshot gating, payload redaction/truncation, fallback status classification, and upstream body preview helpers into `gateway_diagnostics.rs`.
- Kept runtime behavior intentionally conservative: no route schema change, no Tauri command rename, no frontend API contract change, and no provider strategy behavior change.
- Verification: `git diff --check`, editor diagnostics. Rust `cargo test` and DMG build are pending in an environment with `cargo` available.

## 1.13.3 - 2026-06-08

- Reworked the sidebar from 6 nav groups (9 items) into 5 groups: Overview / Apps / Setup / Diagnostics / Advanced, all driven by `t()` for full bilingual coverage.
- Reverted the previous "route CRUD only inside Route Builder" constraint: Claude Desktop, Claude Code and Codex pages now embed inline Add / Edit / Delete for their own routes.
- Kept Route Builder under Advanced as a multi-target editor entry; per-app pages link to it as "Advanced Route Builder" instead of forcing a page jump.
- Added a Dashboard "Next Actions" card that derives up to 5 click-to-fix CTAs from current state (no provider, no route, gateway stopped, app not bound, diagnostics critical, recent failure).
- Added a one-step First-run setup card on the Dashboard when no provider is configured, embedding the existing provider wizard with clean header copy.
- Bilingualized `ProviderSetupWizard`: now accepts an optional `t` and `showHeader`; all labels and preset preview rows go through `t()`.
- Removed hard-coded English/Chinese page-header copy on Dashboard, Provider Console and Unified Diagnostics Center; added 30+ new bilingual entries (Apps / Setup / Diagnostics / Advanced / Add Route / Advanced Route Builder / Next Actions / First-run setup, etc.).
- Verification: `cargo test` (75 passed), `pnpm build`.

## 1.13.2 - 2026-06-02

- Added the Runtime Console refactor with Dashboard, App Workbench, Provider Console, Route Builder, Health Center, and Usage Insights as first-class modules.
- Moved Claude Desktop, Claude Code, and Codex route CRUD into Route Builder so product workbench pages focus on read-only route status and runtime health.
- Added Runtime Console aggregation commands and shared frontend feature modules to keep `App.tsx` smaller and reduce duplicated UI logic.
- Fixed Codex OpenAI login restore by removing Gateway Switch/Codex++ provider configuration and clearing stale API-key auth from `~/.codex/auth.json`.
- Improved Claude Desktop health failures so gateway-offline checks show friendly guidance instead of raw `error sending request` messages.
- Added LoopGuard tool-result compression, repeated tool-call fingerprint detection, warm strategy-change hints, and opt-in debug logging for agent loop diagnosis.
- Verification: `cargo test`, `pnpm exec tsc --noEmit`, `pnpm build`, LoopGuard debug simulation, local `/v1/messages` smoke validation, and DMG mount inspection.

## 1.13.1 - 2026-05-30

- Fixed the Claude Code Direct Provider risk confirmation layout so the checkbox and English confirmation text no longer collapse into a vertical column.
- Reworked the risky Direct Provider warning into a compact confirmation card with stable wrapping and clearer visual hierarchy.
- Added scoped CSS for `.check-row` text wrapping and checkbox sizing to avoid regressions from global word-break rules.
- Changed the sidebar UI version label to use the build-time app version instead of a hardcoded stale string.
- Fixed a follow-up checkbox layout conflict where `.binding-actions input { width: 100%; }` made the risk confirmation text collapse into a vertical column.

## 1.13.0 - 2026-05-30

- Added shared `LoopGuard` for Claude Desktop, Claude Code, and Codex Gateway streaming paths to suppress repeated upstream text chunks without truncating valid long-form output.
- Wired Codex Gateway Responses streaming through the same guard before emitting `response.output_text.delta` events.
- Added duplicate tool-call fingerprint diagnostics and compact request-log summaries for loop suppression activity.
- Changed Xiaomi MiMo Codex compatibility defaults to avoid strict tool-call enforcement and downgrade configured strict mode to `auto`.
- Added regression tests for loop suppression, long unique reports, duplicate tool-call detection, and Xiaomi Codex strict-mode downgrade.

## 1.12.2 - 2026-05-29

- Fixed Claude Code Gateway Route compatibility for Chat-only providers by forcing OpenAI Chat fallback for Xiaomi MiMo, DeepSeek, Moonshot/Kimi, Qwen/DashScope, and generic OpenAI Chat profiles even when an Anthropic URL is configured.
- Added streamed repetition-loop diagnostics that emit a non-fatal `gateway_warning` event and store a request-log warning when upstream text appears to repeat aggressively.
- Changed Claude Code Direct Provider binding UX so risky providers show a warning and require an explicit force checkbox instead of leaving the bind button permanently disabled.
- Added regression tests for Xiaomi Chat fallback routing and repetition-loop warning detection.

## 1.12.1 - 2026-05-29

- Added contextual tooltip help beside every Provider Strategy Overrides flag.
- Tooltip copy explains each flag's effect, when to enable it, default advice, and risk notes for high-impact settings.
- Exposed the `gateway_route_recommended` and `codex_disable_responses` override controls in the provider strategy UI.
- Kept this release as a UI-only compatibility guidance patch; no backend schema changes.

## 1.12.0 - 2026-05-29

- Added a Unified Diagnostics Center that aggregates Claude Desktop, Claude Code, Codex Gateway, Codex++, Providers, and install/runtime health into one scorecard.
- Added failure clustering across recent diagnostic snapshots with provider/status/surface grouping and strategy recommendations for role, tool, reasoning, rate-limit, and server errors.
- Added built-in Provider Presets for OpenRouter, Volcengine Ark DeepSeek, DeepSeek official, Moonshot Kimi, Qwen DashScope, Xiaomi MiMo, Anthropic-compatible, and OpenAI Chat-compatible providers.
- Added preset application that creates or updates provider URLs and compatibility policies without overwriting existing API keys with empty values.
- Added exportable unified diagnostics bundles for local troubleshooting while preserving the v1.10.0 redaction model.
- Updated the app UI version to v1.12.0 and expanded provider setup guidance.

## 1.10.0 - 2026-05-29

- Added real failed-request diagnostic snapshots with sanitized original payload, converted upstream payload, local replay preview, and likely-cause classification for 400/413/429/5xx/network failures.
- Added editable Provider Compatibility Policies so users can override Claude and Codex strategy flags per provider while inheriting automatic profiles by default.
- Added one-click Claude Code repair that backs up settings and switches unsafe Direct Provider bindings to Gateway Route.
- Unified Codex Gateway with Provider Compatibility Profiles, including strict tool-call enforcement, reasoning parameter cleanup, and route diagnostics.
- Added GitHub Release update checks, safe install planning, and Finder reveal helpers to avoid running from DMG or temporary paths.
- Expanded provider profiles for OpenRouter, Xiaomi MiMo, DeepSeek official, Moonshot Kimi, Qwen DashScope, Volcengine Ark, standard Anthropic, and OpenAI Chat fallback providers.
- Added regression tests for provider policy persistence, request snapshot persistence/redaction, diagnostic replay, Codex route diagnostics, version comparison, and safe install warnings.

## 1.9.0 - 2026-05-29

- Added Provider Compatibility Profiles for Claude routes, including `standard_anthropic`, `openai_chat_fallback`, and `volcengine_deepseek_coding`.
- Added Claude Code Route Diagnostics that explain Direct Provider safety, Gateway Route recommendations, and system/tool role conversion behavior.
- Added a redacted Payload Preview command and UI so users can inspect the converted upstream Chat payload without sending a request or consuming tokens.
- Added runtime source detection to warn when Gateway Switch is launched from a DMG or temporary path instead of `/Applications`.
- Added regression tests for route diagnostics, payload preview role conversion, and runtime source classification.

## 1.8.8 - 2026-05-29

- Added Claude Code Gateway compatibility for Volcengine Ark DeepSeek coding models that reject `messages.role = system`.
- Gateway Chat fallback now merges Anthropic `system` instructions into the first user message for Volcengine/DeepSeek routes and converts tool results to user messages for providers that only accept `user` and `assistant` roles.
- Added backend and frontend guards that prevent binding Volcengine DeepSeek via Claude Code Direct Provider mode, with guidance to use Gateway Route instead.
- Added regression tests for Volcengine DeepSeek role-mode detection and user/assistant-only payload conversion.

## 1.8.7 - 2026-05-28

- Added a full Codex++ Upstream Tweak Store view sourced from the approved upstream registry at `https://b-nnett.github.io/codex-plusplus/store/index.json`.
- Added derived archive URLs, source repository links, approved commit display, install status, and installed path reporting for upstream tweak entries.
- Added legacy recommendation mapping for the four requested script names, making clear which names are not exact upstream registry entries and pointing usage-related items to `Bennett's UI Improvements`.
- Hardened Codex++ store validation by requiring schema version 1, safe `owner/repo` values, matching manifest repos, and full 40-character approved commit SHAs.
- Refactored Cold Start diagnostics out of `commands.rs` into `coldstart.rs` while preserving the existing check and repair command surface.
- Added regression tests for archive URL derivation, invalid store entries, installed tweak detection, and legacy recommendation mapping.

## 1.8.5 - 2026-05-28

- Added a Codex++ Recommended Scripts panel for `Codex Context Used Meter`, `Hide Usage Alert`, `Codex Token Usage`, and `Codex List Pagebuster`.
- Added native detection for Codex++ user-script storage before enabling recommended script installation.
- Added safe install gating so Gateway Switch does not write script files into unknown or unsupported runtime locations.
- Added Tauri commands for recommended script status and install workflows.
- Added regression tests for unknown-storage and detected-storage recommended script reports.

## 1.8.4 - 2026-05-28

- Refined the left navigation into five product-oriented groups: Dashboard, Products, Features, General, and System.
- Kept long sidebar labels such as `Claude Code`, `MCP Sync`, and `Cold Start` on a single line instead of forcing manual line breaks.
- Added responsive sidebar behavior: full text labels on wider windows and compact icon-only rail on narrow windows.
- Strengthened grid and card responsiveness with `minmax(0, 1fr)` constraints, safer table scrolling, and overflow protection for dense content.
- Lowered the app minimum window size to `760x560` so users can resize the desktop window more flexibly without layout corruption.

## 1.8.3 - 2026-05-28

- Fixed Codex++ watcher generation so launchd no longer stores a transient `/Volumes/Gateway Switch/...` DMG executable path after running Gateway Switch from a mounted installer.
- Changed the native watcher command to the Codex++ health-check-compatible `CODEX_PLUSPLUS_WATCHER=1 codexplusplus update --watcher --quiet` form.
- Patched staged Codex++ runtime health checks to accept modern macOS `launchctl print gui/$UID/com.codexplusplus.watcher` results when `launchctl list <label>` reports false negatives.
- Reset watcher health logging to `~/Library/Logs/codex-plusplus-watcher.log`, the path read by the Codex++ settings page, so stale EPERM/DMG errors no longer keep the page in Review/Failed state after repair.
- Added installed-app path preference for native shims when Gateway Switch is launched from a DMG, preventing maintenance buttons from re-opening a quarantined disk-image app.

## 1.8.2 - 2026-05-27

- Added a dedicated Codex++ native debug log at `~/Library/Application Support/codex-plusplus/log/native-debug.log`.
- Moved detailed `app.asar.unpacked` diagnostics and Node/npm path resolution details out of the normal install log and into the debug log.
- Expanded unpacked diagnostics with required file sizes, `.node` binary count, and sampled native module paths.
- Added `codexPlusPlus.uiSafeMode`, defaulting to `false`, to allow preconfiguring UI Safe Mode from `config.json`.
- Ensured UI Safe Mode disables only `co.bennett.ui-improvements` while preserving route, script market, history repair, watcher, and CLI shim behavior.
- Added regression coverage for the UI Safe Mode config behavior.

## 1.8.1 - 2026-05-27

- Fixed Codex launch failure when `app.asar.unpacked` was missing native modules required by `better-sqlite3`. Native repair now validates and restores unpacked artifacts from backups before continuing.
- Added detailed codex++ repair logs for unpacked artifact health checks, backup candidate selection, restore source, Node/npm path resolution, and npm execution PATH.
- Fixed GUI and launchd Node/npm resolution by searching common macOS paths and injecting an augmented PATH into npm subprocesses.
- Added UI Safe Mode in the Codex++ enhancement page to disable only `co.bennett.ui-improvements` while preserving routing, script market, history repair, watcher, and CLI shim support.
- Fixed Claude Desktop route loops around `Request too large` by raising the local body limit to 64MiB and preventing explicit upstream errors from being retried through Chat Completions fallback.
- Verification: `PATH="$HOME/.cargo/bin:$PATH" cargo test`, `PATH="$HOME/.cargo/bin:$PATH" cargo test codex_pp::native_install_acceptance_tests::native_real_repair_smoke -- --ignored --nocapture --test-threads=1`, `pnpm build`, and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## 1.8.0 - 2026-05-27

- Added the native codex++ install and repair workflow inside Gateway Switch, covering source download, build orchestration, runtime staging, `Codex.app` patching, and recoverable rollback.
- Added streaming codex++ install logs and preflight checks in the desktop UI.
- Completed the local signing identity path for `install-local`, including automatic local identity provisioning and bundle re-signing.
- Migrated default tweak installation, CLI shims, and the `launchd` watcher to native Gateway Switch generation; watcher and shell entrypoints now use `gateway-switch codexpp ...`.
- Added real-machine acceptance coverage for native `install-local`, rollback, and repair against a real `/Applications/Codex.app`.
- Updated version to `1.8.0` across `package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, the app sidebar labels, and release documentation.
- Verification: `PATH="$HOME/.cargo/bin:$PATH" cargo test` (32 passed, 3 ignored), `PATH="$HOME/.cargo/bin:$PATH" cargo test -- --ignored --nocapture --test-threads=1` (3 passed), `pnpm build`, and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## 1.7.2 - 2026-05-23

- Added the new MCP Sync module for Claude Desktop, Claude Code, and Codex, with status cards, sync preview, conflict summary, execution results, and secret-key masking.
- Added native Rust MCP synchronization for `mcpServers` / `mcp_servers`, preserving non-MCP JSON/TOML fields and creating backups before writes.
- Added Tauri commands for MCP status inspection, preview generation, and one-click sync execution.
- Added local release artifacts for the 1.7.2 macOS DMG and app tarball with SHA256 checksums.
- Updated version to `1.7.2` across `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json`.
- Verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (32 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## 1.7.1 - 2026-05-20

- Enhanced Claude Desktop binding for newer developer-mode model entries: route display names are written as `displayName`, and exposed models default to `supports1m: true` for the 1M-context variant.
- Fixed the Claude page health check button by showing the latest health result inline and surfacing success/failure feedback.
- Reordered the Claude page into the requested five-row layout: gateway/binding status, route editor, Claude aliases, route cards/exposed models, and route table.
- Reordered the Codex page so gateway status and real-model verification stay on row 1, while Codex App binding and context/reasoning notes share row 2.
- Completed Chinese localization for Claude Code runtime notes, Codex context and route-copy text, and Cold Start Doctor diagnostic details.
- Removed temporary Codex stream-disconnect debug instrumentation and debug files before the formal release.
- Updated version to `1.7.1` across `package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, and the app sidebar labels.
- Verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (29 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## 1.7.0 - 2026-05-18

- Added bilingual UI support for Chinese and English, with Chinese as the default language and a Settings toggle for switching the interface language.
- Added persistent `language` storage to app settings while keeping existing settings files backward compatible.
- Reworked the left sidebar to show permanent icon labels for every tab, including Dashboard, Claude, Claude Code, Codex, Cold Start, Providers, Logs, and Settings.
- Fixed the misaligned `Gateway Switch v1.6.3` floating brand tooltip by replacing it with a stable `v1.7.0` sidebar footer label.
- Added the `Cold Start Doctor` page with three phases: readiness overview, execution/repair log, and capability matrix.
- Added backend cold-start status and repair commands for Claude Desktop, Claude Code, Codex App, local gateways, health endpoints, routes, providers, and security risk review.
- Added safe repair behavior that starts stopped local gateways and applies backup-backed Claude Desktop/Codex bindings when valid routes exist.
- Added detailed `[coldstart]` Rust logs and Markdown report generation for later troubleshooting.
- Included the Claude Desktop and Codex cold-start skill reference files under `coldstart/` for future validation workflows.
- Merged the Codex large-request stability fix: the Responses gateway now accepts request bodies above Axum's default limit and includes regression coverage.
- Fixed Volcengine Ark Coding Plan routing by recognizing `/v2` and `/v3` OpenAI-compatible Base URLs such as `/api/coding/v3`, and by preserving complete `/chat/completions` endpoint URLs without appending another `/v1` segment.
- Normalized Codex Responses `developer` messages to Chat Completions `system` messages so providers that only accept `system`, `assistant`, `user`, and `tool` roles can handle Codex traffic.
- Updated version to `1.7.0` across `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json`.
- Verification: `pnpm build`, `cargo test` (28 passed), and `CI=false pnpm tauri build --bundles app,dmg`.

## 1.6.4 - 2026-05-16

- Fixed Xiaomi MiMO Codex routing for the latest OpenAI-compatible Chat Completions behavior: MiMO thinking mode now defaults to `disabled` for Gateway-generated Codex requests so upstream no longer rejects multi-turn tool conversations with `reasoning_content` replay errors.
- Updated Xiaomi/MiMO Codex token mapping from `max_tokens` to `max_completion_tokens`, matching the current MiMO OpenAI API contract.
- Preserved explicit passthrough `thinking` controls when a caller provides them, while keeping the compatibility default scoped only to Xiaomi/MiMO routes.
- Added focused Rust tests for Xiaomi/MiMO Codex compatibility and verified the full Rust test suite.
- Updated version to `1.6.4` across `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json`.
- Verification: `pnpm build`, `cargo test`, and `pnpm tauri build --bundles app`.

## 1.6.3 - 2026-05-12

- Refreshed the whole app with a Claude Warm Native UI: white surfaces, warm paper backgrounds, ink text, oxblood accents, low-saturation semantic colors, and softer native desktop cards.
- Reworked the left navigation into a compact icon rail with hover labels to free space for the main Gateway Switch workbench.
- Redesigned the in-app brand mark, App Icon, and tray/status icon around a white `Gateway Pin` route symbol with a Claude oxblood center point.
- Updated typography to Geist, Fraunces, and Geist Mono and aligned buttons, tables, forms, badges, health bars, provider cards, and route cards with the new visual system.
- Updated version to `1.6.3` across `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json`.
- Verification: `pnpm build`, `PATH="$HOME/.cargo/bin:$PATH" cargo test` (29 passed), and `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`.

## 1.6.2 - 2026-05-11

- Fixed Codex conversation stall: when third-party models describe planned actions in text without emitting `tool_calls`, the gateway now detects this pattern via `has_action_description()` and automatically retries with `tool_choice: "required"` to force tool invocation.
- Added `extract_finish_reason()` to parse `finish_reason` from Chat Completions SSE streams. Truncated responses (`"length"`) now emit `status: "incomplete"` with `incomplete_details.reason: "max_output_tokens"`.
- Added 120-second stream timeout via `tokio::time::timeout` to prevent indefinite waits when upstream providers hang.
- Enhanced system prompt: when tools are present, the gateway now injects a stronger prompt that lists available tool names and explicitly instructs the model to use `tool_calls` instead of describing actions in text.
- Stream errors now set `response.completed` status to `"failed"` instead of `"completed"`, and log `finish_reason` for diagnostics.
- Refactored streaming handler into `process_chat_stream!` macro for code reuse between initial attempt and retry.
- Cloned request body before first `.send()` to enable retry without re-parsing.
- Updated version to `1.6.2` across `package.json`, `Cargo.toml`, `tauri.conf.json`.
- Verification: `pnpm build` passed; `cargo test` passed with 23 tests.

## 1.6.1 - 2026-05-11

- Fixed browser/Vite preview for non-Tauri environments by loading mock Gateway, Claude, Codex, Provider, and log data instead of calling Tauri IPC.
- Reduced frontend full-state polling from 3 seconds to 12 seconds and paused polling while the page is hidden.
- Improved Codex Gateway reliability with third-party Chat Completions models by adding a tool-call guardrail system note and defaulting converted tool requests to `tool_choice: "auto"`.
- Fixed Codex streaming Responses event order so function-call items are completed before the final assistant message completion event.
- Repaired streaming function-call arguments with the shared JSON repair helper before final Responses events.
- Added targeted Rust tests for Codex tool-call guardrails and streaming argument repair.
- Added `CLAUDE.md` with project path, command, and preview notes for AI handoff.
- Verification: `pnpm build` passed; `cargo test` passed with 23 tests.

## 1.6.0

- Added the runtime compatibility layer in `src-tauri/src/compatibility.rs`.
- Added provider and Codex capability profiles.
- Added JSON repair, fake tool/action detection, safety gates, patch validation, context compression, agent recovery, benchmark, and diagnostics helpers.
- Enhanced Claude Gateway tool-call conversion and Chat Completions fallback.
- Enhanced Codex Responses-to-Chat conversion for string input, sync function calls, and streaming argument events.
- Redacted common secrets before writing request log errors.
- Preserved stream request IDs in logs.
- Preserved provider `base_url` during create/update.
