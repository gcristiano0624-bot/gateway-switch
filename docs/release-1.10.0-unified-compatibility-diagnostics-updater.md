# Gateway Switch 1.10.0 Release Notes

Release date: 2026-05-29

Gateway Switch 1.10.0 completes the unified compatibility roadmap across Claude Desktop, Claude Code, and Codex. It adds real failed-request diagnostics, editable provider strategies, one-click Claude Code repair, Codex route diagnostics, update checks, safe install guidance, and a larger provider profile matrix.

## Highlights

- Real failed-request snapshots are stored locally with sanitized original payloads, converted upstream payloads, redaction summaries, and likely-cause analysis.
- Provider strategies are now editable per service provider with nullable overrides that inherit automatic profiles by default.
- Claude Code can be repaired from unsafe Direct Provider mode to Gateway Route with a settings backup.
- Codex Gateway uses the same effective Provider Compatibility Profile as Claude routes and reports strict tool-call / reasoning cleanup behavior.
- Settings includes a GitHub Release update checker and a safe install plan to avoid DMG or temporary-path execution issues.
- Provider profile coverage expands to OpenRouter, Xiaomi MiMo, DeepSeek official, Moonshot Kimi, Qwen DashScope, Volcengine Ark, standard Anthropic, and OpenAI Chat fallback.

## Verification

- `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all`
- `PATH="$HOME/.cargo/bin:$PATH" cargo check`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test provider_policy -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test replay_report -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test codex -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test provider_policy_round_trip -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test request_snapshot_round_trip -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test version_comparison -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test safe_install_plan -- --nocapture`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test test_codex_route_diagnostics_use_effective_provider_policy -- --nocapture`
- `pnpm build`

## Artifact

Expected DMG path after packaging:

```text
src-tauri/target/release/bundle/dmg/Gateway Switch_1.10.0_aarch64.dmg
```
