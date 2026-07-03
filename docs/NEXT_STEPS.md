# Next Steps

**After release: v1.19.0**
**Last updated:** 2026-07-03

This document tracks the remaining roadmap after the v1.19.0 Codex Routing Improvements release.

## High Priority

### 1. Persistent History Store

Replace the in-memory LRU cache (`codex_history.rs`) with SQLite-backed persistence.

- **Problem**: Restarting the app loses all function_call history, causing multi-turn Codex CLI sessions to fail until the client retries with full payload.
- **Solution**: Store response output in SQLite, keyed by response_id, with LRU eviction at the DB level.
- **Size estimate**: 512 responses × ~10KB each = ~5MB (safe)
- **Affected files**: `codex_history.rs`, `database.rs`, `models.rs`

### 2. Developer ID Signing & Notarization

Get the app signed and notarized for proper macOS Gatekeeper compliance.

- **Problem**: Users see "cannot be opened because the developer cannot be verified" warning.
- **Solution**: Join Apple Developer Program, set up code signing in Tauri config, implement notarization.
- **Complexity**: Medium (requires Apple Developer account, CI setup)

### 3. End-to-End Integration Tests

Add integration tests that spin up a mock upstream server and test the full gateway pipeline.

- **Problem**: Current tests are mostly unit tests; no end-to-end streaming tests.
- **Solution**: Add `tokio-test` with mock HTTP server, test full request/response cycle including SSE streaming.
- **Coverage**: Claude gateway, Codex gateway, tool call round-trip, streaming, error handling.

## Medium Priority

### 4. Codex Responses API v2 Compatibility

Monitor and adapt to changes in the Codex Responses API format.

- Track upstream Codex App updates
- Add version detection and format negotiation
- Maintain backward compatibility

### 5. Windows Support

Port the Tauri app to Windows.

- **Effort**: Medium-High
- Windows SDK setup
- Path handling differences
- Registry vs plist for Codex/Claude binding detection

### 6. Provider Health Dashboard

Add real-time provider health monitoring with:

- Latency P50/P95
- Error rates by provider
- Token throughput
- Automatic failover suggestions

### 7. Custom Model Aliases

Let users define custom model name mappings:

- Add a "Model Aliases" table in settings
- Map any model name to any upstream model
- Per-provider or global aliases

## Low Priority / Nice to Have

### 8. Request Replay UI

Let users replay a failed request with one click from the diagnostics page.

- Already have the sanitized payload in snapshots
- Add a "Replay" button in the diagnostics UI
- Show live results in a modal

### 9. Dark Mode

Add dark mode support to the frontend UI.

- System preference detection
- Manual toggle in settings
- CSS variable-based theming

### 10. Import/Export Configuration

Let users export and import their provider and route configuration.

- JSON export of providers + routes
- Import with merge / replace options
- API keys masked in export

### 11. CLI Mode

Add a CLI mode that runs the gateway as a headless daemon without the UI.

- Useful for server deployments
- `gateway-switch daemon` command
- Config via environment variables or config file

## Done (Completed in v1.19.0)

- ✅ CodexToolContext + bidirectional tool restore (Improvement 1)
- ✅ Cross-request function_call history LRU (Improvement 2)
- ✅ Reasoning panel restoration (Improvement 3)
- ✅ Platform-aware reasoning translation (Improvement 4)
- ✅ vLLM/enterprise gateway compatibility (Improvement 5)
- ✅ Volcengine/火山引擎 compatibility fixes
