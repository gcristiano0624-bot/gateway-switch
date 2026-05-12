# Changelog

This file tracks user-visible Gateway Switch changes so future AI agents can quickly understand release history. For deeper architecture context, read `docs/project.md`.

## 1.6.3 - 2026-05-12

- Refreshed the whole app with a Claude Warm Native UI: white surfaces, warm paper backgrounds, ink text, oxblood accents, low-saturation semantic colors, and softer native desktop cards.
- Reworked the left navigation into a compact icon rail with hover labels to free space for the main Gateway Switch workbench.
- Redesigned the in-app brand mark, App Icon, and tray/status icon around a white `Gateway Pin` route symbol with a Claude oxblood center point.
- Updated typography to Geist, Fraunces, and Geist Mono and aligned buttons, tables, forms, badges, health bars, provider cards, and route cards with the new visual system.
- Updated version to `1.6.3` across `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json`.
- Verification: `pnpm build`, `cargo test`, and `pnpm tauri build`.

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
