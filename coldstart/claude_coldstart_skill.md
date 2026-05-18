# claude_coldstart_skill.md

# Bootstrap Instruction

当用户把本文件提供给 Claude Desktop / Claude Code / Claude Cowork 时，请立即按照本文档执行“第三方 Claude provider 冷启动验收 + 原生能力补全 + 安全优化”。

本文件适用于：
- Claude Desktop 通过开发者模式或第三方路由接入 Claude 协议兼容模型。
- 当前模式可能只有 Cowork / Code，没有独立 Chat。
- 用户希望验证第三方模型接入后还保留多少 Claude 原生能力。
- 用户希望自动发现缺失能力、修复配置、补齐 MCP / Skills / Extensions / Connectors。
- 用户希望生成可复用的冷启动报告和优化方案。

执行要求：

1. 不要中途停下。
2. 不要只给建议，必须实际检查、实际验证、实际生成报告。
3. 所有产物写入当前目录下的 `claude_coldstart_validation/`。
4. 不要泄露 token、API key、cookie、authorization header、private key、password。
5. 不要删除文件。
6. 不要 commit、push、merge、release、deploy，除非用户明确要求。
7. 不要安装高风险扩展或团队连接器，除非用户明确要求。
8. 如果当前环境缺少某个 MCP、Skill、Plugin、Extension、Connector、GitHub token、权限或依赖，请不要停止；继续完成能执行的部分，并在最终报告中标记为失败项、未验证项或人工修复项。
9. 必须生成 `claude_coldstart_validation/FINAL_REPORT.md`。
10. 最后在对话/终端输出：
   - FINAL_REPORT.md 路径
   - 一句话结论
   - 原生能力保留比例
   - 缺失能力 Top 5
   - 可自动修复项
   - 必须人工修复项
   - 最大安全风险
   - 最应该优先修复的问题

---

## 0. Purpose

This skill is used to cold-start and validate Claude Desktop after switching to a third-party routed Claude-compatible model.

Primary goal:

> Verify and optimize the full Claude Desktop Agent runtime:
> third-party router -> Claude protocol -> Cowork/Code mode -> tool_use/tool_result -> MCP -> Skills -> Extensions -> Connectors -> filesystem -> shell -> browser -> GitHub -> real task -> security audit.

This is not a model benchmark. This is an Agent capability and production-readiness validation.

---

## 1. Operating Style

默认语言：中文。

行为规则：
- 先执行，后汇报。
- 先给结论，再给证据。
- 能用工具验证的事情，不要凭记忆回答。
- 诊断、修复、迁移、配置、验收类任务不要中途停下。
- 修改文件前先读取。
- 修改后做最小验证。
- 对无法修复的问题，要明确区分：
  - 未安装
  - 未启用
  - 配置错误
  - 权限不足
  - token 缺失
  - auth 失败
  - sandbox 限制
  - mode 限制
  - third-party router 限制
  - MCP schema 错误
  - transport/backend 错误

---

## 2. Recommended Output Directory

Create:

```bash
claude_coldstart_validation/
```

Recommended files:

```text
current_environment.md
native_capability_matrix.md
protocol_compatibility.md
mcp_validation.md
mcp_deep_audit.md
skills_audit.md
extensions_connectors_audit.md
native_gap_matrix.md
file_edit_validation.md
shell_validation.md
cowork_mode_validation.md
code_mode_validation.md
chat_mode_gap_analysis.md
provider_stability.md
github_validation.md
security_audit.md
fix_plan.md
applied_fixes.md
optimized_CLAUDE.md
optimized_mcp_config.json
optimized_claude_config.md
startup_check.sh
coldstart_recommendations.md
FINAL_REPORT.md
```

---

## 3. Phase 1 — Environment Discovery

Collect and record:

### Claude Runtime

Identify:
- Current mode:
  - Chat
  - Cowork
  - Code
  - Other
- Current model name.
- Whether model is official Claude or third-party routed.
- Provider / route / base_url / session path if observable.
- Whether standalone Chat mode exists.
- Whether Cowork mode exists.
- Whether Code mode exists.
- Whether the model supports:
  - tool_use
  - tool_result
  - streaming
  - thinking / reasoning
  - image input
  - artifacts / preview
  - browser automation
  - MCP
  - Skills
  - Plugins
  - Desktop Extensions
  - Connectors / Apps

