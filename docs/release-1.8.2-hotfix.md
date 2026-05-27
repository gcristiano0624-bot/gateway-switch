# Gateway Switch 1.8.2 Hotfix Release Notes

## Summary

Gateway Switch 1.8.2 improves Codex++ native repair diagnostics and makes UI Safe Mode configurable before startup.

## Changes

- Added `~/Library/Application Support/codex-plusplus/log/native-debug.log` for detailed native repair diagnostics.
- Kept `native-install.log` focused on readable install and repair progress.
- Added detailed `app.asar.unpacked` diagnostics: required file status, native module file sizes, `.node` count, and sample module paths.
- Added detailed Node/npm path diagnostics: raw PATH, augmented PATH, and per-candidate existence checks.
- Added `codexPlusPlus.uiSafeMode`, default `false`, for preconfiguring UI Safe Mode.
- UI Safe Mode disables only `co.bennett.ui-improvements`; routing, script market, history repair, watcher, and CLI shim remain active.
- Added regression coverage for UI Safe Mode behavior.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test codex_pp::native_install_acceptance_tests::native_real_repair_smoke -- --ignored --nocapture --test-threads=1`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.2_aarch64.dmg`
