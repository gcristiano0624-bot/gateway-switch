# Gateway Switch 1.15.0 Release Notes

Version: v1.15.0
Date: 2026-06-29
Repository: https://github.com/gcristiano0624-bot/gateway-switch
Predecessor: v1.14.1 (v1.14.2 was an experimental OSS-Mode branch and is intentionally skipped)

## Summary

Gateway Switch 1.15.0 is a stability + clarity release. It fixes a Codex App bug where reasoning models were leaking their internal chain-of-thought into the assistant chat UI, locks in the v1.13.3-style "GPT-* maps to Chinese-model" Codex routing as the supported flow, and skips the v1.14.2 experimental OSS-Mode path entirely.

## Highlights

- **Codex reasoning leak fixed**: `codex_gateway.rs::extract_text_from_delta` and `extract_chat_message_text` no longer fall back to `reasoning_content`. Reasoning models such as DeepSeek-R1 and Kimi K2 now return a clean assistant text reply instead of exposing their internal monologue (the famous "User is just saying hello. This is a simple greeting, no task. I don't need any tools. Just respond warmly." line that was previously showing up as the assistant's reply).
- **v1.13.3 Codex routing is the supported path**: `docs/project.md` section 19 and the README have been aligned with the actual code. The Codex App binding writes `model_provider = "gateway-switch"` with `requires_openai_auth = false` and `experimental_bearer_token = "<auth_token>"`, and provider rewrites live entirely in the `codex_routes` SQLite table. Recommended mappings:
  - `gpt-5.1` → `DeepSeek-V4-Pro` (Volcengine)
  - `gpt-5.2` → `Kimi-K2.7-Code` (Volcengine)
  - `gpt-5.3` → `mimo-v2.5-pro` (Xiaomi)
- **`custom_models` block intentionally not written**: the Codex App's model picker shows the OpenAI official list, and gateway does the rewrite at request time. v1.14.2's "OSS Mode + `custom_models`" path is skipped.
- **Version bump**: `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` all read `1.15.0`.

## Files Changed

- `src-tauri/src/codex_gateway.rs` — drop `reasoning_content` / `reasoning` fall-back in the two text extractors; update `test_extracts_provider_delta_variants` to assert `None` for reasoning-only deltas.
- `package.json` — version 1.14.1 → 1.15.0
- `src-tauri/Cargo.toml` — version 1.14.1 → 1.15.0
- `src-tauri/tauri.conf.json` — version 1.14.1 → 1.15.0
- `CHANGELOG.md` — add 1.15.0 section.
- `docs/project.md` — section 19 (Codex App Binding) now matches the actual code; section 20-22 wording refreshed.
- `README.md` / `README_EN.md` — bind code example updated to 1.15.0 behaviour.
- `release-artifacts/v1.14.2/` — removed (skipped version).
- `release-artifacts/v1.15.0/` — added (this release).

## Verification

- `cargo test --lib` — 82 passed, 0 failed
- `tsc && vite build` — passed
- `pnpm tauri build` — DMG ~7.6 MB produced at `release-artifacts/v1.15.0/Gateway Switch_1.15.0_aarch64.dmg`
- Manual smoke (Codex App, 11 tools, `gpt-5.1` / `gpt-5.2` / `gpt-5.3`):
  - `gpt-5.1` → DeepSeek-V4-Pro: 200 OK, `tool_choice = required` accepted
  - `gpt-5.2` → Kimi-K2.7-Code: 200 OK after switching `tool_call_mode` to `auto`; `required` was rejected with 400 InvalidParameter (fixed by route-level config, not by code change)
  - `gpt-5.3` → mimo-v2.5-pro: 200 OK

## Upgrade Notes

- This is a behavior-preserving change for the Claude Gateway. The Codex Gateway now shows clean text in the chat UI for reasoning models.
- If you previously upgraded to v1.14.2, your existing `~/.codex/config.toml` may still contain a `custom_models = [...]` block written by v1.14.2's `apply()`. After installing 1.15.0, run "Start & Bind Codex App" once to write the v1.15.0 config (no `custom_models` block) and restart the Codex App.
- v1.14.1 is kept as `release-artifacts/v1.14.1/` for users who want to roll back.
