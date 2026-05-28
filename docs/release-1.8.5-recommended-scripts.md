# Gateway Switch 1.8.5 Release Notes

## Summary

Gateway Switch 1.8.5 adds a safe Codex++ Recommended Scripts panel for the four script-market utilities requested by the user.

## Changes

- Added Recommended Scripts status for `Codex Context Used Meter`, `Hide Usage Alert`, `Codex Token Usage`, and `Codex List Pagebuster`.
- Added native Codex++ user-script storage detection before enabling installation.
- Added safe gating so Gateway Switch refuses to write script files when the installed Codex++ runtime does not expose a supported native user-script host.
- Added Tauri commands for recommended-script status and install workflows.
- Kept the existing Codex++ Tweak Store grid unchanged.
- Added regression tests for unknown-storage and detected-storage script reports.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test recommended_scripts_report -- --nocapture`
- `pnpm build`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.5_aarch64.dmg`

