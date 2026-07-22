# Gateway Switch v1.20.0 Release Notes

**Release date:** 2026-07-22
**Version:** 1.20.0
**Tag:** v1.20.0
**Platform:** macOS (aarch64 / Apple Silicon)
**Bundle size:** ~18 MB app / ~7.0 MB DMG

## Highlights

v1.20.0 is the "ChatGPT Merge + Streamlined Codex" release. It fixes all breakages caused by OpenAI merging Codex into ChatGPT (app rename, CLI config changes, model catalog updates), resolves critical routing bugs (tool-name 400 errors, MiMo/GLM infinite loops), and streamlines the app by removing the Codex++ desktop enhancement layer (asar patching + ad-hoc re-signing) in favor of CLI binding + local gateway proxy — which is more sustainable given ChatGPT's frequent update cycle.

### What's New in v1.20.0

#### 1. ChatGPT/Codex Merge Compatibility

- **App detection rewrite**: Locates the Codex desktop app via bundle ID `com.openai.codex` using Spotlight (`mdfind`), with ordered fallback to `/Applications/ChatGPT.app`, `~/Applications/ChatGPT.app`, and legacy `Codex.app` paths. Electron asar validation ensures the correct app is found regardless of rename.
- **CLI config modernization**: Version-adaptive config writer detects Codex CLI version. Builds ≥0.140 get the modern `[auth]` table layout; older builds keep top-level `preferred_auth_method`. Removed deprecated `requires_openai_auth` key.
- **[auth] table preservation**: Config cleanup logic strips only `preferred_auth_method` inside `[auth]`, dropping the header only if nothing else remains, preserving user-owned keys.
- **GPT-5.x model catalog**: Default model list and database seed/backfill updated to gpt-5.6 era (gpt-5.6-sol/terra/luna, gpt-5.5, gpt-5.3-codex, gpt-5.1-codex, gpt-5.1-codex-mini). Model metadata (context window, max output tokens) is written to config.toml to eliminate "Unknown model" warnings.

#### 2. Critical Bug Fixes

- **Tool name sanitization fix**: All tool kinds (including `function`) are now sanitized on send (dots/colons → underscores), fixing 400 InvalidParameter errors on Kimi/Volcengine when MCP tools contain dots or colons. Original names are restored on response items. Added round-trip tests.
- **MiMo/GLM infinite loop fix**: The streaming retry that forcibly set `tool_choice=required` after empty tool responses caused infinite tool-call loops on MiMo and GLM. Added `should_retry_with_required()` guard that detects MiMo (by strategy ID/model name) and GLM (by model name) and skips the forced retry.
- **Bind mode persistence**: `codex_profile.bind_mode` column tracks `relay` (enhanced) vs `official` mode across apply/restore operations.

#### 3. Codex++ Desktop Enhancement Removed

Given ChatGPT's rapid update cycle (every patch invalidates asar patches and ad-hoc re-signatures), the Codex++ desktop enhancement module has been removed. The CLI binding + local HTTP gateway proxy are now the sole enhancement path, which is immune to app updates since it only modifies `~/.codex/config.toml`.

- Removed: Electron asar patching, loader injection, ElectronAsarIntegrity hash updates, ad-hoc codesigning, launchd watcher, tweak market, recommended scripts, CLI headless mode (~3300 lines Rust, ~1500 lines TypeScript/React).
- Removed dependencies: `flate2`, `tar`, `plist`, `sha2` (reduced binary size).
- PATH resolution utilities inlined into `codex_binding.rs`.
- Legacy `[model_providers.CodexPlusPlus]` config cleanup preserved for backward compatibility.

## Installation

### System Requirements

- macOS 12.0 or later (Apple Silicon / aarch64)
- ~50 MB free disk space

### Install Steps

1. Download `Gateway Switch_1.20.0_aarch64.dmg` from the release assets.
2. Double-click to mount the DMG.
3. Drag `Gateway Switch.app` into `Applications`.
4. Launch from `/Applications/Gateway Switch.app`.
5. If Gatekeeper blocks the app, right-click → Open → Open.

### Important: Run from /Applications

Do not run the app directly from the DMG. The app detects when it's launched from a read-only volume and shows a warning. Always copy to `/Applications` first.

## Verification

- Rust unit tests: `cd src-tauri && cargo test` → **117 passed, 0 failed**
- Frontend build: `pnpm build` (tsc + vite production build)
- Tauri build: `pnpm tauri build`
- DMG size: ~7.0 MB
- App size: ~18 MB

## Known Limitations

- Cross-request function_call history is in-memory only; lost on app restart.
- `tool_search` downgrade uses `query` parameter only; advanced search filters are not preserved.
- No Developer ID signing / notarization — Gatekeeper will show a warning on first launch.
- Desktop app patching (Codex++ style) is intentionally removed; only CLI config binding is supported for Codex enhancement.

## Upgrading from v1.19.0

1. Quit Gateway Switch.
2. Replace `/Applications/Gateway Switch.app` with the new version.
3. Launch — all configuration (providers, routes, logs, bind mode) is stored in `~/Library/Application Support/Gateway Switch/` and will be preserved.
4. If you previously had Codex++ desktop enhancement installed, the old user data directory at `~/Library/Application Support/codex-plusplus/` can be safely deleted manually (the app no longer references it).

## Rollback

If you experience issues, the previous stable release is v1.19.0. Configuration is backward compatible.

## Feedback

Report issues at: https://github.com/gcristiano0624-bot/gateway-switch/issues
