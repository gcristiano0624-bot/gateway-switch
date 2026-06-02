import type { Provider, ProviderCompatibilityPolicy } from "../../shared/types/runtime-console";

type CompatibilityPreviewProps = {
  provider: Provider | undefined;
  conflictText: string;
  policy: ProviderCompatibilityPolicy | null;
  autoSummary: string | null;
  t: (text: string) => string;
  onOpenWorkbench: () => void;
  onQuickCheck: () => void;
};

export function CompatibilityPreview({
  provider,
  conflictText,
  policy,
  autoSummary,
  t,
  onOpenWorkbench,
  onQuickCheck,
}: CompatibilityPreviewProps) {
  return (
    <div className="card">
      <div className="card-title">{t("Compatibility Preview")}</div>
      <div className="route-builder-preview">
        <div>
          <span>{t("Provider")}</span>
          <strong>{provider?.name ?? t("Not selected")}</strong>
        </div>
        <div>
          <span>{t("Conflict")}</span>
          <strong>{t(conflictText)}</strong>
        </div>
        <div>
          <span>{t("Policy")}</span>
          <strong>{policy?.notes ?? autoSummary ?? t("Auto profile will be inferred")}</strong>
        </div>
      </div>
      <div className="qa-buttons" style={{ marginTop: 14 }}>
        <button className="btn" onClick={onOpenWorkbench}>{t("Open Workbench")}</button>
        <button className="btn" onClick={onQuickCheck}>{t("Quick Check")}</button>
      </div>
    </div>
  );
}
