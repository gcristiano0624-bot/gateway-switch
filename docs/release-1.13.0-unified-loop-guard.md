# Gateway Switch v1.13.0 - Unified Loop Guard

## Why

Some Chat-only providers can repeat planning text, reasoning fragments, or tool-call plans when used through Claude Desktop, Claude Code, Codex, Obsidian, or Claudian. v1.13.0 addresses this as a router-wide behavior instead of treating it as a single-client issue.

## What's New

- Shared `LoopGuard` module for Claude Gateway and Codex Gateway.
- Repeated text suppression based on normalized chunks and stable sentence/paragraph fingerprints.
- No blind hard stop based on output length; normal long-form reports continue streaming.
- Codex Responses stream filtering before `response.output_text.delta` emission.
- Duplicate tool-call fingerprints recorded in diagnostics.
- Xiaomi MiMo Codex routes default away from strict tool-call enforcement to reduce repeated tool-planning loops.

## Validation

- `cargo check`
- `cargo test loop_guard -- --nocapture`
- Targeted Codex and Xiaomi route compatibility tests
- Full release validation should include `cargo test --locked`, `pnpm build`, and `pnpm tauri build`.

## Note

The app is still ad-hoc signed. Install the `.app` into `/Applications` instead of running directly from the mounted DMG.
