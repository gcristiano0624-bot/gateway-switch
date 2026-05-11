<div align="center">

# Gateway Switch

**Claude Desktop、Claude Code 与 Codex App 的第三方模型路由网关**

[![Version](https://img.shields.io/badge/Version-1.6.2-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey?style=flat-square&logo=apple)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri_2-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

[English](./README_EN.md) | 中文

</div>

---

## Gateway Switch 是什么？

Gateway Switch 是一个 macOS 桌面应用，用来把 Claude Desktop、Claude Code 和 Codex App 的模型请求转发到第三方模型 API。

它解决三类问题：

- **Claude Desktop**：Claude 会校验模型名，第三方模型不能直接伪装成 Claude 模型。Gateway Switch 提供本地 Claude 网关，把 `claude-sonnet-4-6` 等别名映射到真实上游模型；上游可以是 Anthropic Messages 兼容接口，也可以是 OpenAI Chat Completions 兼容接口。
- **Claude Code**：Claude Code 可通过本地 Gateway Route 使用统一路由，也可以通过 Direct Provider 直接绑定支持 Anthropic 协议的第三方地址。
- **Codex App**：Codex 使用 OpenAI Responses API，但很多第三方服务只支持 Chat Completions。Gateway Switch 提供本地 Responses 网关，把 Codex 请求转换为 `/v1/chat/completions`，再把响应转换回 Codex 可解析的 Responses 格式。

Provider 是公共配置，但 Base URL 按协议拆分。Codex 使用 OpenAI Base URL，Claude 与 Claude Code 优先使用 Anthropic Base URL，避免同一个地址被不同产品错误复用。

---

## 1.6.2 更新重点

- **核心修复：Codex 对话中断问题。** 当第三方模型（DeepSeek、MiMo、Qwen 等）在有 tools 可用时只生成文本描述（如"我来读取文件..."）而不调用工具，Codex 会认为本轮结束导致对话中断。现在 Gateway 会自动检测这种情况并用 `tool_choice: "required"` 重试上游请求，强制模型调用工具。
- 新增 `finish_reason` 检测：当上游返回 `finish_reason: "length"`（输出被截断）时，`response.completed` 会正确设置 `status: "incomplete"` 而非 `"completed"`，让 Codex 知道响应不完整。
- 新增流超时机制（120 秒）：上游 Provider 挂起或长时间无数据时，Gateway 不再无限等待，会主动断开并报告超时错误。
- 增强系统提示：当请求包含 tools 时，注入更强的中英文系统提示，明确列出可用工具名称，要求模型必须通过 `tool_calls` 执行操作而非仅用文本描述。
- 流错误不再伪装为正常完成：上游流中断时，`response.completed` 会设置 `status: "failed"` 并在日志中记录 `finish_reason` 信息。
- 版本统一更新为 `1.6.2`。
- 最新验证：`pnpm build` 通过，`cargo test` 通过，Rust 单测 `23 passed`。

## 1.6.1 更新重点

- 修复 Vite 浏览器预览下 Tauri `invoke()` 不存在导致的空白页问题：非 Tauri 环境会自动加载 mock 数据，方便 AI Agent 和开发者验证 UI。
- 将全量状态轮询从 3 秒调整为 12 秒，并在页面不可见时暂停，降低 IPC 与数据库读取压力。
- 增强 Codex Gateway 对第三方 Chat Completions 模型的 agent 兼容性：当 Codex 提供 tools 时，转换请求会明确要求模型输出结构化 `tool_calls`，并默认设置 `tool_choice: "auto"`。
- 修复 Codex 流式 Responses 事件中工具调用完成顺序，避免客户端先看到普通 assistant message 完成后误判任务结束。
- 对流式工具调用参数增加 JSON repair，提升小米 MiMo 等第三方模型输出宽松 JSON 时的可执行性。
- 新增 `CLAUDE.md`，明确本项目目录、构建命令和预览限制，减少后续 AI 从错误目录反复执行命令。
- 版本统一更新为 `1.6.1`。
- 最新验证：`pnpm build` 通过，`cargo test` 通过，Rust 单测 `23 passed`。

## 1.6.0 更新重点

- 升级为 Anthropic-compatible + Responses-compatible 的运行时兼容层，不再只是简单 API forwarding。
- 新增 Provider Capability Profile 与 Codex Capability Profile，用于判断 Chat、Tool Use、Vision、Reasoning、Long Context、Responses、Patch 等能力。
- Claude Gateway 增强 Anthropic Messages 与 Chat Completions fallback 的工具调用兼容，支持 tool arguments JSON repair 与 fake tool-call warning。
- Codex Gateway 增强 Responses API 兼容，支持字符串 `input`、同步 `function_call`、流式 function-call arguments events。
- 新增 Secret Redaction Engine，请求日志写入前会脱敏 API key、GitHub token、JWT、PEM 等敏感信息。
- 新增运行时安全与诊断能力：Command Safety Gate、MCP Path Safety、Patch Validator/Patch Repair、Fake Action Detector、Context Compression、Long Task State Tracker、Agent Recovery、Compatibility Benchmark、Diagnostics Export。
- 修复流式日志 request id 追踪，便于把一次请求的 provider、真实模型、耗时和错误串起来。
- 修复 Provider 保存时 `base_url` 被 `openai_base_url` 覆盖的问题。
- 版本统一更新为 `1.6.0`。

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
- 支持 OpenAI Chat Completions 上游的自动适配，适合小米等只提供 `/v1/chat/completions` 的 Provider。

默认地址：

```text
http://127.0.0.1:3456
```

### Claude Code

- 独立绑定 Claude Code，不影响 Claude Desktop。
- `Gateway Route`：写入本地 Claude Gateway，适合统一路由和 Chat Completions fallback。
- `Direct Provider`：直接写入 Provider 的 Anthropic Base URL、API Key 和模型名。
- Direct Provider 适合已经提供 Anthropic 协议接口的服务，例如配置了 `https://.../anthropic` 的 XiaoMiMo。

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
- 支持 OpenAI Base URL、Anthropic Base URL、Auth Header、Auth Scheme、API Key。
- 内置常见 Provider 预设。
- Claude、Claude Code 和 Codex 共用 Provider 身份与密钥，但按协议使用不同 Base URL。

### Logs

- 查看请求时间、请求模型、Provider、真实上游模型、状态码、耗时。
- 用于确认底层到底调用了哪个模型。
- 错误摘要会自动脱敏，避免 API Key 或 Token 进入日志。

### Runtime Compatibility

- Provider/Codex 能力画像与兼容性 benchmark。
- Tool call JSON repair 与 fake tool/action 检测。
- MCP 路径安全、命令安全、Patch 校验与修复。
- 上下文压缩、长任务状态恢复、诊断包导出。
- 这些能力主要在 Rust 后端的 `src-tauri/src/compatibility.rs` 中实现，并通过 Tauri commands 暴露。

---

## 快速开始

### Claude Desktop 路由

1. 打开 `Providers`，添加一个 Provider。OpenAI 地址填 `/v1` 或等价 Chat Completions 地址；Anthropic 地址填 `/anthropic` 或等价 Messages 地址。
2. 打开 `Claude`，添加或选择 Claude Alias。
3. 创建路由，填写真实上游模型名。
4. 在 `Claude` 页面启动 Claude Gateway。
5. 在 `Claude` 页面绑定 Claude Desktop。
6. 重启 Claude Desktop 后使用对应 Claude 模型。

### Claude Code 绑定

1. 打开 `Providers`，确认目标 Provider 已配置 Anthropic Base URL。
2. 打开 `Claude Code`。
3. 选择 `Direct Provider`，选择 Provider，并填写真实上游模型名，例如 `mimo-v2.5`。
4. 点击 `Bind Claude Code`。
5. 重启 Claude Code 或开启新会话后选择对应模型。

如果 Provider 没有 Anthropic Base URL，请改用 `Gateway Route`，让本地 Gateway 负责协议转换。

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

## Provider URL 与认证怎么填？

协议地址建议：

```text
OpenAI Base URL: https://provider.example.com/v1
Anthropic Base URL: https://provider.example.com/anthropic
```

Codex 只使用 OpenAI Base URL。Claude Code Direct Provider 只使用 Anthropic Base URL。

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

注意：Claude Desktop 绑定里显示的 `Local Gateway Auth` / `x-api-key` 是 **Claude Desktop 到本机 Gateway Switch** 的认证方式；Provider 里的 `Authorization: Bearer ...` 是 **Gateway Switch 到第三方模型服务** 的认证方式。两者属于不同链路，不需要一致。

---

## Claude、Claude Code 与 Codex 的协议区别

三个产品可以共用同一个 Provider/API Key，但它们不应该共用同一个协议地址：

- Claude 使用 `http://127.0.0.1:3456/v1/messages`，对外呈现 Anthropic Messages API。
- Claude Code Direct Provider 使用 Provider 的 Anthropic Base URL。
- Codex 使用 `http://127.0.0.1:3457/v1/responses`，对外呈现 OpenAI Responses API。

当 Claude 路由连接到只支持 Chat Completions 的上游时，Gateway Switch 会先尝试 `/v1/messages`，如果上游不支持，再自动 fallback 到 `/v1/chat/completions`。

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
src-tauri/target/release/bundle/dmg/Gateway Switch_1.6.1_aarch64.dmg
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

- Claude Code Direct Provider 需要上游支持 Anthropic Messages API。
- Codex Gateway 需要上游支持 OpenAI Chat Completions API。
- Claude Gateway 的 fallback 依赖 OpenAI Chat Completions 兼容能力。
- 1.6.0 已提供 MCP/Shell/Patch 安全 gate，但项目当前没有真实 MCP 或 Shell 执行器；后续新增执行入口时应复用这些 gate。
- Codex 的可见思考过程取决于上游是否返回 reasoning 信息。
- Codex 不同登录态/provider 下的历史会话由 Codex App 自己管理，Gateway Switch 无法强制合并。
