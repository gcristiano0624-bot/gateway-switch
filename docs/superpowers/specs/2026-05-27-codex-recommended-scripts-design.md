# Codex++ Recommended Scripts Design

## Goal

Add a Gateway Switch managed "recommended scripts" installer for the four Codex++ scripts the user explicitly needs:

- Codex Context Used Meter
- Hide Usage Alert
- Codex Token Usage
- Codex List Pagebuster

The preferred implementation must reuse Codex++'s native user-script mechanism, so the scripts appear and behave like regular Codex++ scripts rather than as unrelated Gateway Switch-only features.

## Non-Goals

- Do not install every Codex++ store entry by default.
- Do not replace the existing Codex++ Tweak Store.
- Do not bundle unstable or version-sensitive tweaks such as iOS Simulator into this feature.
- Do not modify Codex private databases directly.

## Current Evidence

- Gateway Switch successfully patches `Codex.app` with `codex-plusplus-loader.cjs`.
- `codexpp doctor` passes for the local installation.
- The Codex++ runtime discovers installed tweaks from `~/Library/Application Support/codex-plusplus/tweaks`.
- The four requested scripts are not present in the current local `tweaks` tree, runtime bundle, source tree, or exact GitHub code-search results under their visible names.
- The screenshot state `not_loaded` likely belongs to Codex++'s user-script or market-script layer, not to Gateway Switch's current Tweak Store page.

## Architecture

### Backend

Add a native Rust script management layer in `src-tauri/src/codex_pp.rs`.

Responsibilities:

- Detect Codex++ user-script storage paths from the installed runtime/source when possible.
- Report recommended script status to the frontend.
- Install or re-install the four recommended scripts into the native Codex++ user-script location.
- Preserve existing user scripts and user edits.
- Write diagnostics to the existing Codex++ logs.
- Trigger a safe reload/repair path after installation, without repatching unrelated Codex state unless needed.

### Frontend

Extend the Codex++ market page in `src/App.tsx`.

Responsibilities:

- Show a `Recommended Scripts` panel above the existing tweak store.
- Display four fixed script rows with status: `Installed`, `Missing`, `Unknown`, or `Needs Reload`.
- Provide actions:
  - `Install Recommended Scripts`
  - `Refresh Script Status`
  - `Open Logs`
- Keep the existing Tweak Store grid unchanged.

### Data Model

Introduce frontend/backend data structures equivalent to:

```text
RecommendedScript {
  id: string
  name: string
  description: string
  status: installed | missing | unknown | needs_reload
  path?: string
}

RecommendedScriptsReport {
  storage_mode: codex_user_scripts | unknown
  storage_path?: string
  scripts: RecommendedScript[]
  summary: string
}
```

## Priority A Behavior

Priority A is the approved path.

1. Inspect the installed Codex++ runtime/source for its native user-script storage convention.
2. If a valid native user-script location is found, install the four scripts there.
3. Keep file names and script IDs stable so Codex++ can load them consistently.
4. Refresh status after installation.
5. Ask the user to restart Codex if the current Codex++ runtime cannot hot-reload user scripts.

## Failure Handling

- If the Codex++ user-script location cannot be detected, do not silently install into an arbitrary folder.
- Return a clear `unknown storage mode` state to the UI.
- Do not use the fallback bundled-tweak implementation in this iteration unless the user explicitly approves a follow-up design.
- If a script already exists, back it up before overwriting or offer a reinstall path that preserves the previous file.
- If install succeeds but runtime does not hot-load it, mark the scripts as `Needs Reload`.

## Testing

Run these checks before release:

- `pnpm build`
- `cargo test --locked`
- A focused Rust test for recommended script path/status logic.
- Manual local validation:
  - Install recommended scripts through Gateway Switch.
  - Confirm files exist in the detected Codex++ user-script location.
  - Restart Codex.
  - Confirm the Codex++ script page no longer shows the four scripts as `not_loaded` when the runtime supports them.

## Release Plan

If implementation succeeds:

- Update docs and changelog for `v1.8.5`.
- Package macOS DMG and app zip.
- Upload GitHub Release assets with `SHA256SUMS.txt`.

