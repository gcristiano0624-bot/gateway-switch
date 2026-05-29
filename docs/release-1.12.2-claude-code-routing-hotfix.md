# Gateway Switch v1.12.2 Release Notes

## Highlights

- Fixes Claude Code Gateway Route behavior for Xiaomi MiMo and other Chat-only providers by forcing Chat Completions fallback when the compatibility profile says the provider is not Direct Provider safe.
- Adds non-fatal streamed repetition-loop diagnostics so Gateway Switch can flag repeated upstream text without truncating valid output.
- Makes Claude Code Direct Provider binding usable again for risky providers by adding an explicit force checkbox instead of permanently disabling the bind button.

## Validation

```bash
cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo test --locked
pnpm build
CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build
```

## Notes

Gateway Route remains recommended for Volcengine DeepSeek, Xiaomi MiMo, DeepSeek official, Moonshot/Kimi, Qwen/DashScope, and most OpenAI Chat-compatible services. Direct Provider should be used only when the Anthropic endpoint is known to be protocol-compatible.
