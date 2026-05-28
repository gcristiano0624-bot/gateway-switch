# Changelog

This file tracks user-visible Gateway Switch changes so future AI agents can quickly understand release history. For deeper architecture context, read `docs/project.md`.

## 1.8.6 - 2026-05-28

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
