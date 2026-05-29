# Gateway Switch 1.9.0 Release Notes

## Summary

Gateway Switch 1.9.0 adds Provider Compatibility Profiles, Claude Code route diagnostics, redacted payload preview, and runtime source warnings.

## Changes

- Added compatibility strategies for Claude routes, including `standard_anthropic`, `openai_chat_fallback`, and `volcengine_deepseek_coding`.
- Added Route Diagnostics for Claude Code so users can see whether Direct Provider is safe or Gateway Route is required.
- Added redacted Payload Preview for converted upstream Chat payloads without calling the provider.
- Added runtime source classification for `/Applications`, `/Volumes`, and temporary paths.
- Added regression tests for route diagnostics, payload preview role conversion, and runtime source reports.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test route_diagnostics -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test runtime_source_report -- --nocapture`
- `pnpm build`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.9.0_aarch64.dmg`

