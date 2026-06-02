import type {
  Provider,
  ProviderCompatibilityPolicy,
  ProviderCompatibilityProfile,
} from "../../shared/types/runtime-console";

type StrategyFlagKey = keyof Omit<ProviderCompatibilityPolicy, "provider_id" | "notes" | "updated_by" | "updated_at">;

const STRATEGY_FLAG_HELP: Record<StrategyFlagKey, { title: string; effect: string; when: string; defaultAdvice: string; risk?: string }> = {
  system_to_user: {
    title: "System -> User",
    effect: "把 Anthropic 的 system prompt 合并进第一条 user 消息，避免上游看到 system role。",
    when: "火山 DeepSeek 等只接受 user/assistant 的服务商，或报 messages.role system invalid 时开启。",
    defaultAdvice: "不确定时保持 auto；Provider Preset 会自动给高风险服务商开启。",
  },
  tool_to_user: {
    title: "Tool -> User",
    effect: "把 tool result 转成普通 user 文本，避免上游看到 tool role。",
    when: "服务商报 tool role invalid、tool message unsupported 或工具结果无法继续对话时开启。",
    defaultAdvice: "Chat 兼容但非 Anthropic 兼容的服务商通常建议开启。",
  },
  disable_tools: {
    title: "Disable Tools",
    effect: "完全移除 tools 和 tool_choice，模型只会文本回复。",
    when: "仅在服务商完全不支持工具调用，或频繁因为 tools 参数报错时开启。",
    defaultAdvice: "默认不要开；优先尝试 tool_to_user 或 codex_strict_tool_calls。",
    risk: "高风险：开启后 Claude/Codex 可能无法调用工具。",
  },
  strip_unsupported_params: {
    title: "Strip Params",
    effect: "移除 thinking、reasoning、reasoning_effort 等兼容性差的参数。",
    when: "OpenAI Chat 兼容服务商报 unknown parameter、reasoning unsupported 时开启。",
    defaultAdvice: "大多数第三方 Chat provider 建议开启。",
  },
  direct_provider_safe: {
    title: "Direct Safe",
    effect: "标记 Claude Code 是否可以绕过 Gateway，直接使用该 Provider 的 Anthropic endpoint。",
    when: "只有服务商真实完整支持 Anthropic Messages API 时才设为 true。",
    defaultAdvice: "不确定就保持 false/auto，并使用 Gateway Route。",
    risk: "误开会让 Claude Code 继续发送 system/tool role，可能再次 400。",
  },
  gateway_route_recommended: {
    title: "Gateway Route",
    effect: "提示 Claude Code 优先走 Gateway Route，让 Gateway Switch 负责协议转换。",
    when: "绝大多数第三方 Chat provider、DeepSeek、Moonshot、Qwen、小米、火山都建议开启。",
    defaultAdvice: "第三方服务商默认建议 true；标准 Anthropic 兼容服务商可为 false。",
  },
  codex_disable_responses: {
    title: "Codex Chat Fallback",
    effect: "Codex 不直接走上游 Responses API，而是通过 Gateway 转成 Chat Completions。",
    when: "非 OpenAI 官方 Responses 兼容服务商通常需要开启。",
    defaultAdvice: "OpenAI 官方可关闭；其他 Provider 建议开启。",
  },
  codex_strict_tool_calls: {
    title: "Codex Strict Tools",
    effect: "Codex 有 tools 时强制模型输出结构化 tool_calls，而不是只描述计划。",
    when: "模型经常说“我来查看/我会调用工具”但没有真实 tool_calls 时开启。",
    defaultAdvice: "工具执行不稳定时开启；如果模型不支持工具则不要开。",
    risk: "可能让弱工具模型更容易失败，但能显著减少假执行。",
  },
  codex_strip_reasoning: {
    title: "Codex Strip Reasoning",
    effect: "移除 Codex 请求里的 reasoning/thinking 参数。",
    when: "服务商不支持 reasoning 参数，或报 thinking/reasoning unsupported 时开启。",
    defaultAdvice: "大多数第三方 Chat provider 建议开启。",
  },
};

const FLAG_ROWS: Array<[StrategyFlagKey, string]> = [
  ["system_to_user", "System -> User"],
  ["tool_to_user", "Tool -> User"],
  ["disable_tools", "Disable Tools"],
  ["strip_unsupported_params", "Strip Params"],
  ["direct_provider_safe", "Direct Safe"],
  ["gateway_route_recommended", "Gateway Route"],
  ["codex_disable_responses", "Codex Chat Fallback"],
  ["codex_strict_tool_calls", "Codex Strict Tools"],
  ["codex_strip_reasoning", "Codex Strip Reasoning"],
];

type ProviderStrategyOverridesProps = {
  providers: Provider[];
  policyForProvider: (providerId: string) => ProviderCompatibilityPolicy | undefined;
  autoStrategyForProvider: (providerId: string) => ProviderCompatibilityProfile | undefined;
  onSaveFlag: (providerId: string, key: StrategyFlagKey, value: boolean | null) => void;
  onResetPolicy: (providerId: string) => void;
};

export function ProviderStrategyOverrides({
  providers,
  policyForProvider,
  autoStrategyForProvider,
  onSaveFlag,
  onResetPolicy,
}: ProviderStrategyOverridesProps) {
  return (
    <div className="card" style={{ marginTop: 20 }}>
      <div className="card-title">Provider Strategy Overrides</div>
      <p style={{ color: "var(--muted)", marginBottom: 14 }}>
        Empty fields inherit the automatic profile. Manual overrides affect Claude, Claude Code, and Codex diagnostics.
      </p>
      <div className="route-list" style={{ marginBottom: 0 }}>
        {providers.map(provider => {
          const policy = policyForProvider(provider.id);
          const auto = autoStrategyForProvider(provider.id);
          return (
            <div key={provider.id} className="route-item" style={{ alignItems: "flex-start" }}>
              <div className="route-info">
                <div className="route-name">{provider.name}</div>
                <div className="route-path">
                  auto: {auto?.strategy_id ?? "not routed"} · override: {policy ? "active" : "inherit"}
                </div>
                <div className="qa-buttons" style={{ marginTop: 10 }}>
                  {FLAG_ROWS.map(([key, label]) => {
                    const help = STRATEGY_FLAG_HELP[key];
                    return (
                      <span key={key} className="strategy-flag-wrap">
                        <button
                          className={`btn strategy-flag-btn ${policy?.[key] === true ? "btn-primary" : ""}`}
                          onClick={() => onSaveFlag(provider.id, key, policy?.[key] === true ? null : true)}
                        >
                          {label}: {policy?.[key] === true ? "on" : policy?.[key] === false ? "off" : "auto"}
                        </button>
                        <button
                          type="button"
                          className={`strategy-help-trigger ${help.risk ? "risk" : ""}`}
                          aria-label={`${help.title} help`}
                          title={`${help.effect} ${help.when}`}
                        >
                          ?
                        </button>
                        <span className="strategy-help-popover" role="tooltip">
                          <strong>{help.title}</strong>
                          <span><b>作用：</b>{help.effect}</span>
                          <span><b>什么时候开启：</b>{help.when}</span>
                          <span><b>默认建议：</b>{help.defaultAdvice}</span>
                          {help.risk && <span className="strategy-risk"><b>风险：</b>{help.risk}</span>}
                        </span>
                      </span>
                    );
                  })}
                </div>
              </div>
              <button className="btn" onClick={() => onResetPolicy(provider.id)}>Reset</button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
