# Gateway Switch 1.7.0 Bilingual UI Design

Date: 2026-05-18

## Goal

Gateway Switch 1.7.0 should support two interface languages:

- Chinese
- English

The default language is Chinese. Users can switch language in Settings.

Required technical terms stay in English where translating them would hurt clarity:

- Claude
- Claude Code
- Codex
- Gateway
- Provider
- OpenAI
- Anthropic
- API Key
- Base URL
- MCP
- Responses API
- Chat Completions

## Approach

Use a lightweight frontend i18n layer that fits the current single-file React structure.

Implementation choices:

- Add a `language` field to `AppSettings`.
- Persist language in the existing settings file.
- Use `zh` as the default for existing users and new installs.
- Add `UI_TEXT.zh` and `UI_TEXT.en` dictionaries in `src/App.tsx`.
- Add a small `t` object inside `App()` based on `settings?.language`.
- Replace major user-facing text with dictionary values.

This avoids a larger refactor while still giving the app a real bilingual mode.

## Scope

The first 1.7.0 bilingual pass covers:

- sidebar labels
- page titles and subtitles
- Dashboard summary
- Cold Start Doctor
- Settings
- Request Logs
- common buttons and status labels
- main binding and routing explanations
- toast messages triggered by primary actions

The following remain mostly English by design:

- model names
- provider names
- protocol names
- config field names
- log payload values
- error details returned by runtime systems

## Settings UX

Settings gets a new `Language / 语言` section.

Controls:

- `中文`
- `English`

Behavior:

- Switching language updates the UI immediately.
- Saving Settings persists language with the rest of the app settings.
- Existing settings files without `language` are read as `zh`.

## Backend Compatibility

`AppSettings` adds:

```rust
pub language: String
```

Serde default is used so existing settings files keep loading successfully.

Allowed values:

- `zh`
- `en`

The frontend enforces these values in the UI.

## Verification

Required checks:

- `pnpm build`
- `cargo test`
- diagnostics check

Recommended manual checks:

1. Start the app.
2. Confirm the default UI is Chinese.
3. Open Settings.
4. Switch to English and save.
5. Restart the app.
6. Confirm English is preserved.
7. Switch back to Chinese and save.

## Release Handling

This is still part of version `1.7.0`.

Update:

- `CHANGELOG.md`
- `docs/project.md`
- this design document

Commit and push the update to `main`.
