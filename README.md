<div align="center">

# Gateway Switch

**Claude Desktop 与 Codex App 的第三方模型路由网关**

[![Version](https://img.shields.io/badge/Version-1.3.0-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

[English](./README_EN.md) | 中文

</div>

---

## Gateway Switch 是什么？

Gateway Switch 是一个 macOS 桌面应用，用来把 Claude Desktop 和 Codex App 的模型请求转发到第三方模型 API。

它解决两类问题：

- **Claude Desktop**：Claude 会校验模型名，第三方模型不能直接伪装成 Claude 模型。Gateway Switch 提供本地 Anthropic Messages 网关，把 `claude-sonnet-4-6` 等别名映射到真实上游模型。
- **Codex App**：Codex 使用 OpenAI Responses API，但很多第三方服务只支持 Chat Completions。Gateway Switch 提供本地 Responses 网关，把 Codex 请求转换为 `/v1/chat/completions`，再把响应转换回 Codex 可解析的 Responses 格式。

Provider 是公共配置，Claude 和 Codex 各自拥有独立的路由和绑定流程。

---

## 1.3.0 更新重点

- 新增 Codex Gateway：`/v1/responses` 到 `/v1/chat/completions` 的协议转换。
- 新增 Codex App 一键绑定：自动写入 `~/.codex/config.toml`。
- 新增 Codex Restore：恢复原始 OpenAI 登录/provider 配置，并停止 Codex Gateway。
- 新增真实模型验证：Codex 页面和 Logs 可查看实际调用的 Provider 与 Upstream Model。
- 新增 Claude Alias 和 Codex Model 的自定义添加/删除。
- 修复输入框输入一个字母后失焦的问题。
- 修复 Provider Base URL 带 `/v1` 时重复拼接 `/v1/v1/...` 的问题。
- Dashboard 改为纯仪表盘：只展示状态、最近调用、刷新和健康检查，不再承担启动/绑定动作。
- 导航结构调整为 Dashboard、Products、Shared、System。

---

## 功能概览

### Dashboard

- 查看 Claude Gateway 与 Codex Gateway 的运行状态。
- 查看绑定状态与最近一次真实上游调用。
- 执行 Claude/Codex 健康检查。
- 刷新当前状态。

Dashboard 不负责启动或绑定。启动和绑定都在对应产品页完成。

### Claude

- 管理 Claude 模型别名。
- 创建 Claude 路由：`Claude Alias -> Provider -> Upstream Model`。
- 启动/停止 Claude Gateway。
- 绑定或恢复 Claude Desktop。
- 支持 Anthropic Messages API 的流式和非流式转发。

默认地址：

```text
http://127.0.0.1:3456
```

### Codex

- 管理 Codex Model 名称。
- 创建 Codex 路由：`Codex Model -> Provider -> Upstream Model`。
- 启动/停止 Codex Gateway。
- 一键绑定或恢复 Codex App。
- 将 Responses API 请求转换为 Chat Completions 请求。
- 将 Chat Completions 响应转换回 Responses 格式。
- 在页面上查看最近一次真实调用的模型。

默认地址：

```text
http://127.0.0.1:3457
```

### Providers

- 统一管理第三方 Provider。
- 支持 Base URL、Auth Header、Auth Scheme、API Key。
- 内置常见 Provider 预设。
- Claude 和 Codex 共用 Provider，但路由彼此独立。

### Logs

- 查看请求时间、请求模型、Provider、真实上游模型、状态码、耗时。
- 用于确认底层到底调用了哪个模型。

---

## 快速开始

### Claude Desktop 路由

1. 打开 `Providers`，添加一个支持 Anthropic Messages API 的 Provider。
2. 打开 `Claude`，添加或选择 Claude Alias。
3. 创建路由，填写真实上游模型名。
4. 在 `Claude` 页面启动 Claude Gateway。
5. 在 `Claude` 页面绑定 Claude Desktop。
6. 重启 Claude Desktop 后使用对应 Claude 模型。

### Codex App 路由

1. 打开 `Providers`，添加一个支持 OpenAI Chat Completions 的 Provider。
2. 打开 `Codex`，添加或选择 Codex Model，例如 `gpt-5.5`。
3. 创建路由，填写真实上游模型名。
4. 在 `Codex` 页面选择默认模型。
5. 点击 `Start & Bind Codex App`。
6. 重启 Codex App 后开始使用。

绑定后会写入：

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

## Provider 认证怎么填？

常见组合：

```text
Auth Header: Authorization
Auth Scheme: Bearer
API Key: sk-...
```

如果服务要求 `x-api-key`：

```text
Auth Header: x-api-key
Auth Scheme:
API Key: your-key
```

`Auth Scheme` 为空时，Gateway Switch 会直接把 API Key 写入对应 header。

---

## 如何验证真实模型？

在 Codex 或 Claude 中发起一次请求后：

1. 回到 Gateway Switch。
2. 打开 `Codex` 页面查看 `Verify Real Model`。
3. 或打开 `Logs` 查看更完整的历史。

重点看：

- `Requested Model`：客户端请求的模型名。
- `Provider`：实际命中的 Provider。
- `Real Upstream`：真正发给第三方 API 的模型名。

---

## 关于 Codex 思考模式

Gateway Switch 只做协议转换，不会改变模型本身能力。

如果第三方模型或接口没有通过 Chat Completions 返回 reasoning 字段，Codex 里可能看不到类似 OpenAI 原生模型的思考过程。回复很快通常是正常现象，取决于上游模型速度、接口行为和提示复杂度。

---

## 下载与安装

前往 [Releases](https://github.com/gcristiano0624-bot/gateway-switch/releases/) 下载最新 `.dmg`。

系统要求：

- macOS 12+
- Claude Desktop 或 Codex App，按需安装

---

## 从源码构建

环境要求：

- Node.js 18+
- pnpm 8+
- Rust 1.85+
- Xcode Command Line Tools

命令：

```bash
pnpm install
pnpm build
cd src-tauri && cargo test
cd ..
pnpm tauri build
```

产物：

```text
src-tauri/target/release/bundle/macos/Gateway Switch.app
src-tauri/target/release/bundle/dmg/Gateway Switch_1.3.0_aarch64.dmg
```

---

## 技术文档

完整技术细节、协议转换、数据库结构、绑定策略和发布流程见：

[docs/project.md](./docs/project.md)

---

## 数据存储

Gateway Switch 本地数据：

```text
~/Library/Application Support/Gateway Switch/
```

Claude Desktop 配置：

```text
~/Library/Application Support/Claude-3p/configLibrary/
```

Codex App 配置：

```text
~/.codex/config.toml
```

---

## 已知限制

- Claude Gateway 需要上游支持 Anthropic Messages API。
- Codex Gateway 需要上游支持 OpenAI Chat Completions API。
- Codex 的可见思考过程取决于上游是否返回 reasoning 信息。
- Codex 不同登录态/provider 下的历史会话由 Codex App 自己管理，Gateway Switch 无法强制合并。

