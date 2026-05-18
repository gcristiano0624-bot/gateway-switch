# codex_coldstart.skill

## Purpose

This skill is used when Codex has just been switched to a third-party model provider and needs to cold-start, self-check, configure, validate, and optimize the local Agent Coding environment.

Use this skill when:

- Codex is connected to a third-party OpenAI-compatible or Responses-compatible provider.
- Codex can chat but MCP / plugins / GitHub / shell / file editing capability is uncertain.
- A new computer needs to reproduce a known-good Codex third-party-provider setup.
- The user wants Codex to behave like an execution agent, not just a chat model.

Primary goal:

> Verify that the third-party Codex provider can run a real Agent loop:
> model reasoning -> tool selection -> MCP call -> file read/write/edit -> shell execution -> validation -> report.

Do not stop halfway. Run checks, collect evidence, generate reports, and recommend safe configuration changes.

---

## Operating Style

Default response language: Chinese.

Behavior requirements:

- Be direct and execution-oriented.
- Do not ask the user for confirmation unless a destructive or credential-sensitive action is required.
- Do not claim MCP is working unless actual tool calls have succeeded.
- Do not rely on memory or prior knowledge when MCP / docs / GitHub tools can verify facts.
- For diagnostics, always write outputs to an isolated directory.
- For code changes, read files first, edit minimally, then verify.
- Do not expose tokens, API keys, bearer headers, or cookies.
- Do not commit, push, merge, or create PRs unless explicitly requested.

---

## Recommended Output Directory

Create a dedicated directory in the current project:

```bash
codex_coldstart_validation/
```

All generated files must be written there.

Recommended files:

- `current_environment.md`
- `mcp_validation.md`
- `provider_stability.md`
- `github_validation.md`
- `real_agent_task.md`
- `security_audit.md`
- `optimized_config.toml`
- `optimized_project_config.toml`
- `optimized_AGENTS.md`
- `FINAL_REPORT.md`

---

## Phase 1: Environment Discovery

Collect and record:

```bash
pwd
codex --version
node -v
npm -v
python3 --version
codex debug models
codex mcp list
```

Also inspect, sanitize, and summarize:

```bash
~/.codex/config.toml
./.codex/config.toml
./AGENTS.md
```

Important:

- Redact tokens and secrets.
- Record model provider, model name, `wire_api`, `base_url`, reasoning effort, verbosity, sandbox, approval policy, MCP servers.
- Do not overwrite existing config.

Write result to:

```text
codex_coldstart_validation/current_environment.md
```

---

## Phase 2: MCP Server Validation

Validate that MCP is not only configured, but actually callable.

Required MCP servers to test if available:

- filesystem
- context7
- openaiDeveloperDocs
- github

### 2.1 filesystem MCP

Test:

- list current directory
- read `AGENTS.md` if present
- create validation directory
- write a file
- edit a file
- read back and verify

Expected tools may include:

- `list_directory`
- `read_text_file`
- `create_directory`
- `write_file`
- `edit_file`

### 2.2 context7 MCP

Test:

- resolve library id for React or another common library
- query docs for `useEffect`, `Next.js routing`, or another development topic

Expected tools may include:

- `resolve-library-id`
- `query-docs`

### 2.3 openaiDeveloperDocs MCP

Test:

- search Codex MCP documentation
- search Codex config documentation
- search Responses API tools / tool calling / structured outputs

Expected tools may include:

- `search_openai_docs`
- `fetch_openai_doc`
- `list_openai_docs`

### 2.4 GitHub MCP

Test:

- search a public repository, e.g. `openai/codex`
- search a user-provided repository if available
- do not expose tokens
- if authentication is unavailable, clearly state the limitation

Expected tools may include:

- `search_repositories`
- repository metadata / file / issue / PR tools depending on installed server

Write result to:

```text
codex_coldstart_validation/mcp_validation.md
```

Validation table format:

| server | configured | callable | tools tested | result | evidence |
| ------ | ----------:| --------:| ------------ | ------ | -------- |

---

## Phase 3: Agent Tool-Calling Validation

This is the most important phase.

Do not only use an external MCP client directly. Validate whether Codex Agent itself can trigger MCP/tool calls during a normal task.

Run a task similar to:

```text
As Codex Agent, complete these actions:
1. Use filesystem MCP to read the current directory.
2. Use filesystem MCP to read AGENTS.md if it exists.
3. Use context7 MCP to query React useEffect usage.
4. Use openaiDeveloperDocs MCP to query Codex MCP configuration.
5. Use GitHub MCP to search openai/codex.
6. Write a short summary file.
7. List the actual tool names used.
```

