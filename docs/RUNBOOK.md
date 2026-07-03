# Runbook — Gateway Switch

**Version:** v1.19.0
**Last updated:** 2026-07-03

## Prerequisites

- macOS 12.0+ (Apple Silicon / aarch64)
- Node.js 18+ (for pnpm)
- Rust stable (for Tauri build)
- pnpm (`npm install -g pnpm`)
- Xcode Command Line Tools
- GitHub CLI (`gh`) — for release upload

## Build Commands

### Install Dependencies

```bash
pnpm install
```

### Frontend Only

```bash
# Development (browser preview with mock data)
pnpm dev

# Production build
pnpm build
```

### Tauri App

```bash
# Development (full app with real backend)
pnpm tauri dev

# Production build (app + dmg)
pnpm tauri build
```

## Test Commands

### Rust Tests

```bash
cd src-tauri && cargo test --lib
```

Expected: **116 passed, 3 ignored**

### Frontend Type Check

```bash
pnpm exec tsc --noEmit
```

## Package Commands

### Build App + DMG

```bash
pnpm tauri build
```

Produces:
- App: `src-tauri/target/release/bundle/macos/Gateway Switch.app`
- DMG: `src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg`

### Manual DMG Creation (alternative)

```bash
mkdir -p dist
hdiutil create -volname "Gateway Switch" \
  -srcfolder "src-tauri/target/release/bundle/macos/Gateway Switch.app" \
  -ov -format UDZO \
  dist/Gateway-Switch.dmg
```

## DMG Validation

```bash
# Image info
hdiutil imageinfo "src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg"

# Size check
/usr/bin/du -sh "src-tauri/target/release/bundle/macos/Gateway Switch.app"
/usr/bin/du -sh "src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg"

# App exists and is executable
test -x "src-tauri/target/release/bundle/macos/Gateway Switch.app/Contents/MacOS/Gateway Switch"
```

## GitHub Release Commands

### Authentication

```bash
gh auth status
gh auth login   # if not authenticated
```

### Tag and Push

```bash
# Create annotated tag
git tag -a v1.19.0 -m "Gateway Switch v1.19.0"

# Push branch + tag
git push origin main
git push origin v1.19.0
```

### Create Release

```bash
gh release create v1.19.0 \
  "src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg" \
  --repo gcristiano0624-bot/gateway-switch \
  --title "Gateway Switch v1.19.0" \
  --notes-file docs/RELEASE_NOTES.md
```

### Update Existing Release Asset

```bash
gh release upload v1.19.0 \
  "src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg" \
  --repo gcristiano0624-bot/gateway-switch \
  --clobber
```

### Verify Release

```bash
gh repo view gcristiano0624-bot/gateway-switch --json nameWithOwner,visibility,url,defaultBranchRef
gh release view v1.19.0 --repo gcristiano0624-bot/gateway-switch --json name,tagName,url,isDraft,isPrerelease,assets
```

## Common Failures

### `hdiutil` device errors

If `hdiutil create` fails with "resource busy" or device errors, try:
- Close any mounted DMG volumes
- Run `diskutil list` and `diskutil eject` any stale volumes
- Build with Tauri's built-in bundler instead: `pnpm tauri build`

### Rust compile errors after changing dependencies

```bash
cd src-tauri && cargo clean && cd .. && pnpm tauri build
```

### GitHub auth failures

```bash
gh auth login
# Follow prompts to authenticate with browser
```

### Frontend build fails

```bash
rm -rf node_modules && pnpm install
```

## Smoke Testing After Install

1. Launch `/Applications/Gateway Switch.app`
2. Check that the app opens without error
3. Verify the sidebar shows version `v1.19.0`
4. Go to Dashboard — should show "Gateway stopped" or gateway status
5. Go to Logs — should show request log table
6. Start the gateway
7. Test with Claude Desktop or Codex App
