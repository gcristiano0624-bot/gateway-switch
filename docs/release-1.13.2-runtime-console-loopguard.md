# Gateway Switch 1.13.2 Release Notes

Date: 2026-06-02

## Summary

Gateway Switch 1.13.2 ships the Runtime Console product refactor and the first LoopGuard iteration for agent loop mitigation. The release keeps the product boundary focused on Claude Desktop, Claude Code, and Codex while centralizing route CRUD in Route Builder.

## Highlights

- Runtime Console now includes Dashboard, App Workbench, Provider Console, Route Builder, Health Center, and Usage Insights.
- Claude Desktop, Claude Code, and Codex workbench pages are read-only route and runtime status surfaces.
- Route Builder owns route creation, editing, alias updates, and Codex route configuration.
- Codex restore now returns to OpenAI auth mode by clearing Gateway Switch/Codex++ config and stale API-key auth entries.
- Claude Desktop gateway-offline checks show friendlier health messages.
- LoopGuard compresses oversized `tool_result` payloads, detects repeated tool-call fingerprints, and injects a warm strategy-change hint.

## Validation

- `PATH="$HOME/.cargo/bin:$PATH" cargo test`
- `pnpm exec tsc --noEmit`
- `pnpm build`
- `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`
- `GATEWAY_SWITCH_LOOP_GUARD_DEBUG=1 cargo test simulation_repeated_tool_calls_and_large_result_debug_trace -- --nocapture`
- Local `/v1/messages` smoke request against `cargo run` gateway on port `3456`
- `hdiutil verify` and read-only mount inspection for the final DMG

## Packaging

- App: `Gateway Switch.app`
- DMG: `Gateway Switch_1.13.2_aarch64.dmg`
- Target repo: `https://github.com/gcristiano0624-bot/gateway-switch`
- Tag: `v1.13.2`

## Known Notes

- The macOS package is locally ad-hoc signed for distribution through GitHub Release assets.
- Real binding, gateway, Codex restore, and config operations require the Tauri app runtime; browser preview uses mock UI data.
- Usage Insights in this release focuses on request volume and reliability, not cost accounting.
