<div align="center">

# Gateway Switch

**Third-party model router for Claude Desktop**

[![Version](https://img.shields.io/badge/Version-1.0.0-blue?style=flat-square)](https://github.com/your-username/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/your-username/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

English | [中文](./README.md)

</div>

---

## Why Gateway Switch?

Claude Desktop supports connecting third-party inference services via its Developer Gateway mode. However, since 2025 it has started validating model IDs, rejecting any that aren't official Claude models.

The community workaround is to place an **Anthropic-compatible gateway** in between — translating third-party model names into Claude model aliases. Gateway Switch is the **desktop app version** of this gateway. No more manual YAML editing, no more hand-written Python scripts, no more starting services from the terminal.

**Core value of Gateway Switch:**

- **All-in-one client** — Add providers, configure routes, start the gateway, take over Claude Desktop, all from a GUI
- **Preset templates** — Built-in presets for Volcano Engine Ark, XiaoMiMo, OpenRouter, DeepSeek, SiliconFlow, and more
- **One-click takeover / restore** — Automatic config backup before takeover, one-click restore to original state
- **Full SSE streaming support** — Not simple chunk replacement, but per-event parsing and model field rewriting
- **System tray quick actions** — Start/stop gateway, bind Desktop, all without opening the main window
- **Import / Export** — JSON format config files for easy migration and backup

---

## Features

### Provider Management

- 5 built-in presets for common Anthropic-compatible services, one-click form fill
- Custom provider support with flexible Base URL, Auth Header, Auth Scheme, and API Key
- Per-provider health check (sends `/v1/models` request to verify connectivity)
- Automatic reference check before deletion

### Model Routing

- Map Claude aliases (`claude-opus-4-7`, `claude-sonnet-4-6`, `claude-haiku-4-5`) to any upstream model
- Each alias can bind to a different provider and model
- Real-time view of active routes

### Claude Desktop Takeover

- Auto-detect `~/Library/Application Support/Claude-3p/configLibrary` status
- Automatic backup before takeover, one-click restore
- Auto-inject `inferenceProvider`, `inferenceGatewayBaseUrl`, `inferenceModels` fields
- Marks `managedBy: "Gateway Switch"` for easy identification

### Gateway

- `GET /health` — Health check
- `GET /v1/models` — Claude-style model list
- `POST /v1/messages` — Non-streaming + streaming message forwarding
- `POST /v1/messages/count_tokens` — Token count pass-through
- Supports both `x-api-key` and `Authorization: Bearer` auth headers

### Logging

- Per-request record: timestamp, Claude alias, provider, upstream model, mode (stream/sync), status code, duration
- Retains up to 200 log entries

### Settings

- Customizable listen address / port / auth token
- Auto-start gateway on app launch
- Auto-takeover Claude Desktop on launch
- Config import / export

---

## Quick Start

### Step 1: Add a Provider

Open Gateway Switch, go to the **Providers** page, click a preset button (e.g. **Volcano Engine Ark**), fill in your API Key, click **Add**.

### Step 2: Create a Route

Go to the **Routes** page, select a Claude Alias (e.g. `claude-sonnet-4-6`), select the provider, fill in the upstream model ID, click **Add**.

### Step 3: Start and Bind

Go back to **Dashboard**, click **Start Gateway**, wait for status to change to Running, then click **Bind Claude Desktop**.

### Step 4: Use

Open Claude Desktop, select the corresponding Claude model from the model selector, and start chatting. All requests will be routed through Gateway Switch to your configured third-party service.

---

## Download & Installation

### System Requirements

- macOS 12+
- Claude Desktop installed

### Download from Releases

Go to the [Releases](https://github.com/your-username/gateway-switch/releases) page to download the latest `.dmg` or `.app` file.

### Homebrew

```bash
brew install --cask gateway-switch
```

### Build from Source

```bash
# Prerequisites: Node.js 18+, pnpm 8+, Rust 1.85+, Xcode CLI Tools

git clone https://github.com/your-username/gateway-switch.git
cd gateway-switch
pnpm install
pnpm tauri build --bundles app
```

Build output: `src-tauri/target/release/bundle/macos/Gateway Switch.app`

---

## FAQ

<details>
<summary><b>How is Gateway Switch different from a Python script?</b></summary>

A Python script requires you to manually edit YAML config, start the service from the terminal, and manually modify Claude Desktop's config files. Gateway Switch wraps all of this into a GUI — click a button and you're done. It also automatically backs up the original config so you can restore at any time.

</details>

<details>
<summary><b>Which upstream services are supported?</b></summary>

Any service that implements the Anthropic Messages API format. Built-in presets include: Volcano Engine Ark, XiaoMiMo, OpenRouter, DeepSeek, SiliconFlow. You can also add any custom compatible service.

Note: OpenAI `/v1/chat/completions` format upstreams are not supported.

</details>

<details>
<summary><b>Will taking over Claude Desktop affect normal usage?</b></summary>

No. The takeover only modifies Claude Desktop's inference endpoint settings. Other features (login, history, etc.) are unaffected. After restoring, the config is fully reverted to its pre-takeover state.

</details>

<details>
<summary><b>Where is data stored?</b></summary>

All data is stored under `~/Library/Application Support/Gateway Switch/`:

- `gateway.db` — SQLite database (Providers, Routes, Logs)
- `settings.json` — App settings
- `backups/` — Config export backups

Claude Desktop config backups are stored in `~/Library/Application Support/Claude-3p/configLibrary/backups/`.

</details>

<details>
<summary><b>What if the gateway port is already in use?</b></summary>

Go to Settings, change the Listen Port to another value (e.g. 3457), save, and restart the gateway. You'll also need to re-bind Claude Desktop to update the port.

</details>

<details>
<summary><b>Can I use multiple providers at the same time?</b></summary>

Yes. Each Claude alias can only bind to one provider and one upstream model, but you can create multiple routes to point different aliases to different providers. For example, `claude-opus-4-7` via Volcano Engine and `claude-sonnet-4-6` via DeepSeek.

</details>

---

## Architecture

<details>
<summary><b>Click to expand architecture details</b></summary>

```
┌─────────────────────────────────────────────┐
│              Gateway Switch                  │
│                                              │
│  ┌────────────┐    ┌───────────────────────┐ │
│  │ React + TS │←──→│   Tauri IPC (22 cmd)  │ │
│  │  Frontend  │    └───────────┬───────────┘ │
│  └────────────┘                │             │
│          ┌────────────────────┼─────────┐   │
│          ↓                    ↓         ↓   │
│  ┌─────────────┐  ┌────────────┐  ┌───────┐│
│  │   Gateway   │  │  Database  │  │Desktop││
│  │   (axum)    │  │  (SQLite)  │  │Binding││
│  │    :3456    │  │            │  │       ││
│  └──────┬──────┘  └────────────┘  └───────┘│
│         │                                   │
└─────────┼───────────────────────────────────┘
          ↓
┌───────────────────┐
│ Upstream Provider  │
│ (Anthropic API)    │
└───────────────────┘
```

**Design Principles:**

- **SSOT (Single Source of Truth)** — SQLite as primary storage; settings.json only for device-level preferences
- **Atomic Writes** — Config changes write to temp file first, then rename, preventing corruption
- **Auto Backup** — Config snapshot created before every takeover

</details>

---

## Development Guide

<details>
<summary><b>Click to expand development guide</b></summary>

### Prerequisites

- Node.js 18+
- pnpm 8+
- Rust 1.85+ (via rustup)
- Xcode Command Line Tools

### Commands

```bash
# Install dependencies
pnpm install

# Dev mode (frontend hot reload + backend auto recompile)
pnpm tauri dev

# Build frontend only
pnpm build

# Compile backend only
cd src-tauri && cargo build

# Run tests
cd src-tauri && cargo test

# Build release
pnpm tauri build --bundles app
```

### Tech Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Desktop Framework | Tauri | 2.x |
| Backend Language | Rust | 2021 Edition |
| Async Runtime | tokio | 1.x |
| HTTP Framework | axum | 0.8 |
| HTTP Client | reqwest | 0.12 |
| Database | SQLite (rusqlite) | 0.37 |
| Frontend Framework | React | 19.x |
| Type System | TypeScript | 5.x |
| Styling | TailwindCSS | 4.x |
| Icons | Lucide React | latest |
| Build Tool | Vite | 7.x |

### Project Structure

```
gateway-switch/
├── src/                          # React frontend
│   ├── App.tsx                   # Main app (sidebar + 6 pages)
│   └── App.css                   # Styles (TailwindCSS + CSS variables)
├── src-tauri/                    # Tauri backend
│   ├── Cargo.toml                # Rust dependencies
│   ├── tauri.conf.json           # Tauri config
│   └── src/
│       ├── main.rs               # Entry point
│       ├── lib.rs                # Tauri Builder registration
│       ├── gateway.rs            # Anthropic-compatible gateway
│       ├── database.rs           # SQLite DAO layer
│       ├── desktop_binding.rs    # Claude Desktop config management
│       ├── commands.rs           # Tauri IPC commands
│       ├── tray.rs               # System tray
│       ├── settings.rs           # Settings I/O
│       ├── state.rs              # App state management
│       └── models.rs             # Data type definitions
├── docs/
│   └── PROJECT.md                # Detailed project documentation
├── package.json
├── vite.config.ts
└── tsconfig.json
```

</details>

---

## Contributing

Issues and Pull Requests are welcome. Before submitting a PR, please ensure:

1. `cargo test` passes
2. `pnpm tauri build` builds successfully
3. New features include corresponding test cases

---

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=your-username/gateway-switch&type=Date)](https://star-history.com/#your-username/gateway-switch&Date)

---

## License

[MIT](LICENSE) © Hugo Guan