### Host and Sandbox

Record:
- Host OS.
- Sandbox OS if present.
- Current working directory.
- User home directory.
- Writable directories.
- Whether shell is available.
- Whether filesystem read/write/edit tools are available.
- Whether network is available.
- Whether browser automation is available.

### Config Locations

Inspect non-sensitive config and directory structure.

macOS:
```text
~/Library/Application Support/Claude/
~/Library/Logs/Claude/
~/.claude/
~/.config/claude/
~/.config/Claude/
```

Windows:
```text
%APPDATA%/Claude/
%LOCALAPPDATA%/Claude/
~/.claude/
```

Linux:
```text
~/.config/Claude/
~/.claude/
```

Workspace:
```text
CLAUDE.md
CLAUDE.local.md
AGENTS.md
.mcp.json
claude_desktop_config.json
.claude/
.claude/settings.json
.claude/mcp.json
.claude/extensions/
.claude/skills/
.claude/plugins/
```

Rules:
- Redact secrets.
- Do not write full tokens into reports.
- Distinguish user-level config vs project-level config.
- Identify duplicated or conflicting Claude config directories.

Write:

```text
claude_coldstart_validation/current_environment.md
```

---

## 4. Phase 2 — Native Capability Matrix

Assess at least these 40 capabilities.

For each capability, mark one of:
- Fully Active
- Installed but Inactive
- Configured but Broken
- Missing
- Blocked by Third-party Router
- Blocked by Mode
- Blocked by Auth
- Blocked by Sandbox
- Not Observable

Capabilities:

1. Chat mode
2. Cowork mode
3. Code mode
4. Native file read
5. Native file write
6. Native file edit
7. Shell execution
8. Browser automation
9. Web search
10. Artifacts / preview
11. Image input / vision
12. PDF reading
13. Office document reading
14. MCP discovery
15. MCP tool invocation
16. MCP resources
17. MCP prompts
18. Skills discovery
19. Skills triggering
20. Plugins discovery
21. Plugins triggering
22. Desktop Extensions
23. Connectors / Apps
24. GitHub repo read
25. GitHub issue / PR read
26. GitHub write / PR create
27. Memory
28. Scheduled tasks
29. Long context
30. Multi-turn task list
31. Structured output
32. Parallel tool_use
33. Sequential tool_use
34. tool_result backfill
35. Streaming tool_use
36. Error recovery
37. Permission prompt / approval flow
38. Sandbox isolation
39. Token redaction
40. Project instruction handling

Write:

```text
claude_coldstart_validation/native_capability_matrix.md
claude_coldstart_validation/native_gap_matrix.md
```

---

## 5. Phase 3 — Claude Protocol Compatibility

Behaviorally test third-party router compatibility.

If raw request/response logs are available, inspect them with secrets redacted. If raw logs are not available, use behavioral evidence.

### Request-side compatibility

Check support for:
- messages
- system
- max_tokens
- temperature, if observable
- stream
- tools
- tool_choice, if observable
- thinking / reasoning fields, if observable
- metadata / extra headers, if observable

### Response-side compatibility

Check support for:
- content block
- text block
- tool_use block
- tool_result block
- stop_reason
- usage, if observable
- streaming event
- error event

### Special checks

Verify:
- tool_use id uniqueness
- tool_result backfill
- parallel tool_use
- sequential tool_use
- streaming + tool_use
- stop_reason correctness
- error recognition
- long output truncation
- route-layer field loss
- Claude-to-OpenAI translation loss
- context truncation
- output mismatch after tool result

Write:

```text
claude_coldstart_validation/protocol_compatibility.md
```

Decision rule:
- If tool_use/tool_result/streaming all work, behavior-level protocol compatibility is good.
- If raw logs are unavailable, do not claim wire-level proof. Say behavior-level validation passed, wire-level fidelity not proven.

---

## 6. Phase 4 — MCP Validation

Validate that MCP is not only configured, but actually callable.

For each MCP server, record:
- server name
- transport type if observable
- command / url if observable
- status
- tools count
- resources count if observable
- prompts count if observable
- auth status
- whether tool/list works
- whether at least one low-risk tool call works
- schema errors
- auth errors
- timeout errors
- transport errors
- whether injected into model context
- whether third-party router blocks it