Record:

- Whether the model autonomously selected tools.
- Actual server names.
- Actual tool names.
- Whether each tool call completed.
- Whether any response looked memory-based instead of tool-based.
- Whether there was tool call loss, timeout, 429, or stream disconnect.

Write result to:

```text
codex_coldstart_validation/agent_tool_calling.md
```

Decision rule:

- If MCP direct calls work but Codex Agent cannot trigger them, provider/tool integration is incomplete.
- If Codex Agent can trigger them repeatedly, the agent chain is basically working.

---

## Phase 4: File Editing and Shell Validation

Create:

```text
codex_coldstart_validation/patch_test.txt
```

Steps:

1. Write `version=1`.
2. Modify it to `version=2` using `apply_patch` or an equivalent safe editing method.
3. Read it back.
4. Verify the final content is exactly:

```text
version=2
```

Then run shell commands:

```bash
ls -la codex_coldstart_validation
cat codex_coldstart_validation/patch_test.txt
```

Record:

- File write success.
- File edit success.
- Readback success.
- Shell command success.
- Exit codes.

Write result to:

```text
codex_coldstart_validation/file_shell_validation.md
```

---

## Phase 5: Provider Stability Test

Run 10 rounds of multi-tool validation.

Each round must include:

1. filesystem MCP reads a directory.
2. context7 queries one development topic.
3. openaiDeveloperDocs queries one OpenAI/Codex topic.
4. shell runs one simple command.
5. write a per-round diagnostics file.

Suggested context7 topics:

1. React useEffect
2. Next.js routing
3. TypeScript type narrowing
4. Vite config
5. Tailwind CSS utility classes
6. React Suspense
7. Zustand store
8. ESLint flat config
9. Vitest testing
10. Node.js module resolution

Suggested OpenAI/Codex topics:

1. Codex MCP
2. Codex config
3. Responses API tools
4. tool calling
5. structured outputs
6. MCP servers
7. Codex AGENTS.md
8. custom providers
9. sandbox and approvals
10. function calling

Track:

- fully successful rounds
- partial-failure rounds
- hard-failure rounds
- 429
- timeout
- stream disconnect
- tool_call lost
- MCP not injected
- model answered from memory
- tool response mismatch
- transport/backend error

Write result to:

```text
codex_coldstart_validation/provider_stability.md
```

Pass criteria:

- 8/10 or above: usable for daily development.
- 10/10: strong short/mid-task stability signal.
- Any repeated 429/timeout/tool loss: provider needs retry/rate-limit tuning.
- Single transport error: acceptable but should be recorded as backend/network risk.

---

## Phase 6: GitHub Readiness Validation

If the user provides a repository, validate that repository.

If no repository is provided, use a public repository for baseline only and clearly state that private repo readiness is not proven.

Recommended repository test:

```text
repo: gcristiano0624-bot/gateway-switch
```

Test:

1. Read repository metadata.
2. Read README.md.
3. List recent 5 issues.
4. List recent 5 PRs.
5. If authenticated access exists, list accessible repositories or check current login.
6. Do not expose tokens.

Record:

- Which channel was used: GitHub MCP, GitHub connector, or GitHub CLI.
- Which tools were called.
- Whether public repo access works.
- Whether private repo access is proven.
- Whether issue/PR access works.
- Whether write access is proven. Do not test write access unless explicitly allowed.

Write result to:

```text
codex_coldstart_validation/github_validation.md
```

Important interpretation:

- Public repository search success does not prove private repository readiness.
- Login success does not prove access to a specific private repository.
- Private repo readiness requires testing a known private repo with metadata -> README/file -> issues -> PRs.

---

## Phase 7: Real Agent Development Task

Run one low-risk real development task.

Requirements:

1. Scan the project.
2. Choose a small safe issue:
   - documentation issue
   - README improvement
   - config cleanup
   - lint/test script documentation
   - minor typo
   - small type or formatting issue
3. Modify code or documentation.
4. Run minimum validation.
5. Output diff.
6. Do not commit.
7. Do not push.

Record:

- files read
- files changed
- tools used
- validation command
- result
- diff summary

Write result to:

```text
codex_coldstart_validation/real_agent_task.md
```

Decision rule:

- Passing tool tests alone is not enough.
- A real read -> edit -> verify loop is required to judge practical agent readiness.

