export type RouteBuilderTarget = "claude_desktop" | "claude_code" | "codex";

export type CodexRoute = {
  id: string;
  codex_model: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  tool_call_mode: string;
  enabled: boolean;
};

export type CodexGatewayStatus = {
  running: boolean;
  status: string;
  error: string | null;
};

export type CodexBindingInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  model_provider: string | null;
  model: string | null;
  base_url: string | null;
  backup_path: string | null;
};

export type ModelAlias = {
  id: string;
  alias: string;
  alias_type: "claude" | "codex";
  created_at: string | null;
};

export type Status = {
  gateway_running: boolean;
  gateway_port: number;
  gateway_error?: string | null;
  binding_active: boolean;
  provider_count: number;
  route_count: number;
};

export type Provider = {
  id: string;
  name: string;
  base_url: string;
  openai_base_url: string;
  anthropic_base_url: string | null;
  auth_header: string;
  auth_scheme: string | null;
  api_key: string | null;
  enabled: boolean;
};

export type ModelRoute = {
  id: string;
  claude_alias: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  enabled: boolean;
};

export type DesktopInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  base_url: string | null;
  auth_scheme: string | null;
  models: string[];
  backup_path: string | null;
};

export type ClaudeCodeInfo = {
  config_path: string;
  config_exists: boolean;
  managed: boolean;
  base_url: string | null;
  model: string | null;
  auth_env: string | null;
  backup_path: string | null;
};

export type RequestLog = {
  request_id: string;
  claude_alias: string;
  provider_id: string;
  upstream_model: string;
  status_code: number | null;
  duration_ms: number | null;
  is_stream: boolean;
  error_summary: string | null;
  created_at: string;
};

export type Health = {
  target: string;
  ok: boolean;
  message: string;
  latency_ms: number | null;
};

export type ProviderCompatibilityPolicy = {
  provider_id: string;
  system_to_user: boolean | null;
  tool_to_user: boolean | null;
  disable_tools: boolean | null;
  strip_unsupported_params: boolean | null;
  direct_provider_safe: boolean | null;
  gateway_route_recommended: boolean | null;
  codex_disable_responses: boolean | null;
  codex_strict_tool_calls: boolean | null;
  codex_strip_reasoning: boolean | null;
  notes: string | null;
  updated_by: string;
  updated_at: string | null;
};

export type ProviderCompatibilityProfile = {
  strategy_id: string;
  system_to_user: boolean;
  tool_to_user: boolean;
  disable_tools: boolean;
  strip_unsupported_params: boolean;
  direct_provider_safe: boolean;
  gateway_route_recommended: boolean;
  codex_disable_responses: boolean;
  codex_strict_tool_calls: boolean;
  codex_strip_reasoning: boolean;
  summary: string;
};

export type RouteCompatibilityDiagnostic = {
  route_id: string;
  claude_alias: string;
  display_name: string;
  provider_id: string;
  provider_name: string;
  upstream_model: string;
  strategy: ProviderCompatibilityProfile;
  warnings: string[];
  recommendations: string[];
};

export type RoutePayloadPreview = {
  route_id: string;
  claude_alias: string;
  provider_id: string;
  upstream_model: string;
  strategy_id: string;
  roles: string[];
  payload: unknown;
};

export type RuntimeSourceReport = {
  bundle_path: string;
  is_applications: boolean;
  is_dmg_volume: boolean;
  is_temp_volume: boolean;
  severity: string;
  summary: string;
  recommendation: string;
};

export type AppWorkbenchSummary = {
  app_id: RouteBuilderTarget;
  label: string;
  managed: boolean;
  gateway_running: boolean;
  route_count: number;
  provider_count: number;
  active_model: string | null;
  recent_request_count: number;
  recent_failure_count: number;
  next_action: string;
};

export type DiagnosticsMetric = {
  label: string;
  value: string;
  status: string;
};

export type DiagnosticsAction = {
  id: string;
  label: string;
  target: string;
  severity: string;
  detail: string;
};

export type FailureCluster = {
  key: string;
  provider_id: string | null;
  surface: string;
  status_code: number | null;
  count: number;
  sample_error: string | null;
  recommendation: string;
};

export type DiagnosticsSection = {
  id: string;
  title: string;
  status: string;
  score: number;
  summary: string;
  metrics: DiagnosticsMetric[];
  actions: DiagnosticsAction[];
};

export type UnifiedDiagnosticsReport = {
  generated_at: string;
  status: string;
  score: number;
  summary: string;
  sections: DiagnosticsSection[];
  failure_clusters: FailureCluster[];
};

