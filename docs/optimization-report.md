# Gateway Switch — 开发/编译/预览过程问题汇总

> 本文档记录在使用 Claude Code (Opus 4.7) 对 Gateway Switch 进行 UI 重构和打包编译过程中遇到的所有问题。
> 目标读者：后续负责优化 Gateway Switch 的 AI Agent 或开发者。

---

## 一、前端构建问题

### 1.1 根目录没有 `package.json`，`pnpm build` 无法从上级目录执行

**现象：**

ClaudeGateway 是一个 monorepo 式的项目结构，根目录 `/Users/hugoguan/Documents/trae_projects/ClaudeGateway/` 下没有 `package.json`，但有多个子项目目录（`gateway-switch`、`claude-gateway-desktop` 等）。

当 AI Agent 在根目录执行 `pnpm build` 时，pnpm 递归查找子项目，但由于根目录没有定义 `build` script，直接报错：

```
ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL  Command "build" not found
```

这个错误在会话中重复出现了 **30+ 次**，因为 Agent 不断从当前工作目录（根目录）重试同样的命令，而不是先切换到 `gateway-switch` 子目录。

**根本原因：**

- 根目录缺少 `package.json` 或 workspace 配置
- Agent 的工作目录默认是仓库根目录，而不是项目子目录

**最终解决方式：**

```bash
pnpm --dir /Users/hugoguan/Documents/trae_projects/ClaudeGateway/gateway-switch build
```

**优化建议：**

1. 在根目录添加 `package.json`，定义 workspace 和常用的 build/dev 脚本，比如：
   ```json
   {
     "scripts": {
       "build:gateway": "pnpm --dir gateway-switch build",
       "dev:gateway": "pnpm --dir gateway-switch dev"
     }
   }
   ```
2. 或者在 `CLAUDE.md` 中明确说明子项目的路径和构建命令，避免 Agent 在根目录反复试错。

---

### 1.2 TypeScript 编译报错：未使用的变量

**现象：**

在 UI 重构过程中，我将 Sidebar 的 Logo 从 `<IconLayers />` 组件替换为内联 SVG。替换后没有删除 `IconLayers` 组件的定义，导致 TypeScript 编译报错：

```
src/App.tsx(160,7): error TS6133: 'IconLayers' is declared but its value is never read.
```

**优化建议：**

1. 在 `tsconfig.json` 中可以考虑对未使用变量配置 `"noUnusedLocals": false`（当前为默认严格模式）
2. 或者在代码中保持清理习惯：替换组件引用后立即删除定义
3. CI 流程中可以加 `tsc --noEmit` 作为 pre-commit hook

---

### 1.3 Rust 编译 warning：死代码

**现象：**

Rust 编译产生 2 个 warning：

```
warning: function `count_rows` is never used
   --> src/database.rs:299:8
    |
299 | pub fn count_rows(db: &Path, table: &str) -> Result<i64, String> {
    |        ^^^^^^^^^^

warning: fields `data_dir` and `logs_dir` are never read
  --> src/state.rs:50:9
   |
49 | pub struct AppState {
   |            -------- fields in this struct
50 |     pub data_dir: PathBuf,
   |         ^^^^^^^^
53 |     pub logs_dir: PathBuf,
   |         ^^^^^^^^
```

**分析：**

- `count_rows` 函数在 `database.rs` 中定义但从未被调用，可能是预留接口
- `AppState` 的 `data_dir` 和 `logs_dir` 字段从未被读取，但 `AppState` 派生了 `Clone`，所以只是 unused read，不是 unused field

**优化建议：**

- 如果 `count_rows` 是未来预留的，加 `#[allow(dead_code)]` 并注释说明意图
- 如果不需要，直接删除
- `AppState` 的字段如果确实不需要，可以清理；如果后续会用到，也加注释说明

---

## 二、前端预览问题（Tauri 架构限制）

### 2.1 `invoke()` 在浏览器中完全不可用，页面一片空白

**现象：**

Gateway Switch 是一个 Tauri 2 桌面应用。前端 React 通过 Tauri 的 `invoke()` IPC 机制调用 Rust 后端。在浏览器（Vite dev server，`http://localhost:1420`）中预览时：

1. 页面加载后，`App.tsx` 第 353-384 行的 `loadAll()` 函数同时发起 **12 个 `invoke()` 调用**：

