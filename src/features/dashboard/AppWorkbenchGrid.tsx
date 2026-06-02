import type { AppWorkbenchSummary, RouteBuilderTarget } from "../../shared/types/runtime-console";

type AppWorkbenchGridProps = {
  apps: AppWorkbenchSummary[];
  onOpenWorkbench: (appId: RouteBuilderTarget) => void;
};

export function AppWorkbenchGrid({ apps, onOpenWorkbench }: AppWorkbenchGridProps) {
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
              {app.managed && app.gateway_running ? "ready" : app.managed ? "check" : "setup"}
            </span>
          </div>
          <div className="app-workbench-meta">
            <span>Routes: {app.route_count}</span>
            <span>Gateway: {app.gateway_running ? "running" : "stopped"}</span>
            <span>Model: {app.active_model ?? "not set"}</span>
            <span>Recent: {app.recent_request_count} req / {app.recent_failure_count} fail</span>
          </div>
          <button className="btn" onClick={() => onOpenWorkbench(app.app_id)}>Open workbench</button>
        </div>
      ))}
    </div>
  );
}
