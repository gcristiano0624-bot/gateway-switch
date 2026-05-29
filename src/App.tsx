import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

// ── Types ──
type Page = "dashboard" | "claude" | "claudeCode" | "codex" | "mcpSync" | "coldstart" | "providers" | "logs" | "settings";
type CodexTab = "routes" | "enhance" | "market" | "sessions" | "diagnostics";
type ThemeMode = "system" | "light" | "dark";

type CodexRoute = {
  id: string;
  codex_model: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  tool_call_mode: string;
  enabled: boolean;
};

type CodexGatewayStatus = {
  running: boolean;
  status: string;
  error: string | null;
};

type CodexBindingInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  model_provider: string | null;
  model: string | null;
  base_url: string | null;
  backup_path: string | null;
};

type ModelAlias = {
  id: string;
  alias: string;
  alias_type: "claude" | "codex";
  created_at: string | null;
};

type Status = {
  gateway_running: boolean;
  gateway_port: number;
  gateway_error?: string | null;
  binding_active: boolean;
  provider_count: number;
  route_count: number;
};

type Provider = {
  id: string;
  name: string;
  base_url: string;
  openai_base_url: string;
  anthropic_base_url: string | null;
  auth_header: string;
  auth_scheme: string | null;
  api_key: string | null;
  enabled: boolean;
};

type ModelRoute = {
  id: string;
  claude_alias: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  enabled: boolean;
};

type DesktopInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  base_url: string | null;
  auth_scheme: string | null;
  models: string[];
  backup_path: string | null;
};

type ClaudeCodeInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  base_url: string | null;
  model: string | null;
  auth_env: string | null;
  backup_path: string | null;
};

type RequestLog = {
  request_id: string;
  claude_alias: string;
  provider_id: string;
  upstream_model: string;
  status_code: number | null;
  duration_ms: number | null;
  is_stream: boolean;
  error_summary: string | null;
  created_at: string;
};

type ProviderCompatibilityProfile = {
  strategy_id: string;
  system_to_user: boolean;
  tool_to_user: boolean;
  disable_tools: boolean;
  strip_unsupported_params: boolean;
  direct_provider_safe: boolean;
  gateway_route_recommended: boolean;
  summary: string;
};

type RouteCompatibilityDiagnostic = {
  route_id: string;
  claude_alias: string;
  display_name: string;
  provider_id: string;
  provider_name: string;
  upstream_model: string;
  strategy: ProviderCompatibilityProfile;
  warnings: string[];
  recommendations: string[];
};

type RoutePayloadPreview = {
  route_id: string;
  claude_alias: string;
  provider_id: string;
  upstream_model: string;
  strategy_id: string;
  roles: string[];
  payload: unknown;
};

type RuntimeSourceReport = {
  bundle_path: string;
  is_applications: boolean;
  is_dmg_volume: boolean;
  is_temp_volume: boolean;
  severity: string;
  summary: string;
  recommendation: string;
};

type Settings = {
  auto_start_gateway: boolean;
  auto_takeover_desktop: boolean;
  listen_host: string;
  listen_port: number;
  auth_token: string;
  language: "zh" | "en";
};

type CodexPpInstall = {
  installed: boolean;
  version: string | null;
  codex_version: string | null;
  app_root: string | null;
  user_root: string;
  runtime_dir: string;
  tweaks_dir: string;
  config_path: string;
  state_path: string;
  log_path: string;
  cli_path: string | null;
  auto_update: boolean;
  safe_mode: boolean;
};

type CodexPpTweak = {
  id: string;
  name: string;
  version: string;
  description: string | null;
  scope: string;
  github_repo: string | null;
  author: string | null;
  icon_url: string | null;
  tags: string[];
  permissions: string[];
  dir: string;
  manifest_path: string;
  entry_path: string | null;
  entry_exists: boolean;
  enabled: boolean;
  update_available: boolean;
  latest_version: string | null;
  release_url: string | null;
};

type CodexPpManifest = {
  id: string;
  name: string;
  version: string;
  githubRepo?: string | null;
  author?: { name?: string; url?: string } | string | null;
  description?: string | null;
  scope?: string | null;
  main?: string | null;
  iconUrl?: string | null;
  tags?: string[];
  permissions?: string[];
  minRuntime?: string | null;
};

type CodexPpStoreEntry = {
  id: string;
  manifest: CodexPpManifest;
  repo: string;
  approvedCommitSha: string;
  approvedAt?: string | null;
  approvedBy?: string | null;
  platforms?: string[] | null;
  releaseUrl?: string | null;
  reviewUrl?: string | null;
  archiveUrl?: string | null;
  installed: boolean;
  installed_version: string | null;
  installedPath?: string | null;
};

type CodexPpLegacyRecommendation = {
  name: string;
  exactMatch: boolean;
  replacementEntryId?: string | null;
  note: string;
};

type CodexPpStoreIndex = {
  schemaVersion: number;
  generatedAt: string | null;
  sourceUrl?: string | null;
  fetchedAt?: string | null;
  summary?: string | null;
  legacyRecommendations?: CodexPpLegacyRecommendation[];
  entries: CodexPpStoreEntry[];
};

type CodexPpHealthCheck = {
  name: string;
  status: string;
  detail: string;
};

type CodexPpHealth = {
  checked_at: string;
  status: string;
  title: string;
  summary: string;
  watcher: string;
  checks: CodexPpHealthCheck[];
};

type CodexPpCliResult = {
  action: string;
  command: string;
  success: boolean;
  code: number | null;
  stdout: string;
  stderr: string;
};

type CodexPpPreflightCheck = {
  name: string;
  status: string;
  detail: string;
};

type CodexPpPreflight = {
  ready: boolean;
  install_mode: string;
  summary: string;
  app_path: string | null;
  checks: CodexPpPreflightCheck[];
};

type CodexPpRecommendedScript = {
  id: string;
  name: string;
  description: string;
  file_name: string;
  status: string;
  path: string | null;
};

type CodexPpRecommendedScriptsReport = {
  storage_mode: string;
  storage_path: string | null;
  summary: string;
  scripts: CodexPpRecommendedScript[];
};

type CodexPpLogEvent = {
  session_id: string;
  stream: string;
  line: string;
};

type Health = {
  target: string;
  ok: boolean;
  message: string;
  latency_ms: number | null;
};

type ColdStartStep = {
  id: string;
  label: string;
  target: string;
  status: string;
  detail: string;
  timestamp: string;
};

type ColdStartCapability = {
  name: string;
  target: string;
  status: string;
  detail: string;
};

type ColdStartReport = {
  generated_at: string;
  mode: string;
  verdict: string;
  claude_score: number;
  codex_score: number;
  overall_score: number;
  biggest_risk: string;
  most_important_fix: string;
  report_path: string | null;
  auto_fixes_applied: string[];
  manual_fixes_required: string[];
  steps: ColdStartStep[];
  capabilities: ColdStartCapability[];
};

type McpTargetStatus = {
  target: string;
  label: string;
  config_path: string;
  config_exists: boolean;
  format: string;
  parse_status: string;
  server_count: number;
  writable: boolean;
  backup_path: string | null;
  error: string | null;
};

type McpServerPreview = {
  name: string;
  server_type: string;
  sources: string[];
  completeness: number;
  credential_keys: string[];
  action: string;
  command: string | null;
  url: string | null;
};

type McpSyncPreview = {
  generated_at: string;
  targets: McpTargetStatus[];
  merged_count: number;
  source_count: number;
  conflict_count: number;
  resolved_count: number;
  servers: McpServerPreview[];
  warnings: string[];
  can_sync: boolean;
};

type McpWriteResult = {
  target: string;
  label: string;
  ok: boolean;
  config_path: string;
  backup_path: string | null;
  message: string;
};

type McpSyncResult = {
  generated_at: string;
  preview: McpSyncPreview;
  written_targets: McpWriteResult[];
  logs: string[];
};

// ── Constants ──
const DEFAULT_CLAUDE_ALIASES = [
  "claude-opus-4-7",
  "claude-opus-4-20250514",
  "claude-opus-4-0",
  "claude-sonnet-4-6",
  "claude-sonnet-4-20250514",
  "claude-sonnet-4-5",
  "claude-sonnet-4-0",
  "claude-haiku-4-5",
  "claude-haiku-4-20250414",
  "claude-sonnet-3-7",
  "claude-sonnet-3-5-v2",
  "claude-haiku-3-5",
];

const DEFAULT_CODEX_MODELS = ["gpt-4o", "gpt-4o-mini", "o3", "o4-mini", "o3-pro", "gpt-4.1", "gpt-4.1-mini", "gpt-4.1-nano"];