```typescript
const loadAll = useCallback(async () => {
  try {
    const [s, p, r, d, cc, l, cfg, cs, cr, cb, ca, cma] = await Promise.all([
      invoke<Status>("get_status"),
      invoke<Provider[]>("list_providers"),
      invoke<ModelRoute[]>("list_routes"),
      invoke<DesktopInfo>("get_desktop_info"),
      invoke<ClaudeCodeInfo>("get_claude_code_info"),
      invoke<RequestLog[]>("list_logs"),
      invoke<Settings>("get_settings"),
      invoke<CodexGatewayStatus>("get_codex_status"),
      invoke<CodexRoute[]>("list_codex_routes"),
      invoke<CodexBindingInfo>("get_codex_binding_info"),
      invoke<ModelAlias[]>("list_model_aliases", { aliasType: "claude" }),
      invoke<ModelAlias[]>("list_model_aliases", { aliasType: "codex" }),
    ]);
    // ... setState for each
  } catch (e) {
    setError(String(e));
  }
}, []);
```

2. 所有 `invoke()` 调用全部失败（`window.__TAURI_INTERNALS__` 不存在），整个 `Promise.all` reject
3. 页面因为所有 state 都是 `null`，渲染出一个只有骨架的空白页面（侧边栏和 main 都渲染了，但所有数据都是空的）
4. 错误被 `catch` 捕获后设置到 `error` state，显示一条 toast：`TypeError: Cannot read properties of undefined (reading 'invoke')`

**这是一个严重的开发体验问题。** Agent 在进行 UI 开发时，完全无法通过浏览器预览来验证 CSS 样式是否正确。

**优化建议：**

1. 在 `loadAll()` 中检测运行环境：

```typescript
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

if (!isTauri) {
  // 使用 mock 数据或显示提示
  setStatus({ gateway_running: false, gateway_port: 3456, binding_active: false, provider_count: 0, route_count: 0 });
  return;
}
```

2. 或者提供一个 `dev` 模式的 mock 层，让 UI 可以在浏览器中完整渲染

3. 或者至少在页面上显示一个醒目的提示条：`此为 Tauri 桌面应用，请使用 pnpm tauri dev 启动`

---

### 2.2 3 秒轮询 + 12 个并发 IPC = 过于激进

**现象：**

`App.tsx` 第 388-393 行：

```typescript
useEffect(() => {
  const id = window.setInterval(() => {
    void loadAll();
  }, 3000);
  return () => window.clearInterval(id);
}, [loadAll]);
```

每 3 秒调用一次 `loadAll()`，而 `loadAll()` 每次并发 12 个 `invoke()` 请求。

**影响：**

- 在正常 Tauri 运行中：每 3 秒产生 12 个 IPC 请求，对 SQLite 数据库产生不必要的读压力
- 在浏览器预览中：每 3 秒产生 12 个错误，toast 持续闪烁，页面不断重渲染
- 无法关闭或调整轮询频率

**优化建议：**

1. 轮询间隔改为可配置（默认 10-15 秒），在 Settings 页面暴露配置项
2. 使用 Tauri 的 `event.emit()` / `event.listen()` 做增量通知，替代全量轮询
3. 只轮询当前页面需要的数据，而非每次都拉取全部 12 种数据
4. 页面不可见（`document.hidden`）时暂停轮询

---

## 三、UI 响应式和排版问题

### 3.1 长 URL / 等宽文本不换行，撑破布局

**现象：**

原始 CSS 中，`url-cell`、`protocol-preview code`、`route-path`、`info-val` 等显示 URL 或等宽文本的元素使用了：

```css
.white-space: nowrap;
text-overflow: ellipsis;
```

但缺少 `word-break` / `overflow-wrap` 属性。当 URL 很长时（比如 `https://token-plan-sgp.xiaomimimo.com/anthropic`），在窄列中会直接撑破容器宽度，导致页面出现水平滚动条。

**原始 CSS 中有问题的选择器：**

```css
.url-cell {
  max-width: 300px;        /* ← 不够灵活 */
  white-space: nowrap;      /* ← 不换行 */
  overflow: hidden;         /* ← 截断 */
  text-overflow: ellipsis;
  /* 缺少 word-break */
}

.protocol-preview code {
  white-space: nowrap;      /* ← 不换行 */
  overflow: hidden;
  text-overflow: ellipsis;
  /* 缺少 word-break */
}
```

**已修复：**

我在重写的 CSS 中为所有等宽文本容器添加了：

```css
* {
  word-break: break-word;
  overflow-wrap: break-word;
}
code, pre, .mono, [style*="font-mono"] {
  word-break: break-all;
  overflow-wrap: anywhere;
}
```

**优化建议：**

1. 全局设置 `word-break: break-word` 是正确做法
2. 等宽文本（URL、model name、token）使用 `break-all` 避免溢出
3. 表格容器加 `overflow-x: auto` 允许水平滚动作为 fallback

---

### 3.2 小窗口下侧边栏消失，无替代导航

**现象：**

原始 CSS 响应式断点：

