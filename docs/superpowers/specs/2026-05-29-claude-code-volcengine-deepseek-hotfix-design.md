# Claude Code Volcengine DeepSeek Hotfix Design

## Problem

Claude Code Direct Provider mode writes provider settings directly to `~/.claude/settings.json`.
This only works when the provider exposes a real Anthropic Messages-compatible endpoint.

Volcengine Ark `DeepSeek-V4-Pro` is configured with:

- OpenAI Base URL: `https://ark.cn-beijing.volces.com/api/coding/v3`
- Anthropic Base URL: `https://ark.cn-beijing.volces.com/api/coding`

Claude Code sends Anthropic-style requests that include system instructions. The Volcengine endpoint rejects `messages.role = system` and reports that only `assistant` and `user` are supported.

## Decision

Implement a Gateway Route compatibility hotfix instead of trying to force Direct Provider mode.

## Scope

- Add a Gateway compatibility mode for Volcengine/Ark/DeepSeek coding endpoints.
- When converting Anthropic Messages to OpenAI Chat payloads for these endpoints:
  - Do not emit `role: "system"`.
  - Merge system instructions into the first `user` message.
  - Convert Anthropic tool results into `user` messages instead of `tool` messages, because the endpoint reports only `assistant` and `user` roles.
- Add tests for the conversion behavior.
- Prevent future Claude Code Direct Provider binding for endpoints that are likely OpenAI Chat/Coding compatible rather than Anthropic-compatible.
- Add UI copy that tells users to choose Gateway Route for Volcengine/DeepSeek.

## Non-Goals

- Do not mutate user API keys.
- Do not silently delete the provider's Anthropic URL field.
- Do not remove Direct Provider mode for real Anthropic-compatible providers.
- Do not publish a release until tests pass.

## Expected User Flow

1. User creates or keeps a Claude route such as `claude-sonnet-4-6 -> Volcengine / DeepSeek-V4-Pro`.
2. User binds Claude Code using Gateway Route.
3. Claude Code sends Anthropic requests to Gateway Switch.
4. Gateway Switch converts requests to Volcengine-compatible Chat payloads without `system` or `tool` message roles.

