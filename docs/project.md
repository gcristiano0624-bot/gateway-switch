# Gateway Switch Project Documentation

Version: 1.3.0

This document is the single technical source of truth for Gateway Switch. It merges the former project architecture notes and the Codex Gateway notes into one maintained file.

## 1. Product Goal

Gateway Switch is a macOS desktop app for routing Claude Desktop and Codex App traffic to third-party model providers.

The app solves two related but different protocol problems:

1. Claude Desktop expects Anthropic Messages API semantics and validates Claude model names. Gateway Switch exposes Claude-compatible aliases, rewrites requests to the real upstream model, first tries an Anthropic Messages upstream, and can automatically fall back to an OpenAI Chat Completions upstream when the provider does not support `/v1/messages`.
2. Codex App expects OpenAI Responses API semantics. Many third-party providers only support OpenAI Chat Completions. Gateway Switch exposes a local `/v1/responses` endpoint, converts Responses requests into Chat Completions requests, then converts sync and streaming Chat Completions responses back into Responses-shaped output.

The shared design goal is simple: providers are configured once, then product-specific pages decide how Claude or Codex should map model names to upstream models.

## 2. Version 1.3.0 Scope

Version 1.3.0 is the Codex-focused release.

Main changes:

- Added Codex App routing through a local Responses API gateway on port `3457`.
- Added Responses-to-Chat-Completions request conversion.
- Added Chat-Completions-to-Responses sync response conversion.
- Added Chat-Completions-SSE-to-Responses-SSE streaming conversion.
- Added Codex route management: `Codex Model -> Provider -> Upstream Model`.
- Added editable Claude aliases and Codex model aliases.
- Added Claude Chat Completions fallback for providers that only expose `/v1/chat/completions`.
- Fixed Claude Desktop binding to write the gateway root URL instead of `/v1/messages`, avoiding duplicated `/v1/messages/v1/messages` requests.
- Trimmed route fields before saving and before upstream dispatch to avoid invisible model-name mismatches.
- Added Codex App binding into `~/.codex/config.toml`.
- Added Codex restore logic for returning to the original OpenAI login/provider state.
- Added duplicate `/v1` protection for provider Base URLs.
- Added real upstream model verification through request logs and the Codex page.
- Reworked navigation into Dashboard, Products, Shared Providers, and System.
- Reworked Dashboard into status-only monitoring: status, refresh, and health checks.
- Fixed input remounting that caused text fields to lose focus after each typed character.

## 3. High-Level Architecture

```text
Gateway Switch
├─ React UI
│  ├─ Dashboard: status, health, recent traffic
│  ├─ Claude: aliases, routes, Claude Desktop binding
│  ├─ Codex: models, routes, Codex App binding
│  ├─ Providers: shared upstream provider registry
│  ├─ Logs: request history and real upstream verification
│  └─ Settings: app settings, import/export
├─ Tauri Commands
│  ├─ Provider CRUD
│  ├─ Claude route CRUD
│  ├─ Codex route CRUD
│  ├─ Alias CRUD
│  ├─ Gateway lifecycle commands
│  ├─ Desktop binding commands
│  └─ Health/log/settings commands
├─ Rust Gateways
│  ├─ Claude Gateway: :3456, Anthropic Messages surface with Chat Completions fallback
│  └─ Codex Gateway: :3457, OpenAI Responses compatible
├─ SQLite
│  ├─ providers
│  ├─ model_routes
│  ├─ codex_routes
│  ├─ model_aliases
│  ├─ gateway_profile
│  ├─ codex_profile
│  └─ request_logs
└─ External Configs
   ├─ Claude Desktop configLibrary
   └─ ~/.codex/config.toml
```

## 4. Technology Stack

