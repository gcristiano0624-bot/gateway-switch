# Gateway Switch Runtime Console Refactor Design

## 1. Goal

Gateway Switch will be refactored from an expert-oriented model routing utility into a macOS-first local runtime console for three supported products:

- Claude Desktop
- Claude Code
- Codex

The refactor borrows the strongest product patterns from `cc-switch`: app-centric workbenches, guided provider setup, unified health checks, and usage/reliability visibility. It does not copy `cc-switch` as a broad multi-CLI manager.

The new product direction is:

> Gateway Switch is the local runtime console that makes Claude Desktop, Claude Code, and Codex reliably run through third-party model providers.

## 2. Product Boundaries

### 2.1 In Scope

- Keep macOS as the primary supported platform.
- Keep Rust/Tauri as the native shell and backend foundation.
- Keep support focused on Claude Desktop, Claude Code, and Codex.
- Refactor the UI into a console-style product with app workbenches and shared operations modules.
- Introduce a modern, `cc-switch`-inspired UI stack with controlled bundle size.
- Add backend aggregation APIs for dashboard, app workbench, provider console, health checks, route builder, and usage insights.
- Preserve Gateway Switch's existing runtime compatibility strengths:
  - Claude Messages gateway
  - Codex Responses gateway
  - OpenAI Chat fallback
  - Provider Compatibility Profiles
  - Tool Call Repair
  - Unified Loop Guard
  - Redacted diagnostics

### 2.2 Out of Scope

- No Gemini, OpenCode, OpenClaw, Hermes, or other AI CLI/App support in this refactor.
- No cloud sync or user account system.
- No full cost accounting in the first usage insights version.
- No automatic token-consuming deep checks unless the user explicitly confirms.
- No implicit external client config mutation without user action.
- No rewrite of stable gateway runtime internals unless required by the new UI APIs.

## 3. Reference: What to Borrow From `cc-switch`

`cc-switch` has stronger product packaging in these areas:

- App-first navigation: users choose the target app first, then manage its provider, config, and status.
- Provider onboarding: presets and guided setup reduce configuration burden.
- Unified assets: providers, MCP, prompts, skills, sessions, and usage are organized as product assets.
- Operations visibility: usage, sessions, request trends, proxy health, and failover make the product feel mature.
- UI system: componentized React UI, Tailwind styling, shadcn/Radix-style components, and query-driven data loading.

Gateway Switch should borrow the product organization, not the entire feature surface.

## 4. New Information Architecture

```text
Overview
└─ Runtime Dashboard

Apps
├─ Claude Desktop
├─ Claude Code
└─ Codex

Setup
├─ Provider Console
├─ Route Builder
└─ Setup Wizard

Operations
├─ Health Center
├─ Request Logs
└─ Usage Insights

Assets
└─ MCP Sync

System
├─ Settings
├─ Install Doctor
└─ Diagnostics Export
```

Codex++ Tweaks should not be a standalone top-level navigation item. It belongs inside the Codex workbench as a Codex-specific enhancement area.

## 5. Core User Path

The primary path becomes:

```text
Choose target app
→ Add or select provider
→ Generate recommended route
→ Run quick health check
→ Bind app
→ Observe usage and reliability
```

This replaces the current expert path:

```text
Create provider
→ Manually create route
→ Find binding page
→ Run scattered health checks
→ Inspect logs manually
```

## 6. Module Design

### 6.1 Runtime Dashboard

Purpose:

- Act as the entry point for runtime readiness.
- Tell the user whether Claude Desktop, Claude Code, and Codex are usable now.
- Surface next actions instead of showing passive status only.

Layout:

- Top hero: overall runtime health, Claude Gateway, Codex Gateway, install source.
- App status row: Claude Desktop, Claude Code, Codex.
- Risk area: runtime source warning, unhealthy providers, missing bindings, failing routes.
- Activity area: recent requests, recent failures, recent Loop Guard/Tool Repair events.
- Setup progress: provider configured, route built, app bound, health checked.

Key components:

- `RuntimeStatusHero`
- `AppStatusCard`
- `RiskActionCard`
- `RecentActivityList`
- `SetupProgressStrip`

Data flow:

```text
RuntimeDashboard
→ get_runtime_dashboard()
→ gateway status + codex status + app bindings + provider counts + route counts + recent logs + runtime source
```

Backend scope:

