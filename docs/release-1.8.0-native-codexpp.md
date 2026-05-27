# Gateway Switch 1.8.0 Release Notes

## Summary

Gateway Switch 1.8.0 is the release that turns codex++ support into a native product capability instead of an external wrapper. The app can now install, patch, sign, repair, and maintain `Codex.app` from inside Gateway Switch with a Rust-managed transaction flow.

## User-visible changes

- Added one-click codex++ install and repair from Gateway Switch.
- Added install preflight checks for Node, npm, and `Codex.app` availability.
- Added streaming install and repair logs in the Codex++ UI.
- Added native local signing for `install-local`.
- Added native default tweaks installation.
- Added native CLI shims for `codexplusplus` and `codex-plusplus`.
- Added native `launchd` watcher installation and repair entrypoint management.

## Technical changes

- Reimplemented source download, extract, source switching, and rollback in Rust.
- Reimplemented `app.asar` patching and `ElectronAsarIntegrity` update in Rust.
- Added local signing identity creation and bundle re-signing support.
- Unified GUI actions, watcher execution, and CLI shims on `gateway-switch codexpp ...`.
- Added real-machine acceptance tests for install, rollback, and repair.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test -- --ignored --nocapture --test-threads=1`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local artifacts

Expected local bundle output after packaging:

- `src-tauri/target/release/bundle/macos/Gateway Switch.app`
- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.0_aarch64.dmg`

## Release focus

This release is about making Gateway Switch the single desktop product that owns Claude gateway routing, Codex gateway routing, codex++ install and repair, watcher management, and local runtime maintenance in one place.
