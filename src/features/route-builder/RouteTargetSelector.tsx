import type { RouteBuilderTarget } from "../../shared/types/runtime-console";

type RouteTargetSelectorProps = {
  target: RouteBuilderTarget;
  t: (text: string) => string;
  onTargetChange: (target: RouteBuilderTarget) => void;
};

const TARGETS: Array<[RouteBuilderTarget, string, string]> = [
  ["claude_desktop", "Claude Desktop", "Alias route for Claude Desktop model validation"],
  ["claude_code", "Claude Code", "Gateway route for safer coding agent runs"],
  ["codex", "Codex", "Responses gateway route for Codex App"],
];

export function RouteTargetSelector({ target, t, onTargetChange }: RouteTargetSelectorProps) {
  return (
    <div className="card">
      <div className="card-title">{t("Target App")}</div>
      <div className="route-target-list">
        {TARGETS.map(([value, label, description]) => (
          <button key={value} className={`route-target-card ${target === value ? "active" : ""}`} onClick={() => onTargetChange(value)}>
            <strong>{t(label)}</strong>
            <span>{t(description)}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
