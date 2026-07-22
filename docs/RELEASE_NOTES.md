# Gateway Switch v1.20.1 Release Notes

**Release date:** 2026-07-22
**Version:** 1.20.1
**Tag:** v1.20.1
**Platform:** macOS (aarch64 / Apple Silicon)
**Bundle size:** ~18 MB app / ~7.0 MB DMG

## Highlights

v1.20.1 is a hotfix release for v1.20.0 that resolves two critical regressions: the "Command not found" error when clicking the Claude/Codex Bind buttons, and the 502 Bad Gateway error when using Codex with Volcengine Ark (火山方舟) endpoints.

### What's Fixed in v1.20.1

#### 1. Bind Button "Command Not Found" Fix

- **Root cause**: The async Tauri commands `start_and_bind_claude` and `start_and_bind_codex` (which start the local gateway and apply config binding in a single call) were defined in `commands.rs` with `#[tauri::command]` attributes but were never registered in the `invoke_handler!` macro in `lib.rs`. This caused the frontend `invoke()` calls to fail with "Command start_and_bind_claude not found" / "Command start_and_bind_codex not found".
- **Fix**: Added both commands to the `tauri::generate_handler![]` registration list in `lib.rs`.
- **Verification**: Rust dead-code warnings reduced from 8 to 6 (the two commands are now properly referenced).

#### 2. Codex 502 Bad Gateway on Volcengine/火山引擎 Fix

- **Root cause**: The Volcengine DeepSeek compatibility profile in `gateway_strategy.rs` had `strip_unsupported_params: false` and `codex_strict_tool_calls: true`. This caused two validation failures against Volcengine Ark's strict Chat Completions API:
  - `stream_options: {include_usage: true}` was injected into streaming requests (Ark does not support this parameter → "A parameter specified in the request is not valid").
  - `tool_choice: "required"` was forced (Ark only supports `"auto"`/`"none"` for tool_choice, not `"required"`).
- **Fix**:
  - Set `strip_unsupported_params: true` and `codex_strict_tool_calls: false` for Volcengine DeepSeek.
  - Expanded `apply_codex_provider_policy()` to strip 15+ OpenAI-exclusive parameters for strict providers: `parallel_tool_calls`, `stream_options`, `frequency_penalty`, `presence_penalty`, `response_format`, `seed`, `logprobs`, `top_logprobs`, `logit_bias`, `service_tier`, `modalities`, `prediction`, `audio`, `store`, `metadata`.
  - Added auto-downgrade: `tool_choice: "required"` is automatically downgraded to `"auto"` when `strip_unsupported_params` is active, preventing validation errors on providers that don't support `"required"`.

## Installation

### System Requirements

- macOS 12.0 or later (Apple Silicon / aarch64)
- ~50 MB free disk space

### Install Steps

1. Download `Gateway.Switch_1.20.1_aarch64.dmg` from the release assets.
2. Double-click to mount the DMG.
3. Drag `Gateway Switch.app` into `Applications`.
4. Launch from `/Applications/Gateway Switch.app`.
5. If Gatekeeper blocks the app, right-click → Open → Open.

### Important: Run from /Applications

Do not run the app directly from the DMG. The app detects when it's launched from a read-only volume and shows a warning. Always copy to `/Applications` first.

## Verification

- Rust unit tests: `cd src-tauri && cargo test` → **117 passed, 0 failed**
- Frontend build: `pnpm build` (tsc + vite production build)
- Tauri build: `pnpm tauri build`
- DMG size: ~7.0 MB
- App size: ~18 MB

## Known Limitations

- Cross-request function_call history is in-memory only; lost on app restart.
- `tool_search` downgrade uses `query` parameter only; advanced search filters are not preserved.
- No Developer ID signing / notarization — Gatekeeper will show a warning on first launch.
- Desktop app patching (Codex++ style) is intentionally removed; only CLI config binding is supported for Codex enhancement.

## Upgrading from v1.20.0

1. Quit Gateway Switch (including the menu bar icon).
2. Replace `/Applications/Gateway Switch.app` with the new version.
3. Launch — all configuration (providers, routes, logs, bind mode) is stored in `~/Library/Application Support/Gateway Switch/` and will be preserved.
4. Re-apply Codex binding after launching (the fix is in the gateway code, not in config).

## Upgrading from v1.19.0 or earlier

1. Quit Gateway Switch.
2. Replace `/Applications/Gateway Switch.app` with the new version.
3. Launch — existing configuration is preserved.
4. If you previously had Codex++ desktop enhancement installed, the old user data directory at `~/Library/Application Support/codex-plusplus/` can be safely deleted manually.

## Rollback

If you experience issues, the previous stable release is v1.20.0. Configuration is backward compatible.

## Feedback

Report issues at: https://github.com/gcristiano0624-bot/gateway-switch/issues
