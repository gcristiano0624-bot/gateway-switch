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
}: ProviderSetupWizardProps) {
  const preset = presets.find(item => item.id === presetId);

  return (
    <div className="card" style={{ marginBottom: 20 }}>
      <div className="card-title">Setup Wizard</div>
      <p style={{ color: "var(--muted)", marginBottom: 14 }}>
        选择 Provider 预设、填写 Key、选择目标客户端，然后一键生成 Provider、兼容策略和推荐 Route。
      </p>
      <div className="form-row">
        <div className="form-field">
          <label>Provider Preset</label>
          <select value={presetId} onChange={e => onPresetChange(e.target.value)}>
            {presets.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
        </div>
        <div className="form-field">
          <label>Target App</label>
          <select value={targetApp} onChange={e => onTargetAppChange(e.target.value as RouteBuilderTarget)}>
            <option value="claude_desktop">Claude Desktop</option>
            <option value="claude_code">Claude Code</option>
            <option value="codex">Codex</option>
          </select>
        </div>
        <div className="form-field">
          <label>API Key</label>
          <input type="password" value={apiKey} onChange={e => onApiKeyChange(e.target.value)} placeholder="Leave empty to keep existing key" />
        </div>
        <div className="form-field">
          <label>Create Route</label>
          <select value={applyRoute ? "yes" : "no"} onChange={e => onApplyRouteChange(e.target.value === "yes")}>
            <option value="yes">Create recommended route</option>
            <option value="no">Provider only</option>
          </select>
        </div>
      </div>
      <div className="wizard-preview-card">
        {!preset ? (
          <span>No preset selected</span>
        ) : (
          <>
            <span>Provider: <strong>{preset.name}</strong></span>
            <span>Upstream: <strong>{preset.upstream_model_example}</strong></span>
            <span>Visible model: <strong>{targetApp === "codex" ? preset.recommended_codex_model : preset.recommended_claude_alias}</strong></span>
            <span>Policy: <strong>{preset.recommended_policy.notes ?? "recommended compatibility strategy"}</strong></span>
          </>
        )}
      </div>
      <div className="qa-buttons" style={{ marginTop: 14 }}>
        <button className="btn btn-primary" onClick={onRunWizard}>Run Setup Wizard</button>
        <button className="btn" onClick={onAdvancedRouteBuilder}>Advanced Route Builder</button>
      </div>
    </div>
  );
}