Minimum categories:

### workspace / filesystem

Test:
- list directory
- read file
- write test file
- edit test file
- read back verify

### browser / Chrome

If available:
- inspect current page or open a harmless page
- read title or take screenshot if safe
- do not access sensitive sites

### skills

Test:
- list skills
- read 1-2 SKILL.md descriptions
- determine trigger conditions

### plugins

Test:
- list plugins
- search plugins if supported
- record if 0 installed

### scheduled tasks / memory

Test:
- list tasks
- inspect memory availability
- do not create real scheduled tasks unless user requests

### GitHub

If available:
- search public repo
- read provided repo metadata
- read README
- list issues/PRs if auth exists
- do not test write unless user explicitly requests

### docs / web

If available:
- WebSearch or web_fetch a low-risk technical topic
- record actual tool names

Write:

```text
claude_coldstart_validation/mcp_validation.md
claude_coldstart_validation/mcp_deep_audit.md
```

---

## 7. Phase 5 — Skills Audit

List all Skills.

For each Skill, record:
- skill name
- source
- path
- description
- trigger conditions
- whether SKILL.md exists
- whether readable
- whether dependencies exist
- whether tool dependencies exist
- whether auth is needed
- whether compatible with third-party model
- whether compatible with Cowork mode
- status:
  - Active
  - Blocked
  - Needs Auth
  - Missing Files
  - Missing Dependencies
  - Not Tested

Recommended dependency checks:
- browser-use CLI / Python package
- gh CLI
- Playwright
- Node.js
- Python
- package managers
- SerpAPI / GSC MCP
- NotebookLM cookie/auth
- GitHub token

Run 3 low-risk trigger tests if possible.

Write:

```text
claude_coldstart_validation/skills_audit.md
```

---

## 8. Phase 6 — Plugins / Extensions / Connectors Audit

Audit:

### Plugins

- list installed plugins
- list enabled plugins
- list failed plugins
- list plugin tools
- search available plugins if supported

### Desktop Extensions

- check whether Desktop Extensions / DXT are configured
- list installed extensions
- list enabled extensions
- list failed extensions
- identify whether extension directory exists
- identify whether enterprise policy blocks extension installation

### Connectors / Apps

Check whether these are configured:
- GitHub
- Google Drive
- Slack / Lark
- Notion
- Linear / Jira
- Database / SQLite / Postgres
- Memory / Notes
- Browser / Chrome
- Web search
- PDF / document reader
- Terminal / shell
- Package manager tools

Do not install team connectors by default.

Write:

```text
claude_coldstart_validation/extensions_connectors_audit.md
```

Interpretation:
- Missing extensions/connectors are usually config gaps, not router gaps.
- Desktop Extensions are local MCP-server bundles; they can be powerful and should be installed selectively.
- Connectors and team tools increase data exposure and permission risk.

---

## 9. Phase 7 — File Editing Validation

Create:

```text
claude_coldstart_validation/file_edit_test.txt
```

Steps:
1. Write:
   ```text
   version=1
   ```
2. Read and confirm.
3. Edit to:
   ```text
   version=2
   ```
4. Read and confirm.
5. If shell is available, verify with cat/type.

Record:
- native file tool vs MCP vs shell
- success/failure
- authorization prompts
- sandbox limitations

Write:

```text
claude_coldstart_validation/file_edit_validation.md
```

---

## 10. Phase 8 — Shell / Code Validation

If shell is available, run low-risk commands.

macOS/Linux:
```bash
pwd
ls -la
echo "claude thirdparty shell test"
python3 --version || true
node -v || true
git --version || true
```

Windows:
```bat
cd
dir
echo claude thirdparty shell test
python --version
node -v
git --version
```

Record:
- stdout captured
- stderr captured
- exit code visible
- shell output usable in next reasoning turn
- sandbox boundaries
- approval behavior

Write:

```text
claude_coldstart_validation/shell_validation.md
```

---

## 11. Phase 9 — Cowork Mode Validation

Validate Cowork mode with a small collaboration task:

Task:
- inspect workspace directory
- identify project type
- generate one safe optimization suggestion
- create a small report file
- do not modify core code

