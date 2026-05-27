# Gateway Switch 1.8.1 Hotfix Release Notes

## Summary

Gateway Switch 1.8.1 is a focused hotfix for Codex++ native repair reliability and Claude Desktop large-request routing behavior.

## Fixes

- Restores missing `app.asar.unpacked` native modules from codex++ backups during repair.
- Re-signs restored native modules such as `better_sqlite3.node`, `pty.node`, and `objc-js` modules.
- Resolves Node/npm from common macOS paths when Gateway Switch runs from GUI or launchd.
- Injects an augmented PATH into npm subprocesses so `/usr/bin/env node` works under launchd.
- Adds focused repair logs for unpacked module recovery and command path resolution.
- Adds UI Safe Mode to disable only the page-enhancement tweak while preserving other codex++ features.
- Prevents Claude Desktop `413 Request too large` responses from being retried through Chat Completions fallback.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test codex_pp::native_install_acceptance_tests::native_real_repair_smoke -- --ignored --nocapture --test-threads=1`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.1_aarch64.dmg`
