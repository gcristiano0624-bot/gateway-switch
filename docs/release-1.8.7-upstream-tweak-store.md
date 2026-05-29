# Gateway Switch 1.8.7 Release Notes

## Summary

Gateway Switch 1.8.7 adds a full Codex++ Upstream Tweak Store view and folds the Cold Start diagnostic refactor into the same release.

## Changes

- Added full upstream Codex++ Tweak Store display from `https://b-nnett.github.io/codex-plusplus/store/index.json`.
- Added derived archive URLs using the upstream Codex++ rule: `https://codeload.github.com/<repo>/tar.gz/<approvedCommitSha>`.
- Added source repository, approved commit, archive URL, installed status, installed version, and installed path display for store entries.
- Added legacy recommendation mapping for `Codex Context Used Meter`, `Hide Usage Alert`, `Codex Token Usage`, and `Codex List Pagebuster`.
- Hardened registry validation for schema version, safe GitHub repo names, manifest repo consistency, and full approved commit SHAs.
- Refactored Cold Start check/repair internals into `src-tauri/src/coldstart.rs` while preserving the existing Tauri commands.
- Added regression tests for store archive URL derivation, invalid registry entries, installed tweak detection, and legacy recommendation mapping.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test store -- --nocapture`
- `pnpm build`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.7_aarch64.dmg`

