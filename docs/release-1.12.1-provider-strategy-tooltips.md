# Gateway Switch v1.12.1 Release Notes

## Highlights

- Adds inline tooltip help next to Provider Strategy Overrides controls.
- Explains `system_to_user`, `tool_to_user`, `disable_tools`, `strip_unsupported_params`, `direct_provider_safe`, `gateway_route_recommended`, `codex_disable_responses`, `codex_strict_tool_calls`, and `codex_strip_reasoning`.
- Adds high-risk notes for settings that can break tool use or unsafe Claude Code Direct Provider flows.
- Keeps the release UI-only with no database migration or backend compatibility change.

## Validation

```bash
pnpm build
cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo check
CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build
```
