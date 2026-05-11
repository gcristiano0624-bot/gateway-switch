import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// ── Types ──
type Page = "dashboard" | "claude" | "claudeCode" | "codex" | "providers" | "logs" | "settings";

type CodexRoute = {
  id: string;
  codex_model: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
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

type Settings = {
  auto_start_gateway: boolean;
  auto_takeover_desktop: boolean;
  listen_host: string;
  listen_port: number;
  auth_token: string;
};

type Health = {
  target: string;
  ok: boolean;
  message: string;
  latency_ms: number | null;
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

const MOCK_CODEX_ROUTES: CodexRoute[] = [
  { id: "codex-mimo", codex_model: "gpt-5.2", display_name: "Codex via MiMo", provider_id: "xiaomimo", upstream_model: "mimo-v2.5-pro", enabled: true },
];

const MOCK_SETTINGS: Settings = {
  auto_start_gateway: true,
  auto_takeover_desktop: false,
  listen_host: "127.0.0.1",
  listen_port: 3456,
  auth_token: "gateway-switch-token",
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
  const [status, setStatus] = useState<Status | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [routes, setRoutes] = useState<ModelRoute[]>([]);
  const [desktop, setDesktop] = useState<DesktopInfo | null>(null);
  const [claudeCode, setClaudeCode] = useState<ClaudeCodeInfo | null>(null);
  const [logs, setLogs] = useState<RequestLog[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [codexHealth, setCodexHealth] = useState<Health | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  // Codex state
  const [codexRoutes, setCodexRoutes] = useState<CodexRoute[]>([]);
  const [codexStatus, setCodexStatus] = useState<CodexGatewayStatus | null>(null);
  const [codexBinding, setCodexBinding] = useState<CodexBindingInfo | null>(null);
  const [codexBindModel, setCodexBindModel] = useState("");
  const [claudeAliases, setClaudeAliases] = useState<ModelAlias[]>([]);
  const [codexAliases, setCodexAliases] = useState<ModelAlias[]>([]);
  const [newClaudeAlias, setNewClaudeAlias] = useState("");
  const [newCodexAlias, setNewCodexAlias] = useState("");
  const codexPort = 3457;
  const [cForm, setCForm] = useState({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "" });
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

  const flash = (msg: string, type: "success" | "error" = "success") => {
    if (type === "success") { setSuccess(msg); setError(null); }
    else { setError(msg); setSuccess(null); }
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
      setSettings(MOCK_SETTINGS);
      setCodexStatus(MOCK_CODEX_STATUS);
      setCodexRoutes(MOCK_CODEX_ROUTES);
      setCodexBinding(MOCK_CODEX_BINDING);
      setClaudeAliases(DEFAULT_CLAUDE_ALIASES.map((alias, index) => ({ id: `mock-claude-${index}`, alias, alias_type: "claude", created_at: null })));
      setCodexAliases(DEFAULT_CODEX_MODELS.map((alias, index) => ({ id: `mock-codex-${index}`, alias, alias_type: "codex", created_at: null })));
      return;
    }

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
      setStatus(s);
      setProviders(p);
      setRoutes(r);
      setDesktop(d);
      setClaudeCode(cc);
      setLogs(l);
      setSettings(cfg);
      setCodexStatus(cs);
      setCodexRoutes(cr);
      setCodexBinding(cb);
      setClaudeAliases(ca);
      setCodexAliases(cma);
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

  // ---- Actions ----
  const startGw = async () => { try { await invoke("start_gateway"); await loadAll(); flash("Gateway started"); } catch (e) { flash(String(e), "error"); } };
  const stopGw = async () => { try { await invoke("stop_gateway"); await loadAll(); flash("Gateway stopped"); } catch (e) { flash(String(e), "error"); } };
  const checkHealth = async () => { try { const h = await invoke<Health>("check_gateway_health"); setHealth(h); } catch (e) { flash(String(e), "error"); } };
  const checkCodexHealth = async () => { try { const h = await invoke<Health>("check_codex_health"); setCodexHealth(h); } catch (e) { flash(String(e), "error"); } };
  const bindDesktop = async () => { try { await invoke("apply_binding"); await loadAll(); flash("Desktop bound"); } catch (e) { flash(String(e), "error"); } };
  const restoreDesktop = async () => { try { await invoke("restore_binding"); await loadAll(); flash("Desktop restored"); } catch (e) { flash(String(e), "error"); } };
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
  const restoreClaudeCode = async () => {
    try {
      const info = await invoke<ClaudeCodeInfo>("restore_claude_code_binding");
      setClaudeCode(info);
      await loadAll();
      flash("Claude Code restored");
    } catch (e) { flash(String(e), "error"); }
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
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delRoute = async (id: string) => {
    try { await invoke("delete_route", { id }); flash("Route deleted"); await loadAll(); } catch (e) { flash(String(e), "error"); }
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
      setCForm({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "" });
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delCodexRoute = async (id: string) => {
    try { await invoke("delete_codex_route", { id }); flash("Codex route deleted"); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const editCodexRoute = (r: CodexRoute) => {
    setEditingC(r.id);
    setCForm({ id: r.id, codex_model: r.codex_model, display_name: r.display_name, provider_id: r.provider_id, upstream_model: r.upstream_model });
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
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 3c3.86 0 7 3.14 7 7s-3.14 7-7 7-7-3.14-7-7 3.14-7 7-7z" fill="currentColor" opacity="0.3"/>
            <path d="M12 4C7.58 4 4 7.58 4 12s3.58 8 8 8 8-3.58 8-8-3.58-8-8-8zm3.5 10.5h-3v3a1 1 0 11-2 0v-3h-3a1 1 0 110-2h3v-3a1 1 0 112 0v3h3a1 1 0 110 2z" fill="currentColor"/>
          </svg>
        </div>
        <div className="brand-text">
          <div className="brand-name">Gateway Switch</div>
          <div className="brand-sub">v1.6.1</div>
        </div>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">Dashboard</div>
        <button className={`nav-item ${page === "dashboard" ? "active" : ""}`} onClick={() => setPage("dashboard")}>
          <IconGrid />
          Dashboard
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">Products</div>
        <button className={`nav-item ${page === "claude" ? "active" : ""}`} onClick={() => setPage("claude")}>
          <IconShuffle />
          Claude
          {routes.length > 0 && <span className="nav-badge">{routes.length}</span>}
        </button>
        <button className={`nav-item ${page === "claudeCode" ? "active" : ""}`} onClick={() => setPage("claudeCode")}>
          <IconTerminal />
          Claude Code
        </button>
        <button className={`nav-item ${page === "codex" ? "active" : ""}`} onClick={() => setPage("codex")}>
          <IconTerminal />
          Codex
          {codexRoutes.length > 0 && <span className="nav-badge">{codexRoutes.length}</span>}
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">Shared</div>
        <button className={`nav-item ${page === "providers" ? "active" : ""}`} onClick={() => setPage("providers")}>
          <IconSun />
          Providers
          {providers.length > 0 && <span className="nav-badge">{providers.length}</span>}
        </button>
      </div>

      <div className="nav-group">
        <div className="nav-group-label">System</div>
        <button className={`nav-item ${page === "logs" ? "active" : ""}`} onClick={() => setPage("logs")}>
          <IconTerminal />
          Logs
        </button>
        <button className={`nav-item ${page === "settings" ? "active" : ""}`} onClick={() => setPage("settings")}>
          <IconSettings />
          Settings
        </button>
      </div>

      <div className="sidebar-footer">
        <span className={`status-dot ${status?.gateway_running || codexStatus?.running ? "on" : "off"}`} />
        <span className="status-text">
          Claude <strong>{status?.gateway_running ? "On" : "Off"}</strong> · Codex <strong>{codexStatus?.running ? "On" : "Off"}</strong>
        </span>
      </div>
    </aside>
  );

  // =====================================================
  //  DASHBOARD PAGE
  // =====================================================
  const DashboardPage = () => (
    <div>
      <div className="page-header">
        <h1>Dashboard</h1>
        <p>Read-only product gateway overview</p>
      </div>

      {/* KPI Row */}
      <div className="kpi-row">
        <div className="kpi-card">
          <div className="kpi-icon green">
            <IconPulse />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">Claude Gateway</div>
            {status?.gateway_running ? (
              <span className="kpi-badge green"><span className="dot" /> Running</span>
            ) : (
              <span className="kpi-badge red"><span className="dot" /> Stopped</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon blue">
            <IconMonitor />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">Codex Gateway</div>
            {codexStatus?.running ? (
              <span className="kpi-badge green"><span className="dot" /> Running</span>
            ) : (
              <span className="kpi-badge red"><span className="dot" /> Stopped</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon blue">
            <IconMonitor />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">App Bindings</div>
            {desktop?.managed || codexBinding?.managed || claudeCode?.managed ? (
              <span className="kpi-badge blue"><span className="dot" /> Managed</span>
            ) : (
              <span className="kpi-badge muted"><span className="dot" /> Unmanaged</span>
            )}
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon amber">
            <IconSun />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">Providers</div>
            <div className="kpi-value">{providers.length}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon purple">
            <IconShuffle />
          </div>
          <div className="kpi-info">
            <div className="kpi-label">Routes</div>
            <div className="kpi-value">{routes.length + codexRoutes.length}</div>
          </div>
        </div>
      </div>

      <div className="two-col">
        <div className="card">
          <div className="card-title">Claude</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">Gateway</span>
            <span className="info-val">
              <span className={`badge ${status?.gateway_running ? "badge-green" : "badge-gray"}`}>
                {status?.gateway_running ? "Running" : "Stopped"}
              </span>
            </span>
            <span className="info-key">Binding</span>
            <span className="info-val">{desktop?.managed ? "Claude Desktop uses Gateway Switch" : "Claude Desktop is unmanaged"}</span>
            <span className="info-key">Claude Code</span>
            <span className="info-val">{claudeCode?.managed ? `${claudeCode.model ?? "model"} via ${claudeCode.base_url ?? "Gateway"}` : "Claude Code is unmanaged"}</span>
            <span className="info-key">Last Call</span>
            <span className="info-val">{latestClaudeLog ? `${latestClaudeLog.upstream_model} via ${latestClaudeLog.provider_id}` : "No traffic yet"}</span>
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
            <span className="info-key">Gateway</span>
            <span className="info-val">
              <span className={`badge ${codexStatus?.running ? "badge-green" : "badge-gray"}`}>
                {codexStatus?.running ? "Running" : "Stopped"}
              </span>
            </span>
            <span className="info-key">Binding</span>
            <span className="info-val">{codexBinding?.managed ? `Codex App uses ${codexBinding.model ?? "Gateway Switch"}` : "Codex App uses OpenAI login"}</span>
            <span className="info-key">Last Call</span>
            <span className="info-val">{latestCodexLog ? `${latestCodexLog.upstream_model} via ${latestCodexLog.provider_id}` : "No traffic yet"}</span>
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
      <div className="section-label">Providers</div>
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
        <h1>Providers</h1>
        <p>Share credentials across products, with protocol-specific base URLs for OpenAI and Anthropic clients</p>
      </div>

      {/* Preset grid */}
      <div className="section-label">Quick Add</div>
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
        <div className="card-title">{editingP ? "Edit Provider" : "Add Provider"}</div>
        <div className="form-row">
          <div className="form-field">
            <label>Provider ID</label>
            <input value={pForm.id} disabled={!!editingP} onChange={e => setPForm({ ...pForm, id: e.target.value })} placeholder="e.g. ark" />
          </div>
          <div className="form-field">
            <label>Display Name</label>
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
            <label>Auth Header</label>
            <input value={pForm.auth_header} onChange={e => setPForm({ ...pForm, auth_header: e.target.value })} />
          </div>
          <div className="form-field">
            <label>Auth Scheme</label>
            <input value={pForm.auth_scheme} onChange={e => setPForm({ ...pForm, auth_scheme: e.target.value })} placeholder="Bearer / x-api-key" />
          </div>
          <div className="form-field">
            <label>API Key</label>
            <input type="password" value={pForm.api_key} onChange={e => setPForm({ ...pForm, api_key: e.target.value })} placeholder="Your API key" />
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveProvider}>
            {editingP ? <><IconEdit /> Save</> : <><IconPlus /> Add Provider</>}
          </button>
          {editingP && (
            <button className="btn" onClick={() => {
              setEditingP(null);
              setPForm(emptyProviderForm);
            }}>Cancel</button>
          )}
        </div>
      </div>

      {/* Providers table */}
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Provider</th>
              <th>OpenAI URL</th>
              <th>Anthropic URL</th>
              <th>Auth</th>
              <th>Status</th>
              <th>Actions</th>
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
                    <span className="muted-pill">Not configured</span>
                  )}
                </td>
                <td><span className="badge badge-blue">{p.auth_header}</span></td>
                <td><span className={`badge ${p.enabled ? "badge-green" : "badge-gray"}`}>{p.enabled ? "Active" : "Disabled"}</span></td>
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
                    <h3>No providers configured</h3>
                    <p>Click a preset above to get started.</p>
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
            <p>{aliasType === "claude" ? "Maintain the aliases exposed to Claude Desktop and model routes." : "Maintain the model names Codex can request from this gateway."}</p>
          </div>
          <div className="alias-add">
            <input
              value={value}
              onChange={e => setValue(e.target.value)}
              onKeyDown={e => { if (e.key === "Enter") void addModelAlias(aliasType); }}
              placeholder={placeholder}
            />
            <button className="btn" onClick={() => void addModelAlias(aliasType)}><IconPlus /> Add</button>
          </div>
        </div>
        <div className="alias-chip-list">
          {aliases.map(a => (
            <span key={a.id} className="alias-chip">
              {a.alias}
              <button aria-label={`Delete ${a.alias}`} onClick={() => void removeModelAlias(aliasType, a.id, a.alias)}><IconX /></button>
            </span>
          ))}
          {aliases.length === 0 && <span className="alias-empty">Default aliases will be used until you add a custom one.</span>}
        </div>
      </div>
    );
  };

  const ClaudePage = () => (
    <div>
      <div className="page-header">
        <h1>Claude</h1>
        <p>Configure Claude model routes and Claude Desktop binding</p>
      </div>

      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">Claude Gateway Status</div>
        <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
          <span className="info-key">Status</span>
          <span className="info-val">
            <span className={`badge ${status?.gateway_running ? "badge-green" : "badge-gray"}`}>
              {status?.gateway_running ? "Running" : "Stopped"}
            </span>
          </span>
          <span className="info-key">Port</span>
          <span className="info-val">{status?.gateway_port ?? settings?.listen_port ?? 3456}</span>
          <span className="info-key">Desktop URL</span>
          <span className="info-val">http://127.0.0.1:{status?.gateway_port ?? settings?.listen_port ?? 3456}</span>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          {status?.gateway_running ? (
            <button className="btn btn-danger" onClick={stopGw}><IconStop /> Stop</button>
          ) : (
            <button className="btn btn-primary" onClick={startGw}><IconPlay /> Start</button>
          )}
          <button className="btn" onClick={checkHealth}><IconZap /> Check Health</button>
          <button className="btn" onClick={() => void loadAll()}><IconRefresh /> Refresh</button>
        </div>
      </div>

      {/* Add/Edit form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">{editingR ? "Edit Route" : "Add Route"}</div>
        <div className="form-row">
          <div className="form-field">
            <label>Route ID</label>
            <input value={rForm.id} disabled={!!editingR} onChange={e => setRForm({ ...rForm, id: e.target.value })} placeholder="e.g. sonnet-ark" />
          </div>
          <div className="form-field">
            <label>Claude Alias</label>
            <select value={rForm.claude_alias} onChange={e => setRForm({ ...rForm, claude_alias: e.target.value })}>
              {claudeAliasOptions.map(a => <option key={a} value={a}>{a}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>Display Name</label>
            <input value={rForm.display_name} onChange={e => setRForm({ ...rForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>Provider</label>
            <select value={rForm.provider_id} onChange={e => setRForm({ ...rForm, provider_id: e.target.value })}>
              <option value="">Select provider...</option>
              {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>Upstream Model</label>
            <input value={rForm.upstream_model} onChange={e => setRForm({ ...rForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-v3" />
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveRoute}>
            {editingR ? <><IconEdit /> Save</> : <><IconPlus /> Add Route</>}
          </button>
          {editingR && (
            <button className="btn" onClick={() => {
              setEditingR(null);
              setRForm({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" });
            }}>Cancel</button>
          )}
        </div>
      </div>

      {AliasManager("claude")}

      {/* Route cards */}
      <div className="section-label">Route Cards</div>
      <div className="route-list" style={{ marginBottom: 20 }}>
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
                {r.enabled ? "Active" : "Disabled"}
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
            <h3>No routes configured</h3>
            <p>Add a route above to start mapping models.</p>
          </div>
        )}
      </div>

      {/* Routes table */}
      <div className="section-label">Route Table</div>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Claude Alias</th>
              <th>Display Name</th>
              <th>Provider</th>
              <th>Upstream Model</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {routes.map(r => (
              <tr key={r.id}>
                <td style={{ fontWeight: 600 }}>{r.claude_alias}</td>
                <td>{r.display_name}</td>
                <td><span className="badge badge-blue">{r.provider_id}</span></td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>{r.upstream_model}</td>
                <td><span className={`badge ${r.enabled ? "badge-green" : "badge-gray"}`}>{r.enabled ? "Active" : "Disabled"}</span></td>
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
                    <h3>No routes configured</h3>
                    <p>Add a route above to start mapping models.</p>
                  </div>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="section-label">Claude Desktop</div>
      {DesktopPage()}
    </div>
  );

  const ClaudeCodePage = () => {
    const selectedProvider = providers.find(p => p.id === ccProviderId);
    const gatewayRouteOptions = routes.length > 0 ? routes.filter(r => r.enabled).map(r => r.claude_alias) : claudeAliasOptions;
    const directProviderReady = ccMode === "provider" && !!selectedProvider?.anthropic_base_url && !!ccUpstreamModel.trim();

    return (
      <div>
        <div className="page-header">
          <h1>Claude Code</h1>
          <p>Bind Claude Code independently from Claude Desktop</p>
        </div>

        <div className="two-col">
          <div className="card">
            <div className="card-title">Claude Code Binding</div>
            <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
              <span className="info-key">Config</span>
              <span className="info-val">{claudeCode?.config_path ?? "~/.claude/settings.json"}</span>
              <span className="info-key">Status</span>
              <span className="info-val">
                <span className={`badge ${claudeCode?.managed ? "badge-green" : "badge-gray"}`}>
                  {claudeCode?.managed ? "Managed by Gateway Switch" : "Not bound"}
                </span>
              </span>
              <span className="info-key">Base URL</span>
              <span className="info-val">{claudeCode?.base_url ?? "Not set"}</span>
              <span className="info-key">Model</span>
              <span className="info-val">{claudeCode?.model ?? "Not set"}</span>
              <span className="info-key">Auth Env</span>
              <span className="info-val">{claudeCode?.auth_env ?? "Not set"}</span>
              <span className="info-key">Backup</span>
              <span className="info-val">{claudeCode?.backup_path ? "Available" : "None"}</span>
            </div>
          </div>

          <div className="card">
            <div className="card-title">Connection Mode</div>
            <div className="mode-switch">
              <button className={`mode-option ${ccMode === "gateway" ? "active" : ""}`} onClick={() => setCcMode("gateway")}>
                <IconShuffle />
                <span>Gateway Route</span>
              </button>
              <button className={`mode-option ${ccMode === "provider" ? "active" : ""}`} onClick={() => setCcMode("provider")}>
                <IconSun />
                <span>Direct Provider</span>
              </button>
            </div>

            {ccMode === "gateway" ? (
              <div className="binding-actions" style={{ marginTop: 16 }}>
                <label>Claude Code model</label>
                <select value={ccModel} onChange={e => setCcModel(e.target.value)}>
                  {Array.from(new Set(gatewayRouteOptions)).map(model => (
                    <option key={model} value={model}>{model}</option>
                  ))}
                </select>
                <p>Claude Code will use the local Claude Gateway at `http://127.0.0.1:{status?.gateway_port ?? settings?.listen_port ?? 3456}`. This supports your configured Claude routes, including Chat Completions fallback for providers such as XiaoMiMo.</p>
              </div>
            ) : (
              <div className="binding-actions" style={{ marginTop: 16 }}>
                <label>Provider</label>
                <select value={ccProviderId} onChange={e => {
                  const providerId = e.target.value;
                  setCcProviderId(providerId);
                  const route = routes.find(r => r.provider_id === providerId);
                  if (route && !ccUpstreamModel) setCcUpstreamModel(route.upstream_model);
                }}>
                  <option value="">Select provider...</option>
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
                      <code>{selectedProvider.anthropic_base_url || "Required for Direct Provider"}</code>
                    </div>
                  </div>
                )}
                <label>Upstream model</label>
                <input value={ccUpstreamModel} onChange={e => setCcUpstreamModel(e.target.value)} placeholder="e.g. claude-sonnet-4-5" />
                <p>Direct Provider writes the provider's Anthropic Base URL and API key into Claude Code. Use Gateway Route when a provider only supports OpenAI Chat Completions.</p>
                {selectedProvider && (
                  <div className="route-flow">
                    <span>{selectedProvider.name}</span>
                    <IconArrowRight />
                    <span>{selectedProvider.anthropic_base_url || "Missing Anthropic URL"}</span>
                    <IconArrowRight />
                    <span><b>{ccUpstreamModel || "model"}</b></span>
                  </div>
                )}
              </div>
            )}

            <div className="qa-buttons" style={{ marginTop: 16, marginBottom: 0 }}>
              <button className="btn btn-primary" onClick={bindClaudeCode} disabled={ccMode === "provider" && !directProviderReady}><IconLink /> Bind Claude Code</button>
              <button className="btn" onClick={restoreClaudeCode} disabled={!claudeCode?.managed && !claudeCode?.backup_path}><IconUnlink /> Restore</button>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-title">Runtime Environment</div>
          <div className="note-grid">
            <div>
              <strong>Gateway Route</strong>
              <p>Writes `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, and `ANTHROPIC_MODEL` into `~/.claude/settings.json`. Claude Desktop binding is not touched.</p>
            </div>
            <div>
              <strong>Direct Provider</strong>
              <p>Writes `ANTHROPIC_BASE_URL` from the provider's Anthropic URL. The OpenAI URL is reserved for Codex and Chat Completions fallback.</p>
            </div>
          </div>
        </div>
      </div>
    );
  };

  // =====================================================
  //  CODEX PAGE
  // =====================================================
  const CodexPage = () => (
    <div>
      <div className="page-header">
        <h1>Codex Gateway</h1>
        <p>OpenAI Responses API to Chat Completions API converter for Codex App and Codex CLI</p>
      </div>

      {/* Status + Quick Actions */}
      <div className="two-col">
        <div className="card">
          <div className="card-title">Codex Gateway Status</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">Status</span>
            <span className="info-val">
              <span className={`badge ${codexStatus?.running ? "badge-green" : "badge-gray"}`}>
                {codexStatus?.running ? "Running" : "Stopped"}
              </span>
            </span>
            <span className="info-key">Port</span>
            <span className="info-val">{codexPort}</span>
            <span className="info-key">Endpoint</span>
            <span className="info-val">http://127.0.0.1:{codexPort}/v1/responses</span>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            {codexStatus?.running ? (
              <button className="btn btn-danger" onClick={stopCodex}><IconStop /> Stop</button>
            ) : (
              <button className="btn btn-primary" onClick={startCodex}><IconPlay /> Start</button>
            )}
            <button className="btn" onClick={checkCodexHealth}><IconZap /> Check Health</button>
            <button className="btn" onClick={() => void loadAll()}><IconRefresh /> Refresh</button>
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
          <div className="card-title">Verify Real Model</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">Last Codex Model</span>
            <span className="info-val">{latestCodexLog?.claude_alias ?? "No Codex request yet"}</span>
            <span className="info-key">Provider</span>
            <span className="info-val">{latestCodexLog?.provider_id ?? "-"}</span>
            <span className="info-key">Real Upstream</span>
            <span className="info-val">{latestCodexLog?.upstream_model ?? "-"}</span>
            <span className="info-key">Result</span>
            <span className="info-val">
              {latestCodexLog ? `${latestCodexLog.status_code ?? "pending"} · ${latestCodexLog.duration_ms ?? "-"}ms` : "-"}
            </span>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            <button className="btn" onClick={() => setPage("logs")}><IconSearch /> Open Logs</button>
            <button className="btn" onClick={() => void loadAll()}><IconRefresh /> Refresh</button>
          </div>
        </div>
      </div>

      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">Codex App Binding</div>
        <div className="binding-panel">
          <div className="binding-state">
            <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
              <span className="info-key">Config</span>
              <span className="info-val">{codexBinding?.config_path ?? "~/.codex/config.toml"}</span>
              <span className="info-key">Binding</span>
              <span className="info-val">
                <span className={`badge ${codexBinding?.managed ? "badge-green" : "badge-gray"}`}>
                  {codexBinding?.managed ? "Managed by Gateway Switch" : "Not bound"}
                </span>
              </span>
              <span className="info-key">Provider</span>
              <span className="info-val">{codexBinding?.model_provider ?? "Default Codex provider"}</span>
              <span className="info-key">Default Model</span>
              <span className="info-val">{codexBinding?.model ?? "Not set"}</span>
            </div>
          </div>
          <div className="binding-actions">
            <label>Default model for Codex App</label>
            <select value={codexBindModel} onChange={e => setCodexBindModel(e.target.value)}>
              {Array.from(new Set([...codexRoutes.map(r => r.codex_model), ...codexModelOptions])).map(model => (
                <option key={model} value={model}>{model}</option>
              ))}
            </select>
            <p>Bind writes Gateway Switch into `~/.codex/config.toml` and forces API-key mode for the local gateway. Restart Codex App after binding.</p>
            <div className="qa-buttons" style={{ margin: 0 }}>
              <button className="btn btn-primary" onClick={bindCodexApp}><IconLink /> Start & Bind Codex App</button>
              <button className="btn" onClick={restoreCodexApp} disabled={!codexBinding?.managed && !codexBinding?.backup_path}><IconUnlink /> Restore OpenAI Login</button>
            </div>
          </div>
        </div>
      </div>

      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">Context and Reasoning Notes</div>
        <div className="note-grid">
          <div>
            <strong>Reply speed</strong>
            <p>Gateway Switch converts protocol shape; it does not add or remove a model's native reasoning ability. If the upstream model is fast, or does not expose reasoning tokens through Chat Completions, the visible response can be very quick.</p>
          </div>
          <div>
            <strong>Project history</strong>
            <p>Binding preserves `~/.codex/config.toml` project entries. Existing Codex conversations may still be separated by Codex's own account/provider state, so switching providers can show a different conversation list even when local project trust remains intact.</p>
          </div>
        </div>
      </div>

      {/* Add/Edit route form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div className="card-title">{editingC ? "Edit Codex Route" : "Add Codex Route"}</div>
        <div className="route-explainer">
          <div className="route-explainer-copy">
            <strong>Codex Model must match the model used by Codex CLI.</strong>
            <span>If you do not need a disguised name, set Codex Model and Upstream Model to the same third-party model name.</span>
          </div>
          <div className="route-flow">
            <span>codex -m <b>{cForm.codex_model || "model-name"}</b></span>
            <IconArrowRight />
            <span>{providers.find(p => p.id === cForm.provider_id)?.name || "Provider"}</span>
            <IconArrowRight />
            <span><b>{cForm.upstream_model || "upstream-model"}</b></span>
          </div>
        </div>
        <div className="form-row">
          <div className="form-field">
            <label>Route ID</label>
            <input value={cForm.id} disabled={!!editingC} onChange={e => setCForm({ ...cForm, id: e.target.value })} placeholder="e.g. gpt4o-deepseek" />
          </div>
          <div className="form-field">
            <label>Codex Model (requested by Codex)</label>
            <select value={cForm.codex_model} onChange={e => setCForm({ ...cForm, codex_model: e.target.value })}>
              {codexModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
            </select>
            <span className="field-hint">This is the model name used in `codex -m ...`.</span>
          </div>
          <div className="form-field">
            <label>Display Name</label>
            <input value={cForm.display_name} onChange={e => setCForm({ ...cForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>Provider</label>
            <select value={cForm.provider_id} onChange={e => setCForm({ ...cForm, provider_id: e.target.value })}>
              <option value="">Select provider...</option>
              {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>Upstream Model (real provider model)</label>
            <input value={cForm.upstream_model} onChange={e => setCForm({ ...cForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-chat" />
            <span className="field-hint">This is the actual model name sent to the third-party API.</span>
          </div>
        </div>
        <div className="qa-buttons" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={saveCodexRoute}>
            {editingC ? <><IconEdit /> Save</> : <><IconPlus /> Add Route</>}
          </button>
          {editingC && (
            <button className="btn" onClick={() => {
              setEditingC(null);
              setCForm({ id: "", codex_model: "gpt-4o", display_name: "", provider_id: "", upstream_model: "" });
            }}>Cancel</button>
          )}
        </div>
      </div>

      {AliasManager("codex")}

      {/* Route cards */}
      <div className="section-label">Active Codex Routes</div>
      <div className="route-list" style={{ marginBottom: 20 }}>
        {codexRoutes.length > 0 ? (
          codexRoutes.map(r => (
            <div key={r.id} className="route-item">
              <div className="route-icon" style={{ background: "rgba(217,119,6,0.08)", color: "var(--amber)" }}>
                {r.codex_model.slice(0, 3)}
              </div>
              <div className="route-info">
                <div className="route-name">{r.codex_model}</div>
                <div className="route-path">{r.display_name || r.upstream_model} via {r.provider_id}</div>
              </div>
              <span className={`route-status ${r.enabled ? "active" : "disabled"}`}>
                {r.enabled ? "Active" : "Disabled"}
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
            <h3>No Codex routes configured</h3>
            <p>Add a route above to start mapping Codex models.</p>
          </div>
        )}
      </div>
    </div>
  );

  // =====================================================
  //  DESKTOP PAGE
  // =====================================================
  const DesktopPage = () => (
    <div>
      <div className="two-col">
        {/* Binding Status */}
        <div className="card">
          <div className="card-title">Binding Status</div>
          <div className="info-grid" style={{ marginTop: 0, paddingTop: 0, borderTop: "none" }}>
            <span className="info-key">Config File</span>
            <span className="info-val">{desktop?.config_path ?? "-"}</span>
            <span className="info-key">Base URL</span>
            <span className="info-val">{desktop?.base_url ?? "Not set"}</span>
            <span className="info-key">Local Gateway Auth</span>
            <span className="info-val">{desktop?.auth_scheme ?? "Not set"}</span>
            <span className="info-key">Backup</span>
            <span className="info-val">{desktop?.backup_path ? "Available" : "None"}</span>
            <span className="info-key">Status</span>
            <span className="info-val">
              <span className={`badge ${desktop?.managed ? "badge-green" : "badge-gray"}`}>
                {desktop?.managed ? "Managed" : "Unmanaged"}
              </span>
            </span>
          </div>
          <div className="qa-buttons" style={{ marginTop: 16 }}>
            <button className="btn btn-primary" onClick={bindDesktop}>
              <IconLink /> Bind Desktop
            </button>
            <button className="btn" onClick={restoreDesktop}>
              <IconUnlink /> Restore
            </button>
          </div>
        </div>

        {/* Exposed Models */}
        <div className="card">
          <div className="card-title">Exposed Models</div>
          {desktop?.models && desktop.models.length > 0 ? (
            <div className="route-list">
              {desktop.models.map(m => (
                <div key={m} className="route-item">
                  <div className={`route-icon ${getModelFamily(m)}`}>
                    {getModelAbbrev(m)}
                  </div>
                  <div className="route-info">
                    <div className="route-name">{m}</div>
                    <div className="route-path">Exposed to Claude Desktop</div>
                  </div>
                  <span className="route-status active">
                    <IconCheck /> Active
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-icon">--</div>
              <h3>No models exposed</h3>
              <p>Bind Desktop first to expose models.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );

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
          <h1>Request Logs</h1>
          <p>Monitor gateway request activity</p>
        </div>

        <div className="qa-buttons" style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-xs)", background: "var(--surface)", flex: 1, maxWidth: 360 }}>
            <IconSearch />
            <input
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              placeholder="Search logs..."
              style={{ border: "none", outline: "none", fontSize: 13, flex: 1, background: "transparent", fontFamily: "inherit", color: "var(--fg)", minWidth: 0 }}
            />
          </div>
          <button className="btn" onClick={() => void loadAll()}>
            <IconRefresh /> Refresh
          </button>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>Requested Model</th>
                <th>Provider</th>
                <th>Real Upstream</th>
                <th>Mode</th>
                <th>Status</th>
                <th>Duration</th>
                {filteredLogs.some(l => l.error_summary) && <th>Error</th>}
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
                    <td style={{ fontSize: 12, color: "var(--red)", maxWidth: 200 }}>
                      <span style={{ display: "block", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", wordBreak: "normal" }}>{l.error_summary || "-"}</span>
                    </td>
                  )}
                </tr>
              ))}
              {filteredLogs.length === 0 && (
                <tr>
                  <td colSpan={filteredLogs.some(l => l.error_summary) ? 8 : 7}>
                    <div className="empty-state">
                      <div className="empty-icon">--</div>
                      <h3>{searchQuery ? "No matching logs" : "No logs yet"}</h3>
                      <p>{searchQuery ? "Try a different search query." : "Logs will appear here once requests are made."}</p>
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

  // =====================================================
  //  SETTINGS PAGE
  // =====================================================
  const SettingsPage = () => {
    if (!settings) return <div className="empty-state"><h3>Loading...</h3></div>;
    return (
      <div>
        <div className="page-header">
          <h1>Settings</h1>
          <p>Configure gateway behavior and manage data</p>
        </div>

        <div className="two-col">
          {/* Gateway Configuration */}
          <div className="card">
            <div className="card-title">Gateway Configuration</div>
            <div className="form-row">
              <div className="form-field">
                <label>Listen Host</label>
                <input value={settings.listen_host} onChange={e => setSettings({ ...settings, listen_host: e.target.value })} />
              </div>
              <div className="form-field">
                <label>Listen Port</label>
                <input type="number" value={settings.listen_port} onChange={e => setSettings({ ...settings, listen_port: Number(e.target.value) })} />
              </div>
              <div className="form-field" style={{ gridColumn: "1 / -1" }}>
                <label>Auth Token</label>
                <input value={settings.auth_token} onChange={e => setSettings({ ...settings, auth_token: e.target.value })} />
              </div>
            </div>

            <div style={{ marginTop: 16, display: "flex", flexDirection: "column", gap: 10 }}>
              <div className="toggle-row">
                <span>Auto-start Gateway on launch</span>
                <button className={`toggle ${settings.auto_start_gateway ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_start_gateway: !settings.auto_start_gateway })} />
              </div>
              <div className="toggle-row">
                <span>Auto-bind Claude Desktop on launch</span>
                <button className={`toggle ${settings.auto_takeover_desktop ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_takeover_desktop: !settings.auto_takeover_desktop })} />
              </div>
            </div>

            <div style={{ marginTop: 16 }}>
              <button className="btn btn-primary" onClick={saveSettings}>
                <IconEdit /> Save Settings
              </button>
            </div>
          </div>

          {/* Import / Export */}
          <div className="card">
            <div className="card-title">Import / Export</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
              <div>
                <div style={{ fontSize: 12, fontWeight: 500, color: "var(--muted)", marginBottom: 6 }}>Import Configuration</div>
                <div className="qa-buttons">
                  <input
                    value={importPath}
                    onChange={e => setImportPath(e.target.value)}
                    placeholder="/path/to/config.json"
                    style={{ flex: 1, padding: "8px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-xs)", fontSize: 13, outline: "none", fontFamily: "var(--font-mono)", minWidth: 0, background: "var(--surface)", color: "var(--fg)" }}
                  />
                  <button className="btn" onClick={doImport}><IconUpload /> Import</button>
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 500, color: "var(--muted)", marginBottom: 6 }}>Export Configuration</div>
                <p style={{ fontSize: 13, color: "var(--muted)", marginBottom: 10 }}>
                  Export all providers, routes, and settings to a JSON file.
                </p>
                <button className="btn" onClick={doExport}><IconDownload /> Export to File</button>
              </div>
              <div style={{ padding: 16, background: "var(--bg)", borderRadius: "var(--radius-xs)", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: "var(--fg)", marginBottom: 6 }}>Data Storage</div>
                <div style={{ fontSize: 12.5, color: "var(--muted)", lineHeight: 1.6 }}>
                  All data is stored under:<br />
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
