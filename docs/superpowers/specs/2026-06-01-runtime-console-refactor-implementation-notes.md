# Runtime Console Refactor Implementation Notes

Date: 2026-06-01

## Scope Completed

- Reworked the product shell toward a Runtime Console model for Claude Desktop, Claude Code, and Codex.
- Added Runtime Dashboard, App Workbench summaries, Provider Console, Route Builder, Health Center, and Usage Insights as the primary flow.
- Added Rust aggregation commands for dashboard, app workbench, provider console, and usage insights.
- Added Rust apply/preview commands for Provider Wizard and Route Builder.
- Introduced shadcn-style lightweight UI foundations without replacing the whole UI stack.
- Modularized Runtime Console frontend code into `src/shared` and `src/features`.

## Frontend Structure

- Shared runtime DTOs: `src/shared/types/runtime-console.ts`
- Runtime Console API wrappers: `src/shared/api/runtime-console.ts`
- App workbench components: `src/features/app-workbench/`
- Dashboard components: `src/features/dashboard/`
- Provider Console components: `src/features/provider-console/`
- Route Builder components: `src/features/route-builder/`
- Health Center components: `src/features/health-center/`
- Usage Insights components: `src/features/usage-insights/`

## Backend Commands Added

- `get_runtime_dashboard`
- `get_app_workbench`
- `get_provider_console`
- `get_usage_insights`
- `preview_route_builder`
- `apply_route_builder`
- `preview_provider_wizard`
- `apply_provider_wizard`

## Validation

- Frontend build: `pnpm build`
- Rust tests: `PATH="$HOME/.cargo/bin:$PATH" cargo test` from `src-tauri` (`68 passed; 0 failed; 3 ignored`)
- Local package: `PATH="$HOME/.cargo/bin:$PATH" CI=false pnpm tauri build`
- Diff whitespace check: `git diff --check`
- VS Code diagnostics: no diagnostics reported during implementation

## Local Package

- App bundle: `src-tauri/target/release/bundle/macos/Gateway Switch.app`
- DMG bundle: `src-tauri/target/release/bundle/dmg/Gateway Switch_1.13.1_aarch64.dmg`
- Convenience copy: `release-artifacts/v1.13.1-runtime-console/Gateway Switch_1.13.1_aarch64.dmg`
- Local experience DMG after ad-hoc re-sign: `release-artifacts/v1.13.1-runtime-console/Gateway Switch_1.13.1_aarch64-local.dmg`
- DMG format: UDZO read-only compressed image
- Signing: ad-hoc local signing only; not Developer ID signed or notarized, so Gatekeeper can still reject it until opened explicitly for local testing

## Release Gate Checklist

- Verify `pnpm build`.
- Verify `PATH="$HOME/.cargo/bin:$PATH" cargo test` from `src-tauri`.
- Run the app with `pnpm tauri dev`.
- Test Provider Wizard with at least one OpenAI-compatible provider.
- Test Route Builder for Claude Desktop, Claude Code, and Codex.
- Test Health Center export.
- Test Usage Insights after sending at least one Claude or Codex request through the gateway.
- Confirm Claude Desktop binding still writes `displayName`, `display_name`, `labelOverride`, and `supports1m: true`.
- Confirm Gateway body limit remains 64 MiB.
- Confirm all gateway routes still use Unified Loop Guard.
