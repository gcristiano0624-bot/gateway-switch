import type { UnifiedDiagnosticsReport } from "../../shared/types/runtime-console";

type HealthCenterPanelProps = {
  report: UnifiedDiagnosticsReport;
  onRefresh: () => void;
  onExport: () => void;
  onClaudeGatewayCheck: () => void;
  onCodexGatewayCheck: () => void;
  onNavigate: (target: string) => void;
};

const statusBadge = (status: string) => {
  if (status === "healthy") return "badge-green";
  if (status === "attention") return "badge-amber";
  if (status === "degraded") return "badge-blue";
  if (status === "critical") return "badge-red";
  return "badge-gray";
};

export function HealthCenterPanel({
  report,
  onRefresh,
  onExport,
  onClaudeGatewayCheck,
  onCodexGatewayCheck,
  onNavigate,
}: HealthCenterPanelProps) {
  return (
    <div>
      <div className="page-header page-header-row">
        <div>
          <h1>Health Center</h1>
          <p>统一检查 Runtime、Apps、Providers、Routes、Policies 和最近失败，默认 Quick Check 不消耗 token。</p>
        </div>
        <div className="qa-buttons" style={{ margin: 0 }}>
          <button className="btn" onClick={onRefresh}>Refresh</button>
          <button className="btn btn-primary" onClick={onExport}>Export Report</button>
        </div>
      </div>

      <div className="health-center-hero">
        <div>
          <div className="runtime-eyebrow">Quick Check</div>
          <h2>{report.score}/100 · {report.status}</h2>
          <p>{report.summary}</p>
        </div>
        <div className="runtime-actions">
          <button className="btn" onClick={onClaudeGatewayCheck}>Claude Gateway</button>
          <button className="btn" onClick={onCodexGatewayCheck}>Codex Gateway</button>
          <button className="btn btn-primary" onClick={onRefresh}>Check All</button>
        </div>
      </div>

      <div className="health-section-grid">
        {report.sections.map(section => (
          <div key={section.id} className="health-section-card">
            <div className="app-workbench-head">
              <div>
                <div className="app-workbench-title">{section.title}</div>
                <div className="app-workbench-subtitle">{section.summary}</div>
              </div>
              <span className={`badge ${statusBadge(section.status)}`}>{section.score}</span>
            </div>
            <div className="qa-buttons" style={{ marginTop: 10 }}>
              {section.metrics.map(metric => (
                <span key={`${section.id}-${metric.label}`} className={`badge ${statusBadge(metric.status)}`}>{metric.label}: {metric.value}</span>
              ))}
            </div>
            {section.actions.length > 0 && (
              <div className="qa-buttons" style={{ marginTop: 10 }}>
                {section.actions.map(action => (
                  <button key={action.id} className="btn" onClick={() => onNavigate(action.target)}>{action.label}</button>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="section-label">Failure Clusters</div>
      <div className="table-wrap">
        <table>
          <thead>
            <tr><th>Provider</th><th>Surface</th><th>Status</th><th>Count</th><th>Recommendation</th></tr>
          </thead>
          <tbody>
            {report.failure_clusters.map(cluster => (
              <tr key={cluster.key}>
                <td>{cluster.provider_id ?? "unknown"}</td>
                <td><span className="badge badge-blue">{cluster.surface}</span></td>
                <td>{cluster.status_code ?? "network"}</td>
                <td>{cluster.count}</td>
                <td>{cluster.recommendation}</td>
              </tr>
            ))}
            {report.failure_clusters.length === 0 && (
              <tr><td colSpan={5}><div className="empty-state"><div className="empty-icon">--</div><h3>No failure clusters</h3><p>Recent diagnostics do not show repeated provider failures.</p></div></td></tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