Assess:
- project context understanding
- multi-step planning
- task list tracking
- tool coordination
- deliverable generation
- mode degradation
- unexpected tool overuse

Write:

```text
claude_coldstart_validation/cowork_mode_validation.md
```

---

## 12. Phase 10 — Code Mode Validation

If standalone Code mode exists, test it. If code work happens inside Cowork, mark as “Code via Cowork”.

Low-risk task:
1. Scan project.
2. Find a small safe issue:
   - README wording
   - config documentation
   - lint script documentation
   - minor typo
   - diagnostic doc
3. Modify one low-risk file.
4. Run minimal validation.
5. Output diff.
6. Do not commit.
7. Do not push.

Write:

```text
claude_coldstart_validation/code_mode_validation.md
```

---

## 13. Phase 11 — Chat Mode Gap Analysis

If Chat mode is missing, analyze impact.

Answer:
1. Is chat mode absent?
2. Is absence caused by third-party router, mode, app config, or not observable?
3. Can Cowork replace Chat for light Q&A?
4. What are the costs?
   - larger system prompt
   - higher token overhead
   - slower responses
   - tool over-triggering risk
   - task boundary ambiguity
5. Which tasks should use Cowork?
6. Which tasks should use official Claude or a lightweight chat entry?
7. Is Chat mode absence a blocker?

Write:

```text
claude_coldstart_validation/chat_mode_gap_analysis.md
```

---

## 14. Phase 12 — Provider Stability Test

Run 10 rounds.

Each round should include:
1. directory or file read
2. a low-risk reasoning task
3. MCP tool call if available
4. shell command if available
5. write per-round diagnostics file

Track:
- total rounds
- successful rounds
- partial failures
- hard failures
- 429
- timeout
- stream disconnect
- tool_use lost
- tool_result lost
- MCP injection failures
- mode degradation
- model memory-only response
- route-layer errors
- UI freeze
- output truncation
- transport/backend error

Write:

```text
claude_coldstart_validation/provider_stability.md
```

Interpretation:
- 10/10 means strong short/mid-task signal.
- 8/10 means usable but monitor.
- Repeated stream/tool/backend failures mean router/provider needs tuning.
- A 5-minute test does not prove 30-60 minute long-task reliability.

---

## 15. Phase 13 — GitHub Readiness

Validate GitHub in layers.

### Layer 1: git CLI

Check:
```bash
git --version
git status
git remote -v
```

### Layer 2: gh CLI

Check:
```bash
which gh
gh --version
gh auth status
```

If gh missing, do not install automatically. Generate:
```bash
brew install gh
gh auth login
```

### Layer 3: GitHub MCP / Connector

If available, test:
- repo metadata
- README
- issue list
- PR list

Recommended repo if user has not specified:
```text
gcristiano0624-bot/gateway-switch
```

Do not perform write operations unless explicitly requested.

Record:
- public repo access
- private repo access proven or not
- issue/PR access
- PR create/write not tested unless authorized
- auth blocker

Write:

```text
claude_coldstart_validation/github_validation.md
```

Decision rule:
- Public repo access does not prove private repo readiness.
- gh login does not prove target org/repo permission.
- Private readiness requires a known private repo test.

---

## 16. Phase 14 — Security Audit

Audit:

### Third-party routing risk
- all conversation content may transit third-party provider
- file contents and tool results may transit third-party provider
- logs may be retained
- provider may have different privacy/security controls

### Token risk
- tokens in config
- tokens in logs
- tokens in reports
- tokens accessible to shell/MCP
- tokens in project files

### MCP risk
- filesystem scope too broad
- browser automation risky
- GitHub token scope too broad
- unknown MCP server
- project-level malicious MCP config
- local MCP/extensions running with high privilege

### Extension/Connector risk
- team connectors can expose sensitive data
- DXT/Desktop Extensions package local MCP servers
- unknown extensions should not be installed blindly

### Mode risk
- Cowork/Code may over-trigger tools compared with simple Chat
- automatic task execution risk

### Shell risk
- destructive commands
- install scripts
- sudo
- deployment commands

### Project instruction risk
- prompt injection in CLAUDE.md / AGENTS.md / README
- malicious install instructions
- hidden project config

Write:

```text
claude_coldstart_validation/security_audit.md
```

---

## 17. Phase 15 — Auto Fixes and Fix Plan

Allowed auto-fixes:
1. create output directories
2. generate optimized config drafts
3. generate CLAUDE.md draft
4. generate MCP config examples
5. generate environment variable examples with placeholders only
6. generate startup check script
7. generate dependency install checklist
8. safe path reference fixes with backup

Not allowed without explicit user approval:
1. write real tokens
2. install unknown extensions
3. install high-risk team connectors
4. change system security settings
5. delete files
6. commit/push/merge
7. authorize GitHub/Google/Slack/Lark/Notion accounts
8. perform GitHub write operations

Generate:

```text
claude_coldstart_validation/fix_plan.md
claude_coldstart_validation/applied_fixes.md
claude_coldstart_validation/optimized_CLAUDE.md
claude_coldstart_validation/optimized_mcp_config.json
claude_coldstart_validation/optimized_claude_config.md
claude_coldstart_validation/startup_check.sh
claude_coldstart_validation/.env.example
```

---

## 18. Phase 16 — Global CLAUDE.md Recommendation

Generate a global `CLAUDE.md` draft for:

```text
~/.claude/CLAUDE.md
```

Purpose:
- user-wide preferences
- execution style
- safety rules
- tool usage rules
- third-party provider warnings

Do not include project-specific:
- build commands
- test commands
- business context
- repo-specific rules
- secrets

Recommended global content:
- default Chinese
- direct answer style
- do not stop midway during diagnostics
- read before edit
- verify after edit
- no token leakage
- no automatic commit/push/merge
- use tools when facts can be verified
- third-party route privacy warning
- sensitive work should use official Claude fallback

Write:

```text
claude_coldstart_validation/optimized_global_CLAUDE.md
```

---

## 19. Phase 17 — Coldstart Recommendations for New Machines

Generate a new-computer checklist.

Include:
1. install Claude Desktop
2. configure third-party route
3. verify Cowork/Code modes
4. verify Chat mode availability or absence
5. configure filesystem/workspace
6. configure browser/Chrome automation
7. configure GitHub:
   - install gh
   - gh auth login
   - optional GitHub MCP
8. configure docs/search tools
9. configure memory/tasks if needed
10. create global CLAUDE.md
11. create project CLAUDE.md
12. run this coldstart validation
13. keep official Claude fallback

Write:

```text
claude_coldstart_validation/coldstart_recommendations.md
```

---

## 20. Final Report

Create:

```text
claude_coldstart_validation/FINAL_REPORT.md
```

Required format:

```md
# Claude Third-Party Provider Coldstart Report

## Executive Summary
一句话结论：
- 可作为主力
- 可作为辅助
- 仅适合实验
- 不建议使用

## Environment

## Native Capability Retention

## Current Active Capabilities

## Missing / Inactive Capabilities

## Protocol Compatibility

## MCP Status

## Skills Status

## Plugins / Extensions / Connectors Status

## File Editing Readiness

## Shell / Code Readiness

## Cowork Mode Readiness

## Code Mode Readiness

## Chat Mode Gap

## Provider Stability

## GitHub Readiness

## Router Impact

## Auto Fixes Applied

## Manual Fixes Required

## Security Risks

## Recommended Configuration

## Recommended Startup Checklist

## Recommended Daily Workflow

## Final Verdict
```

Final Verdict must answer:
1. 当前第三方 Claude Desktop 是否适合日常使用？
2. 是否适合 code 模式开发？
3. 是否适合 cowork 模式协作？
4. chat 模式缺失是否严重？
5. MCP 是否实际可用？
6. Skills 是否实际可用？
7. Plugins / Extensions / Connectors 是否缺失？
8. 文件读写是否实际可用？
9. shell 是否实际可用？
10. GitHub / 外部工具是否实际可用？
11. 当前最大风险是什么？
12. 是否建议保留官方 Claude 作为兜底？
13. 如果只能修一个问题，最应该修什么？
14. 这个环境和原生 Claude Desktop 相比，能力损失在哪里？
15. 原生能力保留比例是多少？
16. 修复后预计能达到多少？

---

## 21. Final Terminal / Chat Output

At the end, output:

