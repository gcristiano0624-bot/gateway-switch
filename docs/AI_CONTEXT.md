# AI Context — Gateway Switch v1.19.0

**Last updated:** 2026-07-03
**Current version:** 1.19.0
**Status:** Released

## Project Overview

Gateway Switch is a macOS desktop app (Tauri 2 / Rust + React) that acts as a **runtime compatibility gateway** between AI-native desktop apps (Claude Desktop, Claude Code, Codex App) and third-party model providers.

- **Claude Desktop** → Anthropic Messages API → Gateway → upstream Provider (Anthropic or OpenAI Chat fallback)
- **Claude Code** → Anthropic Messages API → Gateway → upstream Provider
- **Codex App** → OpenAI Responses API → Gateway → OpenAI Chat Completions upstream

## Current Release Status

v1.19.0 is the current release, implementing all 5 Codex routing improvements from the cc-switch analysis:

1. ✅ **CodexToolContext + bidirectional tool restore** (Improvement 1) — `codex_tools.rs`
2. ✅ **Cross-request function_call history LRU** (Improvement 2) — `codex_history.rs`
3. ✅ **Reasoning panel restoration** (Improvement 3) — in `codex_gateway.rs`
4. ✅ **Platform-aware reasoning translation** (Improvement 4) — `gateway_strategy.rs`
5. ✅ **vLLM/enterprise gateway compatibility + include_usage** (Improvement 5) — in `codex_gateway.rs`

Plus Volcengine/火山引擎 compatibility fixes:
- Volcengine detection in reasoning config → no reasoning params injected
- Conditional `include_usage` (gated on `strip_unsupported_params`)
- Strict tool name sanitization for MCP namespace tools (OpenAI-compatible charset only)
- `force_when_tools_present` uses hint instead of `tool_choice = "required"`

## Verification Snapshot

- **Rust tests:** 116 passed, 3 ignored (`cargo test --lib`)
- **Frontend build:** `pnpm build`
- **Bundle:** `pnpm tauri build`
- **App size:** ~19 MB
- **DMG size:** ~7.6 MB
- **Platform:** macOS aarch64 (Apple Silicon)

## Key Files

### Rust Backend (`src-tauri/src/`)

| File | Purpose | Lines (approx) |
|---|---|---|
| `codex_gateway.rs` | Codex Responses → Chat Completions conversion, streaming | ~2000 |
| `codex_tools.rs` | CodexToolContext, bidirectional tool downgrade/restore | ~420 |
| `codex_history.rs` | Cross-request function_call LRU cache | ~280 |
| `gateway_strategy.rs` | Provider compatibility profiles, reasoning config | ~600 |
| `gateway_protocol.rs` | Protocol conversion helpers | ~500 |
| `gateway_diagnostics.rs` | Failure snapshots, diagnostics | ~400 |
| `compatibility.rs` | Tool repair, JSON fix, safety gates | ~800 |
| `gateway.rs` | Claude Gateway (Anthropic Messages) | ~1200 |
| `commands.rs` | Tauri command handlers | ~800 |
| `database.rs` | SQLite persistence | ~500 |
| `models.rs` | Data models | ~200 |

### Frontend (`src/`)

- `App.tsx` — Main app shell with sidebar navigation
- `shared/` — Types, API client, i18n, utilities
- `pages/` — Feature pages (Dashboard, Claude, Claude Code, Codex, Providers, Logs, Settings, etc.)

## How to Run

```bash
# Install deps
pnpm install

# Dev (browser preview with mock data)
pnpm dev

# Dev (Tauri app with real functionality)
pnpm tauri dev

# Build and package
pnpm tauri build

# Run tests
cd src-tauri && cargo test --lib
```

## Recent Work Log

### v1.19.0 (2026-07-01 → 2026-07-03)

- Implemented CodexToolContext + bidirectional tool restore (Improvement 1)
- Implemented cross-request function_call LRU cache (Improvement 2)
- Implemented platform-aware reasoning translation (Improvement 4)
- Reasoning panel already done in v1.16.0 (Improvement 3)
- vLLM compatibility + include_usage already done in v1.16.0 (Improvement 5)
- Fixed Volcengine/火山引擎 compatibility:
  - Reasoning parameter mis-injection
  - `include_usage` unconditional injection
  - MCP namespace tool names with colons/dots (400 on Kimi)
  - `force_when_tools_present` causing endless tool-call loops

## Next Priority

See [NEXT_STEPS.md](./NEXT_STEPS.md) for the full roadmap.

Top items:
- Persistent history store (SQLite-backed instead of in-memory LRU)
- Developer ID signing and notarization
- Windows support
- Codex Responses API v2 compatibility
