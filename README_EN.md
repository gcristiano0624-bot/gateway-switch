<div align="center">

# Gateway Switch

**Third-party model router for Claude Desktop and Codex App**

[![Version](https://img.shields.io/badge/Version-1.3.0-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

English | [中文](./README.md)

</div>

---

## What is Gateway Switch?

Gateway Switch is a macOS desktop app that routes Claude Desktop and Codex App model traffic to third-party model APIs.

It solves two related problems:

- **Claude Desktop** validates Claude model IDs. Gateway Switch exposes local Claude aliases such as `claude-sonnet-4-6`, maps them to real upstream models, and forwards traffic through a local Claude gateway. The upstream can be Anthropic Messages compatible or OpenAI Chat Completions compatible.
- **Codex App** uses OpenAI Responses API, while many third-party providers only support Chat Completions. Gateway Switch exposes a local `/v1/responses` endpoint, converts Codex requests to `/v1/chat/completions`, then converts responses back into Responses format.

Providers are shared. Claude and Codex have separate route and binding workflows.

---

## Version 1.3.0 Highlights

- Added Codex Gateway: OpenAI Responses API to Chat Completions conversion.
- Added one-click Codex App binding through `~/.codex/config.toml`.
- Added Codex restore flow for returning to the original OpenAI login/provider state.
- Added real model verification on the Codex page and in Logs.
- Added editable Claude aliases and Codex model names.
- Added Claude Chat Completions fallback: when an upstream does not support `/v1/messages`, Gateway Switch automatically falls back to `/v1/chat/completions` and converts the result back to Claude Messages shape.
- Fixed Claude Desktop binding that could duplicate `/v1/messages/v1/messages`.
- Fixed leading/trailing whitespace in model names causing upstream model mismatches.
- Fixed input focus loss after typing one character.
- Fixed duplicated `/v1/v1/...` provider URL handling.
- Reworked Dashboard into a pure status view with refresh and health checks only.
- Reorganized navigation into Dashboard, Products, Shared, and System.

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
- Configure Base URL, Auth Header, Auth Scheme, and API Key.
- Use built-in presets or custom providers.
- Claude and Codex share providers but keep separate routes.

### Logs

- View request time, requested model, provider, real upstream model, status code, and duration.
- Use logs to verify which model was actually called.

---

## Quick Start

### Claude Desktop Routing

1. Open `Providers` and add a provider. It can support Anthropic Messages API or OpenAI Chat Completions.
2. Open `Claude` and add or select a Claude alias.
3. Create a route and enter the real upstream model name.
4. Start Claude Gateway from the `Claude` page.
5. Bind Claude Desktop from the `Claude` page.
6. Restart Claude Desktop and use the mapped Claude model.

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

## Provider Auth

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

## Claude vs Codex Protocols

Claude and Codex can share the same Provider/API key, but they do not share the same local protocol endpoint:

- Claude uses `http://127.0.0.1:3456/v1/messages` and presents an Anthropic Messages API surface.
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
src-tauri/target/release/bundle/dmg/Gateway Switch_1.3.0_aarch64.dmg
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

- Claude Gateway requires an Anthropic Messages compatible upstream.
- Codex Gateway requires an OpenAI Chat Completions compatible upstream.
- Visible Codex reasoning depends on whether the upstream returns reasoning data.
- Codex conversation history across different login/provider states is controlled by Codex App, not Gateway Switch.
