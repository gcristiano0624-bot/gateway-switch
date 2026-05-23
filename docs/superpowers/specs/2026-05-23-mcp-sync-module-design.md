# MCP Sync Module Design

## Background

Gateway Switch already manages Claude Desktop, Claude Code, and Codex model routing, binding, and health checks. The project also includes an independent MCP sync utility under `同步功能/`, documented by `sync-mcp-README.md` and `sync-mcp-TECHNICAL.md`.

The utility synchronizes MCP server configuration across three clients:

- Claude Desktop: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Claude Code: `~/.claude/settings.json`
- Codex: `~/.codex/config.toml`

The new module brings this workflow into Gateway Switch as a first-class UI module, so users can inspect, preview, and execute MCP sync without running a Python script manually.

## Goals

- Provide a dedicated MCP sync page in Gateway Switch.
- Read MCP server configuration from Claude Desktop, Claude Code, and Codex.
- Show per-client status cards with path, format, server count, parse status, and risk state.
- Preview the merged MCP server set before writing files.
- Execute one-click sync with automatic backups and clear result logs.
- Preserve each client config's non-MCP fields.
- Avoid exposing secret values in the UI.

## Non-Goals

- Do not implement manual MCP server editing in the first version.
- Do not implement per-conflict manual resolution in the first version.
- Do not store sync history in SQLite in the first version.
- Do not require Python or the external `toml` Python package at runtime.
- Do not change model routing, Gateway runtime, or existing Claude/Codex binding behavior.

## Recommended Approach

Implement MCP sync as a native Rust module plus a dedicated React page.

The Rust backend should port the current Python script's behavior into a new `mcp_sync.rs` module. This avoids a Python dependency, keeps the packaged app self-contained, and matches existing native binding modules such as `claude_code_binding.rs` and `codex_binding.rs`.

The frontend should add a new `mcpSync` page in `src/App.tsx`, following the existing single-file page pattern. The module should be placed in the sidebar under the `Shared` group because it synchronizes shared tool configuration across products.

## Alternatives Considered

### Option A: Dedicated MCP Sync Page

Recommended.

Pros:

- Makes sync visible and understandable.
- Gives enough space for status cards, preview, risk warnings, and logs.
- Fits future expansion for additional sync targets.

Cons:

- Adds another sidebar item.
- Requires a focused page component and additional i18n strings.

### Option B: Add to Settings Page

Pros:

- Minimal navigation changes.
- Simple for a small utility.

Cons:

- Hides a high-impact file-writing workflow in a generic settings page.
- Harder to show preview, conflict, and log details.

### Option C: Add to Cold Start Page

Pros:

- MCP readiness is related to tool capability checks.
- Could reuse diagnostic language.

Cons:

- MCP sync is an active configuration workflow, not only a health check.
- Would make Cold Start too broad.

## Information Architecture

Add a new page:

- Internal page key: `mcpSync`
- Sidebar label: `MCP Sync` / `MCP 同步`
- Sidebar group: `Shared`
- Sidebar position: after `Codex` and before `Cold Start`, keeping product-specific pages grouped before diagnostic pages.

Page sections:

1. Header and actions
2. Sync overview cards
3. Per-client configuration cards
4. Sync preview
5. Conflict and safety panel
6. Execution result log

## Page Layout

### 1. Header and Actions

Title:

- `MCP 配置同步`

Subtitle:

- `在 Claude Desktop、Claude Code 与 Codex 之间同步 MCP Servers 配置。`

Actions:

- `刷新状态`: Reads all three config files and updates status. No writes.
- `预览同步`: Reads, extracts, merges, and returns a write preview. No writes.
- `执行同步`: Creates backups and writes the merged MCP server set to all writable targets.
- `打开配置目录`: Opens the selected target's config directory. Each target card also provides a path copy action.

Button states:

- `执行同步` is disabled while a sync is running.
- `执行同步` is disabled when all readable sources are empty.
- `执行同步` is blocked when any existing target config has a parse error. The first version does not support skipping broken sources.

### 2. Sync Overview Cards

Use four compact KPI cards:

#### Card: 三端状态

Fields:

- `ready_targets`: number of clients with parseable config or creatable config path.
- `blocked_targets`: number of clients with parse or permission errors.
- Status label: `全部可同步`, `部分可同步`, or `需要处理`.

#### Card: 合并后 Servers

Fields:

- `merged_count`: unique MCP server count after merging all readable sources.
- `source_count`: number of clients that contributed at least one server.
- Subtext: `按名称取并集`.

