import type { Provider, RequestLog, UsageInsightsReport } from "../../shared/types/runtime-console";

type UsageInsightsPanelProps = {
  usageInsights: UsageInsightsReport | null;
  logs: RequestLog[];
  providers: Provider[];
  onRefresh: () => void;
  onOpenLogs: () => void;
};

export function UsageInsightsPanel({ usageInsights, logs, providers, onRefresh, onOpenLogs }: UsageInsightsPanelProps) {
  const insightLogs = usageInsights?.recent_logs ?? logs;
  const totalRequests = usageInsights?.total_requests ?? logs.length;
  const failedRequests = usageInsights?.failure_count ?? logs.filter(log => log.status_code == null || log.status_code >= 400).length;
  const successfulRequests = Math.max(totalRequests - failedRequests, 0);
  const successRate = usageInsights?.success_rate ?? (totalRequests ? Math.round((successfulRequests / totalRequests) * 100) : 0);
  const durations = logs.map(log => log.duration_ms).filter((value): value is number => value != null);
  const averageLatency = usageInsights?.average_latency_ms ?? (durations.length ? Math.round(durations.reduce((sum, value) => sum + value, 0) / durations.length) : 0);
  const sortedDurations = [...durations].sort((a, b) => a - b);
  const p95Latency = usageInsights?.p95_latency_ms ?? (sortedDurations.length ? sortedDurations[Math.min(sortedDurations.length - 1, Math.floor(sortedDurations.length * 0.95))] : 0);
  const providerUsage = usageInsights
    ? usageInsights.provider_stats.map(stat => ({ id: stat.provider_id, name: stat.provider_name, count: stat.request_count, failures: stat.failure_count }))
    : providers
      .map(provider => ({
        id: provider.id,
        name: provider.name,
        count: logs.filter(log => log.provider_id === provider.id).length,
        failures: logs.filter(log => log.provider_id === provider.id && (log.status_code == null || log.status_code >= 400)).length,
      }))
      .sort((a, b) => b.count - a.count);
  const statusBuckets = usageInsights
    ? Object.fromEntries(usageInsights.status_buckets.map(bucket => [bucket.status, bucket.count]))
    : logs.reduce<Record<string, number>>((acc, log) => {
      const key = log.status_code == null ? "network" : String(log.status_code);
      acc[key] = (acc[key] ?? 0) + 1;
      return acc;
    }, {});

  return (
    <div>
      <div className="page-header page-header-row">
        <div>
          <h1>Usage Insights</h1>
          <p>第一版只统计本地请求使用量与稳定性，不做成本核算、不上传云端。</p>
        </div>
        <div className="qa-buttons" style={{ margin: 0 }}>
          <button className="btn" onClick={onRefresh}>Refresh</button>
          <button className="btn" onClick={onOpenLogs}>Request Logs</button>
        </div>
      </div>

      <div className="kpi-row">
        <div className="kpi-card"><div className="kpi-icon blue" /><div className="kpi-info"><div className="kpi-label">Requests</div><div className="kpi-value">{totalRequests}</div></div></div>
        <div className="kpi-card"><div className="kpi-icon green" /><div className="kpi-info"><div className="kpi-label">Success Rate</div><div className="kpi-value">{successRate}%</div></div></div>
        <div className="kpi-card"><div className="kpi-icon amber" /><div className="kpi-info"><div className="kpi-label">Avg Latency</div><div className="kpi-value">{averageLatency}ms</div></div></div>
        <div className="kpi-card"><div className="kpi-icon purple" /><div className="kpi-info"><div className="kpi-label">P95 Latency</div><div className="kpi-value">{p95Latency}ms</div></div></div>
        <div className="kpi-card"><div className="kpi-icon amber" /><div className="kpi-info"><div className="kpi-label">Failures</div><div className="kpi-value">{failedRequests}</div></div></div>
      </div>

      <div className="two-col">
        <div className="card">
          <div className="card-title">Provider Usage</div>
          <div className="usage-rank-list">
            {providerUsage.map(({ id, name, count, failures }) => {
              const width = totalRequests ? Math.max(4, Math.round((count / totalRequests) * 100)) : 0;
              return (
                <div key={id} className="usage-rank-row">
                  <div className="usage-rank-head">
                    <strong>{name}</strong>
                    <span>{count} requests · {failures} failures</span>
                  </div>
                  <div className="usage-bar"><span style={{ width: `${width}%` }} /></div>
                </div>
              );
            })}
            {providerUsage.length === 0 && <div className="empty-state"><div className="empty-icon">--</div><h3>No provider usage yet</h3><p>Requests will appear after Claude or Codex traffic passes through the gateway.</p></div>}
          </div>
        </div>

        <div className="card">
          <div className="card-title">Status Breakdown</div>
          <div className="status-bucket-grid">
            {Object.entries(statusBuckets).map(([statusCode, count]) => (
              <div key={statusCode} className="status-bucket">
                <span>{statusCode}</span>
                <strong>{count}</strong>
              </div>
            ))}
            {Object.keys(statusBuckets).length === 0 && <div className="empty-state"><div className="empty-icon">--</div><h3>No status data</h3><p>Gateway request logs do not contain status codes yet.</p></div>}
          </div>
        </div>
      </div>

      <div className="section-label">Recent Reliability Signals</div>
      <div className="table-wrap">
        <table>
          <thead>
            <tr><th>Time</th><th>App / Model</th><th>Provider</th><th>Status</th><th>Duration</th><th>Signal</th></tr>
          </thead>
          <tbody>
            {insightLogs.slice(0, 12).map(log => (
              <tr key={`usage-${log.request_id}-${log.created_at}`}>
                <td style={{ fontSize: 12, color: "var(--muted)", fontFamily: "var(--font-mono)" }}>{log.created_at.replace("T", " ").slice(0, 19)}</td>
                <td>{log.claude_alias}</td>
                <td><span className="badge badge-blue">{log.provider_id}</span></td>
                <td><span className={`badge ${log.status_code && log.status_code < 400 ? "badge-green" : "badge-red"}`}>{log.status_code ?? "network"}</span></td>
                <td>{log.duration_ms != null ? `${log.duration_ms}ms` : "-"}</td>
                <td>{log.error_summary ?? (log.is_stream ? "stream" : "sync")}</td>
              </tr>
            ))}
            {insightLogs.length === 0 && <tr><td colSpan={6}><div className="empty-state"><div className="empty-icon">--</div><h3>No request logs</h3><p>Start using Claude or Codex through Gateway Switch to see usage insights.</p></div></td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}