- Add `get_runtime_dashboard()`.
- Reuse existing health, binding, provider, route, request log, and runtime source logic.

### 6.2 App Workbench

Purpose:

- Give each supported product a dedicated workbench.
- Let users configure by intent: "make Codex work", "make Claude Code use this provider", or "bind Claude Desktop".

Supported app ids:

- `claude_desktop`
- `claude_code`
- `codex`

Shared layout:

- App hero with status, binding, active route, and health.
- Four-card summary: binding state, active provider/route, recent requests, health result.
- Main action rail: Configure Provider, Build Route, Bind App, Run Check, View Logs.
- Diagnostics section with compatibility warnings and suggested fixes.

Claude Desktop workbench:

- Shows local Claude `/v1/messages` gateway.
- Shows Claude aliases and model routes.
- Shows Claude Desktop config binding and backup state.
- Links to route diagnostics and payload preview.

Claude Code workbench:

- Shows Gateway Route vs Direct Provider mode.
- Keeps explicit confirmation for risky direct providers.
- Recommends Gateway Route for Volcengine, DeepSeek, Xiaomi, and chat-only providers when needed.

Codex workbench:

- Shows local `/v1/responses` gateway.
- Shows Codex routes and Codex binding.
- Shows Responses-to-Chat fallback status.
- Contains Codex++ Tweaks as an internal Codex enhancement section.

Key components:

- `AppWorkbenchLayout`
- `AppBindingCard`
- `ActiveRouteCard`
- `AppHealthCard`
- `AppActionRail`
- `RecentAppRequests`
- `CompatibilityNotice`

Data flow:

```text
AppWorkbench(app_id)
→ get_app_workbench(app_id)
→ binding + routes + provider links + gateway health + recent logs + compatibility diagnostics
```

Backend scope:

- Add `get_app_workbench(app_id)`.
- Keep app id validation strict.
- Reuse existing binding and diagnostics modules.

### 6.3 Provider Console

Purpose:

- Upgrade the current Providers page into the shared upstream provider control center.
- Make provider setup understandable for non-expert users.
- Keep advanced compatibility strategy visible and explainable.

Layout:

- Top actions: Add Provider, Open Wizard, Import Preset, Check All.
- Left pane: provider list with health, surface support, and recent failures.
- Right pane: selected provider details.
- Details include identity, endpoints, auth, compatibility policy, linked routes, health, and recent failures.

Provider card fields:

- Provider name
- OpenAI Base URL
- Anthropic Base URL
- Supported surfaces: Claude, Claude Code, Codex
- Health score
- Recent failure count
- Compatibility policy tags

Key components:

- `ProviderList`
- `ProviderIdentityPanel`
- `ProviderEndpointPanel`
- `ProviderCapabilityMatrix`
- `CompatibilityPolicyExplainer`
- `LinkedRoutesPanel`
- `ProviderHealthTimeline`

Data flow:

```text
ProviderConsole
→ get_provider_console()
→ providers + presets + policies + linked routes + provider-scoped logs + health summaries
```

Backend scope:

- Add `get_provider_console()`.
- Return UI-friendly DTOs instead of forcing the frontend to join multiple datasets.
- Reuse providers, presets, policies, routes, codex routes, and request logs.

Provider preset boundary:

- Prioritize quality over quantity.
- First-class presets: OpenRouter, DeepSeek, Volcengine Ark, Moonshot, Qwen, Xiaomi MiMo, Anthropic-compatible, OpenAI-compatible Chat.

### 6.4 Route Builder

Purpose:

- Make route creation a first-class setup module.
- Translate "I want Codex to use DeepSeek" into safe model route creation.

Route Builder is a standalone first-level module under Setup.

Layout:

- Left: target app selector.
- Center: route draft editor.
- Right: policy preview, payload preview, and risk warnings.

Steps:

```text
Select target app
→ Select provider
→ Enter upstream model
→ Set visible alias/model name
→ Confirm compatibility policy
→ Resolve conflicts
→ Save and quick check
```

Route types:

- Claude Desktop alias route
- Claude Code Gateway Route
- Codex Responses Route

Key components:

- `RouteTargetSelector`
- `ProviderModelSelector`
- `AliasNameEditor`
- `RoutePolicyPreview`
- `RouteConflictResolver`
- `RoutePayloadPreview`

Conflict behavior:

- If route id, Claude alias, or Codex model already exists, the UI must offer:
  - Update existing
  - Create copy
  - Skip

Data flow:

```text
RouteBuilder
→ preview_route_builder(payload)
→ user confirms
→ apply_route_builder(payload)
→ refresh App Workbench + Runtime Dashboard
```

Backend scope:

- Add `preview_route_builder()`.
- Add `apply_route_builder()`.
- Reuse `create_route`, `update_route`, `create_codex_route`, and `update_codex_route`.

### 6.5 Health Center

Purpose:

- Replace scattered checks with one operational center.
- Explain failures in action language instead of raw HTTP errors only.

Layout:

- Header: overall health score, last run time, Quick Check button, Deep Check button.
- Sections: Runtime, Apps, Providers, Routes, Policies, Recent Failures.
- Bottom: failure clusters and diagnostics export.

Check result shape:

- Status: `pass`, `warn`, `fail`, `skipped`
- Object: provider, route, app, gateway, binding, policy
- Reason
- Suggested action
- Latency
- HTTP status
- Auto-fix availability

Key components:

- `HealthScoreHeader`
- `HealthRunControls`
- `HealthCheckSection`
- `HealthCheckRow`
- `FailureClusterTable`
- `SuggestedFixDrawer`
- `ExportDiagnosticsButton`

Data flow:

```text
HealthCenter
→ run_runtime_health_check({ mode: "quick" })
→ runtime health run
→ suggested actions can navigate to App Workbench, Provider Console, Route Builder, Settings, or Install Doctor
```

Backend scope:

- Extend the v1.14 health check design into `run_runtime_health_check()`.
- Reuse `check_gateway_health`, `check_codex_health`, `check_provider_health`, route diagnostics, policies, bindings, runtime source, and failure clusters.
- Keep Quick Check token-free by default.
- Deep Check must remain explicit and opt-in.

### 6.6 Usage & Reliability Insights

Purpose:

- Upgrade request logs into a lightweight local operations dashboard.
- Show usage and reliability without requiring cloud sync or full billing.

First version metrics:

- Request count
- Success rate
- Failure count
- Average latency
- P95 latency
- Provider distribution
- App/surface distribution
- HTTP status distribution
- Loop Guard suppression count
- Tool Repair count where available

Cost accounting:

- Not included in the first version.
- Usage means local request and stability usage, not billing cost.

Layout:

- Top filters: date range, app, provider, route, status, stream mode.
- Metric grid: request count, success rate, average latency, failure count.
- Reliability panels: provider reliability ranking, status breakdown, route failures.
- Request explorer: searchable table with diagnostic drawer.

Key components:

- `UsageMetricGrid`
- `ProviderReliabilityRank`
- `StatusCodeBreakdown`
- `RouteFailureTable`
- `RequestLogExplorer`
- `DiagnosticSnapshotDrawer`

Data flow:

```text
UsageInsights(filters)
→ get_usage_insights(filters)
→ aggregated request logs + filtered log rows + failure summaries
```

Backend scope:

- Add `get_usage_insights(filter)`.
- Use SQLite aggregation over `request_logs`.
- Add optional fields later for Loop Guard and Tool Repair metrics if not already persisted.

## 7. Frontend Architecture

### 7.1 UI Stack

Use a `cc-switch`-inspired stack while keeping app size under control:

- React
- TypeScript
- Tailwind CSS
- shadcn/ui-style components
- Radix UI primitives only when needed
- TanStack Query for Tauri command data loading and cache invalidation

Bundle-size guardrails:

- Import only the shadcn/Radix components used by Gateway Switch.
- Avoid large charting libraries in the first implementation unless required.
- Prefer CSS/SVG-based small charts for Usage Insights v1.
- Avoid Redux or heavy global state frameworks.
- Keep Rust/Tauri as the backend and native shell foundation.

### 7.2 Directory Structure

```text
src/
├─ app/
│  ├─ AppShell.tsx
│  ├─ navigation.ts
│  └─ routes.ts
├─ features/
│  ├─ dashboard/
│  ├─ app-workbench/
│  ├─ provider-console/
│  ├─ route-builder/
│  ├─ health-center/
│  ├─ usage-insights/
│  ├─ mcp-sync/
│  ├─ codex-tweaks/
│  └─ settings/
├─ shared/
│  ├─ api/
│  ├─ components/
│  ├─ hooks/
│  ├─ types/
│  └─ i18n/
└─ styles/
```