```css
@media (max-width: 700px) {
  .app-layout {
    grid-template-columns: 1fr;  /* 侧边栏列消失 */
  }
  .sidebar {
    display: none;  /* 直接隐藏 */
  }
}
```

当窗口宽度 < 700px 时，侧边栏完全隐藏，但没有提供替代的导航方式（无汉堡菜单、无底部 tab、无 command palette）。

**实际影响：**

- macOS 桌面应用窗口可以被用户缩小到很窄
- Tauri 没有设置 `minWidth`，窗口可以缩到极小尺寸
- 在 preview 环境中，preview viewport 默认宽度只有 ~547px，正好触发这个断点，导致 Agent 看到的截图只有主内容区，没有侧边栏

**优化建议：**

1. 在 `src-tauri/tauri.conf.json` 中设置最小窗口尺寸：

```json
{
  "app": {
    "windows": [{
      "minWidth": 900,
      "minHeight": 600
    }]
  }
}
```

2. 或者在 <700px 时添加一个 hamburger menu 触发侧边栏 overlay

---

### 3.3 内联 style 泛滥，覆盖性差

**现象：**

`App.tsx` 中大量使用内联 `style={{}}`：

```tsx
<div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
<div className="qa-buttons" style={{ marginTop: 16 }}>
<div className="qa-buttons" style={{ marginTop: 16, marginBottom: 0 }}>
<button className="btn" style={{ padding: "5px 8px" }}>
<td style={{ fontWeight: 600 }}>
<td style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
<div style={{ display: "flex", alignItems: "center", gap: 8, padding: "7px 12px", ... }}>
```

这些内联 style：
- 难以被全局 CSS 覆盖
- 与 CSS 变量系统不一致（比如 `#f8fafc` 硬编码颜色）
- 响应式断点无法影响内联 style
- 不同页面使用不一致的 padding/margin 值

**优化建议：**

1. 将常用的内联 style 提取为 CSS 类，比如 `.info-grid--compact`、`.btn--sm`
2. 搜索组件应该用 class 而不是内联 style
3. 颜色和间距统一使用 CSS 变量

---

## 四、TypeScript / 代码结构问题

### 4.1 所有页面函数定义在同一个组件内

**现象：**

`App.tsx` 有 **1767 行**，所有页面组件（`DashboardPage`、`ClaudePage`、`ClaudeCodePage`、`CodexPage`、`ProvidersPage`、`LogsPage`、`SettingsPage`）全部定义在 `App()` 函数内部。

```typescript
function App() {
  // ... 100+ 行 state 声明
  // ... 100+ 行 action 函数

  const Sidebar = () => (<aside>...</aside>);
  const DashboardPage = () => (<div>...</div>);
  const ProvidersPage = () => (<div>...</div>);
  const ClaudePage = () => (<div>...</div>);
  const ClaudeCodePage = () => (<div>...</div>);
  const CodexPage = () => (<div>...</div>);
  const LogsPage = () => (<div>...</div>);
  const SettingsPage = () => (<div>...</div>);

  const Content = () => {
    switch (page) {
      case "dashboard": return DashboardPage();
      // ...
    }
  };

  return (
    <div className="app-layout">
      {Sidebar()}
      <main className="main-content">{Content()}</main>
    </div>
  );
}
```

**这解释了为什么 project.md §18 特别提到 input focus bug：** 将页面函数作为嵌套 React 组件渲染（`<DashboardPage />`）会导致每次父组件 re-render 时子组件重新挂载，input 失焦。所以代码中使用了函数调用方式（`DashboardPage()`）而不是 JSX 组件方式。

**问题：**

- 1767 行的单文件难以维护
- 所有页面共享同一个闭包，state 和 action 混在一起
- 新开发者很难定位某个页面的代码
- IDE 对内部函数的跳转、重构支持较弱

**优化建议：**

1. 每个页面拆分为独立的 `.tsx` 文件（`DashboardPage.tsx`、`ClaudePage.tsx` 等）
2. 将共享 state 和 actions 提取到 `useAppContext()` 或 zustand store
3. 保持 `App.tsx` 只负责路由和布局（< 100 行）

---

### 4.2 所有类型定义也内联在 App.tsx 顶部

**现象：**

`App.tsx` 前 115 行全是类型定义：

```typescript
type Page = "dashboard" | "claude" | "claudeCode" | ...;
type CodexRoute = { ... };
type CodexGatewayStatus = { ... };
type CodexBindingInfo = { ... };
type ModelAlias = { ... };
type Status = { ... };
type Provider = { ... };
type ModelRoute = { ... };
type DesktopInfo = { ... };
type ClaudeCodeInfo = { ... };
type RequestLog = { ... };
type Settings = { ... };
type Health = { ... };
```

这些类型应该和 Rust 后端的 models.rs 对应，但分散在 TSX 文件中。

