# Gateway Switch: Codex++ 集成 + UI 重构方案

## Context

Gateway Switch 当前是 Claude Desktop / Claude Code / Codex App 的运行时兼容性网关（v1.8.1）。用户希望：

1. **在 Codex 路由下引入 codex-plusplus 核心功能** — 不是注入到 Codex 内部，而是在 Gateway Switch 中提供 codex++ 的管理能力
2. **全面重构 UI** — 从当前的 "Claude Warm Native" 暖色风格，改为更简约的 Codex / Trae Solo 风格

## 重要前提

codex-plusplus 是一个独立的 Electron 注入系统，它 patch Codex.app 的 asar 来加载 tweak。Gateway Switch 作为 Tauri 桌面应用，**不能直接运行 codex++ tweak**，但可以：
- 检测 codex++ 是否已安装
- 读取/管理已安装的 tweak
- 提供 tweak 商店浏览和一键安装
- 提供 codex++ 健康检查
- 管理 Codex 会话历史和修复

---

## Part A: Codex++ 功能集成

### A1. Codex 页面拆分为子 Tab

将当前单一 CodexPage 拆分为多个子页面，通过 Tab 切换：

```
Codex 页面
├── [路由]     ← 现有 Codex 路由管理（Gateway、绑定、路由表）
├── [增强]     ← codex++ 页面增强管理
├── [市场]     ← codex++ 脚本市场（tweak store）
├── [会话]     ← 历史会话修复
└── [诊断]     ← codex++ 健康检查 + Gateway 诊断
```

**实现方式**：在 CodexPage 组件内增加 `codexTab` state，用 Tab bar 切换内容区域。

**关键文件**：
- `src/App.tsx` — CodexPage 组件内新增 Tab 切换逻辑
- `src/App.css` — 新增 Tab bar 样式

### A2. 增强页面 — codex++ Tweak 管理

功能：
- 检测 codex++ 是否安装（读取 `~/.codex-plusplus/state.json`）
- 列出已安装 tweak（扫描 `~/Library/Application Support/codex-plusplus/tweaks/`）
- 显示每个 tweak 的 manifest 信息（名称、版本、描述、作用域）
- 启用/禁用 tweak（修改 `config.json` 中的 tweak enable flags）
- 打开 tweak 目录（调用 Tauri `shell.open`）

**Rust 后端新增**：
- `src-tauri/src/codex_pp.rs` — codex++ 检测和管理模块
  - `detect_codex_pp() -> Option<CodexPpInstall>` — 检测安装状态
  - `list_codex_pp_tweaks() -> Vec<CodexPpTweak>` — 列出已安装 tweak
  - `set_tweak_enabled(id, enabled)` — 启用/禁用 tweak
  - `get_codex_pp_health() -> CodexPpHealth` — 健康检查（复用 codex++ 的 watcher-health 模式）
- `src-tauri/src/commands.rs` — 新增对应 Tauri commands
- `src-tauri/src/lib.rs` — 注册新模块和 commands

**数据结构**（Rust）：
```rust
struct CodexPpInstall {
    installed: bool,
    version: Option<String>,
    user_root: String,
    tweaks_dir: String,
    config_path: String,
}

struct CodexPpTweak {
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    scope: String,  // "renderer" | "main" | "both"
    icon_url: Option<String>,
    entry_exists: bool,
    enabled: bool,
}

struct CodexPpHealth {
    status: String,  // "ok" | "warn" | "error"
    title: String,
    summary: String,
    checks: Vec<HealthCheck>,
}
```

### A3. 市场页面 — Tweak Store

功能：
- 从 codex++ 的 store index URL 拉取 tweak 列表（`https://b-nnett.github.io/codex-plusplus/store/index.json`）
- 展示 tweak 卡片（名称、描述、作者、版本、标签、图标）
- 显示是否已安装及版本对比
- 一键安装 tweak（从 GitHub release 下载 tarball → 解压到 tweaks 目录）
- 安装前自动备份已有 tweak
- 搜索/过滤 tweak（按名称、标签）

**Rust 后端新增**：
- `codex_pp.rs` 扩展：
  - `fetch_tweak_store() -> Vec<StoreEntry>` — 拉取 store index
  - `install_tweak_from_store(repo, commit_sha) -> Result<()>` — 安装 tweak
  - `uninstall_tweak(id) -> Result<()>` — 卸载 tweak

**前端**：
- 新增 `TweakStorePage` 子组件
- 卡片网格布局，每行 2-3 个 tweak 卡片
- 搜索栏 + 标签过滤

### A4. 会话页面 — 历史会话修复

