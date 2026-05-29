# Gateway Switch 1.8.8 Release Notes

## Summary

Gateway Switch 1.8.8 fixes Claude Code usage with Volcengine Ark DeepSeek coding models by routing them through Gateway Route compatibility conversion instead of unsafe Direct Provider binding.

## Changes

- Added a user/assistant-only Chat role mode for Volcengine / Ark / DeepSeek Claude routes.
- Merged Anthropic `system` instructions into the first user message for providers that reject `messages.role = system`.
- Converted Anthropic tool results into user messages for endpoints that reject `tool` roles.
- Blocked Claude Code Direct Provider binding for Volcengine DeepSeek models and directs users to Gateway Route.
- Added frontend warning copy on the Claude Code Direct Provider panel.
- Added regression tests for role-mode detection and user/assistant-only payload conversion.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test volcengine -- --nocapture`
- `pnpm build`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.8_aarch64.dmg`

