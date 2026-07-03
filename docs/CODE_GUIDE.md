# Code Guide

**Project:** Gateway Switch
**Version:** v1.19.0
**Last updated:** 2026-07-03

## Source Layout

### Rust Backend (`src-tauri/src/`)

| File | Lines | Owner | Purpose |
|---|---|---|---|
| `main.rs` | ~30 | - | Tauri entry point |
| `lib.rs` | ~50 | - | Module declarations, re-exports |
| `gateway.rs` | ~1200 | Core | Claude Gateway (Anthropic Messages) |
| `codex_gateway.rs` | ~2000 | Core | Codex Gateway (Responses API) |
| `codex_tools.rs` | ~420 | Core | CodexToolContext, bidirectional tool restore |
| `codex_history.rs` | ~280 | Core | Cross-request function_call LRU cache |
| `gateway_strategy.rs` | ~600 | Core | Provider compatibility profiles, reasoning config |
| `gateway_protocol.rs` | ~500 | Core | Protocol conversion helpers |
| `gateway_diagnostics.rs` | ~400 | Core | Failure snapshots, diagnostics |
| `compatibility.rs` | ~800 | Core | Tool repair, JSON fix, safety gates |
| `commands.rs` | ~800 | Frontend | Tauri command handlers |
| `database.rs` | ~500 | Core | SQLite persistence |
| `models.rs` | ~200 | Core | Data models |

### Frontend (`src/`)

| Path | Purpose |
|---|---|
| `App.tsx` | Main app shell + sidebar navigation |
| `shared/` | Shared types, API client, i18n, utilities |
| `pages/` | Feature pages (Dashboard, Claude, Claude Code, Codex, etc.) |

## Conventions

### Rust

- **Module structure**: New gateway features go in their own module under `src-tauri/src/`
- **Testing**: Unit tests live in `#[cfg(test)] mod tests` at the bottom of each module
- **Error handling**: Use `anyhow::Result` for top-level functions, `thiserror` for domain errors
- **Async**: Tokio runtime (provided by Tauri)
- **Unsafe**: None (zero unsafe blocks)

### TypeScript / React

- **State**: React hooks + Tauri IPC polling
- **Styling**: CSS modules / scoped styles
- **i18n**: `t()` function with Chinese as default
- **Components**: Prefer functional components with hooks

### Git

- **Branches**: `main` is the default branch
- **Commits**: Conventional commit style (feat:, fix:, docs:, chore:)
- **Tags**: Annotated tags for releases (`vX.Y.Z`)

## Release Documentation Rules

### Required docs for each release

1. Update `CHANGELOG.md` with the new version entry
2. Update `docs/RELEASE_NOTES.md` with release highlights and install guide
3. Update `docs/AI_CONTEXT.md` with current status and verification snapshot
4. Update `docs/DOCUMENTATION_INDEX.md` if new docs are added

### Changelog format

```markdown
## X.Y.Z - YYYY-MM-DD

- **Feature name**: Short description.
  - Detail 1
  - Detail 2
- Added N unit tests covering ...
- Verification: `cargo test --lib` (N passed, N ignored).
```

### Release notes content checklist

- [ ] Version and release date
- [ ] Highlights / what's new
- [ ] Installation instructions
- [ ] System requirements
- [ ] Verification results
- [ ] Known limitations
- [ ] Upgrade instructions
- [ ] Rollback instructions

## Adding a New Feature

1. Implement in the appropriate Rust module
2. Add unit tests (target ≥80% coverage for new code)
3. Add Tauri command if frontend-facing
4. Add frontend UI if needed
5. Update CHANGELOG.md
6. Update relevant docs
7. Run `cargo test --lib` and confirm all pass
8. Build with `pnpm tauri build` and verify DMG

## Adding a New Provider Profile

1. Add detection logic in `gateway_strategy.rs`
2. Add strategy flags (`strip_reasoning`, `strip_unsupported_params`, etc.)
3. Add reasoning translation if needed
4. Add unit tests
5. Update provider presets in `commands.rs` if applicable