#### Card: 冲突数量

Fields:

- `conflict_count`: number of same-name servers found in multiple clients with different definitions.
- `resolved_count`: number of conflicts auto-resolved by completeness scoring.
- Subtext: `自动按完整度合并`.

#### Card: 最近同步

Fields:

- `last_run_at`: last sync time for the current app session.
- `last_status`: `成功`, `失败`, or `尚未执行`.
- `last_written_count`: number of targets successfully written in the latest run.

First version can keep `last_run_at` in React state only. Persistent history is deferred.

### 3. Per-Client Configuration Cards

Render three product cards with parallel structure.

#### Claude Desktop Card

Title:

- `Claude Desktop`

Path:

- `~/Library/Application Support/Claude/claude_desktop_config.json`

Fields:

- `config_exists`: whether the file exists.
- `format`: `JSON`
- `parse_status`: `正常`, `文件不存在`, `解析失败`, or `权限不足`.
- `server_count`: number of entries under `mcpServers`.
- `writable`: whether the target directory or file can be written.
- `backup_path`: latest backup path after sync, empty before first sync.

Preservation rule:

- Keep all top-level fields except replacing `mcpServers`.

#### Claude Code Card

Title:

- `Claude Code`

Path:

- `~/.claude/settings.json`

Fields:

- `config_exists`
- `format`: `JSON`
- `parse_status`
- `server_count`: number of entries under `mcpServers`.
- `has_non_mcp_fields`: whether the file contains other settings that must be preserved.
- `writable`
- `backup_path`

Preservation rule:

- Keep all top-level fields except replacing `mcpServers`.

#### Codex Card

Title:

- `Codex`

Path:

- `~/.codex/config.toml`

Fields:

- `config_exists`
- `format`: `TOML`
- `parse_status`
- `server_count`: number of entries under `[mcp_servers]`.
- `has_mcp_section`: whether `mcp_servers` exists.
- `writable`
- `backup_path`

Preservation rule:

- Keep all top-level TOML sections except replacing `mcp_servers`.

## Status Labels

Use consistent status badges:

- `可同步`: file is parseable or can be created, and target is writable.
- `文件不存在`: source has no existing config; it can still be a write target if the directory is creatable.
- `解析失败`: config exists but cannot be parsed.
- `权限不足`: file or parent directory is not writable.
- `已跳过`: target was not read or written in the latest operation.
- `已写入`: target was written successfully.

## Sync Preview

The preview table should show what will happen before any write.

Columns:

- `名称`: MCP server name.
- `类型`: `STDIO` when `command` exists, `SSE / HTTP` when `url` exists, otherwise `未知`.
- `来源`: source badges: `Desktop`, `Code`, `Codex`.
- `完整度`: compact indicators for `command/url`, `args`, `env`, and `headers`.
- `凭证`: show only key names, such as `GITHUB_PERSONAL_ACCESS_TOKEN`, not values.
- `同步动作`: `新增`, `更新`, `保持`, or `冲突合并`.

Preview summary:

- Total unique servers.
- Servers to add per target.
- Servers to update per target.
- Conflicts resolved by automatic policy.
- Targets that will be skipped.

## Merge Policy

Port the policy from `sync-mcp.py`:

1. Merge by server name.
2. Keep the union of all server names.
3. For same-name conflicts, compute completeness score:
   - `command` or `url`: +1
   - non-empty `args`: +1
   - non-empty `env`: +1
   - non-empty `headers`: +1
4. Higher score wins.
5. Equal score merges `env` and `headers`.
6. Other same-score fields use the later source according to fixed source order.

Recommended fixed source order:

1. Claude Desktop
2. Claude Code
3. Codex

This order must be documented in the UI so conflict behavior is predictable.

## Safety and Secrets

Safety requirements:

- Always create backups before writing an existing config file.
- Never show secret values from `env` or `headers`.
- Show only secret key names and whether values exist.
- Preserve non-MCP fields.
- Return explicit per-target write status.
- Fail safely when parsing fails.

Backup locations:

- Claude Desktop: same config directory under `gateway-switch-backups/`.
- Claude Code: `~/.claude/gateway-switch-backups/`.
- Codex: `~/.codex/gateway-switch-backups/`.

Backup filename examples:

- `claude_desktop_config-<timestamp>.json`
- `settings-<timestamp>.json`
- `config-<timestamp>.toml`

## Backend Design

Add a native module:

- `src-tauri/src/mcp_sync.rs`

Core types:

