# Gateway Switch 1.8.3 Hotfix Release Notes

## Summary

Gateway Switch 1.8.3 fixes Codex++ watcher and maintenance-button failures caused by running Gateway Switch from a mounted DMG.

## Changes

- Avoid storing transient `/Volumes/Gateway Switch/...` executable paths in Codex++ CLI shims and launchd watcher state.
- Prefer `/Applications/Gateway Switch.app/Contents/MacOS/gateway-switch` when Gateway Switch is launched from a disk image and the installed app exists.
- Generate the watcher command in the Codex++ health-check-compatible form: `CODEX_PLUSPLUS_WATCHER=1 codexplusplus update --watcher --quiet`.
- Patch staged Codex++ runtime health checks to accept `launchctl print gui/$UID/com.codexplusplus.watcher` when modern macOS returns false negatives from `launchctl list <label>`.
- Write watcher health logs to `~/Library/Logs/codex-plusplus-watcher.log`, matching the path read by the Codex++ settings page.
- Reset stale watcher health log content during watcher installation so old EPERM or DMG-path failures do not keep the settings page in Review/Failed state.

## Local Repair Performed

- Rewrote `~/Library/LaunchAgents/com.codexplusplus.watcher.plist` to remove the DMG path.
- Reloaded `com.codexplusplus.watcher`.
- Patched the currently staged Codex++ runtime health files.
- Verified the runtime watcher health summary is `Auto-repair watcher is ready`.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test ui_safe_mode_only_disables_page_enhancement -- --nocapture`
- `node -e` watcher health smoke using `~/Library/Application Support/codex-plusplus/runtime/watcher-health.js`
- `launchctl print gui/$UID/com.codexplusplus.watcher`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.3_aarch64.dmg`
