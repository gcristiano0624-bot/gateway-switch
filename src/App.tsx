import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  LayoutDashboard, Server, Route, Monitor, FileText, Settings,
  Play, Square, RefreshCw, Link, Unlink, Plus, Trash2, Edit3,
  CheckCircle, XCircle, Zap
} from "lucide-react";
import "./App.css";

type Page = "dashboard" | "providers" | "routes" | "desktop" | "logs" | "settings";

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

const NAV: { key: Page; label: string; icon: typeof LayoutDashboard }[] = [
  { key: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { key: "providers", label: "Providers", icon: Server },
  { key: "routes", label: "Routes", icon: Route },
  { key: "desktop", label: "Desktop", icon: Monitor },
  { key: "logs", label: "Logs", icon: FileText },
  { key: "settings", label: "Settings", icon: Settings },
];

const CLAUDE_ALIASES = [
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

const PROVIDER_PRESETS: Array<{ id: string; name: string; base_url: string; auth_header: string; auth_scheme: string }> = [
  { id: "volcengine", name: "Volcano Engine Ark", base_url: "https://ark.cn-beijing.volces.com/api/v3", auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "xiaomimo", name: "XiaoMiMo", base_url: "https://api.xiaomimo.com/v1", auth_header: "x-api-key", auth_scheme: "" },
  { id: "openrouter", name: "OpenRouter", base_url: "https://openrouter.ai/api/v1", auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "deepseek", name: "DeepSeek", base_url: "https://api.deepseek.com/v1", auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "siliconflow", name: "SiliconFlow", base_url: "https://api.siliconflow.cn/v1", auth_header: "Authorization", auth_scheme: "Bearer" },
  { id: "custom", name: "Custom Provider", base_url: "", auth_header: "x-api-key", auth_scheme: "" },
];

function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [status, setStatus] = useState<Status | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [routes, setRoutes] = useState<ModelRoute[]>([]);
  const [desktop, setDesktop] = useState<DesktopInfo | null>(null);
  const [logs, setLogs] = useState<RequestLog[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Provider form
  const [pForm, setPForm] = useState({ id: "", name: "", base_url: "", auth_header: "x-api-key", auth_scheme: "", api_key: "" });
  const [editingP, setEditingP] = useState<string | null>(null);

  // Route form
  const [rForm, setRForm] = useState({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" });
  const [editingR, setEditingR] = useState<string | null>(null);

  const flash = (msg: string, type: "success" | "error" = "success") => {
    if (type === "success") { setSuccess(msg); setError(null); }
    else { setError(msg); setSuccess(null); }
    setTimeout(() => { setSuccess(null); setError(null); }, 4000);
  };

  const loadAll = useCallback(async () => {
    try {
      const [s, p, r, d, l, cfg] = await Promise.all([
        invoke<Status>("get_status"),
        invoke<Provider[]>("list_providers"),
        invoke<ModelRoute[]>("list_routes"),
        invoke<DesktopInfo>("get_desktop_info"),
        invoke<RequestLog[]>("list_logs"),
        invoke<Settings>("get_settings"),
      ]);
      setStatus(s);
      setProviders(p);
      setRoutes(r);
      setDesktop(d);
      setLogs(l);
      setSettings(cfg);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { void loadAll(); }, [loadAll]);

  // ---- Actions ----
  const startGw = async () => { try { await invoke("start_gateway"); await loadAll(); flash("Gateway started"); } catch (e) { flash(String(e), "error"); } };
  const stopGw = async () => { try { await invoke("stop_gateway"); await loadAll(); flash("Gateway stopped"); } catch (e) { flash(String(e), "error"); } };
  const checkHealth = async () => { try { const h = await invoke<Health>("check_gateway_health"); setHealth(h); } catch (e) { flash(String(e), "error"); } };
  const bindDesktop = async () => { try { await invoke("apply_binding"); await loadAll(); flash("Desktop bound"); } catch (e) { flash(String(e), "error"); } };
  const restoreDesktop = async () => { try { await invoke("restore_binding"); await loadAll(); flash("Desktop restored"); } catch (e) { flash(String(e), "error"); } };

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
      setPForm({ id: "", name: "", base_url: "", auth_header: "x-api-key", auth_scheme: "", api_key: "" });
      await loadAll();
    } catch (e) { flash(String(e), "error"); }
  };
  const delProvider = async (id: string) => {
    try { await invoke("delete_provider", { id }); flash("Provider deleted"); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const editProvider = (p: Provider) => {
    setEditingP(p.id);
    setPForm({ id: p.id, name: p.name, base_url: p.base_url, auth_header: p.auth_header, auth_scheme: p.auth_scheme ?? "", api_key: p.api_key ?? "" });
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

  // ---- Sidebar ----
  const Sidebar = () => (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <h1>Gateway Switch</h1>
        <p>Claude Desktop Router</p>
      </div>
      <nav className="sidebar-nav">
        {NAV.map(n => (
          <button key={n.key} className={`sidebar-item ${page === n.key ? "active" : ""}`} onClick={() => setPage(n.key)}>
            <n.icon /> {n.label}
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className={`dot ${status?.gateway_running ? "dot-green" : "dot-red"}`} />
          <span style={{ color: "#a1a1aa", fontSize: 12 }}>
            Gateway {status?.gateway_running ? "Running" : "Stopped"}
          </span>
        </div>
      </div>
    </aside>
  );

  // ---- Dashboard ----
  const DashboardPage = () => (
    <div>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 20 }}>Dashboard</h2>
      <div className="metrics-row">
        <div className="metric">
          <div className="metric-label">Gateway</div>
          <div className="metric-value">{status?.gateway_running ? <span style={{ color: "var(--green)" }}>Running</span> : <span style={{ color: "var(--red)" }}>Stopped</span>}</div>
        </div>
        <div className="metric">
          <div className="metric-label">Desktop</div>
          <div className="metric-value">{desktop?.managed ? <span style={{ color: "var(--accent)" }}>Managed</span> : <span style={{ color: "var(--text-muted)" }}>Unmanaged</span>}</div>
        </div>
        <div className="metric">
          <div className="metric-label">Providers</div>
          <div className="metric-value">{providers.length}</div>
        </div>
        <div className="metric">
          <div className="metric-label">Routes</div>
          <div className="metric-value">{routes.length}</div>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <div className="card">
          <div className="card-header">
            <span className="card-title">Quick Actions</span>
          </div>
          <div className="btn-group">
            {status?.gateway_running ? (
              <button className="btn btn-danger" onClick={stopGw}><Square size={14} /> Stop Gateway</button>
            ) : (
              <button className="btn btn-primary" onClick={startGw}><Play size={14} /> Start Gateway</button>
            )}
            <button className="btn" onClick={checkHealth}><Zap size={14} /> Health Check</button>
            <button className="btn" onClick={() => void loadAll()}><RefreshCw size={14} /> Refresh</button>
          </div>
          {health && (
            <div style={{ marginTop: 12, padding: 10, background: health.ok ? "var(--green-light)" : "var(--red-light)", borderRadius: 8, fontSize: 13 }}>
              {health.ok ? "Healthy" : "Unhealthy"}: {health.message}
              {health.latency_ms && ` (${health.latency_ms}ms)`}
            </div>
          )}
          <div style={{ marginTop: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid #f4f4f5" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Listen Address</span>
              <span style={{ fontWeight: 600, fontSize: 13 }}>127.0.0.1:{status?.gateway_port ?? 3456}</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Config Path</span>
              <span style={{ fontWeight: 600, fontSize: 13, wordBreak: "break-all", textAlign: "right", maxWidth: "60%" }}>{desktop?.config_path ?? "-"}</span>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">Active Routes</span>
          </div>
          {routes.length === 0 ? (
            <div className="empty">No routes configured yet. Go to Routes page to add one.</div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {routes.map(r => (
                <div key={r.id} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 12px", background: "#fafafa", borderRadius: 8 }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{r.claude_alias}</div>
                    <div style={{ color: "var(--text-muted)", fontSize: 12 }}>{r.display_name} &rarr; {r.upstream_model}</div>
                  </div>
                  <span className={`badge ${r.enabled ? "badge-green" : "badge-gray"}`}>{r.enabled ? "Active" : "Disabled"}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );

  // ---- Providers ----
  const ProvidersPage = () => (
    <div>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 20 }}>Providers</h2>

      {/* Preset selector */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title" style={{ marginBottom: 14 }}>Quick Add from Preset</div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {PROVIDER_PRESETS.map(preset => (
            <button
              key={preset.id}
              className="btn"
              onClick={() => {
                setEditingP(null);
                setPForm({ id: preset.id, name: preset.name, base_url: preset.base_url, auth_header: preset.auth_header, auth_scheme: preset.auth_scheme, api_key: "" });
              }}
            >
              {preset.name}
            </button>
          ))}
        </div>
      </div>

      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title" style={{ marginBottom: 14 }}>{editingP ? "Edit Provider" : "Add Provider"}</div>
        <div className="form-grid">
          <div className="form-field">
            <label>Provider ID</label>
            <input value={pForm.id} disabled={!!editingP} onChange={e => setPForm({ ...pForm, id: e.target.value })} placeholder="e.g. ark" />
          </div>
          <div className="form-field">
            <label>Display Name</label>
            <input value={pForm.name} onChange={e => setPForm({ ...pForm, name: e.target.value })} placeholder="e.g. Volcano Engine" />
          </div>
          <div className="form-field">
            <label>Base URL</label>
            <input value={pForm.base_url} onChange={e => setPForm({ ...pForm, base_url: e.target.value })} placeholder="https://..." />
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
            <input type="password" value={pForm.api_key} onChange={e => setPForm({ ...pForm, api_key: e.target.value })} />
          </div>
        </div>
        <div style={{ marginTop: 14 }}>
          <button className="btn btn-primary" onClick={saveProvider}>
            {editingP ? <Edit3 size={14} /> : <Plus size={14} />} {editingP ? "Save" : "Add"}
          </button>
          {editingP && <button className="btn" style={{ marginLeft: 8 }} onClick={() => { setEditingP(null); setPForm({ id: "", name: "", base_url: "", auth_header: "x-api-key", auth_scheme: "", api_key: "" }); }}>Cancel</button>}
        </div>
      </div>
      <div className="table-wrap">
        <table>
          <thead><tr><th>ID</th><th>Name</th><th>Base URL</th><th>Auth</th><th>Status</th><th>Actions</th></tr></thead>
          <tbody>
            {providers.map(p => (
              <tr key={p.id}>
                <td style={{ fontWeight: 600 }}>{p.id}</td>
                <td>{p.name}</td>
                <td style={{ fontSize: 12, color: "var(--text-muted)" }}>{p.base_url}</td>
                <td><span className="badge badge-blue">{p.auth_header}</span></td>
                <td><span className={`badge ${p.enabled ? "badge-green" : "badge-gray"}`}>{p.enabled ? "Active" : "Disabled"}</span></td>
                <td>
                  <div className="btn-group">
                    <button className="btn btn-sm" onClick={() => editProvider(p)}><Edit3 size={12} /></button>
                    <button className="btn btn-sm btn-danger" onClick={() => delProvider(p.id)}><Trash2 size={12} /></button>
                  </div>
                </td>
              </tr>
            ))}
            {providers.length === 0 && <tr><td colSpan={6} className="empty">No providers configured</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );

  // ---- Routes ----
  const RoutesPage = () => (
    <div>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 20 }}>Model Routes</h2>
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title" style={{ marginBottom: 14 }}>{editingR ? "Edit Route" : "Add Route"}</div>
        <div className="form-grid">
          <div className="form-field">
            <label>Route ID</label>
            <input value={rForm.id} disabled={!!editingR} onChange={e => setRForm({ ...rForm, id: e.target.value })} placeholder="e.g. sonnet-ark" />
          </div>
          <div className="form-field">
            <label>Claude Alias</label>
            <select value={rForm.claude_alias} onChange={e => setRForm({ ...rForm, claude_alias: e.target.value })}>
              {CLAUDE_ALIASES.map(a => <option key={a} value={a}>{a}</option>)}
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
        <div style={{ marginTop: 14 }}>
          <button className="btn btn-primary" onClick={saveRoute}>
            {editingR ? <Edit3 size={14} /> : <Plus size={14} />} {editingR ? "Save" : "Add"}
          </button>
          {editingR && <button className="btn" style={{ marginLeft: 8 }} onClick={() => { setEditingR(null); setRForm({ id: "", claude_alias: "claude-sonnet-4-6", display_name: "", provider_id: "", upstream_model: "" }); }}>Cancel</button>}
        </div>
      </div>
      <div className="table-wrap">
        <table>
          <thead><tr><th>Claude Alias</th><th>Display Name</th><th>Provider</th><th>Upstream Model</th><th>Status</th><th>Actions</th></tr></thead>
          <tbody>
            {routes.map(r => (
              <tr key={r.id}>
                <td style={{ fontWeight: 600 }}>{r.claude_alias}</td>
                <td>{r.display_name}</td>
                <td><span className="badge badge-blue">{r.provider_id}</span></td>
                <td>{r.upstream_model}</td>
                <td><span className={`badge ${r.enabled ? "badge-green" : "badge-gray"}`}>{r.enabled ? "Active" : "Disabled"}</span></td>
                <td>
                  <div className="btn-group">
                    <button className="btn btn-sm" onClick={() => editRoute(r)}><Edit3 size={12} /></button>
                    <button className="btn btn-sm btn-danger" onClick={() => delRoute(r.id)}><Trash2 size={12} /></button>
                  </div>
                </td>
              </tr>
            ))}
            {routes.length === 0 && <tr><td colSpan={6} className="empty">No routes configured</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );

  // ---- Desktop ----
  const DesktopPage = () => (
    <div>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 20 }}>Claude Desktop</h2>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <div className="card">
          <div className="card-title" style={{ marginBottom: 14 }}>Binding Status</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Status</span>
              <span className={`badge ${desktop?.managed ? "badge-green" : "badge-gray"}`}>{desktop?.managed ? "Managed" : "Unmanaged"}</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Config File</span>
              <span style={{ fontSize: 12, wordBreak: "break-all", textAlign: "right", maxWidth: "60%" }}>{desktop?.config_path ?? "-"}</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Base URL</span>
              <span style={{ fontSize: 12 }}>{desktop?.base_url ?? "Not set"}</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)", fontSize: 13 }}>Backup</span>
              <span style={{ fontSize: 12 }}>{desktop?.backup_path ? "Available" : "None"}</span>
            </div>
          </div>
          <div className="btn-group" style={{ marginTop: 16 }}>
            <button className="btn btn-primary" onClick={bindDesktop}><Link size={14} /> Bind Desktop</button>
            <button className="btn" onClick={restoreDesktop}><Unlink size={14} /> Restore</button>
          </div>
        </div>
        <div className="card">
          <div className="card-title" style={{ marginBottom: 14 }}>Exposed Models</div>
          {desktop?.models && desktop.models.length > 0 ? (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {desktop.models.map(m => (
                <div key={m} style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px", background: "#f4f4f5", borderRadius: 8, fontSize: 13 }}>
                  <CheckCircle size={14} style={{ color: "var(--green)" }} /> {m}
                </div>
              ))}
            </div>
          ) : (
            <div className="empty">No models exposed. Bind Desktop first.</div>
          )}
        </div>
      </div>
    </div>
  );

  // ---- Logs ----
  const LogsPage = () => (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
        <h2 style={{ fontSize: 20, fontWeight: 700 }}>Request Logs</h2>
        <button className="btn" onClick={() => void loadAll()}><RefreshCw size={14} /> Refresh</button>
      </div>
      <div className="table-wrap">
        <table>
          <thead><tr><th>Time</th><th>Alias</th><th>Provider</th><th>Upstream</th><th>Mode</th><th>Status</th><th>Duration</th></tr></thead>
          <tbody>
            {logs.map(l => (
              <tr key={l.request_id + l.created_at}>
                <td style={{ fontSize: 12, color: "var(--text-muted)" }}>{l.created_at.replace("T", " ").slice(0, 19)}</td>
                <td style={{ fontWeight: 600 }}>{l.claude_alias}</td>
                <td>{l.provider_id}</td>
                <td style={{ fontSize: 12 }}>{l.upstream_model}</td>
                <td><span className={`badge ${l.is_stream ? "badge-orange" : "badge-blue"}`}>{l.is_stream ? "stream" : "sync"}</span></td>
                <td>
                  <span className={`badge ${l.status_code && l.status_code < 400 ? "badge-green" : l.status_code ? "badge-red" : "badge-gray"}`}>
                    {l.status_code ?? "pending"}
                  </span>
                </td>
                <td>{l.duration_ms ? `${l.duration_ms}ms` : "-"}</td>
              </tr>
            ))}
            {logs.length === 0 && <tr><td colSpan={7} className="empty">No logs yet</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );

  // ---- Settings ----
  const [importPath, setImportPath] = useState("");

  const doImport = async () => {
    if (!importPath) return;
    try { await invoke("import_config", { filePath: importPath }); flash("Config imported"); setImportPath(""); await loadAll(); } catch (e) { flash(String(e), "error"); }
  };
  const doExport = async () => {
    try { const p = await invoke<string>("export_config"); flash(`Exported to ${p}`); } catch (e) { flash(String(e), "error"); }
  };

  const SettingsPage = () => {
    if (!settings) return <div className="empty">Loading...</div>;
    return (
      <div>
        <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 20 }}>Settings</h2>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
          <div className="card">
            <div className="card-title" style={{ marginBottom: 14 }}>Gateway Configuration</div>
            <div className="form-grid">
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
            <div style={{ marginTop: 16, display: "flex", flexDirection: "column", gap: 12 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "10px 14px", background: "#fafafa", borderRadius: 8 }}>
                <span style={{ fontSize: 13 }}>Auto-start Gateway on launch</span>
                <button className={`toggle ${settings.auto_start_gateway ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_start_gateway: !settings.auto_start_gateway })} />
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "10px 14px", background: "#fafafa", borderRadius: 8 }}>
                <span style={{ fontSize: 13 }}>Auto-bind Claude Desktop on launch</span>
                <button className={`toggle ${settings.auto_takeover_desktop ? "on" : ""}`} onClick={() => setSettings({ ...settings, auto_takeover_desktop: !settings.auto_takeover_desktop })} />
              </div>
            </div>
            <div style={{ marginTop: 16 }}>
              <button className="btn btn-primary" onClick={saveSettings}>Save Settings</button>
            </div>
          </div>

          <div className="card">
            <div className="card-title" style={{ marginBottom: 14 }}>Import / Export</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <div>
                <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 6 }}>Import Configuration</div>
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    value={importPath}
                    onChange={e => setImportPath(e.target.value)}
                    placeholder="/path/to/config.json"
                    style={{ flex: 1, padding: "8px 12px", border: "1px solid var(--border)", borderRadius: 8, fontSize: 13 }}
                  />
                  <button className="btn" onClick={doImport}>Import</button>
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 6 }}>Export Configuration</div>
                <button className="btn" onClick={doExport}>Export to File</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  };

  const Content = () => {
    switch (page) {
      case "dashboard": return <DashboardPage />;
      case "providers": return <ProvidersPage />;
      case "routes": return <RoutesPage />;
      case "desktop": return <DesktopPage />;
      case "logs": return <LogsPage />;
      case "settings": return <SettingsPage />;
    }
  };

  return (
    <div className="app-layout">
      <Sidebar />
      <main className="main-content">
        {error && <div className="alert alert-error"><XCircle size={14} style={{ marginRight: 6 }} />{error}</div>}
        {success && <div className="alert alert-success"><CheckCircle size={14} style={{ marginRight: 6 }} />{success}</div>}
        <Content />
      </main>
    </div>
  );
}

export default App;
