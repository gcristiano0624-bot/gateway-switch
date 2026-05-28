# Codex++ Upstream Tweak Store Design

## Goal

Gateway Switch 1.8.6 will turn the Codex++ page into a more faithful control console for the current upstream Codex++ Tweak Store.

The user selected the "all store" scope. Gateway Switch should fetch and display every approved upstream store entry from:

```text
https://b-nnett.github.io/codex-plusplus/store/index.json
```

Each entry should expose its source repository, approved commit, archive URL, description, install status, and safe install action. The feature must make clear that the earlier four requested "script" names are not present as exact entries in the current upstream registry.

## In Scope

- Add a full `Upstream Tweak Store` panel to the Codex++ page.
- Fetch the upstream registry with timeout and clear error reporting.
- Show all currently approved entries from the registry.
- Compute the official archive URL with the same rule used by upstream Codex++:

```text
https://codeload.github.com/<repo>/tar.gz/<approvedCommitSha>
```

- Surface source URLs and metadata so the user can inspect upstream code before installing.
- Reuse the existing Gateway Switch / Codex++ tweak install path instead of inventing a separate script directory.
- Keep the current `Recommended Scripts` panel, but label the four old script names as legacy requested items when no exact upstream match exists.
- Include the existing coldstart module split in the 1.8.6 release, because the user explicitly approved merging those changes into this version.

## Non-Goals

- Do not auto-install all upstream store entries.
- Do not silently write arbitrary JavaScript into `Codex.app`, `app.asar`, or unknown user-script directories.
- Do not claim the four legacy script names are loaded when the upstream registry does not contain them.
- Do not fork or modify upstream tweak code.
- Do not replace Codex++'s own store approval model.
- Do not delete or revert the existing coldstart refactor.

## Current Evidence

- The upstream repo `b-nnett/codex-plusplus` currently implements a `Tweak Store`, not a separate public "script market" registry.
- The registry URL is defined in upstream `packages/runtime/src/tweak-store.ts` as `DEFAULT_TWEAK_STORE_INDEX_URL`.
- The upstream install flow downloads `storeArchiveUrl(entry)`, which expands to the `codeload.github.com/<repo>/tar.gz/<approvedCommitSha>` URL.
- The current registry contains 20 approved entries.
- Exact searches did not find:
  - `Codex Context Used Meter`
  - `Hide Usage Alert`
  - `Codex Token Usage`
  - `Codex List Pagebuster`
- The closest current upstream entry is `co.bennett.ui-improvements` (`Bennett's UI Improvements`), whose description includes hiding upgrade prompts and surfacing usage/message metrics.

## Architecture

### Backend

Extend `src-tauri/src/codex_pp.rs` with an upstream store model.

Responsibilities:

- Fetch the upstream registry from `https://b-nnett.github.io/codex-plusplus/store/index.json`.
- Parse only the fields Gateway Switch needs.
- Validate key safety properties:
  - `schemaVersion == 1`
  - each entry has `id`, `manifest.name`, `repo`, and a full 40-character `approvedCommitSha`
  - archive URLs are derived, not blindly trusted
- Report entries to the frontend with install status based on local `~/Library/Application Support/codex-plusplus/tweaks/<id>`.
- Reuse existing `codex_pp::install_from_store(repo, approved_commit_sha)` for installs when possible.
- Preserve existing recommended-script status behavior from 1.8.5.

### Frontend

Extend the Codex++ page in `src/App.tsx`.

Responsibilities:

- Keep `Recommended Scripts` at the top.
- Add an `Upstream Tweak Store` panel below it.
- Display each entry with:
  - name
  - id
  - description
  - repo
  - approved commit short SHA
  - derived archive URL
  - install status
- Provide actions:
  - `Refresh Store`
  - `Install`
  - `Open GitHub`
  - `Copy Archive URL`
- Show a clear note that the four legacy requested script names were not found as exact upstream registry entries.

### Coldstart Refactor

The working tree already contains a coldstart extraction:

- `src-tauri/src/coldstart.rs`
- `src-tauri/src/lib.rs` with `mod coldstart;`
- `src-tauri/src/commands.rs` calling `coldstart::{run_coldstart_checks, RunMode}`

This refactor is accepted into the 1.8.6 scope.

Implementation rules:

- Treat `commands.rs`, `lib.rs`, and `coldstart.rs` as one compile unit.
- Do not partially commit the coldstart refactor.
- Run Rust tests after integrating store commands, because both features touch command registration.

## Data Model

Backend response shape:

```text
CodexPpUpstreamStoreReport {
  source_url: string
  fetched_at: string
  generated_at?: string
  entries: CodexPpUpstreamStoreEntry[]
  legacy_recommendations: CodexPpLegacyRecommendation[]
  summary: string
}

CodexPpUpstreamStoreEntry {
  id: string
  name: string
  description?: string
  repo: string
  approved_commit_sha: string
  archive_url: string
  release_url?: string
  review_url?: string
  icon_url?: string
  installed: boolean
  installed_path?: string
}

CodexPpLegacyRecommendation {
  name: string
  exact_match: boolean
  replacement_entry_id?: string
  note: string
}
```

## Error Handling

- Registry fetch timeout: show an actionable error and keep the existing local tweak list usable.
- Invalid registry: reject unsafe entries and show a clear `unsupported registry` message.
- Missing network: show "Live registry unavailable" instead of empty success.
- Install failure: keep the entry visible, show the error, and preserve local files.
- Existing installed tweak: show `Installed`; do not overwrite unless the user clicks install/update.

## Testing

Required checks before release:

- `pnpm build`
- `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked`
- focused Rust tests for:
  - archive URL derivation
  - registry validation requiring full commit SHA
  - installed status detection
  - legacy recommendation mapping
- manual validation:
  - Codex++ page loads the upstream store entries
  - `Bennett's UI Improvements` shows repo and archive URL
  - installed tweaks show as installed
  - coldstart check/repair commands still compile and return reports

## Release Plan

If implementation succeeds:

- Bump app version to `1.8.6`.
- Update `README.md`, `README_EN.md`, `CHANGELOG.md`, `docs/project.md`, and release notes.
- Run tests and build.
- Package DMG and app zip.
- Publish `v1.8.6` with `macos-dmg-github-release`.

