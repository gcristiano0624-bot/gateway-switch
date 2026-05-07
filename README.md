<div align="center">

# Gateway Switch

**Claude Desktop 第三方模型路由网关**

[![Version](https://img.shields.io/badge/Version-1.0.0-blue?style=flat-square)](https://github.com/your-username/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/your-username/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

[English](./README_EN.md) | 中文

</div>

---

## 为什么需要 Gateway Switch？

Claude Desktop 支持通过 Developer Gateway 接入第三方推理服务，但从 2025 年起，它开始校验模型 ID 是否合法。直接使用第三方模型会报错。

社区的做法是在中间放一层 **Anthropic 兼容网关**，把第三方模型伪装成 Claude 模型名返回给 Desktop。Gateway Switch 就是这个网关的 **桌面客户端版本**——你不再需要手动编辑 YAML、手搓 Python 脚本、或在终端里启动服务。

**Gateway Switch 的核心价值：**

- **一个客户端搞定一切** — 添加 Provider、配置路由、启动网关、接管 Claude Desktop，全在 GUI 里完成
- **预设模板一键填充** — 内置 Volcano Engine Ark、XiaoMiMo、OpenRouter、DeepSeek、SiliconFlow 等常用服务的预设
- **一键接管 / 一键恢复** — 自动备份原始配置，接管和恢复只需点一下
- **完整 SSE 流式支持** — 不是简单的 chunk 替换，而是逐事件解析和 model 字段重写
- **系统托盘快捷操作** — 不用打开主窗口就能启停网关、绑定 Desktop
- **配置导入 / 导出** — JSON 格式，方便迁移和备份

---

## 功能特性

### Provider 管理

- 内置 5 个常用 Anthropic 兼容服务预设，一键填充表单
- 支持自定义 Provider，灵活配置 Base URL、Auth Header、Auth Scheme、API Key
- Provider 级别的健康检查（发送 `/v1/models` 请求验证连通性）
- 删除前自动检查是否仍被路由引用

### 模型路由

- 将 Claude 别名（`claude-opus-4-7`、`claude-sonnet-4-6`、`claude-haiku-4-5`）映射到任意上游模型
- 一个别名可以绑定不同 Provider 的不同模型
- 实时查看当前活跃的路由列表

### Claude Desktop 接管

- 自动检测 `~/Library/Application Support/Claude-3p/configLibrary` 配置状态
- 接管前自动备份，支持一键恢复到接管前状态
- 接管后自动注入 `inferenceProvider`、`inferenceGatewayBaseUrl`、`inferenceModels` 等字段
- 标记 `managedBy: "Gateway Switch"`，方便识别是否被管理

### 网关

- `GET /health` — 健康检查
- `GET /v1/models` — Claude 风格模型列表
- `POST /v1/messages` — 非流式 + 流式消息转发
- `POST /v1/messages/count_tokens` — Token 计数透传
- 同时支持 `x-api-key` 和 `Authorization: Bearer` 两种认证头

### 日志

- 每条请求记录：时间、Claude 别名、Provider、上游模型、模式（stream/sync）、状态码、耗时
- 最多保留 200 条日志

### 设置

- 监听地址 / 端口 / Auth Token 可自定义
- 开机自动启动网关
- 自动接管 Claude Desktop
- 配置导入 / 导出

---

## 快速开始

### 第 1 步：添加 Provider

打开 Gateway Switch，进入 **Providers** 页面，点击预设按钮（如 **Volcano Engine Ark**），填入你的 API Key，点击 **Add**。

### 第 2 步：创建路由

进入 **Routes** 页面，选择 Claude Alias（如 `claude-sonnet-4-6`），选择 Provider，填入上游模型 ID，点击 **Add**。

### 第 3 步：启动并绑定

回到 **Dashboard**，点击 **Start Gateway**，等待状态变为 Running，然后点击 **Bind Claude Desktop**。

### 第 4 步：使用

打开 Claude Desktop，在模型选择器中选择对应的 Claude 模型，开始对话。所有请求会通过 Gateway Switch 转发到你配置的第三方服务。

---

## 下载与安装

### 系统要求

- macOS 12+
- Claude Desktop 已安装

### 从 Release 下载

前往 [Releases](https://github.com/your-username/gateway-switch/releases) 页面下载最新版本的 `.dmg` 或 `.app` 文件。

### Homebrew

```bash
brew install --cask gateway-switch
```

### 从源码构建

```bash
# 前置条件：Node.js 18+, pnpm 8+, Rust 1.85+, Xcode CLI Tools

git clone https://github.com/your-username/gateway-switch.git
cd gateway-switch
pnpm install
pnpm tauri build --bundles app
```

产物位于 `src-tauri/target/release/bundle/macos/Gateway Switch.app`。

---

## 常见问题

<details>
<summary><b>Gateway Switch 和直接写 Python 脚本有什么区别？</b></summary>

Python 脚本需要你手动编辑 YAML 配置、在终端启动服务、手动修改 Claude Desktop 配置文件。Gateway Switch 把这些全部封装成了图形界面——点击按钮就能完成所有操作，而且自动备份原始配置，随时可以恢复。

</details>

<details>
<summary><b>支持哪些上游服务？</b></summary>

任何实现了 Anthropic Messages API 格式的服务都支持。已内置预设的服务包括：Volcano Engine Ark、XiaoMiMo、OpenRouter、DeepSeek、SiliconFlow。你也可以自定义添加任何兼容服务。

注意：不支持 OpenAI `/v1/chat/completions` 格式的上游。

</details>

<details>
<summary><b>接管 Claude Desktop 后会影响正常使用吗？</b></summary>

不会。接管只是修改了 Claude Desktop 的推理端点配置，其他功能（如登录、历史记录等）不受影响。恢复后配置会完全还原到接管前的状态。

</details>

<details>
<summary><b>数据存储在哪里？</b></summary>

所有数据存储在 `~/Library/Application Support/Gateway Switch/` 目录下：

- `gateway.db` — SQLite 数据库（Providers、Routes、日志）
- `settings.json` — 应用设置
- `backups/` — 配置导出备份

Claude Desktop 的配置备份存储在 `~/Library/Application Support/Claude-3p/configLibrary/backups/`。

</details>

<details>
<summary><b>网关端口被占用怎么办？</b></summary>

进入 Settings 页面，修改 Listen Port 为其他端口（如 3457），保存后重新启动网关。同时需要重新绑定 Claude Desktop 以更新端口。

</details>

<details>
<summary><b>可以同时使用多个 Provider 吗？</b></summary>

可以。每个 Claude 别名只能绑定一个 Provider 和一个上游模型，但你可以创建多个路由，让不同的 Claude 别名指向不同的 Provider。例如 `claude-opus-4-7` 走 Volcano Engine，`claude-sonnet-4-6` 走 DeepSeek。

</details>

---

## 架构概览

<details>
<summary><b>点击展开架构详情</b></summary>

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

**设计原则：**

- **SSOT（单一数据源）** — SQLite 作为主存储，settings.json 仅存储设备级偏好
- **原子写入** — 配置修改先写临时文件再 rename，防止损坏
- **自动备份** — 每次接管前自动创建配置快照

</details>

---

## 开发指南

<details>
<summary><b>点击展开开发指南</b></summary>

### 环境要求

- Node.js 18+
- pnpm 8+
- Rust 1.85+ (via rustup)
- Xcode Command Line Tools

### 开发命令

```bash
# 安装依赖
pnpm install

# 开发模式（前端热更新 + 后端自动重编译）
pnpm tauri dev

# 仅构建前端
pnpm build

# 仅编译后端
cd src-tauri && cargo build

# 运行测试
cd src-tauri && cargo test

# 构建 release
pnpm tauri build --bundles app
```

### 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| 桌面框架 | Tauri | 2.x |
| 后端语言 | Rust | 2021 Edition |
| 异步运行时 | tokio | 1.x |
| HTTP 框架 | axum | 0.8 |
| HTTP 客户端 | reqwest | 0.12 |
| 数据库 | SQLite (rusqlite) | 0.37 |
| 前端框架 | React | 19.x |
| 类型系统 | TypeScript | 5.x |
| 样式 | TailwindCSS | 4.x |
| 图标 | Lucide React | latest |
| 构建工具 | Vite | 7.x |

### 项目结构

```
gateway-switch/
├── src/                          # React 前端
│   ├── App.tsx                   # 主应用（侧边栏 + 6 个页面）
│   └── App.css                   # 样式（TailwindCSS + CSS 变量）
├── src-tauri/                    # Tauri 后端
│   ├── Cargo.toml                # Rust 依赖
│   ├── tauri.conf.json           # Tauri 配置
│   └── src/
│       ├── main.rs               # 程序入口
│       ├── lib.rs                # Tauri Builder 注册
│       ├── gateway.rs            # Anthropic 兼容网关
│       ├── database.rs           # SQLite DAO 层
│       ├── desktop_binding.rs    # Claude Desktop 配置管理
│       ├── commands.rs           # Tauri IPC 命令
│       ├── tray.rs               # 系统托盘
│       ├── settings.rs           # 设置读写
│       ├── state.rs              # 应用状态管理
│       └── models.rs             # 数据类型定义
├── docs/
│   └── PROJECT.md                # 详细项目文档
├── package.json
├── vite.config.ts
└── tsconfig.json
```

</details>

---

## 贡献

欢迎提交 Issue 和 Pull Request。提交 PR 前请确保：

1. `cargo test` 全部通过
2. `pnpm tauri build` 能正常构建
3. 新功能请补充对应的测试用例

---

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=your-username/gateway-switch&type=Date)](https://star-history.com/#your-username/gateway-switch&Date)

---

## 许可证

[MIT](LICENSE) © Hugo Guan
