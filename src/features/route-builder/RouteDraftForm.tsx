import type { Provider } from "../../shared/types/runtime-console";

export type ClaudeRouteDraft = {
  id: string;
  claude_alias: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
};

export type CodexRouteDraft = {
  id: string;
  codex_model: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  tool_call_mode: string;
};

type RouteDraftFormProps = {
  isCodexTarget: boolean;
  targetCopy: string;
  providers: Provider[];
  claudeAliasOptions: string[];
  codexModelOptions: string[];
  claudeForm: ClaudeRouteDraft;
  codexForm: CodexRouteDraft;
  editingClaude: boolean;
  editingCodex: boolean;
  t: (text: string) => string;
  onClaudeFormChange: (form: ClaudeRouteDraft) => void;
  onCodexFormChange: (form: CodexRouteDraft) => void;
  onSaveClaude: () => void;
  onSaveCodex: () => void;
  onCancelClaude: () => void;
  onCancelCodex: () => void;
};

export function RouteDraftForm({
  isCodexTarget,
  targetCopy,
  providers,
  claudeAliasOptions,
  codexModelOptions,
  claudeForm,
  codexForm,
  editingClaude,
  editingCodex,
  t,
  onClaudeFormChange,
  onCodexFormChange,
  onSaveClaude,
  onSaveCodex,
  onCancelClaude,
  onCancelCodex,
}: RouteDraftFormProps) {
  return (
    <div className="card">
      <div className="card-title">{editingCodex || editingClaude ? t("Edit Route") : t("Add Route")}</div>
      <p className="field-hint" style={{ marginBottom: 12 }}>{targetCopy}</p>
      {isCodexTarget ? (
        <div className="form-row">
          <div className="form-field">
            <label>{t("Route ID")}</label>
            <input value={codexForm.id} disabled={editingCodex} onChange={e => onCodexFormChange({ ...codexForm, id: e.target.value })} placeholder="e.g. gpt4o-deepseek" />
          </div>
          <div className="form-field">
            <label>{t("Codex Model (requested by Codex)")}</label>
            <select value={codexForm.codex_model} onChange={e => onCodexFormChange({ ...codexForm, codex_model: e.target.value })}>
              {codexModelOptions.map(model => <option key={model} value={model}>{model}</option>)}
            </select>
            <span className="field-hint">{t("This is the model name used in `codex -m ...`.")}</span>
          </div>
          <div className="form-field">
            <label>{t("Display Name")}</label>
            <input value={codexForm.display_name} onChange={e => onCodexFormChange({ ...codexForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>{t("Provider")}</label>
            <select value={codexForm.provider_id} onChange={e => onCodexFormChange({ ...codexForm, provider_id: e.target.value })}>
              <option value="">{t("Select provider...")}</option>
              {providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>{t("Upstream Model (real provider model)")}</label>
            <input value={codexForm.upstream_model} onChange={e => onCodexFormChange({ ...codexForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-chat" />
            <span className="field-hint">{t("This is the actual model name sent to the third-party API.")}</span>
          </div>
          <div className="form-field">
            <label>{t("Tool Call Mode")}</label>
            <select value={codexForm.tool_call_mode} onChange={e => onCodexFormChange({ ...codexForm, tool_call_mode: e.target.value })}>
              <option value="auto">{t("Auto")}</option>
              <option value="force_when_tools_present">{t("Force When Tools Present")}</option>
              <option value="strict_execution">{t("Strict Execution")}</option>
            </select>
            <span className="field-hint">
              {codexForm.tool_call_mode === "auto" && t("Keeps the model's default behavior. Best compatibility, but weak tool models may only talk.")}
              {codexForm.tool_call_mode === "force_when_tools_present" && t("Default. When Codex sends tools, Gateway asks the upstream model to emit tool_calls first.")}
              {codexForm.tool_call_mode === "strict_execution" && t("If tools are present but no tool_calls are emitted, Gateway marks the response as failed.")}
            </span>
          </div>
          <div className="qa-buttons" style={{ gridColumn: "1 / -1", marginTop: 4 }}>
            <button className="btn btn-primary" onClick={onSaveCodex}>{editingCodex ? t("Save") : t("Add Route")}</button>
            {editingCodex && <button className="btn" onClick={onCancelCodex}>{t("Cancel")}</button>}
          </div>
        </div>
      ) : (
        <div className="form-row">
          <div className="form-field">
            <label>{t("Route ID")}</label>
            <input value={claudeForm.id} disabled={editingClaude} onChange={e => onClaudeFormChange({ ...claudeForm, id: e.target.value })} placeholder="e.g. sonnet-ark" />
          </div>
          <div className="form-field">
            <label>{t("Claude Alias")}</label>
            <select value={claudeForm.claude_alias} onChange={e => onClaudeFormChange({ ...claudeForm, claude_alias: e.target.value })}>
              {claudeAliasOptions.map(alias => <option key={alias} value={alias}>{alias}</option>)}
            </select>
          </div>
          <div className="form-field">
            <label>{t("Display Name")}</label>
            <input value={claudeForm.display_name} onChange={e => onClaudeFormChange({ ...claudeForm, display_name: e.target.value })} placeholder="e.g. DeepSeek V3" />
          </div>
          <div className="form-field">
            <label>{t("Provider")}</label>
            <select value={claudeForm.provider_id} onChange={e => onClaudeFormChange({ ...claudeForm, provider_id: e.target.value })}>
              <option value="">{t("Select provider...")}</option>
              {providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name}</option>)}
            </select>
          </div>
          <div className="form-field" style={{ gridColumn: "1 / -1" }}>
            <label>{t("Upstream Model")}</label>
            <input value={claudeForm.upstream_model} onChange={e => onClaudeFormChange({ ...claudeForm, upstream_model: e.target.value })} placeholder="e.g. deepseek-v3" />
          </div>
          <div className="qa-buttons" style={{ gridColumn: "1 / -1", marginTop: 4 }}>
            <button className="btn btn-primary" onClick={onSaveClaude}>{editingClaude ? t("Save") : t("Add Route")}</button>
            {editingClaude && <button className="btn" onClick={onCancelClaude}>{t("Cancel")}</button>}
          </div>
        </div>
      )}
    </div>
  );
}