| Layer | Technology | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Small macOS app bundle, Rust backend |
| Frontend | React 19 + TypeScript | Single app component with page functions |
| Build | Vite + pnpm | Frontend build and Tauri packaging |
| Backend | Rust 2021 | Type-safe async service layer |
| HTTP server | axum | Local gateways and health endpoints |
| HTTP client | reqwest | JSON and streaming upstream requests |
| Async runtime | tokio | Gateway lifecycle and streaming |
| Database | SQLite via rusqlite | Local persistent configuration |
| Serialization | serde / serde_json | Tauri IPC and request transformation |
| Time/IDs | chrono / uuid | Logs, backups, response IDs |

## 5. Source Layout

| Path | Responsibility |
| --- | --- |
| `src/App.tsx` | Main React UI, page rendering, Tauri command calls |
| `src/App.css` | App styling, dashboard, binding, route, alias, and log layouts |
| `src-tauri/src/lib.rs` | Tauri builder, command registration, startup hooks |
| `src-tauri/src/main.rs` | Native app entry point |
| `src-tauri/src/state.rs` | App state, runtime gateway handles, data paths |
| `src-tauri/src/models.rs` | Shared data models for providers, routes, logs, settings |
| `src-tauri/src/database.rs` | SQLite initialization and CRUD |
| `src-tauri/src/gateway.rs` | Claude/Anthropic-compatible gateway with Chat Completions fallback |
| `src-tauri/src/codex_gateway.rs` | Codex Responses-compatible gateway and conversion layer |
| `src-tauri/src/desktop_binding.rs` | Claude Desktop config read/apply/restore |
| `src-tauri/src/codex_binding.rs` | Codex config read/apply/restore |
| `src-tauri/src/commands.rs` | Tauri IPC command implementations |
| `src-tauri/src/settings.rs` | `settings.json` load/save |
| `src-tauri/src/tray.rs` | macOS tray menu |
| `docs/project.md` | Complete technical documentation |

## 6. Data Storage

App data is stored under:

```text
~/Library/Application Support/Gateway Switch/
```

Important files:

- `gateway.db`: SQLite database containing providers, routes, aliases, profiles, and request logs.
- `settings.json`: app-level settings such as auto-start and Claude listen port.
- `backups/`: exported config backups.

Claude Desktop config is managed under:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

Codex App config is managed at:

```text
~/.codex/config.toml
```

Codex backups are written to:

```text
~/.codex/gateway-switch-backups/
```

## 7. Database Schema

### `providers`

Stores reusable upstream provider definitions.

Fields:

- `id`: stable provider ID.
- `name`: display name.
- `base_url`: provider root or versioned URL.
- `auth_header`: usually `Authorization` or `x-api-key`.
- `auth_scheme`: usually `Bearer`, or empty for raw key headers.
- `api_key`: stored locally.
- `enabled`: provider availability.
- `created_at`: creation timestamp.

### `model_routes`

Claude routing table.

Fields:

- `id`: route ID.
- `claude_alias`: model name exposed to Claude Desktop.
- `display_name`: user-visible label.
- `provider_id`: linked provider.
- `upstream_model`: real model sent to the provider.
- `enabled`: whether the route is active.
- `created_at`: creation timestamp.

### `codex_routes`

Codex routing table.

Fields:

- `id`: route ID.
- `codex_model`: model name requested by Codex App.
- `display_name`: user-visible label.
- `provider_id`: linked provider.
- `upstream_model`: real model sent to the provider.
- `enabled`: whether the route is active.
- `created_at`: creation timestamp.

### `model_aliases`

Editable model alias registry.

Fields:

- `id`: generated UUID.
- `alias`: model alias string.
- `alias_type`: `claude` or `codex`.
- `created_at`: creation timestamp.

### `gateway_profile` and `codex_profile`

Store listen host, listen port, and local auth token for each product gateway.

Defaults:

- Claude Gateway: `127.0.0.1:3456`
- Codex Gateway: `127.0.0.1:3457`
- Token: `gateway-switch-token`

### `request_logs`

Stores recent request traces.

