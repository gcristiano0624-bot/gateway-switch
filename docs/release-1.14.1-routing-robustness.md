# Gateway Switch 1.14.1 Release Notes

Version: v1.14.1
Date: 2026-06-29
Repository: https://github.com/gcristiano0624-bot/gateway-switch

## Summary

Gateway Switch 1.14.1 is a gateway routing robustness patch. It fixes a Claude routing issue for Anthropic-compatible third-party providers, adds upstream stream truncation detection and rate-limit throttling, strips unsupported MiniMax thinking parameters, and logs stream stop reasons for diagnostics.

## Highlights

- **Claude routing fix**: Anthropic-compatible third-party providers now resolve to the correct upstream endpoint instead of falling back to the default Anthropic route.
- **Stream truncation detection**: When an upstream provider cuts off a streaming response mid-flight, the gateway surfaces a clear truncation event rather than silently completing.
- **Rate-limit throttling**: Automatic provider throttling after 429 responses prevents rapid-fire retries that worsen backpressure.
- **MiniMax thinking param strip**: Unsupported thinking parameters are stripped before forwarding to MiniMax-compatible providers, avoiding 400-level rejections.
- **Stream stop-reason logging**: `finish_reason` and upstream cutoff causes are now recorded in diagnostics for post-mortem analysis.

## Commits

- `8600401` — fix Claude routing for Anthropic-compatible providers
- `62b10fd` — surface upstream stream truncation
- `64529c5` — throttle provider after upstream rate limit
- `55c2e99` — strip unsupported MiniMax thinking params
- `30c78fa` — log stream stop reasons for diagnostics

## Verification

- `cargo test` — 82 passed, 0 failed
- `tsc && vite build` — passed
- `pnpm tauri build` — passed
- DMG validation — UDIF read-only, 7.2 MB

## Artifact

- DMG: `Gateway Switch_1.14.1_aarch64.dmg` (7.2 MB)
- SHA256: `ad6dd55702523dbfd9500155558f3e85d577f221`
- Tag: `v1.14.1`

## Known Limitations

- The Provider Wizard health-check feature (designed in `docs/superpowers/specs/2026-05-31-v1.14.0-*`) remains a design draft and is not included in this release.
