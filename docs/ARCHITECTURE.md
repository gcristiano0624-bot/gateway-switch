# Architecture

**App:** Gateway Switch
**Version:** v1.19.0
**Stack:** Tauri 2 (Rust backend + React/TypeScript frontend)

## Runtime Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        macOS Desktop                            │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌───────────────┐     │
│  │Claude Desktop│    │ Claude Code  │    │   Codex App   │     │
│  └──────┬───────┘    └──────┬───────┘    └───────┬───────┘     │
│         │ Anthropic         │ Anthropic          │ Responses   │
│         │ Messages API      │ Messages API       │ API         │
│         ▼                   ▼                    ▼             │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                                                          │   │
│  │          Gateway Switch.app (Tauri WebView)             │   │
│  │                                                          │   │
│  │  Frontend (React)         │     Backend (Rust)          │   │
│  │  ─────────────────        │     ─────────────           │   │
│  │  Dashboard                │     Axum HTTP server        │   │
│  │  App Workbench            │     ─────────────────       │   │
│  │  Provider Console         │                             │   │
│  │  Route Builder            │     Claude Gateway          │   │
│  │  Diagnostics Center       │     (Anthropic → upstream)  │   │
│  │  Settings                 │     Codex Gateway           │   │
│  │                           │     (Responses → Chat)      │   │
│  │                           │     LoopGuard               │   │
│  │                           │     Tool Repair Engine      │   │
│  │                           │     Compatibility Layer     │   │
│  │                           │     SQLite Persistence      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                          │                                       │
│                          │ HTTPS (outbound only)                 │
│                          ▼                                       │
│                 Third-party Model Providers                      │
│                 (OpenAI Chat, Anthropic, etc.)                   │
└─────────────────────────────────────────────────────────────────┘
```

## Gateway Architecture

### Three Gateway Surfaces

1. **Claude Gateway** (`gateway.rs`) — Anthropic Messages API
   - Serves Claude Desktop and Claude Code
   - Supports both native Anthropic upstream and OpenAI Chat fallback
   - Auto protocol fallback: try Anthropic first, fall back to Chat Completions

2. **Codex Gateway** (`codex_gateway.rs`) — OpenAI Responses API
   - Serves Codex App
   - Converts Responses → Chat Completions request
   - Converts Chat Completions response → Responses (sync + streaming SSE)
   - Tool repair, loop guard, reasoning panel

3. **Shared Infrastructure**
   - `gateway_strategy.rs` — Provider compatibility profiles
   - `gateway_protocol.rs` — Protocol conversion helpers
   - `gateway_diagnostics.rs` — Failure snapshots and diagnostics
   - `compatibility.rs` — Tool call repair, JSON fix, safety gates

### Key Data Flow: Codex Gateway

```
Codex App
    │
    ▼ POST /v1/responses
┌─────────────────────────────────────────────────────┐
│ Codex Gateway                                       │
│                                                      │
│ 1. Parse Responses API request                       │
│ 2. Look up route from codex_routes table             │
│ 3. Apply provider policy (strip reasoning, etc.)     │
│ 4. Enrich with history (codex_history.rs)            │
│ 5. Build CodexToolContext (codex_tools.rs)           │
│ 6. Downgrade tools → function                        │
│ 7. Apply reasoning translation                       │
│ 8. Post-policy cleanup (tool_choice, include_usage)  │
│ 9. Convert → Chat Completions                        │
│                                                      │
└──────────────┬───────────────────────────────────────┘
               ▼
        Upstream Provider
        (Chat Completions SSE)
               │
               ▼
┌─────────────────────────────────────────────────────┐
│ Codex Gateway (streaming)                           │
│                                                      │
│ 1. Parse SSE stream from upstream                    │
│ 2. Extract text delta + reasoning delta              │
│ 3. Accumulate and emit function_call arguments       │
│ 4. LoopGuard: suppress duplicate text chunks         │
│ 5. Restore tool types via CodexToolContext           │
│ 6. Record response to history cache                  │
│ 7. Convert → Responses SSE events                    │
│                                                      │
└──────────────┬───────────────────────────────────────┘
               ▼
           Codex App
```

## Packaging Architecture

### Build Pipeline

```
Source Code
    │
    ▼ pnpm build
Frontend (dist/)
    │
    ▼ pnpm tauri build
Rust compile + bundle
    │
    ▼
┌─────────────────────┐
│ Gateway Switch.app  │
│  ├─ Contents/       │
│  │  ├─ MacOS/       │  ← Rust binary
│  │  ├─ Resources/   │  ← WebView assets
│  │  └─ Info.plist   │
│  └─ …               │
└─────────┬───────────┘
          │ hdiutil / tauri-bundle
          ▼
Gateway Switch_1.19.0_aarch64.dmg
```

### Artifacts

| Artifact | Path | Size |
|---|---|---|
| App bundle | `src-tauri/target/release/bundle/macos/Gateway Switch.app` | ~19 MB |
| DMG | `src-tauri/target/release/bundle/dmg/Gateway Switch_1.19.0_aarch64.dmg` | ~7.6 MB |

## Release Upload Model

1. **Source code** is committed to `main` branch on GitHub
2. **Release tag** `v1.19.0` is an annotated tag on the release commit
3. **DMG** is uploaded as a GitHub Release asset (not committed to git)
4. **Release notes** are stored in `docs/RELEASE_NOTES.md` and used as the GitHub Release body
5. **App binary** is unsigned / not notarized (local ad-hoc signature only)

## Data Persistence

All runtime data is stored in `~/Library/Application Support/Gateway Switch/`:

| File | Purpose |
|---|---|
| `gateway.db` | SQLite database (providers, routes, request logs, diagnostics) |
| `settings.json` | App settings (language, theme, etc.) |

The database is **local only** — never uploaded or synced.

## Security Model

- API keys are stored in the local SQLite database
- Error summaries are scanned for secrets before persisting
- All outbound traffic goes directly to configured providers
- No telemetry, no data collection, no remote calls except configured upstreams
- The gateway runs on `127.0.0.1` only (not accessible from the network)
