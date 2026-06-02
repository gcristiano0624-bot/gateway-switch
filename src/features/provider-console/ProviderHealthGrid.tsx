import type {
  CodexRoute,
  ModelRoute,
  Provider,
  ProviderConsoleItem,
  ProviderConsoleReport,
  RequestLog,
} from "../../shared/types/runtime-console";

type ProviderHealthGridProps = {
  providers: Provider[];
  providerConsole: ProviderConsoleReport | null;
  routes: ModelRoute[];
  codexRoutes: CodexRoute[];
  logs: RequestLog[];
  policyNotesForProvider: (providerId: string) => string | null;
};

export function ProviderHealthGrid({
  providers,
  providerConsole,
  routes,
  codexRoutes,
  logs,
  policyNotesForProvider,
}: ProviderHealthGridProps) {
  const fallbackItems: ProviderConsoleItem[] = providers.map(provider => {
    const recentRequestCount = logs.filter(log => log.provider_id === provider.id).length;
    const recentFailureCount = logs.filter(log => log.provider_id === provider.id && (log.status_code == null || log.status_code >= 400)).length;
    const policyNote = policyNotesForProvider(provider.id);
    return {
      provider,
      supports_claude: Boolean(provider.anthropic_base_url),
      supports_codex: Boolean(provider.openai_base_url),
      linked_claude_routes: routes.filter(route => route.provider_id === provider.id).length,
      linked_codex_routes: codexRoutes.filter(route => route.provider_id === provider.id).length,
      recent_request_count: recentRequestCount,
      recent_failure_count: recentFailureCount,
      health_score: provider.enabled ? 75 : 0,
      policy_tags: policyNote ? [policyNote] : [],
    };
  });

  const items = providerConsole?.providers ?? fallbackItems;

  return (
    <div className="card" style={{ marginTop: 20, marginBottom: 20 }}>
      <div className="card-title">Provider Health & Route Links</div>
      <p style={{ color: "var(--muted)", marginBottom: 14 }}>
        Aggregated by the Rust backend: health score, Claude/Codex route links, recent requests, failures, and active compatibility tags.
      </p>
      <div className="provider-console-grid">
        {items.map(item => (
          <div key={item.provider.id} className="provider-console-card">
            <div className="app-workbench-head">
              <div>
                <div className="app-workbench-title">{item.provider.name}</div>
                <div className="app-workbench-subtitle">{item.provider.id}</div>
              </div>
              <span className={`badge ${item.health_score >= 80 ? "badge-green" : item.health_score >= 50 ? "badge-amber" : "badge-red"}`}>{item.health_score}</span>
            </div>
            <div className="provider-console-metrics">
              <span>Claude routes: {item.linked_claude_routes}</span>
              <span>Codex routes: {item.linked_codex_routes}</span>
              <span>Requests: {item.recent_request_count}</span>
              <span>Failures: {item.recent_failure_count}</span>
            </div>
            <div className="qa-buttons" style={{ marginTop: 10 }}>
              {item.supports_claude && <span className="badge badge-blue">Claude</span>}
              {item.supports_codex && <span className="badge badge-purple">Codex</span>}
              {item.policy_tags.slice(0, 3).map(tag => <span key={tag} className="badge badge-amber">{tag}</span>)}
            </div>
          </div>
        ))}
        {providers.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">--</div>
            <h3>No providers yet</h3>
            <p>Use Quick Add or Provider Presets to add your first upstream provider.</p>
          </div>
        )}
      </div>
    </div>
  );
}