```text
FINAL_REPORT: <absolute path>
One-line verdict: <verdict>
Native capability retention: <percentage>
Top missing capabilities: <list>
Auto fixes applied: <list>
Manual fixes required: <list>
Biggest security risk: <risk>
Most important fix: <fix>
Recommended fallback: <yes/no and why>
```

Do not stop before generating the final report.

---

## 22. Known Good Baseline From Prior Validation

A prior validation of a third-party Claude Desktop provider showed:

- Model: `claude-opus-4-7[1m]` via third-party routing.
- Mode: Cowork only; Chat mode unavailable.
- Initial readiness report:
  - ~85% native capability retained.
  - file read/write/edit worked.
  - shell worked.
  - MCP worked.
  - browser automation worked.
  - artifacts/preview worked.
  - tool_use/tool_result protocol worked.
  - streaming stable.
  - 10/10 stability rounds passed.
  - 0 429.
  - 0 timeout.
  - 0 stream disconnect.
  - 0 tool_use lost.
  - 0 tool_result lost.
- Deep recovery audit:
  - 40 capabilities assessed.
  - 30 fully active.
  - 3 installed but inactive.
  - 4 missing.
  - 1 blocked by mode.
  - 2 blocked by auth.
  - fully active: 75%.
  - fixable to 90%+.
  - router-caused hard gap: Chat mode only.
  - main missing capabilities:
    - GitHub MCP / gh CLI
    - Desktop Extensions
    - Connectors / Apps
    - Plugins
    - CLAUDE.md
  - blocked skills:
    - browser-use missing CLI
    - github-upload missing gh CLI
    - seo-audit missing SerpAPI/GSC MCP
    - notebooklm-research needs cookie auth
  - biggest security risk:
    - all conversation, code, and file content may pass through third-party provider.
  - most important fix:
    - install `gh` CLI and configure GitHub MCP / GitHub auth.

Use this as reference only. Every new machine must run its own validation.

---

## 23. Suggested Minimal Manual Fixes After First Run

If the report finds the same common gaps, the user can prioritize:

### Priority 1 — GitHub CLI

```bash
brew install gh
gh auth login
```

Then verify:

```bash
gh repo view gcristiano0624-bot/gateway-switch
gh issue list -R gcristiano0624-bot/gateway-switch --limit 5
gh pr list -R gcristiano0624-bot/gateway-switch --limit 5
```

### Priority 2 — GitHub MCP

Generate a config example using environment variables.

Never write real token into config.

### Priority 3 — Global CLAUDE.md

Place global preferences at:

```text
~/.claude/CLAUDE.md
```

### Priority 4 — Project CLAUDE.md

Place project-specific instructions at:

```text
<project-root>/CLAUDE.md
```

### Priority 5 — browser-use optional

```bash
python3 -m pip install browser-use
```

Only if browser-use skill is actually needed.

### Priority 6 — Avoid unnecessary connectors

Do not install Slack / Lark / Notion / Drive / Jira connectors unless the task requires them.

---

## 24. Copy-Paste Command For Claude

When the user wants one-shot cold-start validation, execute this instruction:

```text
请根据当前目录中的 claude_coldstart_skill.md 执行 Claude Desktop 第三方 provider 冷启动验收、原生能力补全和安全优化。

要求：
1. 不要中途停下。
2. 不要只给建议，必须实际执行。
3. 所有产物写入 claude_coldstart_validation/。
4. 不要泄露 token。
5. 不要删除文件。
6. 不要 commit、push、merge。
7. 执行完整流程：
   - Environment Discovery
   - Native Capability Matrix
   - Protocol Compatibility
   - MCP Validation
   - MCP Deep Audit
   - Skills Audit
   - Plugins / Extensions / Connectors Audit
   - File Editing Validation
   - Shell / Code Validation
   - Cowork Mode Validation
   - Code Mode Validation
   - Chat Mode Gap Analysis
   - Provider Stability Test
   - GitHub Readiness
   - Security Audit
   - Auto Fixes and Fix Plan
   - Global CLAUDE.md Recommendation
   - Coldstart Recommendations
   - Final Report
8. 最后输出 FINAL_REPORT.md 路径、一句话结论、原生能力保留比例、缺失能力 Top 5、最大安全风险、最应该优先修复的问题。
```
