# Changelog

This file tracks user-visible Gateway Switch changes so future AI agents can quickly understand release history. For deeper architecture context, read `docs/project.md`.

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
