# Gateway Switch v1.12.0 Release Notes

## Highlights

- Unified Diagnostics Center for Claude Desktop, Claude Code, Codex Gateway, Codex++, Providers, and Install / Runtime.
- Failure clustering with local-only recommendations for role, tool, reasoning, rate-limit, attachment, and upstream server errors.
- Provider Presets for OpenRouter, Volcengine Ark DeepSeek, DeepSeek official, Moonshot Kimi, Qwen DashScope, Xiaomi MiMo, standard Anthropic-compatible, and OpenAI Chat-compatible providers.
- Safe preset application that preserves existing API keys unless the user supplies a replacement.
- Exportable unified diagnostics bundle for troubleshooting.

## Validation

Run before release:

```bash
pnpm build
cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo test --locked
pnpm tauri build
```

## Notes

This release still does not implement silent app replacement, Apple Developer ID notarization, or real upstream replay by default.