const PROVIDER_PRESETS = [
  { id: "volcengine", name: "Volcano Engine", openai_base_url: "https://ark.cn-beijing.volces.com/api/v3", anthropic_base_url: "", auth_header: "Authorization", auth_scheme: "Bearer", logo: "V", color: "#ef4444", colorBg: "rgba(239,68,68,0.1)", shortUrl: "ark.cn-beijing.volces.com" },
  { id: "xiaomimo", name: "XiaoMiMo", openai_base_url: "https://token-plan-sgp.xiaomimimo.com/v1", anthropic_base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic", auth_header: "Authorization", auth_scheme: "Bearer", logo: "X", color: "#f59e0b", colorBg: "rgba(245,158,11,0.1)", shortUrl: "xiaomimimo.com" },
  { id: "openrouter", name: "OpenRouter", openai_base_url: "https://openrouter.ai/api/v1", anthropic_base_url: "", auth_header: "Authorization", auth_scheme: "Bearer", logo: "OR", color: "#6366f1", colorBg: "rgba(99,102,241,0.1)", shortUrl: "openrouter.ai" },
  { id: "deepseek", name: "DeepSeek", openai_base_url: "https://api.deepseek.com/v1", anthropic_base_url: "", auth_header: "Authorization", auth_scheme: "Bearer", logo: "DS", color: "#3b82f6", colorBg: "rgba(59,130,246,0.1)", shortUrl: "api.deepseek.com" },
  { id: "siliconflow", name: "SiliconFlow", openai_base_url: "https://api.siliconflow.cn/v1", anthropic_base_url: "", auth_header: "Authorization", auth_scheme: "Bearer", logo: "SF", color: "#8b5cf6", colorBg: "rgba(139,92,246,0.1)", shortUrl: "api.siliconflow.cn" },
  { id: "custom", name: "Custom", openai_base_url: "", anthropic_base_url: "", auth_header: "x-api-key", auth_scheme: "", logo: "+", color: "#64748b", colorBg: "rgba(100,116,139,0.1)", shortUrl: "Add your own provider" },
];

const POLL_INTERVAL_MS = 12_000;
const isTauriRuntime = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Language = "zh" | "en";

const ZH_TEXT: Record<string, string> = {
  "Dashboard": "仪表盘",
  "Claude": "Claude",
  "Claude Code": "Claude Code",
  "Codex": "Codex",
  "MCP Sync": "MCP Sync",
  "Cold Start": "冷启动",
  "Providers": "模型服务商",
  "Logs": "日志",
  "Settings": "设置",
  "Products": "产品",
  "Features": "功能",
  "General": "通用",
  "System": "系统",
  "Read-only product gateway overview": "只读的产品网关总览",
  "Claude Gateway": "Claude Gateway",
  "Codex Gateway": "Codex Gateway",
  "App Bindings": "应用绑定",
  "Routes": "路由",
  "Running": "运行中",
  "Stopped": "已停止",
  "Managed": "已接管",
  "Unmanaged": "未接管",
  "Gateway": "Gateway",
  "Binding": "绑定",
  "Last Call": "最近调用",
  "Claude Desktop uses Gateway Switch": "Claude Desktop 正在使用 Gateway Switch",
  "Claude Desktop is unmanaged": "Claude Desktop 未被接管",
  "Claude Code is unmanaged": "Claude Code 未被接管",
  "Codex App uses OpenAI login": "Codex App 正在使用 OpenAI 登录",
  "No traffic yet": "暂无流量",
  "Quick Add": "快速添加",
  "Share credentials across products, with protocol-specific base URLs for OpenAI and Anthropic clients": "跨产品共享凭据，并分别配置 OpenAI 与 Anthropic 协议的 Base URL",
  "Provider updated": "模型服务商已更新",
  "Provider created": "模型服务商已创建",
  "Provider deleted": "模型服务商已删除",
  "Route updated": "路由已更新",
  "Route created": "路由已创建",
  "Route deleted": "路由已删除",
  "Settings saved": "设置已保存",
  "MCP sync status refreshed": "MCP 同步状态已刷新",
  "MCP sync preview generated": "MCP 同步预览已生成",
  "MCP sync completed": "MCP 同步已完成",
  "Path copied": "路径已复制",
  "Config imported": "配置已导入",
  "Exported to": "已导出到",
  "Gateway started": "Gateway 已启动",
  "Gateway stopped": "Gateway 已停止",
  "Desktop bound": "Desktop 已绑定",
  "Desktop binding synced": "Desktop 绑定已同步",
  "Desktop restored": "Desktop 已恢复",
  "Claude Code bound": "Claude Code 已绑定",
  "Claude Code restored": "Claude Code 已恢复",
  "Codex gateway started": "Codex Gateway 已启动",
  "Codex gateway stopped": "Codex Gateway 已停止",
  "Codex App bound to Gateway Switch": "Codex App 已绑定到 Gateway Switch",
  "Codex restored to OpenAI login": "Codex 已恢复为 OpenAI 登录",
  "Codex route updated": "Codex 路由已更新",
  "Codex route created": "Codex 路由已创建",
  "Codex route deleted": "Codex 路由已删除",
  "Choose a default Codex model before binding": "绑定前请选择默认 Codex 模型",
  "Choose a provider for Direct Provider mode": "请选择 Direct Provider 模式使用的模型服务商",
  "Direct Provider mode needs an Anthropic Base URL on the selected provider": "Direct Provider 模式需要所选服务商配置 Anthropic Base URL",
  "Enter the real upstream model name for Claude Code": "请输入 Claude Code 使用的真实上游模型名",
  "Cold start check completed": "冷启动检查已完成",
  "Cold start repair report saved": "冷启动修复报告已保存",
  "Cold Start Doctor": "冷启动修复",
  "Claude Desktop 与 Codex 第三方模型接入后的状态确认、冷启动修复和安全报告": "Claude Desktop 与 Codex 第三方模型接入后的状态确认、冷启动修复和安全报告",
  "Run Check & Safe Fixes": "检查并执行安全修复",
  "Running...": "执行中...",
  "Phase A · Readiness Overview": "阶段 A · 状态总览",
  "Phase B · Execution & Repair Log": "阶段 B · 执行与修复日志",
  "Phase C · Capability Matrix": "阶段 C · 能力矩阵",
  "Overall": "总体",
  "Codex App": "Codex App",
  "MCP / Tools": "MCP / 工具",
  "Security": "安全",
  "Observable checks passed": "可观测检查通过",
  "Third-party routing risk": "第三方路由风险",
  "Needs binding": "需要绑定",
  "Fix Results": "修复结果",
  "Auto fixes applied": "已自动修复",
  "Manual fixes required": "需要手动处理",
  "No automatic fix has been applied in the latest check.": "最近一次检查未执行自动修复。",
  "No manual action required.": "无需手动处理。",
  "Report saved": "报告已保存",
  "Biggest Risk": "最大风险",
  "usable but needs targeted fixes": "可用，但需要针对性修复",
  "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint.": "将 Codex 绑定到 Gateway Switch，并验证本地 /v1/responses 健康检查端点。",
  "Third-party routing may expose prompts, file contents, tool results, and code to upstream providers.": "第三方路由可能会把提示词、文件内容、工具结果和代码暴露给上游服务商。",
  "Environment discovery": "环境发现",
  "Provider and route inventory": "服务商与路由盘点",
  "Codex binding check": "Codex 绑定检查",
  "Generate coldstart report": "生成冷启动报告",
  "Claude Desktop config": "Claude Desktop 配置",
  "Claude Gateway process": "Claude Gateway 进程",
  "Codex config": "Codex 配置",
  "Codex route inventory": "Codex 路由盘点",
  "Third-party routing security": "第三方路由安全",
  "Request Logs": "请求日志",
  "Monitor gateway request activity": "监控网关请求活动",
  "Search logs...": "搜索日志...",
  "Refresh": "刷新",
  "Time": "时间",
  "Requested Model": "请求模型",
  "Provider": "服务商",
  "Real Upstream": "真实上游",
  "Mode": "模式",
  "Status": "状态",
  "Duration": "耗时",
  "Error": "错误",
  "No matching logs": "没有匹配日志",
  "No logs yet": "暂无日志",
  "Try a different search query.": "请尝试其他搜索关键词。",
  "Logs will appear here once requests are made.": "发生请求后，日志会显示在这里。",
  "Configure gateway behavior and manage data": "配置网关行为并管理数据",
  "Gateway Configuration": "Gateway 配置",
  "Language": "语言",
  "Interface Language": "界面语言",
  "Chinese": "中文",
  "English": "English",
  "Listen Host": "监听 Host",
  "Listen Port": "监听端口",
  "Auth Token": "认证 Token",
  "Auto-start Gateway on launch": "启动应用时自动启动 Gateway",
  "Auto-bind Claude Desktop on launch": "启动应用时自动绑定 Claude Desktop",
  "Save Settings": "保存设置",
  "Import / Export": "导入 / 导出",
  "Import Configuration": "导入配置",
  "Export Configuration": "导出配置",
  "Export all providers, routes, and settings to a JSON file.": "将所有服务商、路由和设置导出为 JSON 文件。",
  "Import": "导入",
  "Export to File": "导出到文件",
  "Data Storage": "数据存储",
  "All data is stored under:": "所有数据存储在：",
  "Edit Provider": "编辑服务商",
  "Add Provider": "添加服务商",
  "Provider ID": "服务商 ID",
  "Display Name": "显示名称",
  "Auth Header": "认证 Header",
  "Auth Scheme": "认证方案",
  "Your API key": "你的 API Key",
  "Save": "保存",
  "Cancel": "取消",
  "Actions": "操作",
  "No providers yet": "暂无服务商",
  "Click a preset above to get started.": "点击上方预设开始配置。",
  "Route Cards": "路由卡片",
  "Route Table": "路由表",
  "Active": "启用中",
  "No routes configured": "暂无路由配置",
  "Add a route above to start mapping models.": "请先在上方添加路由来映射模型。",
  "Claude Gateway Status": "Claude Gateway 状态",
  "Configure Claude model routes and Claude Desktop binding": "配置 Claude 模型路由和 Claude Desktop 绑定",
  "Edit Route": "编辑路由",
  "Add Route": "添加路由",
  "Route ID": "路由 ID",
  "Claude Alias": "Claude 别名",
  "Upstream Model": "上游模型",
  "Check Health": "检查健康状态",
  "Start": "启动",
  "Stop": "停止",
  "Route": "路由",
  "Real Model": "真实模型",
  "Claude Desktop": "Claude Desktop",
  "Binding Status": "绑定状态",
  "Config File": "配置文件",
  "Local Gateway Auth": "本地 Gateway 认证",
  "Backup": "备份",
  "Available": "可用",
  "None": "无",
  "Bind Desktop": "绑定 Desktop",
  "Restore": "恢复",
  "Exposed Models": "暴露模型",
  "Exposed to Claude Desktop": "暴露给 Claude Desktop",
  "No models exposed": "暂无暴露模型",
  "Bind Desktop first to expose models.": "请先绑定 Desktop 后再暴露模型。",
  "Bind Claude Code independently from Claude Desktop": "独立于 Claude Desktop 绑定 Claude Code",
  "Claude Code Binding": "Claude Code 绑定",
  "Connection Mode": "连接模式",
  "Gateway Route": "Gateway 路由",
  "Direct Provider": "直连服务商",
  "Claude Code model": "Claude Code 模型",
  "Upstream model": "上游模型",
  "Bind Claude Code": "绑定 Claude Code",
  "Runtime Environment": "运行环境",
  "Codex Gateway Status": "Codex Gateway 状态",
  "OpenAI Responses API to Chat Completions API converter for Codex App and Codex CLI": "面向 Codex App 和 Codex CLI 的 OpenAI Responses API 到 Chat Completions API 转换器",
  "Verify Real Model": "验证真实模型",
  "Open Logs": "打开日志",
  "Codex App Binding": "Codex App 绑定",
  "Default model for Codex App": "Codex App 默认模型",
  "Start & Bind Codex App": "启动并绑定 Codex App",
  "Restore OpenAI Login": "恢复 OpenAI 登录",
  "Context and Reasoning Notes": "上下文与推理说明",
  "Reply speed": "回复速度",
  "Project history": "项目历史",
  "Edit Codex Route": "编辑 Codex 路由",
  "Add Codex Route": "添加 Codex 路由",
  "Codex Model (requested by Codex)": "Codex 请求的模型",
  "Upstream Model (real provider model)": "上游模型（服务商真实模型）",
  "Tool Call Mode": "工具调用模式",
  "Auto": "自动",
  "Force When Tools Present": "有工具时强制执行",
  "Strict Execution": "严格执行",
  "Keeps the model's default behavior. Best compatibility, but weak tool models may only talk.": "保留模型默认行为，兼容性最好，但弱工具模型可能只说不做。",
  "Default. When Codex sends tools, Gateway asks the upstream model to emit tool_calls first.": "默认选项。Codex 带工具时，Gateway 会要求上游模型优先输出 tool_calls。",
  "If tools are present but no tool_calls are emitted, Gateway marks the response as failed.": "如果请求带工具但上游没有输出 tool_calls，Gateway 会将响应标记为失败。",
  "Active Codex Routes": "已启用 Codex 路由",
  "No Codex routes configured": "暂无 Codex 路由",
  "Add a route above to start mapping Codex models.": "请先在上方添加路由来映射 Codex 模型。",
  "Maintain the aliases exposed to Claude Desktop and model routes.": "维护暴露给 Claude Desktop 和模型路由的别名。",
  "Maintain the model names Codex can request from this gateway.": "维护 Codex 可向此 Gateway 请求的模型名称。",
  "Add": "添加",
  "Delete": "删除",
  "Loading...": "加载中...",
  "Config": "配置",
  "Port": "端口",
  "Not configured": "未配置",
  "No providers configured": "暂无服务商配置",
  "Model": "模型",
  "Auth Env": "认证环境变量",
  "Not set": "未设置",
  "Not bound": "未绑定",
  "Disabled": "已禁用",
  "Managed by Gateway Switch": "由 Gateway Switch 接管",
  "Select provider...": "请选择服务商...",
  "Default aliases will be used until you add a custom one.": "未添加自定义别名前会使用默认别名。",
  "Claude Gateway health check passed": "Claude Gateway 健康检查通过",
  "Claude Gateway health check failed": "Claude Gateway 健康检查失败",
  "Codex Gateway health check passed": "Codex Gateway 健康检查通过",
  "Codex Gateway health check failed": "Codex Gateway 健康检查失败",
  "MCP Configuration Sync": "MCP 配置同步",
  "Synchronize MCP Servers across Claude Desktop, Claude Code, and Codex.": "在 Claude Desktop、Claude Code 与 Codex 之间同步 MCP Servers 配置。",
  "Refresh Status": "刷新状态",
  "Preview Sync": "预览同步",
  "Run Sync": "执行同步",
  "Syncing...": "同步中...",
  "Targets Ready": "三端状态",
  "Merged Servers": "合并后 Servers",
  "Conflicts": "冲突数量",
  "Last Sync": "最近同步",
  "Ready": "可同步",
  "Blocked": "阻断",
  "Sources": "来源",
  "Resolved": "已解决",
  "Not run yet": "尚未执行",
  "Written targets": "写入目标",
  "Target Configurations": "三端配置",
  "Parse Status": "解析状态",
  "Servers": "Servers",
  "Writable": "可写",
  "Exists": "存在",
  "Yes": "是",
  "No": "否",
  "Copy Path": "复制路径",
  "Sync Preview": "同步预览",
  "Server": "服务器",
  "Type": "类型",
  "Completeness": "完整度",
  "Credentials": "凭证",
  "Action": "动作",
  "No MCP servers found": "没有发现 MCP Servers",
  "Click Preview Sync to inspect merged servers before writing.": "点击预览同步，在写入前检查合并后的服务器。",
  "Safety & Warnings": "安全与警告",
  "No blocking warnings. Backups are created before writing existing files.": "没有阻断警告。写入已有文件前会自动创建备份。",
  "Execution Result": "执行结果",
  "No sync result yet.": "尚无同步结果。",
  "Write Status": "写入状态",
  "Generated at": "生成时间",
  "Claude Code will use the local Claude Gateway and configured Claude routes, including Chat Completions fallback for providers such as XiaoMiMo.": "Claude Code 将使用本地 Claude Gateway 和已配置的 Claude 路由，并支持小米 MiMO 等服务商的 Chat Completions fallback。",
  "Required for Direct Provider": "直连服务商模式必填",
  "Missing Anthropic URL": "缺少 Anthropic URL",
  "Direct Provider writes the provider's Anthropic Base URL and API key into Claude Code. Use Gateway Route when a provider only supports OpenAI Chat Completions.": "直连服务商会把该服务商的 Anthropic Base URL 和 API Key 写入 Claude Code；当服务商只支持 OpenAI Chat Completions 时，请使用 Gateway 路由。",
  "Writes `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, and `ANTHROPIC_MODEL` into `~/.claude/settings.json`. Claude Desktop binding is not touched.": "写入 `ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN` 和 `ANTHROPIC_MODEL` 到 `~/.claude/settings.json`，不会影响 Claude Desktop 绑定。",
  "Writes `ANTHROPIC_BASE_URL` from the provider's Anthropic URL. The OpenAI URL is reserved for Codex and Chat Completions fallback.": "从服务商的 Anthropic URL 写入 `ANTHROPIC_BASE_URL`；OpenAI URL 保留给 Codex 和 Chat Completions fallback 使用。",
  "Last Codex Model": "最近 Codex 模型",
  "No Codex request yet": "暂无 Codex 请求",
  "Result": "结果",
  "Trace / Error": "追踪 / 错误",
  "Default Codex provider": "默认 Codex 服务商",
  "Default Model": "默认模型",
  "Bind writes Gateway Switch into `~/.codex/config.toml` and forces API-key mode for the local gateway. Restart Codex App after binding.": "绑定会把 Gateway Switch 写入 `~/.codex/config.toml`，并强制本地网关使用 API Key 模式。绑定后请重启 Codex App。",
  "Gateway Switch converts protocol shape; it does not add or remove a model's native reasoning ability. If the upstream model is fast, or does not expose reasoning tokens through Chat Completions, the visible response can be very quick.": "Gateway Switch 只转换协议形态，不会增加或移除模型本身的推理能力。如果上游模型响应很快，或不通过 Chat Completions 暴露推理 token，界面上看到的回复可能会非常快。",
  "Binding preserves `~/.codex/config.toml` project entries. Existing Codex conversations may still be separated by Codex's own account/provider state, so switching providers can show a different conversation list even when local project trust remains intact.": "绑定会保留 `~/.codex/config.toml` 中的项目配置。已有 Codex 会话仍可能按 Codex 自身账号或服务商状态隔离，因此切换服务商后即使本地项目 trust 仍然保留，也可能看到不同的会话列表。",
  "Codex Model must match the model used by Codex CLI.": "Codex Model 必须与 Codex CLI 使用的模型名一致。",
  "If you do not need a disguised name, set Codex Model and Upstream Model to the same third-party model name.": "如果不需要伪装模型名，请把 Codex Model 和 Upstream Model 都设置为同一个第三方模型名。",
  "This is the model name used in `codex -m ...`.": "这是 `codex -m ...` 使用的模型名。",
  "This is the actual model name sent to the third-party API.": "这是发送给第三方 API 的真实模型名。",
  "ready as daily gateway environment": "已可作为日常网关环境使用",
  "not ready for unattended daily use": "尚不适合无人值守的日常使用",
  "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint": "将 Codex 绑定到 Gateway Switch，并验证本地 /v1/responses 健康检查端点",
  "Bind Claude Desktop to Gateway Switch and verify the local /v1/messages health endpoint": "将 Claude Desktop 绑定到 Gateway Switch，并验证本地 /v1/messages 健康检查端点",
  "Prove MCP/GitHub readiness inside Claude Desktop and Codex with real tool calls": "在 Claude Desktop 和 Codex 内用真实工具调用验证 MCP/GitHub 就绪状态",
  "Loaded local app state, settings path, database path, and binding targets": "已加载本地应用状态、设置路径、数据库路径和绑定目标",
  "Loaded local app state and config paths": "已加载本地应用状态和配置路径",
  "Codex is not managed yet; repair can apply a backup-backed binding": "Codex 尚未被接管；修复可以在备份后应用绑定",
  "Compiled UI report and manual remediation list": "已生成界面报告和手动修复清单",
  "Running on local health endpoint": "本地健康检查端点运行中",
  "Not managed by Gateway Switch": "未由 Gateway Switch 接管",
  "Start Claude Gateway": "启动 Claude Gateway",
  "Gateway was stopped; attempting safe start before Desktop validation": "Gateway 已停止；正在 Desktop 验证前尝试安全启动",
  "Claude Gateway start result": "Claude Gateway 启动结果",
  "Claude Gateway start failed": "Claude Gateway 启动失败",
  "Claude Gateway failed to start": "Claude Gateway 启动失败",
  "Apply Claude Desktop binding": "应用 Claude Desktop 绑定",
  "Desktop is not managed by Gateway Switch; creating backup and applying current enabled routes": "Desktop 未由 Gateway Switch 接管；正在创建备份并应用当前启用路由",
  "Applied Claude Desktop Gateway Switch binding with backup": "已在备份后应用 Claude Desktop Gateway Switch 绑定",
  "Claude Desktop binding applied": "Claude Desktop 绑定已应用",
  "Desktop config now points to local Claude Gateway": "Desktop 配置现在指向本地 Claude Gateway",
  "Claude Desktop binding failed": "Claude Desktop 绑定失败",
  "Claude Desktop binding check": "Claude Desktop 绑定检查",
  "Desktop is not managed by Gateway Switch; run repair to apply a safe backup-backed binding": "Desktop 未由 Gateway Switch 接管；请运行修复以在备份后安全绑定",
  "Claude health endpoint": "Claude 健康检查端点",
  "Claude Gateway health check": "Claude Gateway 健康检查",
  "Start Codex Gateway": "启动 Codex Gateway",
  "Codex Gateway was stopped; attempting safe start before config validation": "Codex Gateway 已停止；正在配置验证前尝试安全启动",
  "Codex Gateway start result": "Codex Gateway 启动结果",
  "Codex Gateway start failed": "Codex Gateway 启动失败",
  "Codex Gateway failed to start": "Codex Gateway 启动失败",
  "Codex Gateway process": "Codex Gateway 进程",
  "Apply Codex binding": "应用 Codex 绑定",
  "Codex is not managed by Gateway Switch; creating backup and applying current default route": "Codex 未由 Gateway Switch 接管；正在创建备份并应用当前默认路由",
  "Codex binding applied": "Codex 绑定已应用",
  "Codex config now points to local Responses Gateway": "Codex 配置现在指向本地 Responses Gateway",
  "Codex binding failed": "Codex 绑定失败",
  "Codex binding skipped": "Codex 绑定已跳过",
  "Create at least one enabled Codex route before automatic Codex binding": "自动绑定 Codex 前，请至少创建一条已启用的 Codex 路由",
  "No enabled Codex route is available": "没有可用的已启用 Codex 路由",
  "Codex is not managed by Gateway Switch; run repair to apply a backup-backed binding": "Codex 未由 Gateway Switch 接管；请运行修复以在备份后应用绑定",
  "Codex health endpoint": "Codex 健康检查端点",
  "Codex Gateway health check": "Codex Gateway 健康检查",
  "Provider inventory": "服务商盘点",
  "Claude route inventory": "Claude 路由盘点",
  "Third-party routing may expose prompts, file contents, tool results, and code to upstream providers; keep official providers as fallback for critical/private tasks": "第三方路由可能会把提示词、文件内容、工具结果和代码暴露给上游服务商；关键或私密任务建议保留官方服务商作为 fallback",
  "Review provider privacy policy and avoid sending sensitive repositories to untrusted third-party models": "检查服务商隐私政策，避免把敏感仓库发送给不可信的第三方模型",
  "Enable Auto Start Gateway if Claude Desktop should work immediately after app launch": "如果希望应用启动后 Claude Desktop 立即可用，请开启自动启动 Gateway",
  "Enable Auto Takeover Desktop if Gateway Switch should re-assert Claude Desktop binding on every launch": "如果希望每次启动时重新确认 Claude Desktop 绑定，请开启自动接管 Desktop",
  "Compiled UI report, safe-fix results, manual remediation list, and security notes": "已生成界面报告、安全修复结果、手动修复清单和安全说明",
  "Review": "需检查",
  "OK": "正常",
  "ok": "正常",
  "warn": "警告",
  "error": "错误",
  "fixed": "已修复",
  "running": "执行中",
};

function tx(text: string, language: Language): string {
  if (language !== "zh") return text;
  if (ZH_TEXT[text]) return ZH_TEXT[text];

  const inventory = text.match(/^(\d+) providers, (\d+) Claude routes, (\d+) Codex routes$/);
  if (inventory) return `${inventory[1]} 个服务商，${inventory[2]} 条 Claude 路由，${inventory[3]} 条 Codex 路由`;

  const enabled = text.match(/^(\d+) enabled (providers|Claude routes?|Codex routes?)$/);
  if (enabled) {
    const label = enabled[2] === "providers" ? "已启用服务商" : enabled[2].startsWith("Claude") ? "已启用 Claude 路由" : "已启用 Codex 路由";
    return `${enabled[1]} 个${label}`;
  }

  if (text.startsWith("path=")) return text.replace("managed=true", "已接管").replace("managed=false", "未接管").replace("base_url=not configured", "base_url=未配置");
  if (text.startsWith("status=")) return text.replace("error=none", "error=无");
  if (text.startsWith("unreachable:")) return text.replace("unreachable:", "无法连接：");
  if (text.startsWith("Claude Gateway start:")) return `Claude Gateway 启动：${text.slice("Claude Gateway start:".length).trim()}`;
  if (text.startsWith("Codex Gateway start:")) return `Codex Gateway 启动：${text.slice("Codex Gateway start:".length).trim()}`;
  if (text.startsWith("Claude Gateway failed to start:")) return `Claude Gateway 启动失败：${text.slice("Claude Gateway failed to start:".length).trim()}`;
  if (text.startsWith("Codex Gateway failed to start:")) return `Codex Gateway 启动失败：${text.slice("Codex Gateway failed to start:".length).trim()}`;
  if (text.startsWith("Claude Desktop binding failed:")) return `Claude Desktop 绑定失败：${text.slice("Claude Desktop binding failed:".length).trim()}`;
  if (text.startsWith("Codex binding failed:")) return `Codex 绑定失败：${text.slice("Codex binding failed:".length).trim()}`;
  if (text.startsWith("Applied Codex Gateway Switch binding for model")) return `已为模型 ${text.slice("Applied Codex Gateway Switch binding for model".length).trim()} 应用 Codex Gateway Switch 绑定`;

  return text;
}

function toolCallModeLabel(mode: string | undefined): string {
  if (mode === "auto") return "Auto";
  if (mode === "strict_execution") return "Strict Execution";
  return "Force When Tools Present";
}

function isToolTrace(summary: string | null | undefined): boolean {
  return Boolean(summary?.startsWith("tool_trace:"));
}

function formatLogSummary(summary: string | null | undefined): string {
  if (!summary) return "-";
  if (!isToolTrace(summary)) return summary;
  try {
    const trace = JSON.parse(summary.slice("tool_trace:".length).trim());
    const mode = toolCallModeLabel(trace.mode);
    return `tool trace · ${mode} · choice=${trace.tool_choice} · tools=${trace.request_tools} · calls=${trace.response_tool_calls} · finish=${trace.finish_reason}`;
  } catch {
    return summary;
  }
}

const MOCK_STATUS: Status = {
  gateway_running: false,
  gateway_port: 3456,
  binding_active: false,
  provider_count: 2,
  route_count: 3,
};

const MOCK_CODEX_STATUS: CodexGatewayStatus = {
  running: false,
  status: "browser-preview",
  error: null,
};

const MOCK_PROVIDERS: Provider[] = [
  {
    id: "xiaomimo",
    name: "XiaoMiMo",
    base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
    openai_base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
    anthropic_base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic",
    auth_header: "Authorization",
    auth_scheme: "Bearer",
    api_key: null,
    enabled: true,
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
    openai_base_url: "https://openrouter.ai/api/v1",
    anthropic_base_url: null,
    auth_header: "Authorization",
    auth_scheme: "Bearer",
    api_key: null,
    enabled: true,
  },
];

const MOCK_ROUTES: ModelRoute[] = [
  { id: "sonnet", claude_alias: "claude-sonnet-4-6", display_name: "Claude Sonnet", provider_id: "xiaomimo", upstream_model: "mimo-v2.5-pro", enabled: true },
  { id: "opus", claude_alias: "claude-opus-4-7", display_name: "Claude Opus", provider_id: "openrouter", upstream_model: "anthropic/claude-opus-4.1", enabled: true },
];

const MOCK_ROUTE_DIAGNOSTICS: RouteCompatibilityDiagnostic[] = [
  {
    route_id: "sonnet",
    claude_alias: "claude-sonnet-4-6",
    display_name: "Claude Sonnet",
    provider_id: "xiaomimo",
    provider_name: "XiaoMiMo",
    upstream_model: "mimo-v2.5-pro",
    strategy: {
      strategy_id: "standard_anthropic",
      system_to_user: false,
      tool_to_user: false,
      disable_tools: false,
      strip_unsupported_params: false,
      direct_provider_safe: true,
      gateway_route_recommended: false,
      summary: "Preview profile for an Anthropic-compatible provider.",
    },
    warnings: [],
    recommendations: [],
  },
];

const MOCK_PAYLOAD_PREVIEW: RoutePayloadPreview = {
  route_id: "sonnet",
  claude_alias: "claude-sonnet-4-6",
  provider_id: "xiaomimo",
  upstream_model: "mimo-v2.5-pro",
  strategy_id: "standard_anthropic",
  roles: ["system", "user", "assistant", "tool"],
  payload: { model: "mimo-v2.5-pro", messages: [{ role: "system", content: "Preview only" }] },
};

const MOCK_RUNTIME_SOURCE: RuntimeSourceReport = {
  bundle_path: "/Applications/Gateway Switch.app/Contents/MacOS/gateway-switch",
  is_applications: true,
  is_dmg_volume: false,
  is_temp_volume: false,
  severity: "ok",
  summary: "Gateway Switch is running from /Applications.",
  recommendation: "Runtime source looks stable for launchd watchers and Codex++ repair actions.",
};

const MOCK_CODEX_ROUTES: CodexRoute[] = [
  { id: "codex-mimo", codex_model: "gpt-5.2", display_name: "Codex via MiMo", provider_id: "xiaomimo", upstream_model: "mimo-v2.5-pro", tool_call_mode: "force_when_tools_present", enabled: true },
];

const MOCK_SETTINGS: Settings = {
  auto_start_gateway: true,
  auto_takeover_desktop: false,
  listen_host: "127.0.0.1",
  listen_port: 3456,
  auth_token: "gateway-switch-token",
  language: "zh",
};

const MOCK_DESKTOP: DesktopInfo = {
  config_path: "~/.claude/config.json",
  config_exists: true,
  managed: false,
  base_url: "http://127.0.0.1:3456",
  auth_scheme: "Bearer",
  models: DEFAULT_CLAUDE_ALIASES.slice(0, 4),
  backup_path: null,
};

const MOCK_CLAUDE_CODE: ClaudeCodeInfo = {
  config_path: "~/.claude/settings.json",
  config_exists: true,
  managed: false,
  base_url: "http://127.0.0.1:3456",
  model: "claude-sonnet-4-6",
  auth_env: "ANTHROPIC_AUTH_TOKEN",
  backup_path: null,
};

const MOCK_CODEX_BINDING: CodexBindingInfo = {
  config_path: "~/.codex/config.toml",
  config_exists: true,
  managed: false,
  model_provider: "gateway-switch",
  model: "gpt-5.2",
  base_url: "http://127.0.0.1:3457/v1",
  backup_path: null,
};

const MOCK_CODEX_PP_INSTALL: CodexPpInstall = {
  installed: true,
  version: "0.1.7",
  codex_version: "preview",
  app_root: "/Applications/Codex.app",
  user_root: "~/Library/Application Support/codex-plusplus",
  runtime_dir: "~/Library/Application Support/codex-plusplus/runtime",
  tweaks_dir: "~/Library/Application Support/codex-plusplus/tweaks",
  config_path: "~/Library/Application Support/codex-plusplus/config.json",
  state_path: "~/Library/Application Support/codex-plusplus/state.json",
  log_path: "~/Library/Application Support/codex-plusplus/log/main.log",
  cli_path: "/opt/homebrew/bin/codexplusplus",
  auto_update: true,
  safe_mode: false,
};

const CODEX_PP_UI_IMPROVEMENTS_TWEAK_ID = "co.bennett.ui-improvements";

const MOCK_CODEX_PP_TWEAKS: CodexPpTweak[] = [
  {
    id: "co.bennett.ui-improvements",
    name: "Bennett's UI Improvements",
    version: "1.0.3",
    description: "Quality-of-life UI tweaks for Codex: hide upgrade prompts, surface usage and message metrics.",
    scope: "both",
    github_repo: "b-nnett/codex-plusplus-bennett-ui",
    author: "bennett",
    icon_url: null,
    tags: ["ui", "usage", "upgrade"],
    permissions: ["settings"],
    dir: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.ui-improvements",
    manifest_path: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.ui-improvements/manifest.json",
    entry_path: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.ui-improvements/index.js",
    entry_exists: true,
    enabled: true,
    update_available: false,
    latest_version: null,
    release_url: null,
  },
  {
    id: "co.bennett.custom-keyboard-shortcuts",
    name: "Custom Keyboard Shortcuts",
    version: "0.1.1",
    description: "Discover, remap, and disable Codex keyboard shortcuts.",
    scope: "renderer",
    github_repo: "b-nnett/codex-plusplus-keyboard-shortcuts",
    author: "bennett",
    icon_url: null,
    tags: ["ui", "shortcuts"],
    permissions: [],
    dir: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.custom-keyboard-shortcuts",
    manifest_path: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.custom-keyboard-shortcuts/manifest.json",
    entry_path: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.custom-keyboard-shortcuts/index.js",
    entry_exists: true,
    enabled: true,
    update_available: true,
    latest_version: "0.1.2",
    release_url: "https://github.com/b-nnett/codex-plusplus-keyboard-shortcuts/releases",
  },
];

const MOCK_CODEX_PP_STORE: CodexPpStoreIndex = {
  schemaVersion: 1,
  generatedAt: "browser preview",
  sourceUrl: "https://b-nnett.github.io/codex-plusplus/store/index.json",
  fetchedAt: "browser preview",
  summary: "Preview: 2 upstream tweaks loaded. 0 of 4 legacy requested scripts matched exact upstream entries.",
  legacyRecommendations: [
    {
      name: "Codex Context Used Meter",
      exactMatch: false,
      replacementEntryId: "co.bennett.ui-improvements",
      note: "No exact upstream entry found. Bennett's UI Improvements is the closest approved tweak for hiding prompts and surfacing usage/message metrics.",
    },
    {
      name: "Hide Usage Alert",
      exactMatch: false,
      replacementEntryId: "co.bennett.ui-improvements",
      note: "No exact upstream entry found. Bennett's UI Improvements is the closest approved tweak for hiding prompts and surfacing usage/message metrics.",
    },
    {
      name: "Codex Token Usage",
      exactMatch: false,
      replacementEntryId: "co.bennett.ui-improvements",
      note: "No exact upstream entry found. Bennett's UI Improvements is the closest approved tweak for hiding prompts and surfacing usage/message metrics.",
    },
    {
      name: "Codex List Pagebuster",
      exactMatch: false,
      replacementEntryId: null,
      note: "No exact upstream registry entry found for this legacy script name.",
    },
  ],
  entries: [
    {
      id: "co.bennett.better-terminal",
      manifest: {
        id: "co.bennett.better-terminal",
        name: "Better Terminal",
        version: "1.0.0",
        githubRepo: "b-nnett/codex-plusplus-better-terminal",
        description: "Upgrades Codex terminals with split panes, native popouts, tab controls, shortcuts, and a memory watchdog.",
        author: { name: "bennett" },
        tags: ["terminal", "ui", "productivity"],
        scope: "both",
      },
      repo: "b-nnett/codex-plusplus-better-terminal",
      approvedCommitSha: "b0398c839a42134d5cb301c432d43a9f13ac22e0",
      approvedAt: "browser preview",
      approvedBy: "bennett",
      archiveUrl: "https://codeload.github.com/b-nnett/codex-plusplus-better-terminal/tar.gz/b0398c839a42134d5cb301c432d43a9f13ac22e0",
      installed: false,
      installed_version: null,
      installedPath: null,
    },
    {
      id: "co.bennett.ui-improvements",
      manifest: {
        id: "co.bennett.ui-improvements",
        name: "Bennett's UI Improvements",
        version: "1.0.3",
        githubRepo: "b-nnett/codex-plusplus-bennett-ui",
        description: "Quality-of-life UI tweaks for Codex.",
        author: { name: "bennett" },
        tags: ["ui", "usage"],
        scope: "both",
      },
      repo: "b-nnett/codex-plusplus-bennett-ui",
      approvedCommitSha: "17156ac0cc3402284b09c13c74754eda70388f50",
      approvedAt: "browser preview",
      approvedBy: "bennett",
      archiveUrl: "https://codeload.github.com/b-nnett/codex-plusplus-bennett-ui/tar.gz/17156ac0cc3402284b09c13c74754eda70388f50",
      installed: true,
      installed_version: "1.0.3",
      installedPath: "~/Library/Application Support/codex-plusplus/tweaks/co.bennett.ui-improvements",
    },
  ],
};

const MOCK_CODEX_PP_HEALTH: CodexPpHealth = {
  checked_at: "browser preview",
  status: "warn",
  title: "Codex++ needs review",
  summary: "Preview data only. Real checks run inside Tauri.",
  watcher: "launchd",
  checks: [
    { name: "Install state", status: "ok", detail: "Codex++ 0.1.7" },
    { name: "Runtime", status: "ok", detail: "~/Library/Application Support/codex-plusplus/runtime" },
    { name: "CLI", status: "ok", detail: "/opt/homebrew/bin/codexplusplus" },
    { name: "Safe mode", status: "ok", detail: "normal tweak loading" },
  ],
};

const MOCK_CODEX_PP_PREFLIGHT: CodexPpPreflight = {
  ready: true,
  install_mode: "cli",
  summary: "Preview: Gateway Switch can install or repair codex++ on this machine.",
  app_path: "/Applications/Codex.app",
  checks: [
    { name: "codexplusplus CLI", status: "ok", detail: "/opt/homebrew/bin/codexplusplus" },
    { name: "Node.js", status: "ok", detail: "v22.12.0 at /opt/homebrew/bin/node" },
    { name: "npm", status: "ok", detail: "10.9.0" },
    { name: "curl", status: "ok", detail: "curl 8.x" },
    { name: "tar", status: "ok", detail: "bsdtar 3.x" },
    { name: "Codex.app", status: "ok", detail: "/Applications/Codex.app" },
  ],
};

const MOCK_CODEX_PP_RECOMMENDED_SCRIPTS: CodexPpRecommendedScriptsReport = {
  storage_mode: "unknown",
  storage_path: null,
  summary: "Browser preview: native Codex++ user-script storage is not detected.",
  scripts: [
    {
      id: "codex-context-used-meter",
      name: "Codex Context Used Meter",
      description: "Shows Codex context usage directly in the app UI.",
      file_name: "market-codex-context-used-meter.js",
      status: "unknown",
      path: null,
    },
    {
      id: "hide-usage-alert",
      name: "Hide Usage Alert",
      description: "Hides repeated usage/quota warning banners.",
      file_name: "market-hide-usage-alert.js",
      status: "unknown",
      path: null,
    },
    {
      id: "codex-token-usage",
      name: "Codex Token Usage",
      description: "Displays token input/output/cache metrics.",
      file_name: "market-codex-token-usage.js",
      status: "unknown",
      path: null,
    },
    {
      id: "codex-list-pagebuster",
      name: "Codex List Pagebuster",
      description: "Improves the Codex session list and sidebar navigation ergonomics.",
      file_name: "market-codex-list-pagebuster.js",
      status: "unknown",
      path: null,
    },
  ],
};

const MOCK_LOGS: RequestLog[] = [
  {
    request_id: "preview-1",
    claude_alias: "gpt-5.2",
    provider_id: "xiaomimo",
    upstream_model: "mimo-v2.5-pro",
    status_code: 200,
    duration_ms: 1840,
    is_stream: true,
    error_summary: null,
    created_at: "browser preview",
  },
];

const MOCK_COLDSTART: ColdStartReport = {
  generated_at: "browser preview",
  mode: "check",
  verdict: "usable but needs targeted fixes",
  claude_score: 85,
  codex_score: 72,
  overall_score: 78,
  biggest_risk: "Third-party routing may expose prompts, file contents, tool results, and code to upstream providers.",
  most_important_fix: "Bind Codex to Gateway Switch and verify the local /v1/responses health endpoint.",
  report_path: null,
  auto_fixes_applied: ["Preview: Claude Gateway would be started if stopped"],
  manual_fixes_required: ["Preview: verify GitHub/MCP readiness inside the target desktop apps"],
  steps: [
    { id: "environment", label: "Environment discovery", target: "system", status: "ok", detail: "Loaded local app state and config paths", timestamp: "preview" },
    { id: "inventory", label: "Provider and route inventory", target: "gateway", status: "ok", detail: "2 providers, 2 Claude routes, 1 Codex route", timestamp: "preview" },
    { id: "codex_apply", label: "Codex binding check", target: "Codex", status: "warn", detail: "Codex is not managed yet; repair can apply a backup-backed binding", timestamp: "preview" },
    { id: "report", label: "Generate coldstart report", target: "system", status: "ok", detail: "Compiled UI report and manual remediation list", timestamp: "preview" },
  ],
  capabilities: [
    { name: "Claude Desktop config", target: "Claude", status: "ok", detail: "Managed by Gateway Switch" },
    { name: "Claude Gateway process", target: "Claude", status: "ok", detail: "Running on local health endpoint" },
    { name: "Codex config", target: "Codex", status: "warn", detail: "Not managed by Gateway Switch" },
    { name: "Codex route inventory", target: "Codex", status: "ok", detail: "1 enabled Codex route" },
    { name: "Third-party routing security", target: "Security", status: "warn", detail: "Review provider privacy policy before sending private repositories" },
  ],
};

const MOCK_MCP_PREVIEW: McpSyncPreview = {
  generated_at: "browser preview",
  targets: [
    {
      target: "claude_desktop",
      label: "Claude Desktop",
      config_path: "~/Library/Application Support/Claude/claude_desktop_config.json",
      config_exists: true,
      format: "JSON",
      parse_status: "正常",
      server_count: 3,
      writable: true,
      backup_path: null,
      error: null,
    },
    {
      target: "claude_code",
      label: "Claude Code",
      config_path: "~/.claude/settings.json",
      config_exists: true,
      format: "JSON",
      parse_status: "正常",
      server_count: 2,
      writable: true,
      backup_path: null,
      error: null,
    },
    {
      target: "codex",
      label: "Codex",
      config_path: "~/.codex/config.toml",
      config_exists: true,
      format: "TOML",
      parse_status: "正常",
      server_count: 2,
      writable: true,
      backup_path: null,
      error: null,
    },
  ],
  merged_count: 4,
  source_count: 3,
  conflict_count: 1,
  resolved_count: 1,
  servers: [
    { name: "filesystem", server_type: "STDIO", sources: ["claude_desktop", "claude_code"], completeness: 2, credential_keys: [], action: "同步", command: "npx", url: null },
    { name: "github", server_type: "STDIO", sources: ["claude_desktop", "codex"], completeness: 3, credential_keys: ["GITHUB_PERSONAL_ACCESS_TOKEN"], action: "冲突合并", command: "npx", url: null },
    { name: "fetch", server_type: "STDIO", sources: ["claude_code"], completeness: 2, credential_keys: [], action: "同步", command: "uvx", url: null },
    { name: "brave-search", server_type: "STDIO", sources: ["codex"], completeness: 3, credential_keys: ["BRAVE_API_KEY"], action: "同步", command: "npx", url: null },
  ],
  warnings: [],
  can_sync: true,
};

const MOCK_MCP_RESULT: McpSyncResult = {
  generated_at: "browser preview",
  preview: MOCK_MCP_PREVIEW,
  written_targets: MOCK_MCP_PREVIEW.targets.map(target => ({
    target: target.target,
    label: target.label,
    ok: true,
    config_path: target.config_path,
    backup_path: null,
    message: "已写入 4 个 MCP Servers",
  })),
  logs: [
    "Read 3 MCP sync targets",
    "Merged 4 unique MCP servers",
    "Claude Desktop: 已写入 4 个 MCP Servers",
    "Claude Code: 已写入 4 个 MCP Servers",
    "Codex: 已写入 4 个 MCP Servers",
  ],
};

// ── Helpers ──
function getModelFamily(alias: string): string {
  if (alias.includes("opus")) return "opus";
  if (alias.includes("sonnet")) return "sonnet";
  if (alias.includes("haiku")) return "haiku";
  return "sonnet";
}

function getModelAbbrev(alias: string): string {
  const f = getModelFamily(alias);
  if (f === "opus") return "Op";
  if (f === "sonnet") return "Sn";
  if (f === "haiku") return "Hk";
  return "Md";
}

// ── Inline SVG Icons ──
const IconGrid = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" /><rect x="3" y="14" width="7" height="7" /><rect x="14" y="14" width="7" height="7" />
  </svg>
);