### 7.3 Refactor Rules

- `App.tsx` should become a shell and route coordinator, not a 5000-line feature container.
- Tauri `invoke` calls should be wrapped in `shared/api`.
- Feature pages should use typed hooks, for example `useRuntimeDashboard()` and `useAppWorkbench(appId)`.
- Page components should own layout; hooks should own loading and mutations.
- Existing i18n strings should be preserved and expanded gradually.

## 8. Backend Architecture

### 8.1 New Commands

Aggregation commands:

- `get_runtime_dashboard`
- `get_app_workbench`
- `get_provider_console`
- `get_usage_insights`

Setup commands:

- `preview_provider_wizard`
- `apply_provider_wizard`
- `preview_route_builder`
- `apply_route_builder`

Health commands:

- `run_runtime_health_check`
- `export_runtime_health_report`

### 8.2 New Rust Modules

Suggested modules:

- `runtime_dashboard.rs`
- `app_workbench.rs`
- `provider_wizard.rs`
- `route_builder.rs`
- `health_check.rs`
- `usage_insights.rs`

### 8.3 Stability Guardrails

- Do not rewrite `gateway.rs`, `codex_gateway.rs`, `loop_guard.rs`, or `compatibility.rs` as part of the UI refactor unless a specific API gap requires it.
- Prefer read-only DTO aggregation over database schema churn.
- Preserve all existing compatibility policies and route behavior.
- Preserve the 64 MiB gateway body limit.
- Preserve Claude Desktop binding fields: `displayName`, `display_name`, `labelOverride`, and `supports1m: true`.
- Preserve explicit confirmation for risky Claude Code Direct Provider binding.
- Preserve Unified Loop Guard usage across all gateway routes.

## 9. Implementation Phases

### Phase 1: Product Shell

- Introduce the new navigation and `AppShell`.
- Split `App.tsx` into feature modules.
- Build Runtime Dashboard skeleton.
- Build App Workbench skeleton for the three supported apps.
- Build Provider Console skeleton.

### Phase 2: Setup Loop

- Implement Provider Wizard as the primary add-provider path.
- Implement standalone Route Builder.
- Connect wizard output to route drafts.
- Add conflict handling.
- Add quick check after route creation.

### Phase 3: Operations Loop

- Implement Health Center.
- Add runtime health checks and suggested actions.
- Implement Usage Insights using local request logs.
- Upgrade request log explorer with filters and diagnostic drawers.

### Phase 4: Polish and Release

- Add empty states, tooltips, risk explanations, and safer copy.
- Add responsive layout checks.
- Add focused tests for route builder, provider wizard, health checks, and usage aggregation.
- Update README, README_EN, CHANGELOG, docs/project.md, package version, Cargo version, and Tauri config when releasing.

## 10. Acceptance Criteria

- A new user can configure a provider, generate routes, run quick health check, and bind an app from a guided path.
- Each supported app has a dedicated workbench with status, route, binding, health, and recent activity.
- Provider setup no longer relies on direct preset application as the primary flow.
- Route Builder supports Claude Desktop, Claude Code, and Codex route creation.
- Health Center can run a token-free Quick Check and return actionable results.
- Usage Insights shows local usage and reliability statistics without cost accounting.
- Codex++ Tweaks are accessible inside the Codex workbench, not as a top-level app area.
- The frontend is modular and no longer keeps all feature logic in `App.tsx`.
- UI uses a modern shadcn-style component system while avoiding unnecessary heavy dependencies.

## 11. Testing Strategy

Frontend:

- TypeScript build must pass.
- Feature pages should be smoke-tested through mock data where Tauri is unavailable.
- Route Builder conflict behavior should have focused tests if a test setup exists.

Backend:

- Rust tests for provider wizard draft generation.
- Rust tests for route builder conflict resolution.
- Rust tests for health check aggregation.
- Rust tests for usage aggregation over request logs.

Manual verification:

- `pnpm build`
- `cd src-tauri && cargo test`
- `pnpm tauri dev` for real binding, gateway, and config operations.

## 12. Implementation Notes

- Choose the exact shadcn/Radix component list during implementation to avoid unused dependencies.
- Usage Insights v1 should start with metric cards, tables, and lightweight CSS/SVG charts before adding any heavier charting library.
- Review Deep Check UX copy before enabling token-consuming checks.