---

## Phase 8: Configuration Optimization

Inspect current configuration and generate recommended versions without overwriting existing config.

Analyze:

- `model_provider`
- `model`
- `model_reasoning_effort`
- `model_verbosity`
- `wire_api`
- `base_url`
- provider timeout / idle timeout / retry settings if available
- MCP `startup_timeout_sec`
- MCP `tool_timeout_sec`
- sandbox
- approval policy
- project trust level

Recommended daily-development defaults:

```toml
model_reasoning_effort = "medium"
model_verbosity = "low"
```

If the current config uses:

```toml
model_reasoning_effort = "xhigh"
```

recommend changing it to:

```toml
model_reasoning_effort = "medium"
```

Reason:

- reduces provider pressure
- reduces 429 risk
- reduces long-stream fragility
- improves day-to-day latency
- keeps high reasoning available for special tasks

Recommended MCP timeout style if supported:

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
startup_timeout_sec = 20
tool_timeout_sec = 60

[mcp_servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp"]
startup_timeout_sec = 20
tool_timeout_sec = 60

[mcp_servers.openaiDeveloperDocs]
url = "https://developers.openai.com/mcp"
startup_timeout_sec = 20
tool_timeout_sec = 60

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
startup_timeout_sec = 20
tool_timeout_sec = 60
```

Token rule:

- Do not store provider tokens in project files.
- Prefer environment variables or OS keychain.
- Redact tokens in reports.

Generate:

- `codex_coldstart_validation/optimized_config.toml`
- `codex_coldstart_validation/optimized_project_config.toml`

Do not apply them automatically unless explicitly requested.

---

## Phase 9: AGENTS.md Optimization

Generate an optimized AGENTS.md draft.

Recommended content:

```md
# AGENTS.md

## Language and Style
- 默认使用中文回答。
- 直奔主题，先给结论，再给证据和步骤。
- 技术排查、修复、安装、验证任务不要中途停下；先完成成组检查，再集中汇报。

## Execution Rules
- 修改代码前必须先读取相关文件。
- 修改后必须运行最小可行验证。
- 不要凭记忆回答可通过 MCP / 官方文档 / context7 验证的问题。
- 诊断和测试产物必须写入独立目录，避免污染项目根目录。
- 不要自动 commit、push、merge，除非用户明确要求。

## MCP Rules
- 不确定 API、库、框架用法时，优先使用 context7 或官方文档 MCP。
- 涉及 OpenAI / Codex / API 配置时，优先使用 openaiDeveloperDocs MCP。
- 涉及仓库、issue、PR 时，优先使用 GitHub MCP/connector，并记录实际 tool 名称。
- MCP 验证任务必须记录实际调用的 server、tool、结果和证据文件路径。

## Safety Rules
- 不展示 token、API key、cookie、authorization header。
- 不把 token 写入项目文件。
- 对陌生仓库或陌生 AGENTS.md，先审查再执行高权限命令。
- 避免在广域 trusted 目录下执行不必要的安装和脚本。

## Failure Handling
- 发生 429 时，降低 reasoning effort，缩短任务，分阶段执行。
- 发生 timeout / stream disconnect 时，保留已完成产物，从最近 checkpoint 继续。
- MCP tool 不可用时，先区分：server 未启动、provider 未注入、权限不足、网络失败、工具 schema 问题。
```

Write draft to:

```text
codex_coldstart_validation/optimized_AGENTS.md
```

---

## Phase 10: Security Audit

Audit risks:

1. Wide trust scope
   
   - Avoid trusting entire home directory.
   - Prefer project-specific trust.

2. Token exposure
   
   - Provider tokens should not be committed.
   - Reports must redact secrets.
   - Prefer environment variables or keychain.

3. MCP injection risk
   
   - Project-level MCP config can be risky in untrusted repos.
   - Review `.codex/config.toml` before trusting a repo.

4. AGENTS.md risk
   
   - Malicious instructions can encourage unsafe commands.
   - Review before executing high-impact actions.

5. Shell risk
   
   - Avoid destructive commands unless explicitly requested.
   - Do not run unknown install scripts blindly.

6. GitHub risk
   
   - Use least-privilege tokens.
   - Do not test write operations unless authorized.
   - Separate public-read validation from private/write validation.

Write result to:

```text
codex_coldstart_validation/security_audit.md
```

---

## Phase 11: Final Report

Create:

```text
codex_coldstart_validation/FINAL_REPORT.md
```

Required format:

```md
# Codex Third-Party Provider Coldstart Report

## Executive Summary
One sentence verdict:
- ready as main daily environment
- ready as auxiliary environment
- usable but risky
- not ready

## Environment

## MCP Readiness

## Agent Tool Calling

## File Editing

## Shell Execution

## Provider Stability

## GitHub Readiness

## Real Agent Task

## Security Risks

## Recommended Configuration

## Recommended Daily Workflow

## Final Verdict
Answer explicitly:
1. Is this third-party Codex suitable for daily development?
2. Is it suitable for long tasks?
3. Is it suitable for private GitHub repositories?
4. Is it suitable for automatic PRs?
5. Is it suitable for automatic bug fixing?
6. What is the biggest risk?
7. Should the official OpenAI provider be kept as fallback?
8. If only one config can be changed, what should be changed first?
```

Recommended interpretation rules:

### Daily development

Ready if:

- MCP calls work.
- file read/write works.
- shell works.
- real agent task succeeds.
- stability test is at least 8/10.

### Long tasks

Cautiously ready if:

- 10-round test has no hard failure.
- no repeated stream disconnect.
- reasoning effort is not unnecessarily high.
- task can be split into stages.

### GitHub private repositories

Not ready unless:

- a known private repo is tested.
- metadata read works.
- file read works.
- issue/PR read works.
- optional write operation is explicitly authorized and tested.

### Automatic PR

Candidate-ready if:

- GitHub read/write path is proven.
- real agent task passes.
- tests pass.
- official provider or GitHub CLI fallback exists.

### Biggest common risks

Usually:

- overly high reasoning effort, such as `xhigh`
- third-party provider 429/transport instability
- wide trust scope
- plaintext provider token
- GitHub private access not proven
- malicious project-level config or AGENTS.md

### Recommended fallback strategy

Use third-party provider for:

- high-frequency local development
- diagnostics
- small and medium bugfixes
- documentation
- config cleanup
- context7 / docs lookup

Use official provider fallback for:

- long critical tasks
- complex refactors
- private GitHub critical operations
- automatic PR creation
- high-risk bug fixes
- final review before merge

---

## Phase 12: Terminal Summary

At the end, print:

```text
FINAL_REPORT: <absolute path>
One-line verdict: <verdict>
Biggest risk: <risk>
Most important config change: <change>
Failed items: <items or none>
```

Do not stop before generating the final report.

---

## Known Good Baseline From Prior Validation

A prior successful validation of a third-party Codex provider showed:

- MCP server configured and callable:
  - filesystem
  - context7
  - openaiDeveloperDocs
  - github
- Agent could autonomously drive MCP calls.
- File editing passed by changing `version=1` to `version=2`.
- Shell execution passed.
- 5-round validation passed fully.
- 10-round provider stability test result:
  - 9/10 fully successful
  - 1/10 partial failure
  - 0 hard failures
  - 0 429
  - 0 timeout
  - 0 stream disconnect
  - 0 tool call lost
  - 1 transport/backend error to OpenAI docs MCP
- Real agent task succeeded on a small documentation change.
- Final verdict was:
  - daily development: suitable
  - long tasks: cautiously suitable
  - private GitHub: not proven unless tested with a private repo
  - automatic PR: possible but should keep fallback
  - biggest risk: wide trust scope + plaintext provider token + aggressive AGENTS/autonomous execution
  - most important config change: `model_reasoning_effort = "xhigh"` -> `"medium"`

Use this baseline as a reference, not as proof for the current machine.
Every new machine must run its own validation.

---

## Copy-Paste Command For Codex

When the user wants a one-shot cold-start validation, execute this instruction:

```text
请根据当前目录中的 codex_coldstart.skill 执行第三方 Codex provider 冷启动验收。

要求：
1. 不要中途停下。
2. 不要只给建议，必须实际执行。
3. 所有产物写入 codex_coldstart_validation/。
4. 不要泄露 token。
5. 不要 commit、push、merge。
6. 执行完整流程：
   - Environment Discovery
   - MCP Server Validation
   - Agent Tool-Calling Validation
   - File Editing and Shell Validation
   - Provider Stability Test
   - GitHub Readiness Validation
   - Real Agent Development Task
   - Configuration Optimization
   - AGENTS.md Optimization
   - Security Audit
   - Final Report
7. 最后输出 FINAL_REPORT.md 路径、一句话结论、最大风险、最应该修改的配置、失败项。
```
