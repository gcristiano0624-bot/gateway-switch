# Gateway Switch 1.7.0 Repair Report

Date: 2026-05-18

## Summary

This report captures the 1.7.0 implementation, verification, packaging, and release work for Gateway Switch.

Primary goals completed in this release:

- Updated the left sidebar to permanent icon + text labels.
- Fixed the floating `Gateway Switch v1.6.3` label misalignment by moving version display to the stable sidebar footer.
- Added the new `Cold Start Doctor` workspace with three phases:
  - readiness overview
  - execution and repair log
  - capability matrix and report summary
- Added backend cold-start check and safe-repair commands.
- Added structured cold-start logging and Markdown report generation.
- Released version `1.7.0` locally and on GitHub.

## UI Changes

### Left Navigation

- Keeps the compact sidebar layout.
- Every tab now has a visible text label under the icon.
- Added a dedicated `Cold Start` tab.

Tabs covered:

- Dashboard
- Claude
- Claude Code
- Codex
- Cold Start
- Providers
- Logs
- Settings

### Version Alignment Fix

Previous issue:

- The floating sidebar brand tooltip displayed `Gateway Switch v1.6.3` and appeared visually offset.

Fix applied:

- Removed the floating brand version tooltip behavior.
- Added a stable `v1.7.0` version label to the sidebar footer.

## Cold Start Doctor Design

The final implementation combines the previously reviewed A/B/C concepts into one runtime workspace.

### Phase A: Readiness Overview

Shows the current state of:

- Claude Desktop
- Codex App
- Gateway processes
- MCP / tool readiness
- Security risk

### Phase B: Execution and Repair Log

Shows step-by-step execution records for:

- environment discovery
- provider and route inventory
- binding inspection
- local gateway startup
- health checks
- safe binding repair
- final report generation

### Phase C: Capability Matrix

Shows the final summarized state for:

- Claude Desktop compatibility
- Codex compatibility
- provider and route readiness
- health endpoint status
- security risk
- auto fixes and manual fixes

## Backend Changes

Added new Tauri command endpoints:

- `get_coldstart_status`
- `run_coldstart_repair`

Added new data models:

- `ColdStartReport`
- `ColdStartStep`
- `ColdStartCapability`

Added runtime behavior:

- inspect Claude Desktop, Claude Code, and Codex config state
- inspect provider and route inventory
- inspect local Claude and Codex gateway process state
- run health checks against local endpoints
- safely start stopped local gateways when repair mode is enabled
- safely apply backup-backed Claude Desktop and Codex bindings when routes are available
- generate a Markdown cold-start report to the app backup directory

## Logging Strategy

Detailed logs were added to all major cold-start nodes.

Format:

```text
[coldstart][target][status] label: detail
```

Typical targets:

- `system`
- `gateway`
- `Claude`
- `Codex`
- `Security`
- `capability`

Typical statuses:

- `ok`
- `warn`
- `running`
- `fixed`
- `error`

## Additional Stability Work Included In 1.7.0

Also merged into this release:

- Codex Responses gateway request body limit was raised above Axum default.
- Added regression coverage for large request bodies.

## Coldstart Skill References

Reference workflow files included in repository:

- `coldstart/claude_coldstart_skill.md`
- `coldstart/codex_coldstart.skill.md`

These files are not executed automatically by the app, but the implemented Cold Start Doctor UI and backend were designed using them as feature references.

## Versioning

Updated to `1.7.0` in:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`
- `src/App.tsx`
- release docs and changelog

## Verification

### Build

```bash
pnpm build
```

Result:

- passed

### Rust Tests

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test
```

Result:

- passed
- 27 tests passed

### Local Packaging

```bash
CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build --bundles app,dmg
```

Result:

- passed

Generated artifacts:

- `src-tauri/target/release/bundle/macos/Gateway Switch.app`
- `src-tauri/target/release/bundle/dmg/Gateway Switch_1.7.0_aarch64.dmg`

## GitHub Release

Repository:

- `origin https://github.com/gcristiano0624-bot/gateway-switch.git`

Release:

- `v1.7.0`
- https://github.com/gcristiano0624-bot/gateway-switch/releases/tag/v1.7.0

Uploaded asset:

- `Gateway.Switch_1.7.0_aarch64.dmg`

## Local Runtime Validation Plan

The intended runtime validation after packaging is:

1. Launch packaged `Gateway Switch.app`.
2. Enter `Cold Start` tab.
3. Trigger `Run Check & Safe Fixes`.
4. Observe UI phase updates and Rust cold-start logs.
5. Confirm a cold-start Markdown report is generated.

## Notes

- One unrelated untracked file remained outside this release scope:
  - `streamer-edu-workbench-demo.html`
- It was intentionally not included in the release commit.
