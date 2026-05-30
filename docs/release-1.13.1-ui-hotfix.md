# Gateway Switch v1.13.1 - Claude Code UI Hotfix

## What Changed

- Fixed the Claude Code Direct Provider risk confirmation layout.
- Replaced the loose red warning text and checkbox with a compact risk confirmation card.
- Added scoped wrapping rules so the confirmation sentence stays readable instead of collapsing into vertical letters.
- Fixed the sidebar version label so it follows the build-time package version instead of a hardcoded stale value.
- Fixed a follow-up CSS conflict where generic binding input styles made the checkbox occupy the full row width and squeezed the confirmation sentence into a vertical column.

## Validation

- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`
- `hdiutil imageinfo` for the generated DMG

## Install Note

Install `Gateway Switch.app` into `/Applications`. Do not run it directly from the mounted DMG.
