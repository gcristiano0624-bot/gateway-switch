# Documentation Index

**Current release:** v1.19.0 (2026-07-03)
**App name:** Gateway Switch
**Tech stack:** Tauri 2 (Rust + React/TypeScript)

## Release Documents

| Document | Purpose |
|---|---|
| [RELEASE_NOTES.md](./RELEASE_NOTES.md) | GitHub Release notes, install guide, known limitations |
| [AI_CONTEXT.md](./AI_CONTEXT.md) | AI agent handoff context, current status, verification snapshot |
| [NEXT_STEPS.md](./NEXT_STEPS.md) | Remaining roadmap items after v1.19.0 |

## Architecture & Operations

| Document | Purpose |
|---|---|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Runtime architecture, packaging, release upload model |
| [RUNBOOK.md](./RUNBOOK.md) | Build, test, package, DMG validation, GitHub release commands |
| [CODE_GUIDE.md](./CODE_GUIDE.md) | Source layout, code ownership, release documentation rules |
| [FEATURE_MAP.md](./FEATURE_MAP.md) | Feature coverage matrix and known gaps |

## Project Documents (Historical)

| Document | Purpose |
|---|---|
| [project.md](./project.md) | Deep architecture and design context |
| [prd-codex-routing-improvements.md](./prd-codex-routing-improvements.md) | PRD for v1.16.0–v1.19.0 Codex routing improvements |
| [tech-codex-routing-improvements.md](./tech-codex-routing-improvements.md) | Technical design for Codex routing improvements |
| [optimization-report.md](./optimization-report.md) | cc-switch analysis optimization report |

## Release History

- **v1.19.0** — Codex Routing Improvements (all 5 improvements + Volcengine fixes)
- **v1.15.0** — Reasoning content strip from Codex text stream
- **v1.14.1** — Routing robustness patch (stream truncation, rate limiting, MiniMax)
- **v1.14.0** — Gateway runtime refactor
- **v1.13.x** — Unified Loop Guard, Runtime Console UX
- **v1.12.x** — Unified Diagnostics Center, Provider Presets
- **v1.10.0** — Failed-request diagnostics, Provider Compatibility Policies
- **v1.9.0** — Provider Compatibility Profiles, Route Diagnostics
- **v1.8.x** — Native Codex++ install, MCP Sync, Volcengine DeepSeek support
- **v1.7.x** — Bilingual UI, Cold Start Doctor
- **v1.6.x** — Runtime compatibility layer, Codex conversation stall fix

## Source Layout

```
gateway-switch/
├── src/                          # React frontend (TypeScript)
│   ├── App.tsx                   # Main app shell + sidebar
│   ├── shared/                   # Shared types, API, i18n
│   └── pages/                    # Feature pages
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── main.rs               # Tauri entry point
│   │   ├── lib.rs                # Library root, module declarations
│   │   ├── gateway.rs            # Claude Gateway (Anthropic Messages)
│   │   ├── codex_gateway.rs      # Codex Gateway (Responses API)
│   │   ├── codex_tools.rs        # CodexToolContext + bidirectional tool restore
│   │   ├── codex_history.rs      # Cross-request function_call LRU cache
│   │   ├── gateway_strategy.rs   # Provider compatibility profiles, reasoning config
│   │   ├── gateway_protocol.rs   # Protocol conversion helpers
│   │   ├── gateway_diagnostics.rs # Failure diagnostics and snapshots
│   │   ├── compatibility.rs      # Tool call repair, JSON fix, safety gates
│   │   ├── commands.rs           # Tauri command handlers
│   │   ├── database.rs           # SQLite persistence
│   │   └── models.rs             # Data models
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json                  # Frontend package + Tauri scripts
├── CHANGELOG.md                  # User-visible change history
├── README.md                     # Chinese README
├── README_EN.md                  # English README
└── docs/                         # Documentation (this index)
```

## Build & Test Commands

```bash
# Install dependencies
pnpm install

# Frontend build
pnpm build

# Run tests
cd src-tauri && cargo test --lib

# Tauri dev app
pnpm tauri dev

# Tauri bundle (app + dmg)
pnpm tauri build
```

## External Links

- **GitHub repository:** https://github.com/gcristiano0624-bot/gateway-switch
- **Latest release:** https://github.com/gcristiano0624-bot/gateway-switch/releases/latest
- **Issues:** https://github.com/gcristiano0624-bot/gateway-switch/issues
