import type {
  BackendProviderPreset,
  RouteBuilderTarget,
} from "../../shared/types/runtime-console";

type ProviderSetupWizardProps = {
  presets: BackendProviderPreset[];
  presetId: string;
  targetApp: RouteBuilderTarget;
  apiKey: string;
  applyRoute: boolean;
  onPresetChange: (presetId: string) => void;
  onTargetAppChange: (targetApp: RouteBuilderTarget) => void;
  onApiKeyChange: (apiKey: string) => void;
  onApplyRouteChange: (applyRoute: boolean) => void;
  onRunWizard: () => void;
  onAdvancedRouteBuilder: () => void;
  t?: (text: string) => string;
  showHeader?: boolean;
};

export function ProviderSetupWizard({
  presets,
  presetId,
  targetApp,
  apiKey,
  applyRoute,
  onPresetChange,
  onTargetAppChange,
  onApiKeyChange,
  onApplyRouteChange,
  onRunWizard,
  onAdvancedRouteBuilder,
  t,
  showHeader = true,
}: ProviderSetupWizardProps) {
  const tr = t ?? ((text: string) => text);
  const preset = presets.find(item => item.id === presetId);

  return (
    <div className={showHeader ? "card" : ""} style={showHeader ? { marginBottom: 20 } : undefined}>
      {showHeader && (
        <>
          <div className="card-title">{tr("Setup Wizard")}</div>
          <p style={{ color: "var(--muted)", marginBottom: 14 }}>
            {tr("Pick a preset, paste the API key, choose the target app, and Gateway Switch will create the provider, policy and recommended route in one click.")}
          </p>
        </>
      )}
      <div className="form-row">
        <div className="form-field">
          <label>{tr("Provider Preset")}</label>
          <select value={presetId} onChange={e => onPresetChange(e.target.value)}>
            {presets.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
        </div>
        <div className="form-field">
          <label>{tr("Target App")}</label>
          <select value={targetApp} onChange={e => onTargetAppChange(e.target.value as RouteBuilderTarget)}>
            <option value="claude_desktop">Claude Desktop</option>
            <option value="claude_code">Claude Code</option>
            <option value="codex">Codex</option>
          </select>
        </div>
        <div className="form-field">
          <label>{tr("API Key")}</label>
          <input type="password" value={apiKey} onChange={e => onApiKeyChange(e.target.value)} placeholder={tr("Leave empty to keep existing key")} />
        </div>
        <div className="form-field">
          <label>{tr("Create Route")}</label>
          <select value={applyRoute ? "yes" : "no"} onChange={e => onApplyRouteChange(e.target.value === "yes")}>
            <option value="yes">{tr("Create recommended route")}</option>
            <option value="no">{tr("Provider only")}</option>
          </select>
        </div>
      </div>
      <div className="wizard-preview-card">
        {!preset ? (
          <span>{tr("No preset selected")}</span>
        ) : (
          <>
            <span>{tr("Provider")}: <strong>{preset.name}</strong></span>
            <span>{tr("Upstream")}: <strong>{preset.upstream_model_example}</strong></span>
            <span>{tr("Visible model")}: <strong>{targetApp === "codex" ? preset.recommended_codex_model : preset.recommended_claude_alias}</strong></span>
            <span>{tr("Policy")}: <strong>{preset.recommended_policy.notes ?? tr("recommended compatibility strategy")}</strong></span>
          </>
        )}
      </div>
      <div className="qa-buttons" style={{ marginTop: 14 }}>
        <button className="btn btn-primary" onClick={onRunWizard}>{tr("Run Setup Wizard")}</button>
        <button className="btn" onClick={onAdvancedRouteBuilder}>{tr("Advanced Route Builder")}</button>
      </div>
    </div>
  );
}
