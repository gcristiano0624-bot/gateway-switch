<div align="center">

# Gateway Switch

**Claude Desktop、Claude Code 与 Codex App 的运行时兼容性网关**

> Gateway Switch 不仅仅是一个模型路由器。它是一个**运行时兼容性层**，驻留在 AI 原生桌面应用与第三方模型服务之间，弥合协议鸿沟、修复畸形工具调用、强制安全边界，并在上游 Provider 异常时优雅降级。

[![Version](https://img.shields.io/badge/Version-1.7.1-blue?style=flat-square)](https://github.com/gcristiano0624-bot/gateway-switch/releases)
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

## 架构与工程技术

Gateway Switch 使用 **Rust + React/Tauri** 构建，后端 Rust 代码约 2,700 行。它不只是简单的请求转发，而是一个完整的运行时兼容性层，让第三方模型在期望特定协议语义的 AI 原生客户端中稳定工作。

### 多协议运行时转换

Gateway Switch 桥接**三种不同的 API 接口**，并实现双向转换：

```
Claude Desktop ──→ Anthropic Messages API ──→ Gateway ──→ 上游 Provider
                                                  ↓
                                    Anthropic Messages  （首选）
                                    Chat Completions    （自动 fallback）

Claude Code ────→ Anthropic Messages API ──→ Gateway ──→ Anthropic Base URL

Codex App ──────→ OpenAI Responses API ──→ Gateway ──→ OpenAI Chat Completions
```

Claude 网关执行**自动协议回退**：先尝试上游 Provider 的 Anthropic Messages 端点。如果 Provider 不支持 `/v1/messages`（中国大陆 Provider 如小米 MiMo 常见情况），网关会透明地将请求转换为 OpenAI Chat Completions 格式，发送到 Provider 的 `/v1/chat/completions` 端点，再将响应转回 — 全过程对 Claude Desktop 完全不可见。

Codex 网关处理 **Responses API ↔ Chat Completions** 转换，包括流式 SSE 事件重映射（`response.created`、`response.output_text.delta`、`response.function_call_arguments.delta`、`response.completed`）、`instructions` → system message 转换、`function_call_output` → tool message 转换、`max_output_tokens` → Chat Completions token 参数映射（对 Xiaomi/MiMO 使用 `max_completion_tokens`）等。

### 工具调用修复与可靠性引擎

第三方模型经常输出畸形的工具调用 — 未加引号的 JSON key、尾部逗号、单引号、或包裹在散文中的工具参数。Gateway Switch 包含一个**多层工具调用修复流水线**：

1. **JSON 参数修复**（`repair_json_object`）：从包裹文本中提取 JSON 对象，修复未加引号的 key，将单引号转为双引号，删除尾部逗号。在同步和流式工具调用参数转发给客户端之前都会应用。

2. **伪工具调用检测**（`detect_fake_tool_call`）：识别声称调用了工具但实际没有工具块的文本 — 如 "I called the tool"、"I read the file"、"我已经调用..."。Claude 网关会在可疑的 SSE 文本 delta 上附加 `gateway_warning`。

3. **缺失工具调用重试**（`has_action_description` + `tool_choice: "required"`）：当 Codex 网关检测到模型用文本描述了计划执行的操作（"Let me read the file..."、"我来查看..."）但没有发出结构化的 `tool_calls` 时，会自动用 `tool_choice: "required"` 重试上游请求，强制模型调用工具。这修复了中国大陆模型在 Codex 中对话中断的头号原因。

4. **`finish_reason` 追踪**：网关从上游 SSE 流中解析 `finish_reason`。被截断的响应（`"length"`）会被报告为 `status: "incomplete"` 而非 `"completed"`，让 Codex 知道何时需要请求更多输出。

5. **流超时强制执行**：对上游流读取设置 120 秒超时，防止 Provider 在流式传输中途停止响应时无限挂起。

### 密钥脱敏引擎

在任何请求元数据写入本地 SQLite 日志数据库之前，**密钥脱敏引擎**会扫描错误摘要并替换敏感模式：

- OpenAI API Key（`sk-...`）
- Anthropic API Key
- GitHub Token（`ghp_...`、`gho_...`）
- JWT 类字符串
- AWS Access Key
- PEM 证书块
- 通用 Bearer Token

确保上游错误消息中意外包含的 API Key 或 Token 永远不会持久化到磁盘。

### 安全门控（MCP / Shell / Patch）

Gateway Switch 包含面向 Agent 类执行工作流的预建安全基础设施。这些门控在 `compatibility.rs` 中实现并通过 Tauri 命令暴露，为未来的执行入口做好了准备：

- **MCP 路径安全**（`mcp_path_safety`）：拦截对 `.env`、`.ssh/`、私钥文件（`id_rsa`、`id_ed25519`）、token/cookie 类路径的访问，以及路径穿越攻击和工作区根目录之外的绝对路径。

- **命令安全门控**（`command_safety`）：拦截高风险 Shell 模式 — `rm -rf`、`sudo`、递归 `chmod`、`curl | bash`、全局包安装、直接系统路径修改。

- **Patch 校验器**（`validate_patch`）：校验 unified diff 补丁的文件头、不安全路径、缺失 hunk、和格式错误的 `---`/`+++` 头。包含 **Patch 修复引擎**，可自动修复常见的头漂移（添加 `a/`/`b/` 前缀）。

- **伪动作检测器**（`detect_fake_action`）：检测声称已执行某个动作的文本（"I edited the file"、"我已经修改了..."）但没有实际执行证据。

### Provider 能力画像

Gateway Switch 自动从元数据推断 **Provider 能力画像**：

| 能力 | 检测逻辑 |
|---|---|
| Messages API | 存在 Anthropic Base URL |
| Chat Completions | 默认（所有 Provider） |
| Responses API | OpenAI 类 Provider ID |
| Tool Use | OpenAI / Qwen / Claude 类 |
| Vision | OpenAI / Qwen / Claude 类 |
| Reasoning | DeepSeek / Qwen / OpenAI |
| Streaming | 默认（所有 Provider） |
| JSON 稳定性 | 高（OpenAI/Claude）/ 中 / 低 |
| 工具调用准确度 | 高 / 中（Qwen）/ 低 |
| 最大上下文 | 32K – 128K，根据 Provider 推断 |

Claude 和 Codex 的 `/health` 端点会暴露这些画像，外部工具无需打开桌面 UI 即可检查运行时就绪状态。

### 上下文压缩与 Agent 恢复

面向长时间运行的 Agent 工作流：

- **上下文压缩**（`compress_context`）：实现带工具状态钉住的滑动窗口压缩策略。近期消息和工具相关消息被保留，较早的上下文被摘要。

- **Agent 状态恢复**（`recover_agent_state`）：从对话中重建轻量级状态对象 — 计划、已接触文件、已运行命令、已见错误、已应用补丁、和建议的下一步动作。减少 Agent 在丢失上下文后恢复工作时的长任务漂移。

### 兼容性基准测试套件

`benchmark_provider` 在 8 个维度上对 Provider 进行评级：

| 维度 | A 级 | B 级 | C 级 |
|---|---|---|---|
| Chat | 所有 Provider | — | — |
| Tool Use | OpenAI/Claude + 高准确度 | 支持工具调用 | 其他 |
| MCP | 工具 + 系统提示支持 | 支持工具调用 | 其他 |
| Artifacts | 工具 + 高 JSON 稳定性 | 中等稳定性 | 其他 |
| 长上下文 | 128K+ | 32K+ | <32K |
| Responses 兼容性 | 原生 Responses API | Chat Completions | 其他 |
| Patch 质量 | 工具 + 高 JSON 稳定性 | 支持工具调用 | 其他 |
| Agent 恢复 | 128K + 工具支持 | 32K+ | 其他 |

### 诊断导出

`export_diagnostics` 生成全面的 JSON 包，包含：运行时特性状态、所有 Provider 能力画像、基准测试结果、Provider 配置、路由配置、Codex 路由配置、和近期请求日志 — 远程复现和调试问题所需的一切。

---

## 1.7.1 更新重点

- **增强 Claude Desktop 绑定。** 绑定时会把路由显示名称写入 Claude Desktop 的 `displayName`，并默认写入 `supports1m: true`，对应新版开发者模式中的 1M context variant。
- **修复 Claude 健康检查反馈。** Claude 页面点击“检查健康状态”后会在卡片内展示健康检查结果，并显示成功或失败提示。
- **重排 Claude 与 Codex 页面。** Claude 页面按 Gateway/绑定、编辑路由、Aliases、路由卡片/暴露模型、路由表的顺序展示；Codex 页面前两行改为状态/验证、绑定/上下文说明的两列结构。
- **补齐中文界面文案。** Claude Code 运行环境、Codex 上下文与路由表单、Cold Start Doctor 诊断结果在中文模式下不再大量显示英文。
- **清理调试残留。** 正式包移除了上一轮 Codex stream-disconnect 调试埋点与临时调试文件。
- 版本更新并发布为 `1.7.1`。
- 最新验证：`pnpm build`、`PATH="$HOME/.cargo/bin:$PATH" cargo test`（29 passed）、`CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`。

## 1.7.0 更新重点

- **新增 Cold Start Doctor。** 新增冷启动检查与安全修复页，覆盖 Claude Desktop、Claude Code、Codex App、本地 Gateway、Provider、路由与安全风险检查。
- **新增中英文界面切换。** 中文为默认语言，Settings 中可切换英文；技术诊断词如 Claude、Codex、Gateway、Provider、Responses API、Chat Completions 保持英文。
- **修复 Codex 大请求 413。** Codex Responses 网关显式提高本地请求体上限，避免第二轮/多轮对话携带上下文和工具输出时被 Axum 默认限制拦截。
- **修复火山方舟 Coding Plan 404。** OpenAI Base URL 现在正确识别 `/v2`、`/v3`、`/api/coding/v3` 等版本路径；如果用户直接填写完整 `/chat/completions` 地址，也不会重复追加 `/v1`。
- **修复火山方舟 `developer` role 兼容。** Codex Responses 中的 `developer` 消息会转为 Chat Completions 的 `system` 消息，避免上游只支持 `system`、`assistant`、`user`、`tool` 时返回 400。
- 版本保持并发布为 `1.7.0`。
- 最新验证：`pnpm build`、`cargo test`（28 passed）、`CI=false pnpm tauri build --bundles app,dmg`。

## 1.6.4 更新重点

- **修复 Xiaomi MiMO Codex 路由 502 / `Param Incorrect` 问题。** 小米 MiMO 最新 OpenAI 兼容接口中，`mimo-v2.5` / `mimo-v2.5-pro` 默认启用 thinking；多轮工具调用时上游要求回传历史 `reasoning_content`。Gateway 现在对 Xiaomi/MiMO 的 Codex 转换请求默认注入 `thinking: {"type":"disabled"}`，避免 Codex 对话在工具调用链路中被上游拒绝。
- 将 Xiaomi/MiMO Codex 请求中的 `max_output_tokens` 正确映射为 `max_completion_tokens`，符合当前小米 MiMO OpenAI API 文档，而不是发送兼容性较差的 `max_tokens`。
- 如果调用方显式提供 `thinking` 控制，Gateway 会保留该设置；MiMO 兼容默认值只作用于 Xiaomi/MiMO 路由，不影响 DeepSeek、Qwen、OpenAI 等其他 Provider。
- 新增 Xiaomi/MiMO Codex 兼容性 Rust 单测，覆盖禁用 thinking 与 token 参数改名。
- 版本统一更新为 `1.6.4`。
- 最新验证：`pnpm build`、`cargo test`、`pnpm tauri build --bundles app`。

## 1.6.3 更新重点

- 全新 **Claude Warm Native** UI：采用白底、暖米色纸面背景、深墨文字、Claude 暗红点睛、低饱和状态色和更轻量的 macOS 原生工具质感。
- 左侧导航重构为窄图标栏：保留 Dashboard、Claude、Claude Code、Codex、Providers、Logs、Settings 的完整入口，悬停显示文字提示，释放主工作区空间。
- 重构 App Icon 与状态栏图标：白底图标，中间为 `Gateway Pin` 路由图案与 Claude 暗红中心点，表达多客户端请求被网关路由到正确上游 Provider。
- 前端视觉系统切换到 Geist / Fraunces / Geist Mono 字体组合，并统一卡片、表格、表单、按钮、徽标、健康状态条的 warm native 风格。
- 版本统一更新为 `1.6.3`。
- 最新验证：`pnpm build`、`PATH="$HOME/.cargo/bin:$PATH" cargo test`（29 passed）、`CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build`。

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
src-tauri/target/release/bundle/dmg/Gateway Switch_1.7.1_aarch64.dmg
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
