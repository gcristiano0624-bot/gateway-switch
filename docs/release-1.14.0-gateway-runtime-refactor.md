# Gateway Switch 1.14.0 Release Notes

Version: v1.14.0
Date: 2026-06-12
Repository: https://github.com/gcristiano0624-bot/gateway-switch

## Summary

Gateway Switch 1.14.0 is a backend maintainability release for the Claude Gateway runtime. It splits the previously overloaded `gateway.rs` into focused strategy, protocol, and diagnostics modules while keeping the existing runtime behavior and public command surface stable.

## Highlights

- Provider compatibility profile detection and manual policy application now live in `gateway_strategy.rs`.
- Anthropic/OpenAI Chat payload conversion, tool-call conversion, stream delta extraction, and token estimation now live in `gateway_protocol.rs`.
- Failure snapshot capture rules, payload redaction, likely-cause classification, upstream body previews, and Anthropic fallback status checks now live in `gateway_diagnostics.rs`.
- `gateway.rs` continues to expose the compatibility profile API used by Codex Gateway, so existing backend callers do not need to change.
- No database migration, route schema change, frontend API change, or Tauri command rename is included in this release.

## Verification

- `git diff --check` — passed
- Editor diagnostics — passed
- `cargo test` — blocked in the current shell because `cargo` is not available
- `pnpm tauri build` / DMG generation — pending a Rust toolchain environment

## Artifact

- Planned DMG: `Gateway Switch_1.14.0_aarch64.dmg`
- Planned tag: `v1.14.0`

## Known Limitations

- This release does not implement the Provider Wizard health-check design draft under `docs/superpowers/specs/2026-05-31-v1.14.0-*`; those documents remain design context for a future feature release.
- The current execution environment needs Rust/Cargo restored before the final macOS DMG can be built and uploaded.