```rust
pub struct McpServerEntry {
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
}

pub struct McpTargetStatus {
    pub target: String,
    pub label: String,
    pub config_path: String,
    pub config_exists: bool,
    pub format: String,
    pub parse_status: String,
    pub server_count: usize,
    pub writable: bool,
    pub backup_path: Option<String>,
    pub error: Option<String>,
}

pub struct McpSyncPreview {
    pub generated_at: String,
    pub targets: Vec<McpTargetStatus>,
    pub merged_count: usize,
    pub conflict_count: usize,
    pub servers: Vec<McpServerPreview>,
    pub warnings: Vec<String>,
}

pub struct McpSyncResult {
    pub generated_at: String,
    pub preview: McpSyncPreview,
    pub written_targets: Vec<McpWriteResult>,
    pub logs: Vec<String>,
}
```

Tauri commands:

- `get_mcp_sync_status`: read target metadata and counts.
- `preview_mcp_sync`: read, parse, merge, and return preview without writing.
- `run_mcp_sync`: backup and write merged config to all valid targets.

Command placement:

- Register commands in `src-tauri/src/commands.rs`.
- Add `mod mcp_sync;` in `src-tauri/src/lib.rs`.
- Register command handlers in `tauri::generate_handler!`.

TOML handling:

- Add the Rust `toml` crate for parsing and serializing Codex config.
- Preserve non-MCP sections when writing Codex config.
- The first version preserves semantic non-MCP sections but may normalize TOML comments and formatting.

## Frontend Design

Add frontend types in `src/App.tsx`:

- `McpTargetStatus`
- `McpServerPreview`
- `McpSyncPreview`
- `McpWriteResult`
- `McpSyncResult`

State:

- `mcpStatus`
- `mcpPreview`
- `mcpResult`
- `mcpLoading`
- `mcpSyncing`

Actions:

- `loadMcpSyncStatus`
- `previewMcpSync`
- `runMcpSync`

UI components can stay inside `App.tsx` for consistency with the current codebase. If the file grows too large during implementation, extract only the new MCP page into a focused component as a targeted refactor.

## Error Handling

Expected cases:

- Config file missing: show `文件不存在`; allow target creation during sync.
- JSON parse error: block write by default and show exact file path plus parser message.
- TOML parse error: block write by default and show exact file path plus parser message.
- Permission error: mark target `权限不足`; do not write.
- Empty sources: disable `执行同步` and show `没有可同步的 MCP Servers`.
- Backup failure: abort writing that target and show failure in result log.
- Partial write failure: show per-target status and keep successful writes visible.

## Testing Strategy

Backend unit tests:

- Extract MCP servers from Claude Desktop JSON.
- Extract MCP servers from Claude Code JSON.
- Extract MCP servers from Codex TOML.
- Merge union from three sources.
- Prefer higher completeness score.
- Merge `env` and `headers` for equal scores.
- Preserve non-MCP JSON fields.
- Preserve non-MCP TOML sections.
- Create backups before writes.

Frontend verification:

- Page renders with empty status.
- Status cards render parseable, missing, and error states.
- Preview table masks secret values.
- Sync button disables during running state.
- Toast displays success and error messages.

Manual validation:

- Run preview against real user configs.
- Sync a config with one server in only one client and verify it appears in all three.
- Sync configs with same-name conflicts and verify documented merge policy.
- Verify non-MCP settings remain unchanged.

## First-Version Acceptance Criteria

- A new `MCP Sync / MCP 同步` page is available from the sidebar.
- The page shows three target cards for Claude Desktop, Claude Code, and Codex.
- The page shows the merged server count and conflict count.
- `刷新状态` reads all targets without writing.
- `预览同步` shows merged server rows without writing.
- `执行同步` writes all valid targets after backup.
- Secret values are masked in UI.
- Non-MCP fields are preserved.
- Parse and write errors are shown per target.
- Backend tests cover extraction, merge, and write preservation.

## Deferred Enhancements

- Persistent sync history in SQLite.
- Manual per-server conflict selection.
- MCP server editor with add, update, and delete actions.
- Import/export MCP server bundles.
- Scheduled or automatic sync after app startup.
- More targets, such as Cursor, Windsurf, or VS Code MCP config.
- Integration with Cold Start diagnostics as a read-only MCP readiness check.

## Final Design Decisions

- Sidebar placement is fixed after `Codex` and before `Cold Start`.
- Parse errors hard-block `执行同步` in the first version.
- Codex TOML writing preserves semantic non-MCP sections but may normalize comments and formatting.