export type RuntimeDashboardReport = {
  generated_at: string;
  overall_status: string;
  overall_score: number;
  claude_gateway: Health;
  codex_gateway: Health;
  provider_count: number;
  claude_route_count: number;
  codex_route_count: number;
  apps: AppWorkbenchSummary[];
  recent_failures: RequestLog[];
  recent_activity: RequestLog[];
  runtime_source: RuntimeSourceReport;
};

export type AppWorkbenchReport = {
  generated_at: string;
  app: AppWorkbenchSummary;
  desktop: DesktopInfo | null;
  claude_code: ClaudeCodeInfo | null;
  codex_binding: CodexBindingInfo | null;
  claude_routes: ModelRoute[];
  codex_routes: CodexRoute[];
  providers: Provider[];
  recent_logs: RequestLog[];
  diagnostics: UnifiedDiagnosticsReport;
};

export type BackendProviderPreset = {
  id: string;
  name: string;
  description: string;
  base_url: string;
  openai_base_url: string;
  anthropic_base_url: string | null;
  auth_header: string;
  auth_scheme: string | null;
  recommended_claude_alias: string;
  recommended_codex_model: string;
  upstream_model_example: string;
  recommended_policy: ProviderCompatibilityPolicy;
  warnings: string[];
};

export type UsageProviderStat = {
  provider_id: string;
  provider_name: string;
  request_count: number;
  failure_count: number;
  success_rate: number;
};

export type UsageStatusBucket = {
  status: string;
  count: number;
};

export type UsageInsightsReport = {
  generated_at: string;
  total_requests: number;
  success_rate: number;
  failure_count: number;
  average_latency_ms: number | null;
  p95_latency_ms: number | null;
  provider_stats: UsageProviderStat[];
  status_buckets: UsageStatusBucket[];
  recent_logs: RequestLog[];
};

export type ProviderConsoleItem = {
  provider: Provider;
  supports_claude: boolean;
  supports_codex: boolean;
  linked_claude_routes: number;
  linked_codex_routes: number;
  recent_request_count: number;
  recent_failure_count: number;
  health_score: number;
  policy_tags: string[];
};

export type ProviderConsoleReport = {
  generated_at: string;
  providers: ProviderConsoleItem[];
  presets: BackendProviderPreset[];
  policies: ProviderCompatibilityPolicy[];
};

export type RouteBuilderPayload = {
  target_app: RouteBuilderTarget;
  route_id: string;
  visible_model: string;
  display_name: string;
  provider_id: string;
  upstream_model: string;
  tool_call_mode?: string | null;
  conflict_strategy?: string | null;
};

export type RouteBuilderApplyReport = {
  claude_routes: ModelRoute[];
  codex_routes: CodexRoute[];
};

export type ProviderWizardApplyReport = {
  provider: Provider;
  providers: Provider[];
  policies: ProviderCompatibilityPolicy[];
  route_report: RouteBuilderApplyReport | null;
};

export type ProviderWizardPayload = {
  preset_id: string;
  api_key?: string | null;
  target_app?: RouteBuilderTarget | null;
  route_id?: string | null;
  visible_model?: string | null;
  display_name?: string | null;
  upstream_model?: string | null;
  apply_route?: boolean | null;
};

export type FailedRequestDiagnosticCandidate = {
  request_id: string;
  surface: string;
  claude_alias: string | null;
  provider_id: string | null;
  upstream_model: string | null;
  status_code: number | null;
  error_summary: string | null;
  redaction_summary: string;
  created_at: string | null;
};

export type RequestReplayReport = {
  request_id: string;
  surface: string;
  provider_id: string | null;
  upstream_model: string | null;
  strategy_id: string;
  original_payload: unknown;
  converted_payload: unknown | null;
  redaction_summary: string;
  likely_cause: string;
  local_only: boolean;
};

export type CodexRouteDiagnostic = {
  route_id: string;
  codex_model: string;
  display_name: string;
  provider_id: string;
  provider_name: string;
  upstream_model: string;
  tool_call_mode: string;
  strategy: ProviderCompatibilityProfile;
  warnings: string[];
  recommendations: string[];
};

export type UpdateCheckReport = {
  current_version: string;
  latest_version: string | null;
  update_available: boolean;
  release_url: string | null;
  asset_names: string[];
  summary: string;
  error: string | null;
};

export type SafeInstallPlan = {
  current_exe: string;
  is_applications: boolean;
  is_dmg_volume: boolean;
  is_temp_volume: boolean;
  applications_app_exists: boolean;
  release_artifacts_dir: string | null;
  steps: string[];
  warning: string | null;
};