功能：
- 读取 Codex 的会话数据（Codex 会话存储在本地 IndexedDB 或 SQLite 中）
- 列出最近会话（时间、项目、模型、状态）
- 检测损坏/异常会话（空内容、未完成、工具调用失败等）
- 提供修复操作（清理损坏条目、重置会话状态、导出会话数据）
- 与 Gateway 请求日志关联（展示某次会话经过 Gateway 的请求链路）

**Rust 后端新增**：
- `codex_pp.rs` 扩展：
  - `list_codex_sessions() -> Vec<CodexSession>` — 列出会话
  - `detect_session_issues() -> Vec<SessionIssue>` — 检测问题
  - `repair_session(id, action) -> Result<()>` — 修复操作
  - `export_session(id) -> Result<String>` — 导出会话数据

**注意**：Codex 会话存储位置需要进一步调研确认。初步判断在 `~/.codex/` 下的某个数据库文件中，或者通过 Codex 的 Electron IPC 接口获取。如果无法直接读取，可以改为提供配置检查和手动修复指引。

### A5. 诊断页面 — 健康检查增强

功能：
- 检测 codex++ 安装状态和 watcher 健康
- 检测 Codex.app 完整性（签名、asar hash）
- 检测 launchd/systemd watcher 是否正常
- 检测 codex++ 自动更新状态
- 检测 Codex Gateway 健康（已有）
- 检测 Provider 连接性（已有）
- 一键诊断报告导出（合并 Gateway + codex++ 诊断）

**Rust 后端新增**：
- `codex_pp.rs` 扩展：
  - 检测 launchd plist（macOS）
  - 检测 Codex.app 是否被 codex++ patch
  - 检测 watcher 日志

---

## Part B: UI 重构

### B1. 设计方向 ✅ 已确认：Codex 蓝色风格 + 深色模式

从当前 "Claude Warm Native"（暖米色纸面 + 深墨文字 + Claude 暗红点睛）改为 **Codex 蓝色简约风格**，支持浅色/深色双主题。

**浅色主题配色**：
```css
:root[data-theme="light"] {
  --bg:           #FAFAFA;
  --surface:      #FFFFFF;
  --fg:           #171717;
  --fg-secondary: #525252;
  --muted:        #A3A3A3;
  --border:       #E5E5E5;
  --accent:       #2563EB;    /* Codex 蓝 */
  --accent-hover: #1D4ED8;
  --accent-light: #EFF6FF;
  --green:        #16A34A;
  --red:          #DC2626;
  --amber:        #D97706;
}
```

**深色主题配色**：
```css
:root[data-theme="dark"] {
  --bg:           #0A0A0A;
  --surface:      #171717;
  --fg:           #E5E5E5;
  --fg-secondary: #A3A3A3;
  --muted:        #737373;
  --border:       #262626;
  --accent:       #3B82F6;
  --accent-hover: #60A5FA;
  --accent-light: #1E293B;
  --green:        #22C55E;
  --red:          #EF4444;
  --amber:        #F59E0B;
}
```

**字体**：
- 移除 Fraunces 衬线字体（过于装饰性）
- 保留 Geist + Geist Mono
- 字号统一：正文 13px，标题 15px，小字 12px
- 行高收紧：1.4（当前 1.55）

**间距**：
- 卡片内边距：16px（当前更大）
- 卡片间距：12px
- 按钮 padding：6px 12px（当前更大）
- Tab 高度：36px

**圆角**：
- 统一 8px（当前 14px，偏大）
- 按钮圆角 6px

**阴影**：
- 浅色模式：大幅减弱，改用 1px border 分隔
- 深色模式：无阴影，纯靠 border 和背景色差分层

**主题切换**：
- Settings 页面增加主题切换选项（浅色/深色/跟随系统）
- 默认跟随系统 `prefers-color-scheme`
- 通过 `data-theme` 属性切换 CSS 变量

### B2. 导航结构 → 顶部 Tab 栏 ✅ 已确认

**去掉左侧 sidebar**，改为顶部水平 Tab 栏（类似 Trae Solo）：

