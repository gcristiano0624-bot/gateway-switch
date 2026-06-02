import type {
  AppWorkbenchReport,
  AppWorkbenchSummary,
  CodexRoute,
  ModelRoute,
  RequestLog,
  RouteBuilderTarget,
} from "../../shared/types/runtime-console";

type AppWorkbenchOverviewProps = {
  appId: RouteBuilderTarget;
  report?: AppWorkbenchReport;
  summaries: AppWorkbenchSummary[];
  logs: RequestLog[];
  routes: ModelRoute[];
  codexRoutes: CodexRoute[];
  t: (text: string) => string;
  onConfigureProvider: () => void;
  onRunCheck: () => void;
};

const appRecentLogs = (
  appId: RouteBuilderTarget,
  logs: RequestLog[],
  routes: ModelRoute[],
  codexRoutes: CodexRoute[],
) => appId === "codex"
  ? logs.filter(log => codexRoutes.some(route => route.codex_model === log.claude_alias))
  : logs.filter(log => routes.some(route => route.claude_alias === log.claude_alias));

export function AppWorkbenchOverview({
  appId,
  report,
  summaries,
  logs,
  routes,
  codexRoutes,
  t,
  onConfigureProvider,
  onRunCheck,
}: AppWorkbenchOverviewProps) {
  const summary = report?.app ?? summaries.find(app => app.app_id === appId);
  if (!summary) return null;
  const recentLogs = report?.recent_logs ?? appRecentLogs(appId, logs, routes, codexRoutes);

  return (
    <div className="workbench-overview">
      <div className="workbench-overview-main">
        <div className="runtime-eyebrow">{t("App Workbench")}</div>
        <h2>{summary.label}</h2>
        <p>{t(summary.next_action)}</p>
      </div>
      <div className="workbench-overview-grid">
        <div><span>{t("Binding")}</span><strong>{summary.managed ? t("managed") : t("setup needed")}</strong></div>
        <div><span>{t("Gateway")}</span><strong>{summary.gateway_running ? t("running") : t("stopped")}</strong></div>
        <div><span>{t("Routes")}</span><strong>{summary.route_count}</strong></div>
        <div><span>{t("Provider")}</span><strong>{summary.provider_count}</strong></div>
        <div><span>{t("Model")}</span><strong>{summary.active_model ?? t("not set")}</strong></div>
        <div><span>{t("Recent")}</span><strong>{summary.recent_request_count} / {summary.recent_failure_count}</strong></div>
      </div>
      <div className="workbench-overview-actions">
        <button className="btn btn-primary" onClick={onConfigureProvider}>{t("Configure Provider")}</button>
        <button className="btn" onClick={onRunCheck}>{t("Run Check")}</button>
      </div>
      {recentLogs.length > 0 && (
        <div className="workbench-recent-strip">
          {recentLogs.slice(0, 3).map(log => (
            <span key={`${appId}-${log.request_id}-${log.created_at}`}>
              {log.claude_alias} · {log.status_code ?? "network"} · {log.duration_ms ?? "-"}ms
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
