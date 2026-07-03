import { invoke } from "@tauri-apps/api/core";
import type {
  AppWorkbenchReport,
  ProviderConsoleReport,
  ProviderWizardApplyReport,
  ProviderWizardPayload,
  RouteBuilderApplyReport,
  RouteBuilderPayload,
  RuntimeConsoleBundle,
  RuntimeDashboardReport,
  UsageInsightsReport,
} from "../types/runtime-console";

export const getRuntimeDashboard = () =>
  invoke<RuntimeDashboardReport>("get_runtime_dashboard");

export const getAppWorkbench = (appId: "claude_desktop" | "claude_code" | "codex") =>
  invoke<AppWorkbenchReport>("get_app_workbench", { appId });

export const getProviderConsole = () =>
  invoke<ProviderConsoleReport>("get_provider_console");

export const getUsageInsights = () =>
  invoke<UsageInsightsReport>("get_usage_insights");

export const getRuntimeConsoleBundle = () =>
  invoke<RuntimeConsoleBundle>("get_runtime_console_bundle");

export const applyRouteBuilder = (payload: RouteBuilderPayload) =>
  invoke<RouteBuilderApplyReport>("apply_route_builder", { payload });

export const applyProviderWizard = (payload: ProviderWizardPayload) =>
  invoke<ProviderWizardApplyReport>("apply_provider_wizard", { payload });