const IconSun = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="5" /><line x1="12" y1="1" x2="12" y2="3" /><line x1="12" y1="21" x2="12" y2="23" /><line x1="4.22" y1="4.22" x2="5.64" y2="5.64" /><line x1="18.36" y1="18.36" x2="19.78" y2="19.78" /><line x1="1" y1="12" x2="3" y2="12" /><line x1="21" y1="12" x2="23" y2="12" /><line x1="4.22" y1="19.78" x2="5.64" y2="18.36" /><line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
  </svg>
);

const IconShuffle = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="16 3 21 3 21 8" /><line x1="4" y1="20" x2="21" y2="3" /><polyline points="21 16 21 21 16 21" /><line x1="15" y1="15" x2="21" y2="21" /><line x1="4" y1="4" x2="9" y2="9" />
  </svg>
);

const IconMonitor = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" /><line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />
  </svg>
);

const IconTerminal = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" />
  </svg>
);

const IconSettings = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

const IconPulse = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
  </svg>
);

const IconPlay = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

const IconStop = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="6" y="6" width="12" height="12" rx="1" />
  </svg>
);

const IconRefresh = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="23 4 23 10 17 10" /><polyline points="1 20 1 14 7 14" /><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
  </svg>
);

const IconZap = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const IconLink = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
  </svg>
);

const IconUnlink = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M18.84 12.25l1.72-1.71a5 5 0 0 0-7.07-7.07l-3 3a5 5 0 0 0-.54 6.54" /><path d="M5.16 11.75l-1.72 1.71a5 5 0 0 0 7.07 7.07l3-3a5 5 0 0 0 .54-6.54" /><line x1="2" y1="2" x2="22" y2="22" />
  </svg>
);

const IconPlus = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
  </svg>
);

const IconEdit = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
  </svg>
);

const IconTrash = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </svg>
);

const IconSearch = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
  </svg>
);

const IconCheck = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const IconX = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
  </svg>
);

const IconArrowRight = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="5" y1="12" x2="19" y2="12" /><polyline points="12 5 19 12 12 19" />
  </svg>
);

const IconDownload = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" />
  </svg>
);

const IconUpload = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" />
  </svg>
);

