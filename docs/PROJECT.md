# Gateway Switch 项目详细文档

> 本文档面向开发者和维护者，记录项目的完整技术细节、设计决策和内部实现。

---

## 1. 项目背景

### 1.1 问题描述

Claude Desktop 支持通过 Developer Gateway 模式接入第三方推理服务。但从 2025 年起，Claude Desktop 开始验证接入的模型是否为合法的 Claude 模型 ID（如 `claude-opus-4-7`、`claude-sonnet-4-6`）。这导致直接使用第三方模型时会报错。

### 1.2 解决方案

社区发现可以通过一层 **Anthropic 兼容网关** 来解决：

```
Claude Desktop
  → 请求 model: claude-sonnet-4-6
  → 本地网关（Gateway Switch）
  → 将 model 改写为真实上游模型 ID
  → 转发到第三方 Anthropic 兼容服务
  → 将响应中的 model 改回 claude-sonnet-4-6
  → 返回给 Claude Desktop
```

### 1.3 上游兼容性

Gateway Switch 只支持 **Anthropic Messages API 格式** 的上游服务。不支持 OpenAI `/v1/chat/completions` 格式。

已验证的上游服务：

| 服务 | Base URL | 认证方式 |
|------|----------|----------|
| Volcano Engine Ark | `https://ark.cn-beijing.volces.com/api/v3` | `Authorization: Bearer` |
| XiaoMiMo | `https://api.xiaomimo.com/v1` | `x-api-key` |
| OpenRouter | `https://openrouter.ai/api/v1` | `Authorization: Bearer` |
| DeepSeek | `https://api.deepseek.com/v1` | `Authorization: Bearer` |
| SiliconFlow | `https://api.siliconflow.cn/v1` | `Authorization: Bearer` |

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────┐
│                  Gateway Switch                  │
│                                                  │
│  ┌──────────────┐    ┌─────────────────────────┐ │
│  │   React UI   │←──→│    Tauri IPC Commands   │ │
│  │  (Frontend)  │    │      (21 commands)      │ │
│  └──────────────┘    └───────────┬─────────────┘ │
│                                  │               │
│         ┌────────────────────────┼───────────┐   │
│         ↓                        ↓           ↓   │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────┐ │
│  │  Gateway    │  │  Database    │  │ Desktop │ │
│  │  (axum)     │  │  (SQLite)    │  │ Binding │ │
│  │  :3456      │  │              │  │         │ │
│  └──────┬──────┘  └──────────────┘  └─────────┘ │
│         │                                        │
└─────────┼────────────────────────────────────────┘
          │
          ↓
┌───────────────────┐
│  Upstream Provider │
│  (Anthropic API)   │
└───────────────────┘
```

### 2.2 技术选型决策

| 决策 | 选择 | 原因 |
|------|------|------|
| 桌面框架 | Tauri 2 | 体积小（~15MB）、原生性能、Rust 后端 |
| 后端语言 | Rust | 类型安全、高性能、tokio 异步运行时 |
| HTTP 框架 | axum | tokio 生态、Tower 中间件支持 |
| HTTP 客户端 | reqwest | 流式响应支持、rustls TLS |
| 数据库 | SQLite (rusqlite) | 零配置、嵌入式、单文件 |
| 前端框架 | React 19 + TypeScript | 组件化、类型安全 |
| CSS 方案 | TailwindCSS 4 | 实用优先、Vite 插件集成 |
| 图标库 | Lucide React | 轻量、风格统一 |

### 2.3 数据流

**正常消息转发流程：**

```
1. Claude Desktop → POST /v1/messages (model: "claude-sonnet-4-6")
2. 网关验证 x-api-key / Authorization header
3. 从数据库查找 model_routes，匹配 claude_alias = "claude-sonnet-4-6"
4. 获取关联的 provider 信息（base_url、认证头）
5. 将请求 body 中的 model 改写为 upstream_model（如 "deepseek-v3"）
6. 转发请求到上游 provider 的 /v1/messages
7. 如果是流式响应（stream: true）：
   a. 逐行读取 SSE 事件
   b. 解析每个 data: JSON
   c. 递归重写所有 model 字段为 "claude-sonnet-4-6"
   d. 以 SSE 格式流式返回给 Claude Desktop
8. 如果是非流式响应：
   a. 解析完整 JSON 响应
   b. 递归重写 model 字段
   c. 返回给 Claude Desktop