```
┌─────────────────────────────────────────────────────────────┐
│ ⬡ Gateway Switch   Dashboard  Claude  Claude Code  Codex    │
│                    Providers  MCP Sync  Logs  Settings       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                    Main Content Area                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**实现细节**：
- 顶部栏高度：48px，左侧 logo + 应用名，右侧为 Tab 项
- Tab 项：水平排列，active 状态用蓝色下划线标识
- Tab 项按功能分组：主要（Dashboard / Claude / Claude Code / Codex）、辅助（Providers / MCP Sync）、系统（Logs / Settings / Cold Start）
- 状态指示器（Gateway 运行状态）移至顶部栏右侧，作为小圆点 + 文字
- 移除 `app-layout` 的 `grid-template-columns: 92px 1fr`，改为单列布局
- `Sidebar` 组件重构为 `TopNav` 组件

**关键文件**：
- `src/App.tsx` — `Sidebar()` → `TopNav()`
- `src/App.css` — `.sidebar` → `.topnav`，`.app-layout` 改为 `grid-template-rows: 48px 1fr`

### B3. 卡片和表单简化

- 移除 `body` 的 radial-gradient 背景装饰
- 卡片：纯白底 + 1px `#E5E5E5` border，无阴影
- 表单输入框：更紧凑，高度 32px，圆角 6px
- 按钮：更小（当前偏大），primary 用蓝色，danger 用红色
- Badge/Tag：更小更轻，pill 形状
- 健康检查条：简化为小圆点 + 文字

### B4. 页面头部简化

当前页面头部有大标题 + 描述文字，占用空间大。
改为：单行标题 + 右侧操作按钮，紧凑排列。

### B5. 响应式适配

当前没有响应式。保持 1024px+ 桌面宽度，但确保小窗口不溢出。

---

## 实施计划

### Phase 1: UI 重构基础（先做，因为后续功能都基于新 UI）
1. 重写 `App.css` — 新配色（Codex 蓝）、新间距、新字体、深色/浅色双主题 CSS 变量
2. 重构侧边栏 → 顶部 Tab 栏（`Sidebar()` → `TopNav()`）
3. 重构卡片、表单、按钮组件样式
4. 简化页面头部
5. 实现主题切换（Settings 中增加选项，`data-theme` 属性切换）
6. 确保 `pnpm build` 通过

### Phase 2: Codex 子页面结构
1. 在 CodexPage 内实现 Tab 切换
2. 将现有路由管理放入 [路由] Tab
3. 准备其他 Tab 的空壳

### Phase 3: Rust 后端 — codex++ 集成
1. 新建 `codex_pp.rs` 模块
2. 实现 codex++ 检测和 tweak 列表
3. 实现 tweak store 拉取
4. 实现 tweak 安装/卸载
5. 实现健康检查
6. 注册 Tauri commands

### Phase 4: 前端 — 各子页面实现
1. [增强] 页面 — tweak 列表 + 启用/禁用
2. [市场] 页面 — store 卡片 + 安装/卸载
3. [会话] 页面 — 会话列表 + 检测/修复
4. [诊断] 页面 — 健康检查面板

### Phase 5: 验证
1. `pnpm build` — 前端构建通过
2. `cargo test` — Rust 测试通过
3. `pnpm tauri build` — 完整打包通过
4. 手动验证 UI 各页面
5. 验证 codex++ 检测和 tweak 管理（需要 codex++ 已安装环境）

---

## 已确认的决策

| 决策 | 选择 | 说明 |
|------|------|------|
| 导航结构 | **顶部 Tab 栏** | 去掉左侧 sidebar，改为顶部水平 Tab 栏 |
| 配色风格 | **Codex 蓝色风格** | 白色/浅灰底 + 蓝色强调色（#2563EB） |
| 深色模式 | **需要支持** | 浅色/深色双主题，默认跟随系统 |

## 待确认 / 降级方案

1. **会话修复可行性**：Codex 会话存储格式未确认（Electron IndexedDB / SQLite），如果无法直接读取，降级为"配置检查 + 手动修复指引"
2. **截图**：5 张参考截图（图1-5）用户已确认会重新发送，可进一步细化 UI 方向

## 关键文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/App.css` | **重写** | 全新配色、双主题、去掉暖色装饰 |
| `src/App.tsx` | **重构** | Sidebar→TopNav、CodexPage 拆分子 Tab、深色模式逻辑 |
| `src-tauri/src/codex_pp.rs` | **新建** | codex++ 检测、tweak 管理、store、健康检查 |
| `src-tauri/src/commands.rs` | **修改** | 注册 codex++ 相关 Tauri commands |
| `src-tauri/src/lib.rs` | **修改** | 注册 codex_pp 模块 |
| `src-tauri/src/models.rs` | **修改** | 新增 codex++ 相关数据结构 |

## 验证

1. `pnpm build` — 前端构建通过
2. `PATH="$HOME/.cargo/bin:$PATH" cargo test` — Rust 测试通过
3. `CI=false PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build` — 完整打包通过
4. `pnpm tauri dev` — 手动验证各页面 UI 和功能
5. 浅色/深色主题切换验证
6. codex++ 检测和 tweak 管理验证（需要 codex++ 已安装环境）