Fields:

- `request_id`: generated request ID.
- `claude_alias`: historical field name now used as requested model for both Claude and Codex.
- `provider_id`: selected provider.
- `upstream_model`: real upstream model.
- `status_code`: upstream/local response status.
- `duration_ms`: measured request duration.
- `is_stream`: whether the request was streaming.
- `error_summary`: upstream or conversion error text.
- `created_at`: timestamp.

Logs are the primary way to verify which model was actually called.

## 8. Claude Gateway

### Endpoint Surface

Claude Gateway listens on the configured Claude profile, default:

```text
http://127.0.0.1:3456
```

Endpoints:

- `GET /health`
- `GET /v1/models`
- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

### Request Flow

```text
Claude Desktop
-> POST /v1/messages model=claude-sonnet-4-6
-> Gateway Switch validates auth
-> Gateway Switch resolves model_routes by claude_alias
-> Gateway Switch rewrites request model to upstream_model
-> Gateway Switch posts to provider /v1/messages
-> If /v1/messages is unsupported or returns a non-Messages response, Gateway Switch falls back to provider /v1/chat/completions
-> Gateway Switch rewrites or converts response model fields back to claude_alias
-> Claude Desktop receives a Claude-compatible response
```

### Chat Completions Fallback

Some providers are OpenAI-compatible but do not implement Anthropic Messages. For those providers, Claude Gateway uses a conservative fallback path:

```text
Claude Desktop /v1/messages
-> Gateway Switch tries Provider /v1/messages
-> Provider returns unsupported status or non-Anthropic response
-> Gateway Switch converts the Claude Messages request to Chat Completions
-> Gateway Switch calls Provider /v1/chat/completions
-> Gateway Switch converts the Chat Completions response back to Claude Messages shape
-> Claude Desktop
```

This keeps Anthropic-compatible providers working as before while allowing providers such as XiaoMiMo to be used from Claude Desktop.

### Auth

Claude Gateway accepts:

- `x-api-key: <gateway-token>`
- `Authorization: Bearer <gateway-token>`

The app writes `x-api-key` into Claude Desktop binding by default. This is local gateway auth between Claude Desktop and Gateway Switch. It is separate from Provider auth, such as `Authorization: Bearer <provider-key>`, which Gateway Switch uses when calling the upstream provider.

### Provider URL Handling

The gateway appends the required endpoint to the configured provider Base URL. It also avoids double-appending `/v1`.

Examples:

- `https://api.example.com` + `messages` becomes `https://api.example.com/v1/messages`
- `https://api.example.com/v1` + `messages` becomes `https://api.example.com/v1/messages`

### Streaming

Claude streaming uses Anthropic SSE events. For Anthropic-compatible upstreams, Gateway Switch reads the upstream byte stream, splits lines, parses `data:` JSON when present, recursively rewrites `model` fields, and passes non-JSON SSE lines through unchanged.

For Chat Completions fallback upstreams, Gateway Switch converts Chat Completions SSE deltas into Anthropic Messages SSE events: `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, and `message_stop`.

## 9. Claude Desktop Binding

Claude Desktop binding reads and writes:

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

The active config is determined through `_meta.json` and its `appliedId`.

Binding writes fields such as:

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:3456",
  "inferenceGatewayApiKey": "gateway-switch-token",
  "inferenceGatewayAuthScheme": "x-api-key",
  "inferenceModels": [
    { "name": "claude-sonnet-4-6" }
  ],
  "managedBy": "Gateway Switch"
}
```

Restore uses the latest backup created before Gateway Switch took over.

## 10. Codex Gateway

### Endpoint Surface

Codex Gateway listens on the configured Codex profile, default:

```text
http://127.0.0.1:3457
```

Endpoints:

- `GET /health`
- `GET /v1/models`
- `POST /v1/responses`

### Why This Gateway Exists