9. 记录请求日志到数据库
```

---

## 3. 后端实现

### 3.1 模块清单

| 文件 | 职责 | 行数 |
|------|------|------|
| `main.rs` | 程序入口 | ~5 |
| `lib.rs` | Tauri Builder 注册、setup hook（自动启动） | ~65 |
| `gateway.rs` | axum 网关服务：路由、认证、转发、SSE | ~320 |
| `database.rs` | SQLite 建表、CRUD、日志 | ~230 |
| `desktop_binding.rs` | Claude-3p configLibrary 读写 | ~110 |
| `commands.rs` | 21 个 Tauri IPC 命令 | ~220 |
| `tray.rs` | 系统托盘菜单 | ~70 |
| `settings.rs` | settings.json 读写 | ~25 |
| `state.rs` | 应用状态、网关运行时管理 | ~75 |
| `models.rs` | 所有数据结构定义 | ~120 |

### 3.2 网关认证

网关支持两种认证方式，兼容不同的 Claude Desktop 配置：

```rust
// 优先检查 x-api-key header（Claude Desktop 常用方式）
if let Some(v) = headers.get("x-api-key") {
    if v == profile.auth_token { return Ok(()); }
}

// 回退到 Authorization: Bearer header
if let Some(v) = headers.get(header::AUTHORIZATION) {
    if v == format!("Bearer {}", profile.auth_token) { return Ok(()); }
}
```

### 3.3 SSE 流式处理

SSE 流式处理是核心难点。Anthropic 的 SSE 事件流格式：

```
event: message_start
data: {"type":"message_start","message":{...}}

event: content_block_start
data: {"type":"content_block_start","index":0,...}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},...}

event: message_stop
data: {"type":"message_stop"}
```

网关的处理方式：

1. 使用 `bytes_stream()` 获取上游响应的字节流
2. 按 `\n` 分割行
3. 对以 `data: ` 开头的行，解析 JSON 并递归重写所有 `model` 字段
4. 对非 `data:` 行（如 `event:` 行、空行）直接透传
5. 使用 `async-stream` 宏生成 `Body::from_stream`

### 3.4 Claude Desktop 配置管理

配置文件位置：`~/Library/Application Support/Claude-3p/configLibrary/`

关键文件：
- `_meta.json` — 包含 `appliedId` 字段，指向当前活跃的配置文件
- `{appliedId}.json` — 实际配置内容

接管时写入的字段：

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:3456/v1/messages",
  "inferenceGatewayApiKey": "gateway-switch-token",
  "inferenceGatewayAuthScheme": "x-api-key",
  "inferenceModels": [
    { "name": "claude-opus-4-7" },
    { "name": "claude-sonnet-4-6" },
    { "name": "claude-haiku-4-5" }
  ],
  "managedBy": "Gateway Switch",
  "managedAt": "2026-05-07T12:00:00Z"
}
```

恢复时从 `backups/` 子目录读取最近的备份文件覆盖当前配置。

### 3.5 Tauri IPC 命令列表

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `get_status` | - | `AppStatus` | 应用状态总览 |
| `get_settings` | - | `AppSettings` | 读取设置 |
| `save_settings` | `AppSettings` | `AppSettings` | 保存设置 |
| `get_profile` | - | `GatewayProfile` | 网关配置 |
| `list_providers` | - | `Vec<Provider>` | Provider 列表 |
| `create_provider` | `CreateProvider` | `Vec<Provider>` | 创建 Provider |
| `update_provider` | `UpdateProvider` | `Vec<Provider>` | 更新 Provider |
| `delete_provider` | `id: String` | `Vec<Provider>` | 删除 Provider |
| `list_routes` | - | `Vec<ModelRoute>` | 路由列表 |
| `create_route` | `CreateModelRoute` | `Vec<ModelRoute>` | 创建路由 |
| `update_route` | `UpdateModelRoute` | `Vec<ModelRoute>` | 更新路由 |
| `delete_route` | `id: String` | `Vec<ModelRoute>` | 删除路由 |
| `start_gateway` | - | `String` | 启动网关 |
| `stop_gateway` | - | `String` | 停止网关 |
| `list_logs` | - | `Vec<RequestLog>` | 请求日志 |
| `get_desktop_info` | - | `DesktopInfo` | Desktop 配置状态 |
| `apply_binding` | - | `DesktopInfo` | 接管 Claude Desktop |
| `restore_binding` | - | `DesktopInfo` | 恢复原始配置 |
| `check_gateway_health` | - | `HealthStatus` | 网关健康检查 |
| `check_provider_health` | `id: String` | `HealthStatus` | Provider 健康检查 |
| `export_config` | - | `String` | 导出配置到文件 |
| `import_config` | `file_path: String` | `String` | 从文件导入配置 |

---

## 4. 前端实现

### 4.1 页面结构

