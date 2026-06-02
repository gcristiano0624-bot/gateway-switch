# Agent LoopGuard Tool-Result Design

Date: 2026-06-02

## Background

Gateway Switch can route Claude Desktop traffic to OpenAI-compatible third-party providers. In long agent sessions, very large `tool_result` payloads can occupy the context window and make the model lose track of failed attempts. A real failure case showed repeated tool patterns such as `web_fetch`, `Read`, and shell commands after a 286K HTML result entered the conversation.

The existing `LoopGuard` suppresses repeated streamed text from the upstream response. It does not yet protect the request payload that carries historical tool calls and tool results into the next model turn.

## Scope

This iteration adds two low-risk safeguards for Claude Desktop routes:

- Compress oversized Anthropic `tool_result` content before conversion to OpenAI Chat messages.
- Apply the same compression to direct Anthropic-compatible upstream payloads.
- Detect repeated assistant `tool_use` fingerprints in the recent request history and inject one strategy-change hint.

This version does not block requests, add new database tables, or expose advanced tuning UI. Defaults are hard-coded so the feature can be validated safely.

## Defaults

- Tool-result threshold: `50_000` characters.
- Retained result preview: first `1_200` characters and last `400` characters.
- Tool-call window: last `5` calls seen in the request payload.
- Loop threshold: same normalized `tool_name + args` fingerprint appears `3` times in that window.
- Intervention mode: warm hint injection, not hard blocking.

## Data Flow

1. Claude Desktop sends an Anthropic Messages payload to the local gateway.
2. Gateway Switch converts Anthropic content blocks to OpenAI-compatible Chat messages.
3. During conversion, `LoopGuard` scans assistant `tool_use` blocks and user `tool_result` blocks.
4. Oversized `tool_result` content is replaced with a structured digest that preserves status, size, head, and tail.
5. Repeated tool-call fingerprints trigger one appended user hint telling the model to change strategy.
6. A compact `loop_guard` summary is written to `request_logs.error_summary`.

## Compatibility

The feature only changes the upstream payload. It preserves the client-facing Anthropic API response shape, existing provider routes, and request snapshot flow. For providers that require user/assistant-only roles, the compressed tool result is still emitted as a user message.

## Diagnostics

The existing request log field is used for first-version visibility:

```text
loop_guard: suppressed_text=0 repeated_segments=0 duplicate_tool_calls=2 large_tool_results=1 compressed_tool_result_chars=236000 tool_loop_hints=1
```

Health Center and Logs can already surface `error_summary`, so no database migration is required in this phase.

## Tests

Rust tests should cover:

- Large `tool_result` is compressed below the threshold and includes original size metadata.
- Repeated identical `tool_use` patterns inject one LoopGuard hint.
- Direct Anthropic-compatible payloads receive the same protection as OpenAI Chat fallback payloads.
- Normal distinct tool calls do not inject a hint.
- Existing Anthropic-to-Chat tool conversion remains compatible.