// ── Main App ──
function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [codexTab, setCodexTab] = useState<CodexTab>("routes");
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem("gw-theme");
    if (saved === "system" || saved === "light" || saved === "dark") return saved;
    return "system";
  });

  // Apply theme to document
  useEffect(() => {
    const apply = () => {
      const resolved = theme === "system"
        ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
        : theme;
      document.documentElement.setAttribute("data-theme", resolved);
    };
    apply();
    localStorage.setItem("gw-theme", theme);
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  const [status, setStatus] = useState<Status | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [routes, setRoutes] = useState<ModelRoute[]>([]);
  const [desktop, setDesktop] = useState<DesktopInfo | null>(null);
  const [claudeCode, setClaudeCode] = useState<ClaudeCodeInfo | null>(null);
  const [logs, setLogs] = useState<RequestLog[]>([]);
  const [routeDiagnostics, setRouteDiagnostics] = useState<RouteCompatibilityDiagnostic[]>([]);
  const [payloadPreview, setPayloadPreview] = useState<RoutePayloadPreview | null>(null);
  const [runtimeSource, setRuntimeSource] = useState<RuntimeSourceReport | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [codexHealth, setCodexHealth] = useState<Health | null>(null);
  const [coldStart, setColdStart] = useState<ColdStartReport | null>(null);
  const [coldStartRunning, setColdStartRunning] = useState(false);
  const [mcpPreview, setMcpPreview] = useState<McpSyncPreview | null>(null);
  const [mcpResult, setMcpResult] = useState<McpSyncResult | null>(null);
  const [mcpLoading, setMcpLoading] = useState(false);
  const [mcpSyncing, setMcpSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  // Codex state
  const [codexRoutes, setCodexRoutes] = useState<CodexRoute[]>([]);
  const [codexStatus, setCodexStatus] = useState<CodexGatewayStatus | null>(null);
  const [codexBinding, setCodexBinding] = useState<CodexBindingInfo | null>(null);
  const [codexPpInstall, setCodexPpInstall] = useState<CodexPpInstall | null>(null);
  const [codexPpTweaks, setCodexPpTweaks] = useState<CodexPpTweak[]>([]);
  const [codexPpStore, setCodexPpStore] = useState<CodexPpStoreIndex | null>(null);
  const [codexPpHealth, setCodexPpHealth] = useState<CodexPpHealth | null>(null);
  const [codexPpPreflight, setCodexPpPreflight] = useState<CodexPpPreflight | null>(null);
  const [codexPpRecommendedScripts, setCodexPpRecommendedScripts] = useState<CodexPpRecommendedScriptsReport | null>(null);
  const [codexPpCli, setCodexPpCli] = useState<CodexPpCliResult | null>(null);
  const [codexPpLogLines, setCodexPpLogLines] = useState<string[]>([]);
  const [codexPpLoading, setCodexPpLoading] = useState(false);
  const [codexPpSearch, setCodexPpSearch] = useState("");
  const [codexBindModel, setCodexBindModel] = useState("");
  const [claudeAliases, setClaudeAliases] = useState<ModelAlias[]>([]);
  const [codexAliases, setCodexAliases] = useState<ModelAlias[]>([]);
  const [newClaudeAlias, setNewClaudeAlias] = useState("");
  const [newCodexAlias, setNewCodexAlias] = useState("");
  const codexPort = 3457;
  const [cForm, setCForm] = useState({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "", tool_call_mode: "force_when_tools_present" });
  const [editingC, setEditingC] = useState<string | null>(null);
  const [ccMode, setCcMode] = useState<"gateway" | "provider">("gateway");
  const [ccModel, setCcModel] = useState("claude-sonnet-4-6");
  const [ccProviderId, setCcProviderId] = useState("");
  const [ccUpstreamModel, setCcUpstreamModel] = useState("");

  // Provider form
  const emptyProviderForm = { id: "", name: "", base_url: "", openai_base_url: "", anthropic_base_url: "", auth_header: "x-api-key", auth_scheme: "", api_key: "" };
  const [pForm, setPForm] = useState(emptyProviderForm);
  const [editingP, setEditingP] = useState<string | null>(null);

  // Route form
  const [rForm, setRForm] = useState({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" });
  const [editingR, setEditingR] = useState<string | null>(null);

  // Settings
  const [importPath, setImportPath] = useState("");
  const claudeAliasOptions = claudeAliases.length > 0 ? claudeAliases.map(a => a.alias) : DEFAULT_CLAUDE_ALIASES;
  const codexModelOptions = codexAliases.length > 0 ? codexAliases.map(a => a.alias) : DEFAULT_CODEX_MODELS;
  const latestCodexLog = logs.find(l => codexRoutes.some(r => r.codex_model === l.claude_alias));
  const latestClaudeLog = logs.find(l => routes.some(r => r.claude_alias === l.claude_alias));
  const language: Language = settings?.language === "en" ? "en" : "zh";
  const t = (text: string) => tx(text, language);

  const flash = (msg: string, type: "success" | "error" = "success") => {
    const translated = msg.startsWith("Exported to ")
      ? `${t("Exported to")} ${msg.slice("Exported to ".length)}`
      : msg.startsWith("Cold start repair report saved:")
        ? `${t("Cold start repair report saved")}: ${msg.slice("Cold start repair report saved:".length).trim()}`
        : msg.startsWith("Claude Gateway health check failed:")
          ? `${t("Claude Gateway health check failed")}: ${msg.slice("Claude Gateway health check failed:".length).trim()}`
          : msg.startsWith("Codex Gateway health check failed:")
            ? `${t("Codex Gateway health check failed")}: ${msg.slice("Codex Gateway health check failed:".length).trim()}`
            : t(msg);
    if (type === "success") { setSuccess(translated); setError(null); }
    else { setError(translated); setSuccess(null); }
    setTimeout(() => { setSuccess(null); setError(null); }, 4000);
  };

  const loadAll = useCallback(async () => {
    if (!isTauriRuntime) {
      setStatus(MOCK_STATUS);
      setProviders(MOCK_PROVIDERS);
      setRoutes(MOCK_ROUTES);
      setDesktop(MOCK_DESKTOP);
      setClaudeCode(MOCK_CLAUDE_CODE);
      setLogs(MOCK_LOGS);
      setRouteDiagnostics(MOCK_ROUTE_DIAGNOSTICS);
      setPayloadPreview(MOCK_PAYLOAD_PREVIEW);
      setRuntimeSource(MOCK_RUNTIME_SOURCE);
      setSettings(MOCK_SETTINGS);
      setCodexStatus(MOCK_CODEX_STATUS);
      setCodexRoutes(MOCK_CODEX_ROUTES);
      setCodexBinding(MOCK_CODEX_BINDING);
      setColdStart(MOCK_COLDSTART);
      setMcpPreview(MOCK_MCP_PREVIEW);
      setCodexPpInstall(MOCK_CODEX_PP_INSTALL);
      setCodexPpTweaks(MOCK_CODEX_PP_TWEAKS);
      setCodexPpStore(MOCK_CODEX_PP_STORE);
      setCodexPpHealth(MOCK_CODEX_PP_HEALTH);
      setCodexPpPreflight(MOCK_CODEX_PP_PREFLIGHT);
      setCodexPpRecommendedScripts(MOCK_CODEX_PP_RECOMMENDED_SCRIPTS);
      setClaudeAliases(DEFAULT_CLAUDE_ALIASES.map((alias, index) => ({ id: `mock-claude-${index}`, alias, alias_type: "claude", created_at: null })));
      setCodexAliases(DEFAULT_CODEX_MODELS.map((alias, index) => ({ id: `mock-codex-${index}`, alias, alias_type: "codex", created_at: null })));
      return;
    }

    try {
      const [s, p, r, d, cc, l, rd, rs, cfg, cs, cr, cb, cold, mcp, ca, cma, cppInstall, cppTweaks, cppHealth, cppPreflight, cppScripts] = await Promise.all([
        invoke<Status>("get_status"),
        invoke<Provider[]>("list_providers"),
        invoke<ModelRoute[]>("list_routes"),
        invoke<DesktopInfo>("get_desktop_info"),
        invoke<ClaudeCodeInfo>("get_claude_code_info"),
        invoke<RequestLog[]>("list_logs"),
        invoke<RouteCompatibilityDiagnostic[]>("get_route_diagnostics"),
        invoke<RuntimeSourceReport>("get_runtime_source_report"),
        invoke<Settings>("get_settings"),
        invoke<CodexGatewayStatus>("get_codex_status"),
        invoke<CodexRoute[]>("list_codex_routes"),
        invoke<CodexBindingInfo>("get_codex_binding_info"),
        invoke<ColdStartReport>("get_coldstart_status"),
        invoke<McpSyncPreview>("get_mcp_sync_status"),
        invoke<ModelAlias[]>("list_model_aliases", { aliasType: "claude" }),
        invoke<ModelAlias[]>("list_model_aliases", { aliasType: "codex" }),
        invoke<CodexPpInstall>("detect_codex_pp"),
        invoke<CodexPpTweak[]>("list_codex_pp_tweaks"),
        invoke<CodexPpHealth>("get_codex_pp_health"),
        invoke<CodexPpPreflight>("get_codex_pp_preflight"),
        invoke<CodexPpRecommendedScriptsReport>("get_codex_pp_recommended_scripts"),
      ]);
      setStatus(s);
      setProviders(p);
      setRoutes(r);
      setDesktop(d);
      setClaudeCode(cc);
      setLogs(l);
      setRouteDiagnostics(rd);
      setRuntimeSource(rs);
      setSettings(cfg);
      setCodexStatus(cs);
      setCodexRoutes(cr);
      setCodexBinding(cb);
      setColdStart(cold);
      setMcpPreview(mcp);
      setClaudeAliases(ca);
      setCodexAliases(cma);
      setCodexPpInstall(cppInstall);
      setCodexPpTweaks(cppTweaks);
      setCodexPpHealth(cppHealth);
      setCodexPpPreflight(cppPreflight);
      setCodexPpRecommendedScripts(cppScripts);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { void loadAll(); }, [loadAll]);

  useEffect(() => {
    const id = window.setInterval(() => {
      if (!document.hidden) void loadAll();
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [loadAll]);

  useEffect(() => {
    const firstModel = codexRoutes.find(r => r.enabled)?.codex_model ?? codexModelOptions[0] ?? "";
    if (!codexBindModel && firstModel) setCodexBindModel(firstModel);
  }, [codexBindModel, codexModelOptions, codexRoutes]);

  useEffect(() => {
    const firstClaudeModel = routes.find(r => r.enabled)?.claude_alias ?? claudeAliasOptions[0] ?? "";
    if (!ccModel && firstClaudeModel) setCcModel(firstClaudeModel);
  }, [ccModel, claudeAliasOptions, routes]);

  useEffect(() => {
    if (page === "codex" && codexTab === "market" && !codexPpStore && !codexPpLoading) {
      void refreshCodexPp(true);
    }
  }, [page, codexTab, codexPpStore, codexPpLoading]);

  // ---- Actions ----
  const startGw = async () => {
    try {
      await invoke("start_gateway");
      await loadAll();
      await checkHealth();
    } catch (e) { flash(String(e), "error"); }
  };
  const stopGw = async () => { try { await invoke("stop_gateway"); await loadAll(); flash("Gateway stopped"); } catch (e) { flash(String(e), "error"); } };
  const checkHealth = async () => {
    try {
      const h = await invoke<Health>("check_gateway_health");
      setHealth(h);
      flash(h.ok ? "Claude Gateway health check passed" : `Claude Gateway health check failed: ${h.message}`, h.ok ? "success" : "error");
    } catch (e) { flash(String(e), "error"); }
  };
  const checkCodexHealth = async () => {
    try {
      const h = await invoke<Health>("check_codex_health");
      setCodexHealth(h);
      flash(h.ok ? "Codex Gateway health check passed" : `Codex Gateway health check failed: ${h.message}`, h.ok ? "success" : "error");
    } catch (e) { flash(String(e), "error"); }
  };
  const bindDesktop = async () => { try { await invoke("apply_binding"); await loadAll(); flash("Desktop bound"); } catch (e) { flash(String(e), "error"); } };
  const restoreDesktop = async () => { try { await invoke("restore_binding"); await loadAll(); flash("Desktop restored"); } catch (e) { flash(String(e), "error"); } };
  const syncDesktopBindingIfManaged = async () => {
    if (!desktop?.managed) return;
    const info = await invoke<DesktopInfo>("apply_binding");
    setDesktop(info);
    flash("Desktop binding synced");
  };
  const bindClaudeCode = async () => {
    try {
      if (ccMode === "provider") {
        const provider = providers.find(p => p.id === ccProviderId);
        if (!provider) {
          flash("Choose a provider for Direct Provider mode", "error");
          return;
        }
        if (!provider.anthropic_base_url) {
          flash("Direct Provider mode needs an Anthropic Base URL on the selected provider", "error");
          return;
        }
        if (!ccUpstreamModel.trim()) {
          flash("Enter the real upstream model name for Claude Code", "error");
          return;
        }
        if (needsClaudeCodeGatewayRoute(provider, ccUpstreamModel)) {
          flash("Volcengine DeepSeek is not safe for Claude Code Direct Provider mode. Use Gateway Route so Gateway Switch can merge system/tool roles.", "error");
          return;
        }
      }
      const payload = ccMode === "gateway"
        ? { mode: "gateway", model: ccModel }
        : { mode: "provider", model: ccUpstreamModel, provider_id: ccProviderId, upstream_model: ccUpstreamModel };
      const info = await invoke<ClaudeCodeInfo>("apply_claude_code_binding", { payload });
      setClaudeCode(info);
      await loadAll();
      flash("Claude Code bound");
    } catch (e) { flash(String(e), "error"); }
  };
  const loadPayloadPreview = async (claudeAlias: string) => {
    try {
      const preview = isTauriRuntime
        ? await invoke<RoutePayloadPreview>("preview_route_payload", { claudeAlias })
        : MOCK_PAYLOAD_PREVIEW;
      setPayloadPreview(preview);
      flash("Payload preview generated");
    } catch (e) {
      flash(String(e), "error");
    }
  };
  const restoreClaudeCode = async () => {
    try {
      const info = await invoke<ClaudeCodeInfo>("restore_claude_code_binding");
      setClaudeCode(info);
      await loadAll();
      flash("Claude Code restored");
    } catch (e) { flash(String(e), "error"); }
  };

  const refreshMcpStatus = async () => {
    setMcpLoading(true);
    try {
      const preview = isTauriRuntime ? await invoke<McpSyncPreview>("get_mcp_sync_status") : MOCK_MCP_PREVIEW;
      setMcpPreview(preview);
      flash("MCP sync status refreshed");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setMcpLoading(false);
    }
  };

  const previewMcpSync = async () => {
    setMcpLoading(true);
    try {
      const preview = isTauriRuntime ? await invoke<McpSyncPreview>("preview_mcp_sync") : MOCK_MCP_PREVIEW;
      setMcpPreview(preview);
      flash("MCP sync preview generated");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setMcpLoading(false);
    }
  };

  const runMcpSync = async () => {
    setMcpSyncing(true);
    try {
      const result = isTauriRuntime ? await invoke<McpSyncResult>("run_mcp_sync") : MOCK_MCP_RESULT;
      setMcpResult(result);
      setMcpPreview(result.preview);
      flash("MCP sync completed");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setMcpSyncing(false);
    }
  };

  const copyPath = async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      flash("Path copied");
    } catch {
      flash(path);
    }
  };

  // Provider CRUD
  const saveProvider = async () => {
    try {
      if (editingP) {
        await invoke("update_provider", { payload: { ...pForm, enabled: true } });
        flash("Provider updated");
      } else {
        await invoke("create_provider", { payload: pForm });
        flash("Provider created");
      }
      setEditingP(null);
      setPForm(emptyProviderForm);
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delProvider = async (id: string) => {
    try { await invoke("delete_provider", { id }); flash("Provider deleted"); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const editProvider = (p: Provider) => {
    setEditingP(p.id);
    setPForm({ id: p.id, name: p.name, base_url: p.openai_base_url, openai_base_url: p.openai_base_url, anthropic_base_url: p.anthropic_base_url ?? "", auth_header: p.auth_header, auth_scheme: p.auth_scheme ?? "", api_key: p.api_key ?? "" });
  };

  // Route CRUD
  const saveRoute = async () => {
    try {
      if (editingR) {
        await invoke("update_route", { payload: { ...rForm, enabled: true } });
        flash("Route updated");
      } else {
        await invoke("create_route", { payload: rForm });
        flash("Route created");
      }
      setEditingR(null);
      setRForm({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" });
      await syncDesktopBindingIfManaged();
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delRoute = async (id: string) => {
    try {
      await invoke("delete_route", { id });
      flash("Route deleted");
      await syncDesktopBindingIfManaged();
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const editRoute = (r: ModelRoute) => {
    setEditingR(r.id);
    setRForm({ id: r.id, claude_alias: r.claude_alias, display_name: r.display_name, provider_id: r.provider_id, upstream_model: r.upstream_model });
  };

  // Settings
  const saveSettings = async () => {
    if (!settings) return;
    try { await invoke("save_settings", { payload: settings }); flash("Settings saved"); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };

  const doImport = async () => {
    if (!importPath) return;
    try { await invoke("import_config", { filePath: importPath }); flash("Config imported"); setImportPath(""); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const doExport = async () => {
    try { const p = await invoke<string>("export_config"); flash(`Exported to ${p}`); } catch (e) { flash(String(e), "error"); }
  };

  // Codex actions
  const startCodex = async () => { try { await invoke("start_codex_gateway"); await loadAll(); flash("Codex gateway started"); } catch (e) { flash(String(e), "error"); } };
  const stopCodex = async () => { try { await invoke("stop_codex_gateway"); await loadAll(); flash("Codex gateway stopped"); } catch (e) { flash(String(e), "error"); } };
  const bindCodexApp = async () => {
    if (!codexBindModel) {
      flash("Choose a default Codex model before binding", "error");
      return;
    }
    try {
      await invoke("start_codex_gateway");
      const info = await invoke<CodexBindingInfo>("apply_codex_binding", { model: codexBindModel });
      setCodexBinding(info);
      await loadAll();
      flash("Codex App bound to Gateway Switch");
    } catch (e) { flash(String(e), "error"); }
  };
  const restoreCodexApp = async () => {
    try {
      const info = await invoke<CodexBindingInfo>("restore_codex_binding");
      await invoke("stop_codex_gateway");
      setCodexBinding(info);
      await loadAll();
      flash("Codex restored to OpenAI login");
    } catch (e) { flash(String(e), "error"); }
  };
  const runColdStartRepair = async () => {
    setColdStartRunning(true);
    try {
      const report = isTauriRuntime
        ? await invoke<ColdStartReport>("run_coldstart_repair")
        : { ...MOCK_COLDSTART, mode: "repair", report_path: "~/Library/Application Support/Gateway Switch/backups/coldstart/preview.md" };
      await loadAll();
      setColdStart(report);
      setPage("coldstart");
      flash(report.report_path ? `Cold start repair report saved: ${report.report_path}` : "Cold start check completed");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setColdStartRunning(false);
    }
  };

  const refreshCodexPp = async (includeStore = false) => {
    setCodexPpLoading(true);
    try {
      if (!isTauriRuntime) {
        setCodexPpInstall(MOCK_CODEX_PP_INSTALL);
        setCodexPpTweaks(MOCK_CODEX_PP_TWEAKS);
        setCodexPpHealth(MOCK_CODEX_PP_HEALTH);
        setCodexPpPreflight(MOCK_CODEX_PP_PREFLIGHT);
        setCodexPpRecommendedScripts(MOCK_CODEX_PP_RECOMMENDED_SCRIPTS);
        if (includeStore) setCodexPpStore(MOCK_CODEX_PP_STORE);
        return;
      }
      const [install, tweaks, health, preflight, scripts] = await Promise.all([
        invoke<CodexPpInstall>("detect_codex_pp"),
        invoke<CodexPpTweak[]>("list_codex_pp_tweaks"),
        invoke<CodexPpHealth>("get_codex_pp_health"),
        invoke<CodexPpPreflight>("get_codex_pp_preflight"),
        invoke<CodexPpRecommendedScriptsReport>("get_codex_pp_recommended_scripts"),
      ]);
      setCodexPpInstall(install);
      setCodexPpTweaks(tweaks);
      setCodexPpHealth(health);
      setCodexPpPreflight(preflight);
      setCodexPpRecommendedScripts(scripts);
      if (includeStore) {
        setCodexPpStore(await invoke<CodexPpStoreIndex>("fetch_codex_pp_store"));
      }
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const toggleCodexPpTweak = async (id: string, enabled: boolean) => {
    setCodexPpLoading(true);
    try {
      const tweaks = isTauriRuntime
        ? await invoke<CodexPpTweak[]>("set_codex_pp_tweak_enabled", { id, enabled })
        : codexPpTweaks.map(tw => tw.id === id ? { ...tw, enabled } : tw);
      setCodexPpTweaks(tweaks);
      flash(enabled ? "Tweak enabled" : "Tweak disabled");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const setCodexPpUiSafeMode = async (enabled: boolean) => {
    setCodexPpLoading(true);
    try {
      const tweaks = isTauriRuntime
        ? await invoke<CodexPpTweak[]>("set_codex_pp_tweak_enabled", {
          id: CODEX_PP_UI_IMPROVEMENTS_TWEAK_ID,
          enabled: !enabled,
        })
        : codexPpTweaks.map(tw =>
          tw.id === CODEX_PP_UI_IMPROVEMENTS_TWEAK_ID ? { ...tw, enabled: !enabled } : tw
        );
      setCodexPpTweaks(tweaks);
      flash(
        enabled
          ? "UI safe mode enabled: page enhancement disabled"
          : "UI safe mode disabled: page enhancement enabled",
        "success",
      );
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const installCodexPpTweak = async (entry: CodexPpStoreEntry) => {
    setCodexPpLoading(true);
    try {
      const tweaks = isTauriRuntime
        ? await invoke<CodexPpTweak[]>("install_codex_pp_tweak", { repo: entry.repo, approvedCommitSha: entry.approvedCommitSha })
        : [...codexPpTweaks];
      setCodexPpTweaks(tweaks);
      await refreshCodexPp(true);
      flash("Tweak installed");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const installCodexPpRecommendedScripts = async () => {
    setCodexPpLoading(true);
    try {
      const report = isTauriRuntime
        ? await invoke<CodexPpRecommendedScriptsReport>("install_codex_pp_recommended_scripts")
        : { ...MOCK_CODEX_PP_RECOMMENDED_SCRIPTS };
      setCodexPpRecommendedScripts(report);
      flash("Recommended scripts installed. Restart Codex if they do not hot-load.");
    } catch (e) {
      await refreshCodexPp(false);
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const uninstallCodexPpTweak = async (id: string) => {
    setCodexPpLoading(true);
    try {
      const tweaks = isTauriRuntime
        ? await invoke<CodexPpTweak[]>("uninstall_codex_pp_tweak", { id })
        : codexPpTweaks.filter(tw => tw.id !== id);
      setCodexPpTweaks(tweaks);
      flash("Tweak removed");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      setCodexPpLoading(false);
    }
  };

  const runCodexPpCli = async (action: string) => {
    setCodexPpLoading(true);
    setCodexPpLogLines([]);
    let unlisten: null | (() => void) = null;
    try {
      const sessionId = `codexpp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      unlisten = isTauriRuntime
        ? await listen<CodexPpLogEvent>("codex-pp-cli-log", event => {
          if (event.payload.session_id !== sessionId) return;
          const prefix = event.payload.stream === "system" ? ">" : event.payload.stream === "stderr" ? "!" : "";
          setCodexPpLogLines(current => [...current, prefix ? `${prefix} ${event.payload.line}` : event.payload.line]);
        })
        : null;
      const result = isTauriRuntime
        ? await invoke<CodexPpCliResult>("run_codex_pp_cli", { action, sessionId })
        : { action, command: `codexplusplus ${action}`, success: true, code: 0, stdout: "browser preview", stderr: "" };
      setCodexPpCli(result);
      if (!isTauriRuntime) {
        setCodexPpLogLines([`> $ ${result.command}`, result.stdout || "(browser preview)"]);
      } else if (!result.stdout && !result.stderr && action !== "install" && action !== "install-local") {
        setCodexPpLogLines(current => current.length > 0 ? current : ["(no output)"]);
      }
      const shouldRefreshStore = action === "install" || action === "install-local" || action === "update";
      await refreshCodexPp(shouldRefreshStore);
      if (action === "install" || action === "install-local" || !result.success) {
        setCodexTab("diagnostics");
      }
      flash(result.success ? "Codex++ command completed" : "Codex++ command failed", result.success ? "success" : "error");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      unlisten?.();
      setCodexPpLoading(false);
    }
  };

  const openCodexPpPath = async (kind: string) => {
    try {
      const path = isTauriRuntime ? await invoke<string>("open_codex_pp_path", { kind }) : kind;
      flash(`Opened ${path}`);
    } catch (e) {
      flash(String(e), "error");
    }
  };

  const saveCodexRoute = async () => {
    try {
      if (editingC) {
        await invoke("update_codex_route", { payload: { ...cForm, enabled: true } });
        flash("Codex route updated");
      } else {
        await invoke("create_codex_route", { payload: cForm });
        flash("Codex route created");
      }
      setEditingC(null);
      setCForm({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "", tool_call_mode: "force_when_tools_present" });
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delCodexRoute = async (id: string) => {
    try { await invoke("delete_codex_route", { id }); flash("Codex route deleted"); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const editCodexRoute = (r: CodexRoute) => {
    setEditingC(r.id);
    setCForm({ id: r.id, codex_model: r.codex_model, display_name: r.display_name, provider_id: r.provider_id, upstream_model: r.upstream_model, tool_call_mode: r.tool_call_mode || "force_when_tools_present" });
  };

  const addModelAlias = async (aliasType: "claude" | "codex") => {
    const alias = (aliasType === "claude" ? newClaudeAlias : newCodexAlias).trim();
    if (!alias) return;
    try {
      const updated = await invoke<ModelAlias[]>("create_model_alias", { payload: { alias, alias_type: aliasType } });
      if (aliasType === "claude") {
        setClaudeAliases(updated);
        setNewClaudeAlias("");
        setRForm(current => ({ ...current, claude_alias: alias }));
      } else {
        setCodexAliases(updated);
        setNewCodexAlias("");
        setCForm(current => ({ ...current, codex_model: alias }));
      }
      flash(`${aliasType === "claude" ? "Claude alias" : "Codex model"} added`);
    } catch (e) { flash(String(e), "error"); }
  };

  const removeModelAlias = async (aliasType: "claude" | "codex", id: string, alias: string) => {
    try {
      const updated = await invoke<ModelAlias[]>("delete_model_alias", { id, aliasType });
      if (aliasType === "claude") {
        setClaudeAliases(updated);
        if (rForm.claude_alias === alias) {
          setRForm(current => ({ ...current, claude_alias: updated[0]?.alias ?? DEFAULT_CLAUDE_ALIASES[0] }));
        }
      } else {
        setCodexAliases(updated);
        if (cForm.codex_model === alias) {
          setCForm(current => ({ ...current, codex_model: updated[0]?.alias ?? DEFAULT_CODEX_MODELS[0] }));
        }
      }
      flash(`${aliasType === "claude" ? "Claude alias" : "Codex model"} removed`);
    } catch (e) { flash(String(e), "error"); }
  };

  // =====================================================
  //  SIDEBAR
  // =====================================================
  const Sidebar = () => (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <div className="brand-icon">
          <svg viewBox="0 0 24 24" fill="none">
            <path d="M12 3.25c4.25 0 7.55 3.22 7.55 7.35 0 5.18-7.55 10.15-7.55 10.15S4.45 15.78 4.45 10.6C4.45 6.47 7.75 3.25 12 3.25Z" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round"/>
            <path d="M8.2 10.55h7.6M12 6.75v7.6" stroke="currentColor" strokeWidth="1.35" strokeLinecap="round"/>
            <circle cx="12" cy="10.55" r="2.45" fill="currentColor"/>
          </svg>
        </div>
        <div className="brand-text">
          <div className="brand-name">Gateway Switch</div>
          <div className="brand-sub">v1.9.0</div>
        </div>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">{t("Dashboard")}</div>
        <button className={`nav-item ${page === "dashboard" ? "active" : ""}`} aria-label={t("Dashboard")} title={t("Dashboard")} onClick={() => setPage("dashboard")}>
          <IconGrid />
          <span className="nav-label">{t("Dashboard")}</span>
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">{t("Products")}</div>
        <button className={`nav-item ${page === "claude" ? "active" : ""}`} aria-label={t("Claude")} title={t("Claude")} onClick={() => setPage("claude")}>
          <IconShuffle />
          <span className="nav-label">{t("Claude")}</span>
          {routes.length > 0 && <span className="nav-badge">{routes.length}</span>}
        </button>
        <button className={`nav-item ${page === "claudeCode" ? "active" : ""}`} aria-label={t("Claude Code")} title={t("Claude Code")} onClick={() => setPage("claudeCode")}>
          <IconTerminal />
          <span className="nav-label">{t("Claude Code")}</span>
        </button>
        <button className={`nav-item ${page === "codex" ? "active" : ""}`} aria-label={t("Codex")} title={t("Codex")} onClick={() => setPage("codex")}>
          <IconTerminal />
          <span className="nav-label">{t("Codex")}</span>
          {codexRoutes.length > 0 && <span className="nav-badge">{codexRoutes.length}</span>}
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">{t("Features")}</div>
        <button className={`nav-item ${page === "mcpSync" ? "active" : ""}`} aria-label={t("MCP Sync")} title={t("MCP Sync")} onClick={() => setPage("mcpSync")}>
          <IconShuffle />
          <span className="nav-label">{t("MCP Sync")}</span>
          {mcpPreview?.merged_count ? <span className="nav-badge">{mcpPreview.merged_count}</span> : null}
        </button>
        <button className={`nav-item ${page === "coldstart" ? "active" : ""}`} aria-label={t("Cold Start")} title={t("Cold Start")} onClick={() => setPage("coldstart")}>
          <IconZap />
          <span className="nav-label">{t("Cold Start")}</span>
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">{t("General")}</div>
        <button className={`nav-item ${page === "providers" ? "active" : ""}`} aria-label={t("Providers")} title={t("Providers")} onClick={() => setPage("providers")}>
          <IconSun />
          <span className="nav-label">{t("Providers")}</span>
          {providers.length > 0 && <span className="nav-badge">{providers.length}</span>}
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">{t("System")}</div>
        <button className={`nav-item ${page === "logs" ? "active" : ""}`} aria-label={t("Logs")} title={t("Logs")} onClick={() => setPage("logs")}>
          <IconTerminal />
          <span className="nav-label">{t("Logs")}</span>
        </button>
        <button className={`nav-item ${page === "settings" ? "active" : ""}`} aria-label={t("Settings")} title={t("Settings")} onClick={() => setPage("settings")}>
          <IconSettings />
          <span className="nav-label">{t("Settings")}</span>
        </button>
      </div>

      <div className="sidebar-footer">
        <span className={`status-dot ${status?.gateway_running || codexStatus?.running ? "on" : "off"}`} />
        <span className="status-text">
          Claude <strong>{status?.gateway_running ? t("Running") : t("Stopped")}</strong> · Codex <strong>{codexStatus?.running ? t("Running") : t("Stopped")}</strong>
        </span>
        <span className="sidebar-version">v1.9.0</span>
      </div>
    </aside>
  );

  // =====================================================
  //  DASHBOARD PAGE
  // =====================================================
  const DashboardPage = () => (
    <div>
      <div className="page-header">
        <h1>{t("Dashboard")}</h1>
        <p>{t("Read-only product gateway overview")}</p>
      </div>

      {/* KPI Row */}
      <div className="kpi-row">
        <div className="kpi-card">
          <div className="kpi-icon green">
            <IconPulse />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">{t("Claude Gateway")}</div>
            {status?.gateway_running ? (
              <span className="kpi-badge green"><span className="dot" /> {t("Running")}</span>
            ) : (
              <span className="kpi-badge red"><span className="dot" /> {t("Stopped")}</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon blue">
            <IconMonitor />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">{t("Codex Gateway")}</div>
            {codexStatus?.running ? (
              <span className="kpi-badge green"><span className="dot" /> {t("Running")}</span>
            ) : (
              <span className="kpi-badge red"><span className="dot" /> {t("Stopped")}</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon blue">
            <IconMonitor />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">{t("App Bindings")}</div>
            {desktop?.managed || codexBinding?.managed || claudeCode?.managed ? (
              <span className="kpi-badge blue"><span className="dot" /> {t("Managed")}</span>
            ) : (
              <span className="kpi-badge muted"><span className="dot" /> {t("Unmanaged")}</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon amber">
            <IconSun />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">{t("Providers")}</div>
            <div className="kpi-value">{providers.length}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon purple">
            <IconShuffle />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">{t("Routes")}</div>
            <div className="kpi-value">{routes.length + codexRoutes.length}</div>
          </div>
        </div>
      </div>

      {runtimeSource && runtimeSource.severity !== "ok" && (
        <div className="card" style={{ borderColor: "var(--warning)", marginBottom: 16 }}>
          <div className="card-title">Runtime Source Warning</div>
          <p style={{ color: "var(--muted)", marginBottom: 12 }}>{runtimeSource.summary}</p>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">Bundle</span>
            <span className="info-val">{runtimeSource.bundle_path}</span>
            <span className="info-key">Recommendation</span>
            <span className="info-val">{runtimeSource.recommendation}</span>
          </div>
        </div>
      )}

      <div className="two-col">
        <div className="card">
          <div className="card-title">Claude</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">{t("Gateway")}</span>
            <span className="info-val">
              <span className={`badge ${status?.gateway_running ? "badge-green" : "badge-gray"}`}>
                {status?.gateway_running ? t("Running") : t("Stopped")}
              </span>
            </span>
            <span className="info-key">{t("Binding")}</span>
            <span className="info-val">{desktop?.managed ? t("Claude Desktop uses Gateway Switch") : t("Claude Desktop is unmanaged")}</span>
            <span className="info-key">Claude Code</span>
            <span className="info-val">{claudeCode?.managed ? `${claudeCode.model ?? "model"} via ${claudeCode.base_url ?? "Gateway"}` : t("Claude Code is unmanaged")}</span>
            <span className="info-key">{t("Last Call")}</span>
            <span className="info-val">{latestClaudeLog ? `${latestClaudeLog.upstream_model} via ${latestClaudeLog.provider_id}` : t("No traffic yet")}</span>
          </div>

          {health && (
            <>
              <div className="health-row">
                <span className="health-label">Claude</span>
                <div className="health-bar-track">
                  <div className={`health-bar-fill ${health.ok ? "" : "err"}`} style={{ width: health.ok ? "100%" : "0%" }} />
                </div>
                <span className={`health-text ${health.ok ? "ok" : "err"}`}>
                  {health.ok ? "100%" : "0%"}
                </span>
              </div>
              <div className="health-row">
                <span className="health-label" />
                <span style={{ fontSize: 12, color: "var(--muted)" }}>
                  {health.message}
                  {health.latency_ms != null && ` - ${health.latency_ms}ms`}
                </span>
              </div>
            </>
          )}
        </div>

        <div className="card">
          <div className="card-title">Codex</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">{t("Gateway")}</span>
            <span className="info-val">
              <span className={`badge ${codexStatus?.running ? "badge-green" : "badge-gray"}`}>
                {codexStatus?.running ? t("Running") : t("Stopped")}
              </span>
            </span>
            <span className="info-key">{t("Binding")}</span>
            <span className="info-val">{codexBinding?.managed ? `Codex App uses ${codexBinding.model ?? "Gateway Switch"}` : t("Codex App uses OpenAI login")}</span>
            <span className="info-key">{t("Last Call")}</span>
            <span className="info-val">{latestCodexLog ? `${latestCodexLog.upstream_model} via ${latestCodexLog.provider_id}` : t("No traffic yet")}</span>
          </div>
          {codexHealth && (
            <div className="health-row">
              <span className="health-label">Codex</span>
              <div className="health-bar-track">
                <div className={`health-bar-fill ${codexHealth.ok ? "" : "err"}`} style={{ width: codexHealth.ok ? "100%" : "0%" }} />
              </div>
              <span className={`health-text ${codexHealth.ok ? "ok" : "err"}`}>{codexHealth.message}</span>
            </div>
          )}
        </div>
      </div>

      {/* Providers preview */}
      <div className="section-label">{t("Providers")}</div>
      <div className="providers-grid">
        {PROVIDER_PRESETS.map(preset => {
          const isConnected = providers.some(p => p.id === preset.id && p.enabled);
          return (
            <div
              key={preset.id}
              className="provider-card read-only"
            >
              <div className="provider-logo" style={{ background: preset.colorBg, color: preset.color }}>
                {preset.logo}
              </div>
              <div className="provider-info">
                <div className="provider-name">{preset.name}</div>
                <div className="provider-models">{preset.shortUrl}</div>
              </div>
              <span className={`provider-status ${isConnected ? "connected" : "disconnected"}`} />
            </div>
          );
        })}
      </div>
    </div>
  );

  // =====================================================
  //  PROVIDERS PAGE
  // =====================================================
  const ProvidersPage = () => (
    <div>
      <div className="page-header">
        <h1>{t("Providers")}</h1>
        <p>{t("Share credentials across products, with protocol-specific base URLs for OpenAI and Anthropic clients")}</p>
      </div>

      {/* Preset grid */}
      <div className="section-label">{t("Quick Add")}</div>
      <div className="providers-grid">
        {PROVIDER_PRESETS.map(preset => {
          const isConnected = providers.some(p => p.id === preset.id && p.enabled);
          return (
            <div
              key={preset.id}
              className="provider-card"
              onClick={() => {
                setEditingP(null);
                setPForm({ id: preset.id, name: preset.name, base_url: preset.openai_base_url, openai_base_url: preset.openai_base_url, anthropic_base_url: preset.anthropic_base_url, auth_header: preset.auth_header, auth_scheme: preset.auth_scheme, api_key: "" });
              }}
            >
              <div className="provider-logo" style={{ background: preset.colorBg, color: preset.color }}>
                {preset.logo}
              </div>
              <div className="provider-info">
                <div className="provider-name">{preset.name}</div>
                <div className="provider-models">{preset.shortUrl}</div>
              </div>
              <span className={`provider-status ${isConnected ? "connected" : "disconnected"}`} />
            </div>
          );
        })}
      </div>

      {/* Add/Edit form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">{editingP ? t("Edit Provider") : t("Add Provider")}</div>
        <div className="form-row">
          <div className="form-field">
            <label>{t("Provider ID")}</label>
            <input value={pForm.id} disabled={!!editingP} onChange={e => setPForm({ ...pForm, id: e.target.value })} placeholder="e.g. ark" />
          </div>
          <div className="form-field">
            <label>{t("Display Name")}</label>
            <input value={pForm.name} onChange={e => setPForm({ ...pForm, name: e.target.value })} placeholder="e.g. Volcano Engine" />
          </div>
          <div className="form-field">
            <label>OpenAI Base URL</label>
            <input value={pForm.openai_base_url} onChange={e => setPForm({ ...pForm, base_url: e.target.value, openai_base_url: e.target.value })} placeholder="https://.../v1" />
          </div>
          <div className="form-field">
            <label>Anthropic Base URL</label>
            <input value={pForm.anthropic_base_url} onChange={e => setPForm({ ...pForm, anthropic_base_url: e.target.value })} placeholder="https://.../anthropic" />
          </div>
          <div className="form-field">
            <label>{t("Auth Header")}</label>
            <input value={pForm.auth_header} onChange={e => setPForm({ ...pForm, auth_header: e.target.value })} />
          </div>
          <div className="form-field">
            <label>{t("Auth Scheme")}</label>
            <input value={pForm.auth_scheme} onChange={e => setPForm({ ...pForm, auth_scheme: e.target.value })} placeholder="Bearer / x-api-key" />
          </div>
          <div className="form-field">
            <label>API Key</label>
            <input type="password" value={pForm.api_key} onChange={e => setPForm({ ...pForm, api_key: e.target.value })} placeholder={t("Your API key")} />
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveProvider}>
            {editingP ? <><IconEdit /> {t("Save")}</> : <><IconPlus /> {t("Add Provider")}</>}
          </button>
          {editingP && (
            <button className="btn" onClick={() => {
              setEditingP(null);
              setPForm(emptyProviderForm);
            }}>{t("Cancel")}</button>
          )}
        </div>
      </div>

      {/* Providers table */}
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>{t("Provider")}</th>
              <th>OpenAI URL</th>
              <th>Anthropic URL</th>
              <th>Auth</th>
              <th>{t("Status")}</th>
              <th>{t("Actions")}</th>
            </tr>
          </thead>
          <tbody>
            {providers.map(p => (
              <tr key={p.id}>
                <td style={{ fontWeight: 600 }}>{p.name}</td>
                <td><span className="url-cell">{p.openai_base_url}</span></td>
                <td>
                  {p.anthropic_base_url ? (
                    <span className="url-cell">{p.anthropic_base_url}</span>
                  ) : (
                    <span className="muted-pill">{t("Not configured")}</span>
                  )}
                </td>
                <td><span className="badge badge-blue">{p.auth_header}</span></td>
                <td><span className={`badge ${p.enabled ? "badge-green" : "badge-gray"}`}>{p.enabled ? t("Active") : t("Disabled")}</span></td>
                <td>
                  <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                    <button className="btn" style={{ padding: "5px 8px" }} onClick={() => editProvider(p)}><IconEdit /></button>
                    <button className="btn btn-danger" style={{ padding: "5px 8px" }} onClick={() => delProvider(p.id)}><IconTrash /></button>
                  </div>
                </td>
              </tr>
            ))}
            {providers.length === 0 && (
              <tr>
                <td colSpan={6}>
                  <div className="empty-state">
                    <div className="empty-icon">--</div>
                    <h3>{t("No providers configured")}</h3>
                    <p>{t("Click a preset above to get started.")}</p>
                  </div>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );

  // =====================================================
  //  ROUTES PAGE
  // =====================================================
  const AliasManager = (aliasType: "claude" | "codex") => {
    const aliases = aliasType === "claude" ? claudeAliases : codexAliases;
    const value = aliasType === "claude" ? newClaudeAlias : newCodexAlias;
    const setValue = aliasType === "claude" ? setNewClaudeAlias : setNewCodexAlias;
    const title = aliasType === "claude" ? "Claude Aliases" : "Codex Models";
    const placeholder = aliasType === "claude" ? "e.g. claude-sonnet-4-7" : "e.g. gpt-5.1-codex";

    return (
      <div className="alias-manager">
        <div className="alias-manager-head">
          <div>
            <div className="card-title">{title}</div>
            <p>{aliasType === "claude" ? t("Maintain the aliases exposed to Claude Desktop and model routes.") : t("Maintain the model names Codex can request from this gateway.")}</p>
          </div>
          <div className="alias-add">
            <input
              value={value}
              onChange={e => setValue(e.target.value)}
              onKeyDown={e => { if (e.key === "Enter") void addModelAlias(aliasType); }}
              placeholder={placeholder}
            />
            <button className="btn" onClick={() => void addModelAlias(aliasType)}><IconPlus /> {t("Add")}</button>
          </div>
        </div>
        <div className="alias-chip-list">
          {aliases.map(a => (
            <span key={a.id} className="alias-chip">
              {a.alias}
              <button aria-label={`Delete ${a.alias}`} onClick={() => void removeModelAlias(aliasType, a.id, a.alias)}><IconX /></button>
            </span>
          ))}
          {aliases.length === 0 && <span className="alias-empty">{t("Default aliases will be used until you add a custom one.")}</span>}
        </div>
      </div>
    );
  };
  const DesktopBindingCard = () => (
    <div className="card">
      <div className="card-title">{t("Binding Status")}</div>
      <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
        <span className="info-key">{t("Config File")}</span>
        <span className="info-val">{desktop?.config_path ?? "-"}</span>
        <span className="info-key">Base URL</span>
        <span className="info-val">{desktop?.base_url ?? t("Not set")}</span>
        <span className="info-key">{t("Local Gateway Auth")}</span>
        <span className="info-val">{desktop?.auth_scheme ?? t("Not set")}</span>
        <span className="info-key">{t("Backup")}</span>
        <span className="info-val">{desktop?.backup_path ? t("Available") : t("None")}</span>
        <span className="info-key">{t("Status")}</span>
        <span className="info-val">
          <span className={`badge ${desktop?.managed ? "badge-green" : "badge-gray"}`}>
            {desktop?.managed ? t("Managed") : t("Unmanaged")}
          </span>
        </span>
      </div>
      <div className="qa-buttons" style={{ marginTop: 16 }}>
        <button className="btn btn-primary" onClick={bindDesktop}>
          <IconLink /> {t("Bind Desktop")}
        </button>
        <button className="btn" onClick={restoreDesktop}>
          <IconUnlink /> {t("Restore")}
        </button>
      </div>
    </div>
  );

  const DesktopExposedModelsCard = () => (
    <div className="card">
      <div className="card-title">{t("Exposed Models")}</div>
      {desktop?.models && desktop.models.length > 0 ? (
        <div className="route-list" style={{ marginBottom: 0 }}>
          {desktop.models.map(m => (
            <div key={m} className="route-item">
              <div className={`route-icon ${getModelFamily(m)}`}>
                {getModelAbbrev(m)}
              </div>
              <div className="route-info">
                <div className="route-name">{m}</div>
                <div className="route-path">{t("Exposed to Claude Desktop")}</div>
              </div>
              <span className="route-status active">
                <IconCheck /> {t("Active")}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className="empty-state">
          <div className="empty-icon">--</div>
          <h3>{t("No models exposed")}</h3>
          <p>{t("Bind Desktop first to expose models.")}</p>
        </div>
      )}
    </div>
  );

const needsClaudeCodeGatewayRoute = (provider: Provider | undefined, upstreamModel: string) => {
  if (!provider) return false;
  const key = [
    provider.id,
    provider.name,
    provider.base_url,
    provider.openai_base_url,
    upstreamModel,
  ].join(" ").toLowerCase();
  return (key.includes("volc") || key.includes("ark.cn-") || key.includes("火山")) && key.includes("deepseek");
};


  const ClaudePage = () => (
    <div>
      <div className="page-header">
        <h1>{t("Claude")}</h1>
        <p>{t("Configure Claude model routes and Claude Desktop binding")}</p>
      </div>

      <div className="two-col">
        <div className="card">
          <div className="card-title">{t("Claude Gateway Status")}</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">{t("Status")}</span>
            <span className="info-val">
              <span className={`badge ${status?.gateway_running ? "badge-green" : "badge-gray"}`}>
                {status?.gateway_running ? t("Running") : t("Stopped")}
              </span>
            </span>
            <span className="info-key">{t("Port")}</span>
            <span className="info-val">{status?.gateway_port ?? settings?.listen_port ?? 3456}</span>
            <span className="info-key">Desktop URL</span>
            <span className="info-val">http://127.0.0.1:{status?.gateway_port ?? settings?.listen_port ?? 3456}</span>
          </div>
          {health && (
            <div className="health-row">
              <span className="health-label">Claude</span>
              <div className="health-bar-track">
                <div className={`health-bar-fill ${health.ok ? "" : "err"}`} style={{ width: health.ok ? "100%" : "0%" }} />
              </div>
              <span className={`health-text ${health.ok ? "ok" : "err"}`}>
                {health.message}{health.latency_ms != null ? ` · ${health.latency_ms}ms` : ""}
              </span>
            </div>
          )}
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            {status?.gateway_running ? (
              <button className="btn btn-danger" onClick={stopGw}><IconStop /> {t("Stop")}</button>
            ) : (
              <button className="btn btn-primary" onClick={startGw}><IconPlay /> {t("Start")}</button>
            )}
            <button className="btn" onClick={checkHealth}><IconZap /> {t("Check Health")}</button>
            <button className="btn" onClick={() => void loadAll()}><IconRefresh /> {t("Refresh")}</button>
          </div>
        </div>

        {DesktopBindingCard()}
      </div>

      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">{editingR ? t("Edit Route") : t("Add Route")}</div>
        <div className="form-row">
          <div className="form-field">
            <label>{t("Route ID")}</label>
            <input value={rForm.id} disabled={!!editingR} onChange={e => setRForm({ ...rForm, id: e.target.value })} placeholder="e.g. sonnet-ark" />
          </div>
          <div className="form-field">
            <label>{t("Claude Alias")}</label>
            <select value={rForm.claude_alias} onChange={e => setRForm({ ...rForm, claude_alias: e.target.value })}>
              {claudeAliasOptions.map(a => <option key={a} value={a}>{a}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>{t("Display Name")}</label>
            <input value={rForm.display_name} onChange={e => setRForm({ ...rForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>{t("Provider")}</label>
            <select value={rForm.provider_id} onChange={e => setRForm({ ...rForm, provider_id: e.target.value })}>
              <option value="">{t("Select provider...")}</option>
              {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>{t("Upstream Model")}</label>
            <input value={rForm.upstream_model} onChange={e => setRForm({ ...rForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-v3" />
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveRoute}>
            {editingR ? <><IconEdit /> {t("Save")}</> : <><IconPlus /> {t("Add Route")}</>}
          </button>
          {editingR && (
            <button className="btn" onClick={() => {
              setEditingR(null);
              setRForm({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" });
            }}>{t("Cancel")}</button>
          )}
        </div>
      </div>

      {AliasManager("claude")}

      <div className="two-col">
        <div className="card">
          <div className="card-title">{t("Route Cards")}</div>
          <div className="route-list" style={{ marginBottom: 0 }}>
            {routes.length > 0 ? (
              routes.map(r => (
                <div key={r.id} className="route-item">
                  <div className={`route-icon ${getModelFamily(r.claude_alias)}`}>
                    {getModelAbbrev(r.claude_alias)}
                  </div>
                  <div className="route-info">
                    <div className="route-name">{r.claude_alias}</div>
                    <div className="route-path">{r.display_name || r.upstream_model} via {r.provider_id}</div>
                  </div>
                  <span className={`route-status ${r.enabled ? "active" : "disabled"}`}>
                    {r.enabled ? t("Active") : t("Disabled")}
                  </span>
                  <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                    <button className="btn" style={{ padding: "5px 8px" }} onClick={() => editRoute(r)}><IconEdit /></button>
                    <button className="btn btn-danger" style={{ padding: "5px 8px" }} onClick={() => delRoute(r.id)}><IconTrash /></button>
                  </div>
                </div>
              ))
            ) : (
              <div className="empty-state">
                <div className="empty-icon">--</div>
                <h3>{t("No routes configured")}</h3>
                <p>{t("Add a route above to start mapping models.")}</p>
              </div>
            )}
          </div>
        </div>

        {DesktopExposedModelsCard()}
      </div>

      <div className="section-label">{t("Route Table")}</div>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Claude Alias</th>
              <th>{t("Display Name")}</th>
              <th>{t("Provider")}</th>
              <th>{t("Upstream Model")}</th>
              <th>{t("Status")}</th>
              <th>{t("Actions")}</th>
            </tr>
          </thead>
          <tbody>
            {routes.map(r => (
              <tr key={r.id}>
                <td style={{ fontWeight: 600 }}>{r.claude_alias}</td>
                <td>{r.display_name}</td>
                <td><span className="badge badge-blue">{r.provider_id}</span></td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>{r.upstream_model}</td>
                <td><span className={`badge ${r.enabled ? "badge-green" : "badge-gray"}`}>{r.enabled ? t("Active") : t("Disabled")}</span></td>
                <td>
                  <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                    <button className="btn" style={{ padding: "5px 8px" }} onClick={() => editRoute(r)}><IconEdit /></button>
                    <button className="btn btn-danger" style={{ padding: "5px 8px" }} onClick={() => delRoute(r.id)}><IconTrash /></button>
                  </div>
                </td>
              </tr>
            ))}
            {routes.length === 0 && (
              <tr>
                <td colSpan={6}>
                  <div className="empty-state">
                    <div className="empty-icon">--</div>
                    <h3>{t("No routes configured")}</h3>
                    <p>{t("Add a route above to start mapping models.")}</p>
                  </div>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );

  const ClaudeCodePage = () => {
    const selectedProvider = providers.find(p => p.id === ccProviderId);
    const gatewayRouteOptions = routes.length > 0 ? routes.filter(r => r.enabled).map(r => r.claude_alias) : claudeAliasOptions;
    const directProviderReady = ccMode === "provider" && !!selectedProvider?.anthropic_base_url && !!ccUpstreamModel.trim();
    const directProviderBlocked = ccMode === "provider" && needsClaudeCodeGatewayRoute(selectedProvider, ccUpstreamModel);
    const selectedRouteDiagnostic = routeDiagnostics.find(d => d.claude_alias === ccModel);

    return (
      <div>
        <div className="page-header">
          <h1>{t("Claude Code")}</h1>
          <p>{t("Bind Claude Code independently from Claude Desktop")}</p>
        </div>

        <div className="two-col">
          <div className="card">
            <div className="card-title">{t("Claude Code Binding")}</div>
            <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
              <span className="info-key">{t("Config")}</span>
              <span className="info-val">{claudeCode?.config_path ?? "~/.claude/settings.json"}</span>
              <span className="info-key">{t("Status")}</span>
              <span className="info-val">
                <span className={`badge ${claudeCode?.managed ? "badge-green" : "badge-gray"}`}>
                  {claudeCode?.managed ? t("Managed by Gateway Switch") : t("Not bound")}
                </span>
              </span>
              <span className="info-key">Base URL</span>
              <span className="info-val">{claudeCode?.base_url ?? t("Not set")}</span>
              <span className="info-key">{t("Model")}</span>
              <span className="info-val">{claudeCode?.model ?? t("Not set")}</span>
              <span className="info-key">{t("Auth Env")}</span>
              <span className="info-val">{claudeCode?.auth_env ?? t("Not set")}</span>
              <span className="info-key">{t("Backup")}</span>
              <span className="info-val">{claudeCode?.backup_path ? t("Available") : t("None")}</span>
            </div>
          </div>

          <div className="card">
            <div className="card-title">{t("Connection Mode")}</div>
            <div className="mode-switch">
              <button className={`mode-option ${ccMode === "gateway" ? "active" : ""}`} onClick={() => setCcMode("gateway")}>
                <IconShuffle />
                <span>{t("Gateway Route")}</span>
              </button>
              <button className={`mode-option ${ccMode === "provider" ? "active" : ""}`} onClick={() => setCcMode("provider")}>
                <IconSun />
                <span>{t("Direct Provider")}</span>
              </button>
            </div>

            {ccMode === "gateway" ? (
              <div className="binding-actions" style={{ marginTop: 16 }}>
                <label>{t("Claude Code model")}</label>
                <select value={ccModel} onChange={e => setCcModel(e.target.value)}>
                  {Array.from(new Set(gatewayRouteOptions)).map(model => (
                    <option key={model} value={model}>{model}</option>
                  ))}
                </select>
                <p>{t("Claude Code will use the local Claude Gateway and configured Claude routes, including Chat Completions fallback for providers such as XiaoMiMo.")}</p>
              </div>
            ) : (
              <div className="binding-actions" style={{ marginTop: 16 }}>
                <label>{t("Provider")}</label>
                <select value={ccProviderId} onChange={e => {
                  const providerId = e.target.value;
                  setCcProviderId(providerId);
                  const route = routes.find(r => r.provider_id === providerId);
                  if (route && !ccUpstreamModel) setCcUpstreamModel(route.upstream_model);
                }}>
                  <option value="">{t("Select provider...")}</option>
                  {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                </select>
                {selectedProvider && (
                  <div className="protocol-preview">
                    <div>
                      <span>OpenAI</span>
                      <code>{selectedProvider.openai_base_url}</code>
                    </div>
                    <div className={selectedProvider.anthropic_base_url ? "" : "missing"}>
                      <span>Anthropic</span>
                      <code>{selectedProvider.anthropic_base_url || t("Required for Direct Provider")}</code>
                    </div>
                  </div>
                )}
                <label>{t("Upstream model")}</label>
                <input value={ccUpstreamModel} onChange={e => setCcUpstreamModel(e.target.value)} placeholder="e.g. claude-sonnet-4-5" />
                <p>{t("Direct Provider writes the provider's Anthropic Base URL and API key into Claude Code. Use Gateway Route when a provider only supports OpenAI Chat Completions.")}</p>
                {directProviderBlocked && (
                  <p style={{ color: "var(--danger)", fontSize: 12, marginTop: -4 }}>
                    Volcengine DeepSeek rejects Claude Code Direct Provider requests with `messages.role = system`. Switch to Gateway Route so Gateway Switch can convert system/tool roles into user messages.
                  </p>
                )}
                {selectedProvider && (
                  <div className="route-flow">
                    <span>{selectedProvider.name}</span>
                    <IconArrowRight />
                    <span>{selectedProvider.anthropic_base_url || t("Missing Anthropic URL")}</span>
                    <IconArrowRight />
                    <span><b>{ccUpstreamModel || "model"}</b></span>
                  </div>
                )}
              </div>
            )}

            <div className="qa-buttons" style={{ marginTop: 16, marginBottom: 0 }}>
              <button className="btn btn-primary" onClick={bindClaudeCode} disabled={ccMode === "provider" && (!directProviderReady || directProviderBlocked)}><IconLink /> {t("Bind Claude Code")}</button>
              <button className="btn" onClick={restoreClaudeCode} disabled={!claudeCode?.managed && !claudeCode?.backup_path}><IconUnlink /> {t("Restore")}</button>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-title">Route Diagnostics</div>
          <p style={{ color: "var(--muted)", marginBottom: 14 }}>
            Provider capability profile explains whether Claude Code can use Direct Provider or must use Gateway Route.
          </p>
          <div className="route-list" style={{ marginBottom: 0 }}>
            {routeDiagnostics.map(diagnostic => (
              <div key={diagnostic.route_id} className="route-item">
                <div className="route-info">
                  <div className="route-name">{diagnostic.claude_alias} → {diagnostic.upstream_model}</div>
                  <div className="route-path">
                    {diagnostic.provider_name} · {diagnostic.strategy.strategy_id} · {diagnostic.strategy.summary}
                  </div>
                  {diagnostic.warnings.length > 0 && (
                    <div className="route-path" style={{ color: "var(--warning)" }}>
                      {diagnostic.warnings.join(" · ")}
                    </div>
                  )}
                </div>
                <span className={`badge ${diagnostic.strategy.direct_provider_safe ? "badge-green" : "badge-amber"}`}>
                  {diagnostic.strategy.direct_provider_safe ? "direct safe" : "gateway recommended"}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="card">
          <div className="card-title">{t("Runtime Environment")}</div>
          <div className="note-grid">
            <div>
              <strong>{t("Gateway Route")}</strong>
              <p>{t("Writes `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, and `ANTHROPIC_MODEL` into `~/.claude/settings.json`. Claude Desktop binding is not touched.")}</p>
            </div>
            <div>
              <strong>{t("Direct Provider")}</strong>
              <p>{t("Writes `ANTHROPIC_BASE_URL` from the provider's Anthropic URL. The OpenAI URL is reserved for Codex and Chat Completions fallback.")}</p>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-title">Payload Preview</div>
          <p style={{ color: "var(--muted)", marginBottom: 14 }}>
            Preview uses a fixed redacted sample request. It does not call the upstream provider or consume tokens.
          </p>
          <div className="qa-buttons" style={{ marginTop: 0 }}>
            <button className="btn" onClick={() => void loadPayloadPreview(ccModel)}>
              <IconPulse /> Preview Selected Route
            </button>
            {selectedRouteDiagnostic && (
              <span className={`badge ${selectedRouteDiagnostic.strategy.system_to_user ? "badge-amber" : "badge-green"}`}>
                {selectedRouteDiagnostic.strategy.strategy_id}
              </span>
            )}
          </div>
          {payloadPreview && (
            <>
              <div className="info-grid">
                <span className="info-key">Route</span>
                <span className="info-val">{payloadPreview.claude_alias} → {payloadPreview.upstream_model}</span>
                <span className="info-key">Roles</span>
                <span className="info-val">{payloadPreview.roles.join(" / ")}</span>
              </div>
              <pre className="log-view" style={{ maxHeight: 260 }}>{JSON.stringify(payloadPreview.payload, null, 2)}</pre>
            </>
          )}
        </div>
      </div>
    );
  };

  // =====================================================
  //  CODEX PAGE
  // =====================================================
  const CodexRoutesPage = () => (
    <div>
      <div className="page-header">
        <h1>{t("Codex Gateway")}</h1>
        <p>{t("OpenAI Responses API to Chat Completions API converter for Codex App and Codex CLI")}</p>
      </div>

      {/* Status + Quick Actions */}
      <div className="two-col">
        <div className="card">
          <div className="card-title">{t("Codex Gateway Status")}</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">{t("Status")}</span>
            <span className="info-val">
              <span className={`badge ${codexStatus?.running ? "badge-green" : "badge-gray"}`}>
                {codexStatus?.running ? t("Running") : t("Stopped")}
              </span>
            </span>
            <span className="info-key">{t("Port")}</span>
            <span className="info-val">{codexPort}</span>
            <span className="info-key">Endpoint</span>
            <span className="info-val">http://127.0.0.1:{codexPort}/v1/responses</span>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            {codexStatus?.running ? (
              <button className="btn btn-danger" onClick={stopCodex}><IconStop /> {t("Stop")}</button>
            ) : (
              <button className="btn btn-primary" onClick={startCodex}><IconPlay /> {t("Start")}</button>
            )}
            <button className="btn" onClick={checkCodexHealth}><IconZap /> {t("Check Health")}</button>
            <button className="btn" onClick={() => void loadAll()}><IconRefresh /> {t("Refresh")}</button>
          </div>
          {codexHealth && (
            <div className="health-row">
              <span className="health-label">Codex</span>
              <div className="health-bar-track">
                <div className={`health-bar-fill ${codexHealth.ok ? "" : "err"}`} style={{ width: codexHealth.ok ? "100%" : "0%" }} />
              </div>
              <span className={`health-text ${codexHealth.ok ? "ok" : "err"}`}>{codexHealth.message}</span>
            </div>
          )}
        </div>

        <div className="card">
          <div className="card-title">{t("Verify Real Model")}</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">{t("Last Codex Model")}</span>
            <span className="info-val">{latestCodexLog?.claude_alias ?? t("No Codex request yet")}</span>
            <span className="info-key">{t("Provider")}</span>
            <span className="info-val">{latestCodexLog?.provider_id ?? "-"}</span>
            <span className="info-key">{t("Real Upstream")}</span>
            <span className="info-val">{latestCodexLog?.upstream_model ?? "-"}</span>
            <span className="info-key">{t("Result")}</span>
            <span className="info-val">
              {latestCodexLog ? `${latestCodexLog.status_code ?? "pending"} · ${latestCodexLog.duration_ms ?? "-"}ms` : "-"}
            </span>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            <button className="btn" onClick={() => setPage("logs")}><IconSearch /> {t("Open Logs")}</button>
            <button className="btn" onClick={() => void loadAll()}><IconRefresh /> {t("Refresh")}</button>
          </div>
        </div>
      </div>

      <div className="two-col">
        <div className="card">
          <div className="card-title">{t("Codex App Binding")}</div>
          <div className="binding-panel binding-panel-compact">
            <div className="binding-state">
              <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
                <span className="info-key">{t("Config")}</span>
                <span className="info-val">{codexBinding?.config_path ?? "~/.codex/config.toml"}</span>
                <span className="info-key">{t("Binding")}</span>
                <span className="info-val">
                  <span className={`badge ${codexBinding?.managed ? "badge-green" : "badge-gray"}`}>
                    {codexBinding?.managed ? t("Managed by Gateway Switch") : t("Not bound")}
                  </span>
                </span>
                <span className="info-key">{t("Provider")}</span>
                <span className="info-val">{codexBinding?.model_provider ?? t("Default Codex provider")}</span>
                <span className="info-key">{t("Default Model")}</span>
                <span className="info-val">{codexBinding?.model ?? t("Not set")}</span>
              </div>
            </div>
            <div className="binding-actions">
              <label>{t("Default model for Codex App")}</label>
              <select value={codexBindModel} onChange={e => setCodexBindModel(e.target.value)}>
                {Array.from(new Set([...codexRoutes.map(r => r.codex_model), ...codexModelOptions])).map(model => (
                  <option key={model} value={model}>{model}</option>
                ))}
              </select>
              <p>{t("Bind writes Gateway Switch into `~/.codex/config.toml` and forces API-key mode for the local gateway. Restart Codex App after binding.")}</p>
              <div className="qa-buttons compact-actions" style={{ margin: 0 }}>
                <button className="btn btn-primary" onClick={bindCodexApp}><IconLink /> {t("Start & Bind Codex App")}</button>
                <button className="btn" onClick={restoreCodexApp} disabled={!codexBinding?.managed && !codexBinding?.backup_path}><IconUnlink /> {t("Restore OpenAI Login")}</button>
              </div>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-title">{t("Context and Reasoning Notes")}</div>
          <div className="note-grid">
            <div>
              <strong>{t("Reply speed")}</strong>
              <p>{t("Gateway Switch converts protocol shape; it does not add or remove a model's native reasoning ability. If the upstream model is fast, or does not expose reasoning tokens through Chat Completions, the visible response can be very quick.")}</p>
            </div>
            <div>
              <strong>{t("Project history")}</strong>
              <p>{t("Binding preserves `~/.codex/config.toml` project entries. Existing Codex conversations may still be separated by Codex's own account/provider state, so switching providers can show a different conversation list even when local project trust remains intact.")}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Add/Edit route form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">{editingC ? t("Edit Codex Route") : t("Add Codex Route")}</div>
        <div className="route-explainer">
          <div className="route-explainer-copy">
            <strong>{t("Codex Model must match the model used by Codex CLI.")}</strong>
            <span>{t("If you do not need a disguised name, set Codex Model and Upstream Model to the same third-party model name.")}</span>
          </div>
          <div className="route-flow">
            <span>codex -m <b>{cForm.codex_model || "model-name"}</b></span>
            <IconArrowRight />
            <span>{providers.find(p => p.id === cForm.provider_id)?.name || t("Provider")}</span>
            <IconArrowRight />
            <span><b>{cForm.upstream_model || "upstream-model"}</b></span>
          </div>
        </div>
        <div className="form-row">
          <div className="form-field">
            <label>{t("Route ID")}</label>
            <input value={cForm.id} disabled={!!editingC} onChange={e => setCForm({ ...cForm, id: e.target.value })} placeholder="e.g. gpt4o-deepseek" />
          </div>
          <div className="form-field">
            <label>{t("Codex Model (requested by Codex)")}</label>
            <select value={cForm.codex_model} onChange={e => setCForm({ ...cForm, codex_model: e.target.value })}>
              {codexModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
            </select>
            <span className="field-hint">{t("This is the model name used in `codex -m ...`.")}</span>
          </div>
          <div className="form-field">
            <label>{t("Display Name")}</label>
            <input value={cForm.display_name} onChange={e => setCForm({ ...cForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>{t("Provider")}</label>
            <select value={cForm.provider_id} onChange={e => setCForm({ ...cForm, provider_id: e.target.value })}>
              <option value="">{t("Select provider...")}</option>
              {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>{t("Upstream Model (real provider model)")}</label>
            <input value={cForm.upstream_model} onChange={e => setCForm({ ...cForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-chat" />
            <span className="field-hint">{t("This is the actual model name sent to the third-party API.")}</span>
          </div>
          <div className="form-field">
            <label>{t("Tool Call Mode")}</label>
            <select value={cForm.tool_call_mode} onChange={e => setCForm({ ...cForm, tool_call_mode: e.target.value })}>
              <option value="auto">{t("Auto")}</option>
              <option value="force_when_tools_present">{t("Force When Tools Present")}</option>
              <option value="strict_execution">{t("Strict Execution")}</option>
            </select>
            <span className="field-hint">
              {cForm.tool_call_mode === "auto" && t("Keeps the model's default behavior. Best compatibility, but weak tool models may only talk.")}
              {cForm.tool_call_mode === "force_when_tools_present" && t("Default. When Codex sends tools, Gateway asks the upstream model to emit tool_calls first.")}
              {cForm.tool_call_mode === "strict_execution" && t("If tools are present but no tool_calls are emitted, Gateway marks the response as failed.")}
            </span>
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveCodexRoute}>
            {editingC ? <><IconEdit /> {t("Save")}</> : <><IconPlus /> {t("Add Route")}</>}
          </button>
          {editingC && (
            <button className="btn" onClick={() => {
              setEditingC(null);
              setCForm({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "", tool_call_mode: "force_when_tools_present" });
            }}>{t("Cancel")}</button>
          )}
        </div>
      </div>

      {AliasManager("codex")}

      {/* Route cards */}
      <div className="section-label">{t("Active Codex Routes")}</div>
      <div className="route-list" style={{ marginBottom: 20 }}>
        {codexRoutes.length > 0 ? (
          codexRoutes.map(r => (
            <div key={r.id} className="route-item">
              <div className="route-icon" style={{ background: "rgba(217,119,6,0.08)", color: "var(--amber)" }}>
                {r.codex_model.slice(0, 3)}
              </div>
              <div className="route-info">
                <div className="route-name">{r.codex_model}</div>
                <div className="route-path">{r.display_name || r.upstream_model} via {r.provider_id} · {t(toolCallModeLabel(r.tool_call_mode))}</div>
              </div>
              <span className={`route-status ${r.enabled ? "active" : "disabled"}`}>
                {r.enabled ? t("Active") : t("Disabled")}
              </span>
              <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                <button className="btn" style={{ padding: "5px 8px" }} onClick={() => editCodexRoute(r)}><IconEdit /></button>
                <button className="btn btn-danger" style={{ padding: "5px 8px" }} onClick={() => delCodexRoute(r.id)}><IconTrash /></button>
              </div>
            </div>
          ))
        ) : (
          <div className="empty-state">
            <div className="empty-icon">--</div>
            <h3>{t("No Codex routes configured")}</h3>
            <p>{t("Add a route above to start mapping Codex models.")}</p>
          </div>
        )}
      </div>
    </div>
  );

  const codexPpStatusBadge = (status: string | undefined) => {
    if (status === "ok") return "badge-green";
    if (status === "warn") return "badge-amber";
    if (status === "error") return "badge-red";
    if (status === "installed") return "badge-green";
    if (status === "missing" || status === "needs_reload") return "badge-amber";
    if (status === "unknown") return "badge-gray";
    return "badge-gray";
  };

  const storeEntries = (codexPpStore?.entries ?? []).filter(entry => {
    const q = codexPpSearch.trim().toLowerCase();
    if (!q) return true;
    return [
      entry.manifest.name,
      entry.manifest.description ?? "",
      entry.repo,
      ...(entry.manifest.tags ?? []),
    ].join(" ").toLowerCase().includes(q);
  });

  const codexPpLogText = codexPpLogLines.length > 0
    ? codexPpLogLines.join("\n")
    : [codexPpCli?.stdout, codexPpCli?.stderr].filter(Boolean).join("\n") || "(no output)";

  const CodexPpPreflightCard = () => (
    <div className="card">
      <div className="card-title">安装前预检</div>
      <p style={{ color: "var(--muted)", marginBottom: 14 }}>
        {codexPpPreflight?.summary ?? "Run preflight to verify Node.js, npm, bootstrap tools, and Codex.app before patching."}
      </p>
      <div className="route-list" style={{ marginBottom: 0 }}>
        {(codexPpPreflight?.checks ?? []).map(check => (
          <div key={check.name} className="route-item">
            <div className="route-info">
              <div className="route-name">{check.name}</div>
              <div className="route-path">{check.detail}</div>
            </div>
            <span className={`badge ${codexPpStatusBadge(check.status)}`}>{check.status}</span>
          </div>
        ))}
      </div>
      <div className="qa-buttons" style={{ marginTop: 16 }}>
        <button className="btn" onClick={() => void refreshCodexPp(false)} disabled={codexPpLoading}>
          <IconRefresh /> Run Preflight
        </button>
        <span className={`badge ${codexPpPreflight?.ready ? "badge-green" : "badge-red"}`}>
          {codexPpPreflight?.ready ? "Ready to install" : "Blocked"}
        </span>
      </div>
    </div>
  );

  const CodexPpOverviewCard = () => (
    <div className="card">
      <div className="card-title">Codex++</div>
      <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
        <span className="info-key">{t("Status")}</span>
        <span className="info-val">
          <span className={`badge ${codexPpInstall?.installed ? "badge-green" : "badge-gray"}`}>
            {codexPpInstall?.installed ? "Installed" : "Not installed"}
          </span>
        </span>
        <span className="info-key">{t("Version")}</span>
        <span className="info-val">{codexPpInstall?.version ?? "-"}</span>
        <span className="info-key">Auto Update</span>
        <span className="info-val">{codexPpInstall?.auto_update ? "Enabled" : "Disabled"}</span>
        <span className="info-key">Safe Mode</span>
        <span className="info-val">{codexPpInstall?.safe_mode ? "On" : "Off"}</span>
        <span className="info-key">CLI</span>
        <span className="info-val">{codexPpInstall?.cli_path ?? "Bootstrap via Gateway Switch"}</span>
        <span className="info-key">Install Mode</span>
        <span className="info-val">{codexPpPreflight?.install_mode ?? "-"}</span>
        <span className="info-key">User Root</span>
        <span className="info-val">{codexPpInstall?.user_root ?? "-"}</span>
      </div>
      <div className="qa-buttons" style={{ marginTop: 16 }}>
        {!codexPpInstall?.installed && (
          <>
            <button className="btn btn-primary" onClick={() => void runCodexPpCli("install")} disabled={codexPpLoading}>
              Install Codex++
            </button>
            <button className="btn" onClick={() => void runCodexPpCli("install-local")} disabled={codexPpLoading}>
              Install with Local Signing
            </button>
          </>
        )}
        {codexPpInstall?.installed && (
          <>
            <button className="btn btn-primary" onClick={() => void runCodexPpCli("install")} disabled={codexPpLoading}>
              Reapply Patch
            </button>
            <button className="btn" onClick={() => void runCodexPpCli("install-local")} disabled={codexPpLoading}>
              Reapply with Local Signing
            </button>
          </>
        )}
        <button className="btn" onClick={() => void refreshCodexPp(true)} disabled={codexPpLoading}><IconRefresh /> {t("Refresh")}</button>
        <button className="btn" onClick={() => void openCodexPpPath("root")}>Open Root</button>
        <button className="btn" onClick={() => void openCodexPpPath("tweaks")}>Open Tweaks</button>
      </div>
      <p style={{ marginTop: 12, marginBottom: 0, color: "var(--muted)", fontSize: 12 }}>
        Gateway Switch will auto-detect an existing <code>codexplusplus</code> CLI. If none is found, it falls back to the official bootstrap installer and patches <code>Codex.app</code> for you.
      </p>
    </div>
  );

  const CodexPpEnhancePage = () => {
    const uiEnhancement = codexPpTweaks.find(tw => tw.id === CODEX_PP_UI_IMPROVEMENTS_TWEAK_ID);
    const uiSafeModeOn = uiEnhancement ? !uiEnhancement.enabled : false;
    return (
    <div>
      <div className="page-header page-header-row">
        <div>
          <h1>Codex++ 页面增强</h1>
          <p>可从这里一键安装并 patch Codex.app，安装完成后再读取本地 tweak manifest、入口文件和启用状态。</p>
        </div>
        <div className="qa-buttons" style={{ margin: 0 }}>
          <button className="btn" onClick={() => void refreshCodexPp(false)} disabled={codexPpLoading}><IconRefresh /> {t("Refresh")}</button>
          <button className="btn" onClick={() => void runCodexPpCli("safe-mode-status")} disabled={codexPpLoading}>Safe Mode</button>
          <button className="btn btn-primary" onClick={() => void setCodexPpUiSafeMode(true)} disabled={codexPpLoading || uiSafeModeOn}>
            UI Safe On
          </button>
        </div>
      </div>
      <div className="two-col">
        {CodexPpOverviewCard()}
        {CodexPpPreflightCard()}
      </div>
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">UI Safe Mode</div>
        <p style={{ color: "var(--muted)", marginBottom: 14 }}>
          一键禁用页面增强 tweak，保留路由、脚本市场、历史会话修复、watcher 和 CLI shim。适合 Codex UI 错位或设置页异常时临时排障。
        </p>
        <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
          <span className="info-key">Managed Tweak</span>
          <span className="info-val">{CODEX_PP_UI_IMPROVEMENTS_TWEAK_ID}</span>
          <span className="info-key">Current State</span>
          <span className="info-val">{uiEnhancement ? (uiSafeModeOn ? "UI safe mode on" : "Page enhancement active") : "Tweak not installed"}</span>
          <span className="info-key">Other Features</span>
          <span className="info-val">Kept enabled</span>
        </div>
        <div className="qa-buttons">
          <button className="btn btn-primary" onClick={() => void setCodexPpUiSafeMode(true)} disabled={codexPpLoading || !uiEnhancement || uiSafeModeOn}>
            Disable Page Enhancement
          </button>
          <button className="btn" onClick={() => void setCodexPpUiSafeMode(false)} disabled={codexPpLoading || !uiEnhancement || !uiSafeModeOn}>
            Re-enable Page Enhancement
          </button>
        </div>
      </div>
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">Tweak Summary</div>
        <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
          <span className="info-key">Installed Tweaks</span>
          <span className="info-val">{codexPpTweaks.length}</span>
          <span className="info-key">Enabled</span>
          <span className="info-val">{codexPpTweaks.filter(tw => tw.enabled).length}</span>
          <span className="info-key">Updates</span>
          <span className="info-val">{codexPpTweaks.filter(tw => tw.update_available).length}</span>
          <span className="info-key">Tweaks Dir</span>
          <span className="info-val">{codexPpInstall?.tweaks_dir ?? "-"}</span>
        </div>
      </div>
      <div className="tweak-grid">
        {codexPpTweaks.map(tweak => (
          <div key={tweak.id} className="tweak-card">
            <div className="tweak-card-head">
              <div className="tweak-card-icon">{tweak.icon_url ? <img src={tweak.icon_url} alt="" /> : tweak.name.slice(0, 2)}</div>
              <div className="tweak-card-info">
                <div className="tweak-card-name">{tweak.name}</div>
                <div className="tweak-card-version">{tweak.version} · {tweak.scope}</div>
              </div>
              <span className={`badge ${tweak.enabled ? "badge-green" : "badge-gray"}`}>{tweak.enabled ? t("Active") : t("Disabled")}</span>
            </div>
            <p className="tweak-card-desc">{tweak.description ?? tweak.id}</p>
            <div className="tweak-card-tags">
              {tweak.tags.map(tag => <span key={tag} className="tweak-tag">{tag}</span>)}
              {!tweak.entry_exists && <span className="tweak-tag">missing entry</span>}
              {tweak.update_available && <span className="tweak-tag">update {tweak.latest_version}</span>}
            </div>
            <div className="tweak-card-footer">
              <span className="tweak-card-author">{tweak.author ?? tweak.github_repo ?? tweak.id}</span>
              <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                <button className="btn" onClick={() => void toggleCodexPpTweak(tweak.id, !tweak.enabled)} disabled={codexPpLoading}>
                  {tweak.enabled ? "Disable" : "Enable"}
                </button>
                <button className="btn btn-danger" onClick={() => void uninstallCodexPpTweak(tweak.id)} disabled={codexPpLoading}><IconTrash /></button>
              </div>
            </div>
          </div>
        ))}
        {codexPpTweaks.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">++</div>
            <h3>No Codex++ tweaks found</h3>
            <p>Install Codex++ first from the overview card, then open the market tab to add approved tweaks.</p>
          </div>
        )}
      </div>
    </div>
    );
  };

  const CodexPpMarketPage = () => (
    <div>
      <div className="page-header page-header-row">
        <div>
          <h1>Codex++ 脚本市场</h1>
          <p>优先恢复 Codex++ 原生推荐脚本；下方仍保留官方 Tweak Store。</p>
        </div>
        <div className="qa-buttons" style={{ margin: 0 }}>
          <input value={codexPpSearch} onChange={e => setCodexPpSearch(e.target.value)} placeholder="Search tweaks..." style={{ minWidth: 220 }} />
          <button className="btn" onClick={() => void refreshCodexPp(true)} disabled={codexPpLoading}><IconRefresh /> Refresh Store</button>
        </div>
      </div>
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">Recommended Scripts</div>
        <p style={{ color: "var(--muted)", marginBottom: 14 }}>
          {codexPpRecommendedScripts?.summary ?? "Detecting Codex++ native user-script storage..."}
        </p>
        <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
          <span className="info-key">Storage Mode</span>
          <span className="info-val">{codexPpRecommendedScripts?.storage_mode ?? "-"}</span>
          <span className="info-key">Storage Path</span>
          <span className="info-val">{codexPpRecommendedScripts?.storage_path ?? "Not detected"}</span>
        </div>
        <div className="route-list" style={{ marginBottom: 0 }}>
          {(codexPpRecommendedScripts?.scripts ?? []).map(script => (
            <div key={script.id} className="route-item">
              <div className="route-info">
                <div className="route-name">{script.name}</div>
                <div className="route-path">{script.file_name} · {script.description}</div>
              </div>
              <span className={`badge ${codexPpStatusBadge(script.status)}`}>{script.status}</span>
            </div>
          ))}
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={() => void installCodexPpRecommendedScripts()} disabled={codexPpLoading || codexPpRecommendedScripts?.storage_mode !== "codex_user_scripts"}>
            Install Recommended Scripts
          </button>
          <button className="btn" onClick={() => void refreshCodexPp(false)} disabled={codexPpLoading}>
            <IconRefresh /> Refresh Script Status
          </button>
          <button className="btn" onClick={() => void openCodexPpPath("log")}>
            Open Logs
          </button>
        </div>
        {codexPpRecommendedScripts?.storage_mode !== "codex_user_scripts" && (
          <p style={{ marginTop: 12, marginBottom: 0, color: "var(--muted)", fontSize: 12 }}>
            当前 Codex++ runtime 未暴露原生用户脚本目录。Gateway Switch 会保持安全门禁，不会把脚本写入未知路径。
          </p>
        )}
      </div>
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">Upstream Tweak Store</div>
        <p style={{ color: "var(--muted)", marginBottom: 14 }}>
          {codexPpStore?.summary ?? "Fetches the live approved Codex++ Tweak Store registry and derives safe archive URLs from approved commits."}
        </p>
        <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
          <span className="info-key">Source URL</span>
          <span className="info-val">{codexPpStore?.sourceUrl ?? "https://b-nnett.github.io/codex-plusplus/store/index.json"}</span>
          <span className="info-key">Generated At</span>
          <span className="info-val">{codexPpStore?.generatedAt ?? "-"}</span>
          <span className="info-key">Fetched At</span>
          <span className="info-val">{codexPpStore?.fetchedAt ?? "-"}</span>
          <span className="info-key">Entries</span>
          <span className="info-val">{codexPpStore?.entries.length ?? 0}</span>
        </div>
        {(codexPpStore?.legacyRecommendations?.length ?? 0) > 0 && (
          <div className="route-list" style={{ marginTop: 14, marginBottom: 0 }}>
            {codexPpStore?.legacyRecommendations?.map(item => (
              <div key={item.name} className="route-item">
                <div className="route-info">
                  <div className="route-name">{item.name}</div>
                  <div className="route-path">
                    {item.note}{item.replacementEntryId ? ` · Replacement: ${item.replacementEntryId}` : ""}
                  </div>
                </div>
                <span className={`badge ${item.exactMatch ? "badge-green" : "badge-amber"}`}>
                  {item.exactMatch ? "matched" : "legacy"}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="tweak-grid">
        {storeEntries.map(entry => (
          <div key={entry.id} className="tweak-card">
            <div className="tweak-card-head">
              <div className="tweak-card-icon">{entry.manifest.iconUrl ? <img src={entry.manifest.iconUrl} alt="" /> : entry.manifest.name.slice(0, 2)}</div>
              <div className="tweak-card-info">
                <div className="tweak-card-name">{entry.manifest.name}</div>
                <div className="tweak-card-version">{entry.manifest.version} · {entry.manifest.scope ?? "renderer"}</div>
              </div>
              <span className={`badge ${entry.installed ? "badge-green" : "badge-gray"}`}>{entry.installed ? "Installed" : "Remote"}</span>
            </div>
            <p className="tweak-card-desc">{entry.manifest.description ?? entry.repo}</p>
            <div className="tweak-card-tags">
              {(entry.manifest.tags ?? []).map(tag => <span key={tag} className="tweak-tag">{tag}</span>)}
              <span className="tweak-tag">{entry.approvedCommitSha.slice(0, 7)}</span>
              {entry.installed_version && <span className="tweak-tag">installed {entry.installed_version}</span>}
            </div>
            <div className="info-grid" style={{ marginTop: 10, paddingTop: 10 }}>
              <span className="info-key">Repo</span>
              <span className="info-val">{entry.repo}</span>
              <span className="info-key">Archive</span>
              <span className="info-val">{entry.archiveUrl ?? "Derived after registry validation"}</span>
              {entry.installedPath && (
                <>
                  <span className="info-key">Installed Path</span>
                  <span className="info-val">{entry.installedPath}</span>
                </>
              )}
            </div>
            <div className="tweak-card-footer">
              <span className="tweak-card-author">{entry.repo}</span>
              <div className="qa-buttons" style={{ margin: 0, gap: 4 }}>
                <button className="btn btn-primary" onClick={() => void installCodexPpTweak(entry)} disabled={codexPpLoading}>
                  {entry.installed ? "Reinstall" : "Install"}
                </button>
                <button className="btn" onClick={() => window.open(`https://github.com/${entry.repo}`, "_blank")}>GitHub</button>
                {entry.archiveUrl && <button className="btn" onClick={() => void copyPath(entry.archiveUrl ?? "")}>Copy URL</button>}
                {entry.releaseUrl && <button className="btn" onClick={() => window.open(entry.releaseUrl ?? "", "_blank")}>Release</button>}
                {entry.reviewUrl && <button className="btn" onClick={() => window.open(entry.reviewUrl ?? "", "_blank")}>Review</button>}
              </div>
            </div>
          </div>
        ))}
        {storeEntries.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">++</div>
            <h3>No store entries loaded</h3>
            <p>Click Refresh Store to fetch the approved Codex++ tweak index.</p>
          </div>
        )}
      </div>
    </div>
  );

  const CodexPpSessionsPage = () => (
    <div>
      <div className="page-header">
        <h1>历史会话修复</h1>
        <p>安全版先提供 Codex++ 会话修复指引和维护入口，不直接写未知私有会话数据库。</p>
      </div>
      <div className="two-col">
        <div className="card">
          <div className="card-title">安全修复策略</div>
          <div className="note-grid">
            <div><strong>自动修复边界</strong><p>Gateway Switch 只调用 codex++ CLI 或管理公开配置，不猜测 IndexedDB/SQLite 私有结构。</p></div>
            <div><strong>推荐流程</strong><p>先运行 status/doctor，再根据结果执行 repair 或 update-codex，最后重启 Codex。</p></div>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            <button className="btn" onClick={() => void runCodexPpCli("status")} disabled={codexPpLoading}>Status</button>
            <button className="btn" onClick={() => void runCodexPpCli("doctor")} disabled={codexPpLoading}>Doctor</button>
            <button className="btn btn-primary" onClick={() => void runCodexPpCli("repair")} disabled={codexPpLoading}>Repair</button>
          </div>
        </div>
        {CodexPpOverviewCard()}
      </div>
    </div>
  );

  const CodexPpDiagnosticsPage = () => (
    <div>
      <div className="page-header page-header-row">
        <div>
          <h1>Codex++ 诊断维护</h1>
          <p>检查 watcher、runtime、CLI、safe mode，并提供受控维护命令。</p>
        </div>
        <div className="qa-buttons" style={{ margin: 0 }}>
          <button className="btn" onClick={() => void refreshCodexPp(false)} disabled={codexPpLoading}><IconRefresh /> {t("Refresh")}</button>
          <button className="btn" onClick={() => void runCodexPpCli("doctor")} disabled={codexPpLoading}>Doctor</button>
        </div>
      </div>
      <div className="two-col">
        <div className="card">
          <div className="card-title">{codexPpHealth?.title ?? "Codex++ Health"}</div>
          <p style={{ color: "var(--muted)", marginBottom: 14 }}>{codexPpHealth?.summary ?? "No health report yet."}</p>
          <div className="route-list" style={{ marginBottom: 0 }}>
            {(codexPpHealth?.checks ?? []).map(check => (
              <div key={check.name} className="route-item">
                <div className="route-info">
                  <div className="route-name">{check.name}</div>
                  <div className="route-path">{check.detail}</div>
                </div>
                <span className={`badge ${codexPpStatusBadge(check.status)}`}>{t(check.status)}</span>
              </div>
            ))}
          </div>
        </div>
        {CodexPpPreflightCard()}
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <div className="card-title">Maintenance Commands</div>
        <div className="qa-buttons" style={{ marginTop: 0 }}>
          <button className="btn btn-primary" onClick={() => void runCodexPpCli("install")} disabled={codexPpLoading}>Install / Patch</button>
          <button className="btn" onClick={() => void runCodexPpCli("install-local")} disabled={codexPpLoading}>Install Local</button>
          <button className="btn" onClick={() => void runCodexPpCli("status")} disabled={codexPpLoading}>Status</button>
          <button className="btn btn-primary" onClick={() => void runCodexPpCli("repair")} disabled={codexPpLoading}>Repair</button>
          <button className="btn" onClick={() => void runCodexPpCli("repair-local")} disabled={codexPpLoading}>Repair Local</button>
          <button className="btn" onClick={() => void runCodexPpCli("update")} disabled={codexPpLoading}>Update Codex++</button>
          <button className="btn" onClick={() => void runCodexPpCli("update-codex")} disabled={codexPpLoading}>Update Codex</button>
          <button className="btn" onClick={() => void runCodexPpCli("safe-mode-on")} disabled={codexPpLoading}>Safe On</button>
          <button className="btn" onClick={() => void runCodexPpCli("safe-mode-off")} disabled={codexPpLoading}>Safe Off</button>
        </div>
        {(codexPpCli || codexPpLogLines.length > 0 || codexPpLoading) && (
          <div className="cli-output">
            <strong>{codexPpCli?.command ?? "codex++ live output"}</strong>
            <pre>{codexPpLogText}</pre>
          </div>
        )}
      </div>
    </div>
  );

  const CodexPage = () => (
    <div>
      <div className="codex-tabs">
        {[
          ["routes", "路由"],
          ["enhance", "页面增强"],
          ["market", "脚本市场"],
          ["sessions", "历史会话修复"],
          ["diagnostics", "诊断维护"],
        ].map(([id, label]) => (
          <button key={id} className={`codex-tab ${codexTab === id ? "active" : ""}`} onClick={() => setCodexTab(id as CodexTab)}>
            {label}
          </button>
        ))}
      </div>
      {codexTab === "routes" && CodexRoutesPage()}
      {codexTab === "enhance" && CodexPpEnhancePage()}
      {codexTab === "market" && CodexPpMarketPage()}
      {codexTab === "sessions" && CodexPpSessionsPage()}
      {codexTab === "diagnostics" && CodexPpDiagnosticsPage()}
    </div>
  );

  // =====================================================
  //  MCP SYNC PAGE
  // =====================================================
  const McpSyncPage = () => {
    const preview = mcpPreview ?? MOCK_MCP_PREVIEW;
    const readyTargets = preview.targets.filter(tg => tg.writable && tg.parse_status !== "解析失败" && tg.parse_status !== "权限不足").length;
    const blockedTargets = preview.targets.length - readyTargets;
    const lastWritten = mcpResult?.written_targets.filter(r => r.ok).length ?? 0;
    const mcpStatusBadge = (target: McpTargetStatus) => {
      if (target.parse_status === "正常") return "badge-green";
      if (target.parse_status === "文件不存在") return target.writable ? "badge-amber" : "badge-red";
      if (target.parse_status === "解析失败" || target.parse_status === "权限不足") return "badge-red";
      return "badge-gray";
    };
    const sourceLabel = (source: string) => {
      if (source === "claude_desktop") return "Desktop";
      if (source === "claude_code") return "Code";
      if (source === "codex") return "Codex";
      return source;
    };

    return (
      <div>
        <div className="page-header page-header-row">
          <div>
            <h1>{t("MCP Configuration Sync")}</h1>
            <p>{t("Synchronize MCP Servers across Claude Desktop, Claude Code, and Codex.")}</p>
          </div>
          <div className="qa-buttons" style={{ margin: 0 }}>
            <button className="btn" onClick={refreshMcpStatus} disabled={mcpLoading || mcpSyncing}>
              <IconRefresh /> {t("Refresh Status")}
            </button>
            <button className="btn" onClick={previewMcpSync} disabled={mcpLoading || mcpSyncing}>
              <IconSearch /> {t("Preview Sync")}
            </button>
            <button className="btn btn-primary" onClick={runMcpSync} disabled={!preview.can_sync || mcpLoading || mcpSyncing}>
              <IconShuffle /> {mcpSyncing ? t("Syncing...") : t("Run Sync")}
            </button>
          </div>
        </div>

        <div className="kpi-row mcp-kpi-row">
          <div className="kpi-card">
            <div className="kpi-icon green"><IconCheck /></div>
            <div className="kpi-info">
              <div className="kpi-label">{t("Targets Ready")}</div>
              <div className="kpi-value">{readyTargets}/{preview.targets.length}</div>
              <span className={`kpi-badge ${blockedTargets ? "red" : "green"}`}>{blockedTargets ? `${blockedTargets} ${t("Blocked")}` : t("Ready")}</span>
            </div>
          </div>
          <div className="kpi-card">
            <div className="kpi-icon blue"><IconShuffle /></div>
            <div className="kpi-info">
              <div className="kpi-label">{t("Merged Servers")}</div>
              <div className="kpi-value">{preview.merged_count}</div>
              <span className="kpi-badge blue">{preview.source_count} {t("Sources")}</span>
            </div>
          </div>
          <div className="kpi-card">
            <div className="kpi-icon amber"><IconZap /></div>
            <div className="kpi-info">
              <div className="kpi-label">{t("Conflicts")}</div>
              <div className="kpi-value">{preview.conflict_count}</div>
              <span className="kpi-badge amber">{preview.resolved_count} {t("Resolved")}</span>
            </div>
          </div>
          <div className="kpi-card">
            <div className="kpi-icon purple"><IconDownload /></div>
            <div className="kpi-info">
              <div className="kpi-label">{t("Last Sync")}</div>
              <div className="kpi-value">{mcpResult ? lastWritten : "--"}</div>
              <span className="kpi-badge muted">{mcpResult ? t("Written targets") : t("Not run yet")}</span>
            </div>
          </div>
        </div>

        <div className="section-label">{t("Target Configurations")}</div>
        <div className="mcp-target-grid">
          {preview.targets.map(target => (
            <div className="card mcp-target-card" key={target.target}>
              <div className="mcp-target-head">
                <div>
                  <div className="card-title">{target.label}</div>
                  <span className={`badge ${mcpStatusBadge(target)}`}>{t(target.parse_status)}</span>
                </div>
                <span className="badge badge-blue">{target.format}</span>
              </div>
              <div className="info-grid">
                <span className="info-key">{t("Config File")}</span>
                <span className="info-val">{target.config_path}</span>
                <span className="info-key">{t("Servers")}</span>
                <span className="info-val">{target.server_count}</span>
                <span className="info-key">{t("Exists")}</span>
                <span className="info-val">{target.config_exists ? t("Yes") : t("No")}</span>
                <span className="info-key">{t("Writable")}</span>
                <span className="info-val">{target.writable ? t("Yes") : t("No")}</span>
                <span className="info-key">{t("Backup")}</span>
                <span className="info-val">{target.backup_path ?? t("None")}</span>
              </div>
              {target.error && <p className="mcp-warning-line">{target.error}</p>}
              <button className="btn" onClick={() => void copyPath(target.config_path)}>
                <IconDownload /> {t("Copy Path")}
              </button>
            </div>
          ))}
        </div>

        <div className="two-col">
          <div className="card">
            <div className="card-title">{t("Safety & Warnings")}</div>
            {preview.warnings.length > 0 ? (
              <div className="mcp-warning-list">
                {preview.warnings.map(warning => <p key={warning}><IconX /> {t(warning)}</p>)}
              </div>
            ) : (
              <div className="mcp-safe-note"><IconCheck /> {t("No blocking warnings. Backups are created before writing existing files.")}</div>
            )}
          </div>
          <div className="card">
            <div className="card-title">{t("Execution Result")}</div>
            {mcpResult ? (
              <div className="mcp-result-list">
                <div className="report-path">
                  <span>{t("Generated at")}</span>
                  <code>{mcpResult.generated_at}</code>
                </div>
                {mcpResult.written_targets.map(result => (
                  <div className="mcp-write-row" key={result.target}>
                    <span className={`badge ${result.ok ? "badge-green" : "badge-red"}`}>{result.ok ? t("OK") : t("Error")}</span>
                    <strong>{result.label}</strong>
                    <code>{result.message}</code>
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty-state">
                <div className="empty-icon">--</div>
                <h3>{t("No sync result yet.")}</h3>
                <p>{t("Click Preview Sync to inspect merged servers before writing.")}</p>
              </div>
            )}
          </div>
        </div>

        <div className="section-label">{t("Sync Preview")}</div>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t("Server")}</th>
                <th>{t("Type")}</th>
                <th>{t("Sources")}</th>
                <th>{t("Completeness")}</th>
                <th>{t("Credentials")}</th>
                <th>{t("Action")}</th>
              </tr>
            </thead>
            <tbody>
              {preview.servers.map(server => (
                <tr key={server.name}>
                  <td style={{ fontWeight: 600 }}>{server.name}</td>
                  <td><span className="badge badge-blue">{server.server_type}</span></td>
                  <td>
                    <div className="mcp-source-list">
                      {server.sources.map(source => <span key={source} className="badge badge-gray">{sourceLabel(source)}</span>)}
                    </div>
                  </td>
                  <td>{server.completeness}/4</td>
                  <td>
                    {server.credential_keys.length > 0 ? (
                      <div className="mcp-secret-list">{server.credential_keys.map(key => <code key={key}>{key}</code>)}</div>
                    ) : (
                      <span className="muted-pill">{t("None")}</span>
                    )}
                  </td>
                  <td><span className={`badge ${server.action === "冲突合并" ? "badge-amber" : "badge-green"}`}>{t(server.action)}</span></td>
                </tr>
              ))}
              {preview.servers.length === 0 && (
                <tr>
                  <td colSpan={6}>
                    <div className="empty-state">
                      <div className="empty-icon">--</div>
                      <h3>{t("No MCP servers found")}</h3>
                      <p>{t("Click Preview Sync to inspect merged servers before writing.")}</p>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {mcpResult && (
          <div className="card" style={{ marginTop: 20 }}>
            <div className="card-title">{t("Logs")}</div>
            <div className="mcp-log-list">
              {mcpResult.logs.map(log => <code key={log}>{log}</code>)}
            </div>
          </div>
        )}
      </div>
    );
  };

  // =====================================================
  //  LOGS PAGE
  // =====================================================
  const LogsPage = () => {
    const filteredLogs = searchQuery
      ? logs.filter(l =>
          l.claude_alias.toLowerCase().includes(searchQuery.toLowerCase()) ||
          l.provider_id.toLowerCase().includes(searchQuery.toLowerCase()) ||
          l.upstream_model.toLowerCase().includes(searchQuery.toLowerCase()) ||
          l.request_id.toLowerCase().includes(searchQuery.toLowerCase())
        )
      : logs;

    return (
      <div>
        <div className="page-header">
          <h1>{t("Request Logs")}</h1>
          <p>{t("Monitor gateway request activity")}</p>
        </div>

        <div className="qa-buttons" style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-xs)", background: "var(--surface)", flex: 1, maxWidth: 360 }}>
            <IconSearch />
            <input
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              placeholder={t("Search logs...")}
              style={{ border: "none", outline: "none", fontSize: 13, flex: 1, background: "transparent", fontFamily: "inherit", color: "var(--fg)", minWidth: 0 }}
            />
          </div>
          <button className="btn" onClick={() => void loadAll()}>
            <IconRefresh /> {t("Refresh")}
          </button>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t("Time")}</th>
                <th>{t("Requested Model")}</th>
                <th>{t("Provider")}</th>
                <th>{t("Real Upstream")}</th>
                <th>{t("Mode")}</th>
                <th>{t("Status")}</th>
                <th>{t("Duration")}</th>
                {filteredLogs.some(l => l.error_summary) && <th>{t("Trace / Error")}</th>}
              </tr>
            </thead>
            <tbody>
              {filteredLogs.map(l => (
                <tr key={l.request_id + l.created_at}>
                  <td style={{ fontSize: 12, color: "var(--muted)", fontFamily: "var(--font-mono)" }}>
                    {l.created_at.replace("T", " ").slice(0, 19)}
                  </td>
                  <td style={{ fontWeight: 600 }}>{l.claude_alias}</td>
                  <td><span className="badge badge-blue">{l.provider_id}</span></td>
                  <td style={{ fontSize: 12 }}>{l.upstream_model}</td>
                  <td><span className={`badge ${l.is_stream ? "badge-amber" : "badge-blue"}`}>{l.is_stream ? "stream" : "sync"}</span></td>
                  <td>
                    <span className={`badge ${l.status_code && l.status_code < 400 ? "badge-green" : l.status_code ? "badge-red" : "badge-gray"}`}>
                      {l.status_code ?? "pending"}
                    </span>
                  </td>
                  <td>{l.duration_ms ? `${l.duration_ms}ms` : "-"}</td>
                  {filteredLogs.some(lg => lg.error_summary) && (
                    <td style={{ fontSize: 12, color: isToolTrace(l.error_summary) ? "var(--muted)" : "var(--red)", maxWidth: 320 }}>
                      <span title={l.error_summary || ""} style={{ display: "block", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", wordBreak: "normal" }}>{formatLogSummary(l.error_summary)}</span>
                    </td>
                  )}
                </tr>
              ))}
              {filteredLogs.length === 0 && (
                <tr>
                  <td colSpan={filteredLogs.some(l => l.error_summary) ? 8 : 7}>
                    <div className="empty-state">
                      <div className="empty-icon">--</div>
                      <h3>{searchQuery ? t("No matching logs") : t("No logs yet")}</h3>
                      <p>{searchQuery ? t("Try a different search query.") : t("Logs will appear here once requests are made.")}</p>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    );
  };

  const statusBadgeClass = (statusText: string) => {
    if (statusText === "ok" || statusText === "fixed") return "badge-green";
    if (statusText === "error") return "badge-red";
    return "badge-amber";
  };

  const scoreClass = (score: number) => {
    if (score >= 85) return "ok";
    if (score >= 70) return "warn";
    return "bad";
  };

  // =====================================================
  //  COLD START PAGE
  // =====================================================
  const ColdStartPage = () => {
    const report = coldStart ?? MOCK_COLDSTART;
    const phaseSteps = report.steps.slice(-8);
    const matrix = report.capabilities;
    return (
      <div>
        <div className="page-header page-header-row">
          <div>
            <h1>{t("Cold Start Doctor")}</h1>
            <p>{t("Claude Desktop 与 Codex 第三方模型接入后的状态确认、冷启动修复和安全报告")}</p>
          </div>
          <button className="btn btn-primary" onClick={runColdStartRepair} disabled={coldStartRunning}>
            <IconZap /> {coldStartRunning ? t("Running...") : t("Run Check & Safe Fixes")}
          </button>
        </div>

        <div className="cold-hero">
          <div>
            <span className="eyebrow">{t("Phase A · Readiness Overview")}</span>
            <h2>{t(report.verdict)}</h2>
            <p>{t(report.most_important_fix)}</p>
          </div>
          <div className={`score-ring ${scoreClass(report.overall_score)}`}>
            <strong>{report.overall_score}%</strong>
            <span>{t("Overall")}</span>
          </div>
        </div>

        <div className="cold-score-grid">
          <div className="cold-score-card">
            <span>Claude Desktop</span>
            <strong>{report.claude_score}%</strong>
            <em>{desktop?.managed ? t("Managed") : t("Needs binding")}</em>
          </div>
          <div className="cold-score-card">
            <span>{t("Codex App")}</span>
            <strong>{report.codex_score}%</strong>
            <em>{codexBinding?.managed ? t("Managed") : t("Needs binding")}</em>
          </div>
          <div className="cold-score-card">
            <span>{t("MCP / Tools")}</span>
            <strong>{matrix.filter(i => i.status === "ok").length}/{matrix.length}</strong>
            <em>{t("Observable checks passed")}</em>
          </div>
          <div className="cold-score-card">
            <span>{t("Security")}</span>
            <strong>{matrix.some(i => i.target === "Security" && i.status !== "ok") ? t("Review") : t("OK")}</strong>
            <em>{t("Third-party routing risk")}</em>
          </div>
        </div>

        <div className="two-col">
          <div className="card">
            <div className="card-title">{t("Phase B · Execution & Repair Log")}</div>
            <div className="cold-timeline">
              {phaseSteps.map((step, index) => (
                <div key={`${step.id}-${index}`} className="cold-step">
                  <span className={`cold-step-dot ${step.status}`} />
                  <div>
                    <div className="cold-step-head">
                      <strong>{t(step.label)}</strong>
                      <span className={`badge ${statusBadgeClass(step.status)}`}>{t(step.status)}</span>
                    </div>
                    <p>{t(step.detail)}</p>
                    <small>{step.target} · {step.timestamp}</small>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="card">
            <div className="card-title">{t("Fix Results")}</div>
            <div className="cold-result-block">
              <strong>{t("Auto fixes applied")}</strong>
              {report.auto_fixes_applied.length > 0 ? (
                report.auto_fixes_applied.map(item => <p key={item}><IconCheck /> {t(item)}</p>)
              ) : (
                <p className="muted-line">{t("No automatic fix has been applied in the latest check.")}</p>
              )}
            </div>
            <div className="cold-result-block">
              <strong>{t("Manual fixes required")}</strong>
              {report.manual_fixes_required.length > 0 ? (
                report.manual_fixes_required.slice(0, 5).map(item => <p key={item}><IconArrowRight /> {t(item)}</p>)
              ) : (
                <p className="muted-line">{t("No manual action required.")}</p>
              )}
            </div>
            {report.report_path && (
              <div className="report-path">
                <span>{t("Report saved")}</span>
                <code>{report.report_path}</code>
              </div>
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-title">{t("Phase C · Capability Matrix")}</div>
          <div className="cold-matrix">
            {matrix.map(item => (
              <div key={`${item.target}-${item.name}`} className="cold-matrix-item">
                <div>
                  <span>{item.target}</span>
                  <strong>{t(item.name)}</strong>
                </div>
                <span className={`badge ${statusBadgeClass(item.status)}`}>{t(item.status)}</span>
                <p>{t(item.detail)}</p>
              </div>
            ))}
          </div>
        </div>

        <div className="cold-risk">
          <strong>{t("Biggest Risk")}</strong>
          <p>{t(report.biggest_risk)}</p>
        </div>
      </div>
    );
  };

  // =====================================================
  //  SETTINGS PAGE
  // =====================================================
  const SettingsPage = () => {
    if (!settings) return <div className="empty-state"><h3>{t("Loading...")}</h3></div>;
    return (
      <div>
        <div className="page-header">
          <h1>{t("Settings")}</h1>
          <p>{t("Configure gateway behavior and manage data")}</p>
        </div>

        <div className="two-col">
          {/* Gateway Configuration */}
          <div className="card">
            <div className="card-title">{t("Gateway Configuration")}</div>
            <div className="form-row" style={{ marginBottom: 16 }}>
              <div className="form-field" style={{ gridColumn: "1 / -1" }}>
                <label>{t("Interface Language")}</label>
                <select value={settings.language ?? "zh"} onChange={e => setSettings({ ...settings, language: e.target.value as Language })}>
                  <option value="zh">{t("Chinese")}</option>
                  <option value="en">{t("English")}</option>
                </select>
                <span className="field-hint">{language === "zh" ? "默认中文，必要技术名词保留英文。" : "Default Chinese is available; required technical terms stay in English."}</span>
              </div>
              <div className="form-field" style={{ gridColumn: "1 / -1" }}>
                <label>Theme</label>
                <div className="theme-options">
                  {(["system", "light", "dark"] as ThemeMode[]).map(mode => (
                    <button key={mode} className={`theme-option ${theme === mode ? "active" : ""}`} onClick={() => setTheme(mode)}>
                      {mode === "system" ? "System" : mode === "light" ? "Light" : "Dark"}
                    </button>
                  ))}
                </div>
                <span className="field-hint">主题只影响本机界面，不写入 Gateway 配置文件。</span>
              </div>
            </div>
            <div className="form-row">
              <div className="form-field">
                <label>{t("Listen Host")}</label>
                <input value={settings.listen_host} onChange={e => setSettings({ ...settings, listen_host: e.target.value })} />
              </div>
              <div className="form-field">
                <label>{t("Listen Port")}</label>
                <input type="number" value={settings.listen_port} onChange={e => setSettings({ ...settings, listen_port: Number(e.target.value) })} />
              </div>
              <div className="form-field" style={{ gridColumn: "1 / -1" }}>
                <label>{t("Auth Token")}</label>
                <input value={settings.auth_token} onChange={e => setSettings({ ...settings, auth_token: e.target.value })} />
              </div>
            </div>

            <div style={{ marginTop: 16, display: "flex", flexDirection: "column", gap: 10 }}>
              <div className="toggle-row">
                <span>{t("Auto-start Gateway on launch")}</span>
                <button className={`toggle ${settings.auto_start_gateway ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_start_gateway: !settings.auto_start_gateway })} />
              </div>
              <div className="toggle-row">
                <span>{t("Auto-bind Claude Desktop on launch")}</span>
                <button className={`toggle ${settings.auto_takeover_desktop ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_takeover_desktop: !settings.auto_takeover_desktop })} />
              </div>
            </div>

            <div style={{ marginTop: 16 }}>
              <button className="btn btn-primary" onClick={saveSettings}>
                <IconEdit /> {t("Save Settings")}
              </button>
            </div>
          </div>

          {/* Import / Export */}
          <div className="card">
            <div className="card-title">{t("Import / Export")}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
              <div>
                <div style={{ fontSize: 12, fontWeight: 500, color: "var(--muted)", marginBottom: 6 }}>{t("Import Configuration")}</div>
                <div className="qa-buttons">
                  <input
                    value={importPath}
                    onChange={e => setImportPath(e.target.value)}
                    placeholder="/path/to/config.json"
                    style={{ flex: 1, padding: "8px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-xs)", fontSize: 13, outline: "none", fontFamily: "var(--font-mono)", minWidth: 0, background: "var(--surface)", color: "var(--fg)" }}
                  />
                  <button className="btn" onClick={doImport}><IconUpload /> {t("Import")}</button>
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 500, color: "var(--muted)", marginBottom: 6 }}>{t("Export Configuration")}</div>
                <p style={{ fontSize: 13, color: "var(--muted)", marginBottom: 10 }}>
                  {t("Export all providers, routes, and settings to a JSON file.")}
                </p>
                <button className="btn" onClick={doExport}><IconDownload /> {t("Export to File")}</button>
              </div>
              <div style={{ padding: 16, background: "var(--bg)", borderRadius: "var(--radius-xs)", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: "var(--fg)", marginBottom: 6 }}>{t("Data Storage")}</div>
                <div style={{ fontSize: 12.5, color: "var(--muted)", lineHeight: 1.6 }}>
                  {t("All data is stored under:")}<br />
                  <code style={{ fontSize: 11.5, fontFamily: "var(--font-mono)", wordBreak: "break-all" }}>~/Library/Application Support/Gateway Switch/</code>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  };

  // =====================================================
  //  PAGE ROUTER
  // =====================================================
  const Content = () => {
    switch (page) {
      case "dashboard": return DashboardPage();
      case "claude": return ClaudePage();
      case "claudeCode": return ClaudeCodePage();
      case "codex": return CodexPage();
      case "mcpSync": return McpSyncPage();
      case "coldstart": return ColdStartPage();
      case "providers": return ProvidersPage();
      case "logs": return LogsPage();
      case "settings": return SettingsPage();
    }
  };

  return (
    <div className="app-layout">
      {Sidebar()}
      <main className="main-content">
        {Content()}
        {/* Toast notifications - fixed position, non-blocking */}
        {error && (
          <div className="toast toast-error">
            <IconX />
            <span>{error}</span>
          </div>
        )}
        {success && (
          <div className="toast toast-success">
            <IconCheck />
            <span>{success}</span>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
