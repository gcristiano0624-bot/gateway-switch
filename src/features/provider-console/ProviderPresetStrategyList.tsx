import type { BackendProviderPreset, Provider } from "../../shared/types/runtime-console";

type ProviderPresetStrategyListProps = {
  presets: BackendProviderPreset[];
  providers: Provider[];
  onApplyPreset: (preset: BackendProviderPreset) => void;
};

export function ProviderPresetStrategyList({ presets, providers, onApplyPreset }: ProviderPresetStrategyListProps) {
  return (
    <div className="card" style={{ marginTop: 20, marginBottom: 20 }}>
      <div className="card-title">Provider Presets & Strategy Templates</div>
      <p style={{ color: "var(--muted)", marginBottom: 14 }}>
        Built-in presets create or update provider URLs and apply the recommended compatibility strategy without overwriting existing API keys.
      </p>
      <div className="route-list" style={{ marginBottom: 0 }}>
        {presets.map(preset => {
          const connected = providers.some(provider => provider.id === preset.id);
          const policy = preset.recommended_policy;
          const enabledFlags = [
            policy.system_to_user ? "system_to_user" : null,
            policy.tool_to_user ? "tool_to_user" : null,
            policy.strip_unsupported_params ? "strip_params" : null,
            policy.gateway_route_recommended ? "gateway_route" : null,
            policy.codex_disable_responses ? "codex_chat_fallback" : null,
            policy.codex_strict_tool_calls ? "strict_tools" : null,
            policy.codex_strip_reasoning ? "strip_reasoning" : null,
          ].filter((flag): flag is string => Boolean(flag));
          return (
            <div key={preset.id} className="route-item" style={{ alignItems: "flex-start" }}>
              <div className="route-info">
                <div className="route-name">{preset.name}</div>
                <div className="route-path">{preset.description}</div>
                <div className="route-path">
                  model: {preset.upstream_model_example} · Claude alias: {preset.recommended_claude_alias} · Codex: {preset.recommended_codex_model}
                </div>
                <div className="qa-buttons" style={{ marginTop: 8 }}>
                  {enabledFlags.map(flag => <span key={flag} className="badge badge-blue">{flag}</span>)}
                  {preset.warnings.map(warning => <span key={warning} className="badge badge-amber">{warning}</span>)}
                </div>
              </div>
              <button className={`btn ${connected ? "" : "btn-primary"}`} onClick={() => onApplyPreset(preset)}>
                {connected ? "Reapply" : "Apply"}
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
