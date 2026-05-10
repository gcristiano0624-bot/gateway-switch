<div align="center">

# Gateway Switch

**Third-party model router for Claude Desktop, Claude Code, and Codex App**

[![Version](https://img.shields.io/badge/Version-1.5.0-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

English | [中文](./README.md)

</div>

---

## What is Gateway Switch?

Gateway Switch is a macOS desktop app that routes Claude Desktop, Claude Code, and Codex App model traffic to third-party model APIs.

It solves three related problems:

- **Claude Desktop** validates Claude model IDs. Gateway Switch exposes local Claude aliases such as `claude-sonnet-4-6`, maps them to real upstream models, and forwards traffic through a local Claude gateway. The upstream can be Anthropic Messages compatible or OpenAI Chat Completions compatible.
- **Claude Code** can use local Gateway Route mode for unified routing, or Direct Provider mode for third-party Anthropic-compatible endpoints.
- **Codex App** uses OpenAI Responses API, while many third-party providers only support Chat Completions. Gateway Switch exposes a local `/v1/responses` endpoint, converts Codex requests to `/v1/chat/completions`, then converts responses back into Responses format.

Providers are shared, but protocol URLs are separate. Codex uses the OpenAI Base URL. Claude and Claude Code prefer the Anthropic Base URL so one provider URL is not accidentally reused by incompatible clients.

---

## Version 1.5.0 Highlights

- Providers now store separate `OpenAI Base URL` and `Anthropic Base URL` values.
- Added an independent Claude Code page with `Gateway Route` and `Direct Provider` binding modes.
- Claude Code Direct Provider writes only the provider's Anthropic Base URL and blocks binding when it is missing.
- Claude Desktop prefers the provider's Anthropic Base URL, with local Gateway fallback to Chat Completions when needed.
- Codex Gateway always uses OpenAI Base URL and keeps converting Responses requests to Chat Completions.
- XiaoMiMo presets and legacy data migration now include the Anthropic endpoint for models such as `mimo-v2.5`.
- Provider and Claude Code screens now show both protocol URLs to reduce misconfiguration.
- Refreshed the UI with a brighter, tighter cc switch-inspired style.
- App version is now `1.5.0`.

---

## Features

### Dashboard

- View Claude Gateway and Codex Gateway status.
- View binding status and latest upstream call.
- Run Claude/Codex health checks.
- Refresh current state.

Dashboard does not start gateways or bind apps. Startup and binding live on the relevant product page.

### Claude

- Manage Claude model aliases.
- Create routes: `Claude Alias -> Provider -> Upstream Model`.
- Start/stop Claude Gateway.
- Bind or restore Claude Desktop.
- Supports Anthropic Messages streaming and non-streaming forwarding.
- Supports automatic adaptation for OpenAI Chat Completions upstreams, useful for providers that only expose `/v1/chat/completions`.

Default address:

```text
http://127.0.0.1:3456
```

### Claude Code

- Bind Claude Code independently from Claude Desktop.
- `Gateway Route`: writes the local Claude Gateway, useful for unified routing and Chat Completions fallback.
- `Direct Provider`: writes the provider's Anthropic Base URL, API key, and model name directly into Claude Code.
- Direct Provider is intended for providers that expose an Anthropic-compatible endpoint, such as XiaoMiMo with `https://.../anthropic`.

### Codex

- Manage Codex model names.
- Create routes: `Codex Model -> Provider -> Upstream Model`.
- Start/stop Codex Gateway.
- Bind or restore Codex App.
- Convert Responses API requests to Chat Completions.
- Convert Chat Completions responses back to Responses format.
- Verify the latest real upstream model directly on the Codex page.

Default address:

```text
http://127.0.0.1:3457
```

### Providers

- Manage shared third-party providers.
- Configure OpenAI Base URL, Anthropic Base URL, Auth Header, Auth Scheme, and API Key.
- Use built-in presets or custom providers.
- Claude, Claude Code, and Codex share provider identity and keys, but use protocol-specific Base URLs.

### Logs

- View request time, requested model, provider, real upstream model, status code, and duration.
- Use logs to verify which model was actually called.

---

## Quick Start

### Claude Desktop Routing

1. Open `Providers` and add a provider. Use `/v1` or an equivalent Chat Completions URL for OpenAI Base URL; use `/anthropic` or an equivalent Messages URL for Anthropic Base URL.
2. Open `Claude` and add or select a Claude alias.
3. Create a route and enter the real upstream model name.
4. Start Claude Gateway from the `Claude` page.
5. Bind Claude Desktop from the `Claude` page.
6. Restart Claude Desktop and use the mapped Claude model.

### Claude Code Binding

1. Open `Providers` and confirm the target provider has an Anthropic Base URL.
2. Open `Claude Code`.
3. Select `Direct Provider`, choose the provider, and enter the real upstream model, such as `mimo-v2.5`.
4. Click `Bind Claude Code`.
5. Restart Claude Code or open a new session, then choose the bound model.

If the provider does not have an Anthropic Base URL, use `Gateway Route` so the local gateway can handle protocol conversion.

### Codex App Routing

1. Open `Providers` and add a provider that supports OpenAI Chat Completions.
2. Open `Codex` and add or select a Codex model, such as `gpt-5.5`.
3. Create a route and enter the real upstream model name.
4. Select the default Codex model.
5. Click `Start & Bind Codex App`.
6. Restart Codex App and start using it.

Binding writes:

```toml
model_provider = "gateway-switch"
model = "gpt-5.5"
preferred_auth_method = "apikey"

[model_providers.gateway-switch]
name = "Gateway Switch"
base_url = "http://127.0.0.1:3457/v1"
wire_api = "responses"
requires_openai_auth = false
experimental_bearer_token = "gateway-switch-token"
```

---

## Provider URLs And Auth

Recommended URL split:

```text
OpenAI Base URL: https://provider.example.com/v1
Anthropic Base URL: https://provider.example.com/anthropic
```

Codex only uses OpenAI Base URL. Claude Code Direct Provider only uses Anthropic Base URL.

Common setup:

```text
Auth Header: Authorization
Auth Scheme: Bearer
API Key: sk-...
```

For providers that require `x-api-key`:

```text
Auth Header: x-api-key
Auth Scheme:
API Key: your-key
```

When `Auth Scheme` is empty, Gateway Switch sends the raw API key in the configured header.

Note: `Local Gateway Auth` / `x-api-key` in the Claude Desktop binding is the auth method from **Claude Desktop to local Gateway Switch**. Provider auth such as `Authorization: Bearer ...` is the auth method from **Gateway Switch to the third-party model service**. They are separate links and do not need to match.

---

## Claude, Claude Code, And Codex Protocols

The products can share the same Provider/API key, but they should not share one protocol URL:

- Claude uses `http://127.0.0.1:3456/v1/messages` and presents an Anthropic Messages API surface.
- Claude Code Direct Provider uses the provider's Anthropic Base URL.
- Codex uses `http://127.0.0.1:3457/v1/responses` and presents an OpenAI Responses API surface.

When a Claude route points to a Chat Completions-only upstream, Gateway Switch first tries `/v1/messages`; if unsupported, it automatically falls back to `/v1/chat/completions`.

---

## Verify The Real Model

After sending a request from Claude or Codex:

1. Return to Gateway Switch.
2. Open the `Codex` page and check `Verify Real Model`.
3. Or open `Logs` for full history.

Important fields:

- `Requested Model`: the model requested by the client.
- `Provider`: the matched provider.
- `Real Upstream`: the actual model sent to the third-party API.

---

## Codex Reasoning Notes

Gateway Switch only converts protocol shape. It does not add or remove model reasoning capability.

If a third-party provider does not return reasoning fields through Chat Completions, Codex may only show final text. Fast replies are normal and depend on the upstream model, provider behavior, and prompt complexity.

---

## Download

Download the latest `.dmg` from [Releases](https://github.com/gcristiano0624-bot/gateway-switch/releases/).

Requirements:

- macOS 12+
- Claude Desktop or Codex App, depending on your use case

---

## Build From Source

Requirements:

- Node.js 18+
- pnpm 8+
- Rust 1.85+
- Xcode Command Line Tools

Commands:

```bash
pnpm install
pnpm build
cd src-tauri && cargo test
cd ..
pnpm tauri build
```

Artifacts:

```text
src-tauri/target/release/bundle/macos/Gateway Switch.app
src-tauri/target/release/bundle/dmg/Gateway Switch_1.5.0_aarch64.dmg
```

---

## Technical Documentation

Full architecture, protocol conversion, database schema, binding strategy, and release process:

[docs/project.md](./docs/project.md)

---

## Data Storage

Gateway Switch app data:

```text
~/Library/Application Support/Gateway Switch/
```

Claude Desktop config:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

Codex App config:

```text
~/.codex/config.toml
```

---

## Known Limits

- Claude Code Direct Provider requires an Anthropic Messages compatible upstream.
- Codex Gateway requires an OpenAI Chat Completions compatible upstream.
- Claude Gateway fallback requires OpenAI Chat Completions compatibility.
- Visible Codex reasoning depends on whether the upstream returns reasoning data.
- Codex conversation history across different login/provider states is controlled by Codex App, not Gateway Switch.