**优化建议：**

1. 提取到 `src/types.ts` 或 `src/bindings.ts`
2. 可以考虑用 `ts-rs` 或 `specta` 从 Rust 自动生成 TypeScript 类型

---

## 五、Claude Code 开发流程中的问题

### 5.1 AI Agent 反复从错误目录执行命令

**现象：**

Claude Code 的工作目录是仓库根目录 `/Users/hugoguan/Documents/trae_projects/ClaudeGateway/`，但 gateway-switch 项目在子目录 `/Users/hugoguan/Documents/trae_projects/ClaudeGateway/gateway-switch/`。

Agent 反复执行 `pnpm build`（从根目录），得到 "Command 'build' not found" 错��后，又重试同样的命令。这个循环重复了 30+ 次，浪费了大量 context window 和时间。

**原因：**

- Bash tool 的 `cwd` 默认是仓库根目录
- Agent 没有先 `cd` 到子目录
- 没有使用 `--dir` 参数
- 没有检查 `pwd` 和 `package.json` 的位置

**优化建议：**

在 `CLAUDE.md` 中明确说明项目结构和构建命令：

```markdown
## 项目结构
- `gateway-switch/` — Tauri 桌面应用（React + Rust）
  - 构建：`pnpm --dir gateway-switch build` 或 `cd gateway-switch && pnpm build`
  - 开发：`pnpm --dir gateway-switch tauri dev`
  - 打包：`pnpm --dir gateway-switch tauri build`
```

---

### 5.2 预览截图一直是白色/空白

**现象：**

Agent 使用 preview 工具（`preview_screenshot`、`preview_snapshot`）来验证 UI 修改。但：

1. Vite dev server 启动后，页面在浏览器中渲染
2. Tauri `invoke()` 失败，所有数据为 null
3. 默认 viewport 只有 ~547px 宽，触发 <700px 响应式断点
4. 侧边栏隐藏，主内容区渲染了但是很小
5. 截图看起来几乎全白

Agent 通过 `preview_snapshot`（accessibility tree）确认了内容确实存在，但无法通过视觉截图验证样式效果。

**优化建议：**

1. Tauri 应用提供 browser 模式的 mock 数据层（见 §2.1）
2. 或者创建一个独立的 `preview.html` 静态页面，用纯 HTML/CSS 展示 UI 设计
3. 或者在 `preview_start` 时自动设置合理的 viewport 尺寸（1280x900）

---

## 六、综合优化建议优先级

| 优先级 | 问题 | 建议方案 | 影响范围 |
|--------|------|----------|----------|
| **P0** | invoke() 无降级，浏览器预览完全不可用 | 添加环境检测 + mock 数据层 | 开发体验 |
| **P1** | 3 秒轮询 12 个 IPC 调用 | 改为 10-15 秒 + 增量推送 | 性能、CPU |
| **P1** | 根目录无 package.json | 添加 workspace 配置或 CLAUDE.md 说明 | 开发流程 |
| **P2** | 1767 行单文件 TSX | 拆分页面组件 + 提取 shared state | 可维护性 |
| **P2** | Rust 死代码 warning | 清理或标注 #[allow(dead_code)] | 代码质量 |
| **P2** | 内联 style 泛滥 | 提取为 CSS class | 可维护性 |
| **P3** | 小窗口无替代导航 | 设置 minWidth 或加 hamburger menu | 用户体验 |
| **P3** | URL/等宽文本溢出 | 全局 word-break + overflow-wrap | 排版 |
| **P3** | Toast 消息队列 | 改为消息队列，支持多条 | 用户体验 |
| **P3** | 类型定义分散 | 提取到 types.ts | 代码组织 |

---

## 七、相关文件参考

| 文件 | 路径 | 说明 |
|------|------|------|
| 前端主文件 | `gateway-switch/src/App.tsx` (1767 行) | 所有 UI 逻辑 |
| 前端样式 | `gateway-switch/src/App.css` (1191 行) | 全部 CSS |
| 入口 HTML | `gateway-switch/index.html` | Vite 入口 |
| Vite 配置 | `gateway-switch/vite.config.ts` | 构建配置 |
| Rust 入口 | `gateway-switch/src-tauri/src/lib.rs` | Tauri 命令注册 |
| 数据库 | `gateway-switch/src-tauri/src/database.rs` | SQLite 操作（count_rows 未使用） |
| 状态管理 | `gateway-switch/src-tauri/src/state.rs` | AppState（data_dir/logs_dir 未使用） |
| 数据模型 | `gateway-switch/src-tauri/src/models.rs` | Rust 数据结构 |
| 项目文档 | `gateway-switch/docs/project.md` | 完整技术文档 |
