import type { AppWorkbenchSummary, RouteBuilderTarget } from "../../shared/types/runtime-console";

type AppWorkbenchGridProps = {
  apps: AppWorkbenchSummary[];
  onOpenWorkbench: (appId: RouteBuilderTarget) => void;
  t: (text: string) => string;
};

export function AppWorkbenchGrid({ apps, onOpenWorkbench, t }: AppWorkbenchGridProps) {
  return (
    <div className="app-workbench-grid">
      {apps.map(app => (
        <div key={app.app_id} className="app-workbench-card">
          <div className="app-workbench-head">
            <div>
              <div className="app-workbench-title">{app.label}</div>
              <div className="app-workbench-subtitle">{app.next_action}</div>
            </div>
            <span className={`badge ${app.managed && app.gateway_running ? "badge-green" : app.managed || app.gateway_running ? "badge-amber" : "badge-gray"}`}>
              {app.managed && app.gateway_running ? t("ready") : app.managed ? t("check") : t("setup")}
            </span>
          </div>
          <div className="app-workbench-meta">
            <span>{t("Routes:")} {app.route_count}</span>
            <span>{t("Gateway:")} {app.gateway_running ? t("running") : t("stopped")}</span>
            <span>{t("Model:")} {app.active_model ?? t("not set")}</span>
            <span>{t("Recent:")} {app.recent_request_count} req / {app.recent_failure_count} fail</span>
          </div>
          <button className="btn" onClick={() => onOpenWorkbench(app.app_id)}>{t("Open workbench")}</button>
        </div>
      ))}
    </div>
  );
}
