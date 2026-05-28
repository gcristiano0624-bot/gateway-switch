# Gateway Switch 1.8.4 Release Notes

## Summary

Gateway Switch 1.8.4 improves the desktop UI navigation structure and responsive layout behavior.

## Changes

- Regrouped the left sidebar into Dashboard, Products, Features, General, and System.
- Moved MCP Sync and Cold Start out of Products into the Features group.
- Kept `Claude Code`, `MCP Sync`, and `Cold Start` on a single line instead of using manual line breaks.
- Added a fluid sidebar width on desktop and an icon-only rail for narrow windows.
- Hardened content responsiveness with safer grid constraints, table scrolling, and card overflow protection.
- Reduced the minimum Tauri window size to `760x560`.

## Validation

- `pnpm build`
- `git diff --check`
- VS Code diagnostics for `src/App.tsx`
- VS Code diagnostics for `src/App.css`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`

## Local Artifact

- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.8.4_aarch64.dmg`