Codex App talks to OpenAI-compatible Responses API. A large number of third-party providers only expose Chat Completions. Gateway Switch bridges that gap:

```text
Codex App /v1/responses
-> Gateway Switch
-> Provider /v1/chat/completions
-> Gateway Switch converts response back to Responses shape
-> Codex App
```

### Codex Route Model

Each Codex route maps:

```text
Codex Model -> Provider -> Real Upstream Model
```

Example:

```text
gpt-5.5 -> XiaoMiMo -> xiaomi-real-model-name
```

`Codex Model` must match what Codex requests. `Upstream Model` must match the real model ID expected by the third-party API.

If no model name disguise is needed, both fields can be identical.

### Responses Request Conversion

Gateway Switch converts:

- `instructions` to a system message.
- `input` message items to Chat Completions messages.
- `function_call_output` to tool messages.
- `function_call` to assistant `tool_calls`.
- `max_output_tokens` to `max_tokens`.
- `temperature`, `top_p`, and `tool_choice` are passed through when present.
- `tools` of type `function` are converted to Chat Completions function tools.

### Sync Response Conversion

For non-stream responses, Gateway Switch converts:

- `choices[0].message.content` to `output[0].content[0].text`.
- `usage.prompt_tokens` to `usage.input_tokens`.
- `usage.completion_tokens` to `usage.output_tokens`.
- `usage.total_tokens` is preserved or derived.
- `model` is rewritten to the Codex-requested model.

The Responses output includes fields required by Codex, including `status`, `output`, and detailed token usage.

### Streaming Response Conversion

For streaming, Gateway Switch emits Responses-compatible SSE events:

- `response.created`
- `response.output_item.added`
- `response.content_part.added`
- `response.output_text.delta`
- `response.output_text.done`
- `response.content_part.done`
- `response.output_item.done`
- `response.completed`

Provider delta variants supported:

- `choices[0].delta.content`
- `choices[0].delta.reasoning_content`
- `choices[0].delta.reasoning`
- `choices[0].delta.text`
- content arrays with `text` or `content`

The gateway estimates usage for streaming when the provider does not send final usage data.

### Error Handling

Upstream non-2xx responses become `502 Bad Gateway` responses with a compact upstream error message. This made provider configuration problems visible in Codex instead of producing silent disconnects.

Common upstream errors:

- 401/403: wrong API key, auth header, or auth scheme.
- 404: wrong provider Base URL or unsupported endpoint.
- 429: provider quota or rate limit.
- 5xx: provider outage.

## 11. Codex App Binding

Binding writes to:

```text
~/.codex/config.toml
```

Gateway Switch writes:

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

Important fields:

- `model_provider = "gateway-switch"` makes Codex use the local provider.
- `model` selects the default Codex model exposed by Gateway Switch.
- `preferred_auth_method = "apikey"` makes Codex prefer API-key mode over OAuth for this local provider.
- `wire_api = "responses"` tells Codex to use the Responses API surface.
- `requires_openai_auth = false` prevents OpenAI OAuth from being required for this provider.
- `experimental_bearer_token` lets Codex App launched from Finder carry the local gateway token without terminal environment variables.

Restore returns to the latest unmanaged backup. If no clean backup exists, Gateway Switch removes its managed `gateway-switch` config block while preserving unrelated Codex config sections.

## 12. Reasoning Behavior With Third-Party Models

Gateway Switch does not add or remove model reasoning ability. It only converts protocol shape.

If the provider exposes reasoning through a Chat Completions delta field such as `reasoning_content`, Gateway Switch can forward it as output text. If the provider does not expose reasoning data, Codex will only see final text.

Fast responses are normal when:

- The upstream model is fast.
- The prompt is simple.
- The provider returns only final text.
- The model does not expose visible reasoning over Chat Completions.

## 13. Verifying The Real Model

The recommended verification path is:

1. Open the Codex page.
2. Send a message from Codex App.
3. Check the `Verify Real Model` card.
4. For detailed history, open Logs.