| 页面 | 组件 | 功能 |
|------|------|------|
| Dashboard | 内联组件 | 状态卡片、快速操作、路由摘要、健康检查 |
| Providers | 内联组件 | 预设快选、表单、CRUD 表格 |
| Routes | 内联组件 | 路由表单、Claude Alias 选择、CRUD 表格 |
| Desktop | 内联组件 | 绑定状态、模型列表、绑定/恢复按钮 |
| Logs | 内联组件 | 请求日志表格、刷新按钮 |
| Settings | 内联组件 | 网关配置表单、开关、导入/导出 |

### 4.2 Provider 预设

```typescript
const PROVIDER_PRESETS = [
  { id: "volcengine", name: "Volcano Engine Ark",
    base_url: "https://ark.cn-beijing.volces.com/api/v3",
    auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "xiaomimo", name: "XiaoMiMo",
    base_url: "https://api.xiaomimo.com/v1",
    auth_header: "x-api-key", auth_scheme: "" },
  { id: "openrouter", name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
    auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "deepseek", name: "DeepSeek",
    base_url: "https://api.deepseek.com/v1",
    auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "siliconflow", name: "SiliconFlow",
    base_url: "https://api.siliconflow.cn/v1",
    auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "custom", name: "Custom Provider",
    base_url: "", auth_header: "x-api-key", auth_scheme: "" },
];
```

### 4.3 样式系统

使用 TailwindCSS 4，通过 CSS 变量定义主题：

```css
:root {
  --bg: #f5f5f7;           /* 页面背景 */
  --sidebar-bg: #1d1d1f;   /* 侧边栏背景 */
  --card-bg: #ffffff;      /* 卡片背景 */
  --border: #e4e4e7;       /* 边框 */
  --text: #18181b;         /* 主文字 */
  --text-muted: #71717a;   /* 次要文字 */
  --accent: #2563eb;       /* 强调色 */
  --green: #16a34a;        /* 成功 */
  --red: #dc2626;          /* 错误 */
  --orange: #d97706;       /* 警告 */
}
```

---

## 5. 数据库 Schema

```sql
CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    auth_header TEXT NOT NULL DEFAULT 'x-api-key',
    auth_scheme TEXT,
    api_key TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE model_routes (
    id TEXT PRIMARY KEY,
    claude_alias TEXT NOT NULL,
    display_name TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE gateway_profile (
    id TEXT PRIMARY KEY CHECK (id = 'default'),
    listen_host TEXT NOT NULL,
    listen_port INTEGER NOT NULL,
    auth_token TEXT NOT NULL
);

CREATE TABLE request_logs (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    claude_alias TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    status_code INTEGER,
    duration_ms INTEGER,
    is_stream INTEGER NOT NULL DEFAULT 0,
    error_summary TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

---

## 6. 测试

### 6.1 自动化测试

```bash
cd src-tauri && cargo test
```

| 测试 | 覆盖 |
|------|------|
| `test_list_models_auth` | 无认证返回 401、有认证返回 200 + 正确模型列表 |
| `test_messages_model_rewrite` | 非流式消息的 model 字段从上游值重写为 Claude 别名 |
| `apply_and_restore` | 接管写入正确配置、恢复还原原始配置 |

### 6.2 手动测试清单

- [ ] 启动应用，网关自动启动
- [ ] 添加 Provider（使用预设 + 自定义 API Key）
- [ ] 创建 Model Route
- [ ] Dashboard 健康检查通过
- [ ] 绑定 Claude Desktop
- [ ] Claude Desktop 选择模型并发起对话
- [ ] 验证流式响应正常
- [ ] 查看请求日志
- [ ] 恢复 Claude Desktop 配置
- [ ] 导出/导入配置
- [ ] 系统托盘菜单操作

---

## 7. 构建与发布

### 7.1 开发模式

```bash
pnpm install
pnpm tauri dev
```

### 7.2 Release 构建

```bash
pnpm tauri build --bundles app
```

产物：`src-tauri/target/release/bundle/macos/Gateway Switch.app`

### 7.3 DMG 构建

```bash
pnpm tauri build
```

产物：`src-tauri/target/release/bundle/dmg/Gateway Switch_1.0.0_aarch64.dmg`

---

## 8. 未来规划

### 8.1 短期

- [ ] OpenAI 兼容格式转译（支持 `/v1/chat/completions` 上游）
- [ ] Provider 连接测试（发送测试请求验证 API Key）
- [ ] 路由优先级和故障切换
- [ ] 深色主题

### 8.2 中期

- [ ] 多语言支持（中文 / 英文 / 日文）
- [ ] 自动更新（Tauri updater 插件）
- [ ] 用量统计仪表盘
- [ ] MCP Server 管理

### 8.3 长期

- [ ] 云端配置同步
- [ ] 团队协作功能
- [ ] 企业级权限管理