The important log fields are:

- `Requested Model`: what Claude or Codex requested.
- `Provider`: which provider route was selected.
- `Real Upstream`: the actual model ID sent to the third-party API.
- `Status`: whether the upstream call succeeded.
- `Duration`: response time.

## 14. Project History And Context Limits

Gateway Switch preserves local config sections such as `[projects...]` when binding or restoring Codex. This preserves project trust and local config as much as possible.

Codex App conversation history, account state, and provider-specific cloud state are controlled by Codex itself. Switching between OpenAI login and a local Gateway provider can show different conversation lists. Gateway Switch cannot force different Codex account/provider states to share one conversation database unless Codex App exposes that capability.

## 15. Frontend UX Model

Navigation is grouped as:

- Dashboard: status only, refresh, health checks, recent traffic.
- Products: Claude and Codex product-specific setup.
- Shared: Providers, reused by both products.
- System: Logs and Settings.

Dashboard intentionally does not perform binding or gateway startup in version 1.3.0. Startup and binding live on the product page they affect:

- Claude page: Claude route setup and Claude Desktop binding.
- Codex page: Codex route setup and Codex App binding.

This prevents confusion between the Claude gateway and Codex gateway.

## 16. Input Focus Bug Fix

The app originally rendered page functions as nested React components, which changed component identity on every parent render. That caused input fields to remount after typing one character, losing focus.

The fix is to call internal page functions directly in the content switch instead of rendering them as nested component tags. This keeps input elements stable during state updates.

## 17. Tauri IPC Commands

Provider and route commands:

- `list_providers`
- `create_provider`
- `update_provider`
- `delete_provider`
- `list_routes`
- `create_route`
- `update_route`
- `delete_route`
- `list_codex_routes`
- `create_codex_route`
- `update_codex_route`
- `delete_codex_route`
- `list_model_aliases`
- `create_model_alias`
- `delete_model_alias`

Lifecycle commands:

- `start_gateway`
- `stop_gateway`
- `start_codex_gateway`
- `stop_codex_gateway`
- `get_status`
- `get_codex_status`

Binding commands:

- `get_desktop_info`
- `apply_binding`
- `restore_binding`
- `get_codex_binding_info`
- `apply_codex_binding`
- `restore_codex_binding`

Health and settings commands:

- `check_gateway_health`
- `check_codex_health`
- `check_provider_health`
- `get_settings`
- `save_settings`
- `export_config`
- `import_config`
- `list_logs`

## 18. Build And Release

Development:

```bash
pnpm install
pnpm tauri dev
```

Frontend build:

```bash
pnpm build
```

Rust tests:

```bash
cd src-tauri
cargo test
```

Release build:

```bash
pnpm tauri build
```

macOS artifacts:

```text
src-tauri/target/release/bundle/macos/Gateway Switch.app
src-tauri/target/release/bundle/dmg/Gateway Switch_1.3.0_aarch64.dmg
```

## 19. Release Checklist

- Frontend build passes.
- Rust tests pass.
- Claude Gateway health check passes.
- Codex Gateway health check passes.
- Claude route can rewrite model request and response fields.
- Codex route can convert Responses to Chat Completions and back.
- Codex streaming response completes with `response.completed`.
- Logs show requested model, provider, and real upstream model.
- Claude Desktop binding creates a backup and can restore.
- Codex binding creates a backup and can restore.
- DMG version matches `package.json`, `Cargo.toml`, and `tauri.conf.json`.

## 20. Known Limitations

- Claude Gateway requires an Anthropic Messages-compatible upstream.
- Codex Gateway requires a Chat Completions-compatible upstream.
- Codex visible reasoning depends on what the upstream model/provider returns.
- Gateway Switch cannot merge Codex cloud/account conversation history across provider states.
- API keys are stored locally for convenience; the app is designed for local personal use.
