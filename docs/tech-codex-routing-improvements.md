# Tech Doc: Codex Routing Improvements (v1.16.0 → v1.19.0)

Date: 2026-06-30
Companion to: `docs/prd-codex-routing-improvements.md`
Reference: [cc-switch](https://github.com/farion1231/cc-switch) at `/tmp/cc-switch`
Our codebase: `/Users/hugoguan/Documents/01. AI_Projects/03. Trae_Projects/gateway-switch`

## Current State (v1.15.0) — Where to Start

All changes land in `src-tauri/src/codex_gateway.rs` (the 85k file that handles all Codex gateway I/O). The 4 functions most touched by the 5 improvements:

| Function | Lines | Role |
|---|---|---|
| `convert_request` | ~844–1000 | Codex Responses → Chat Completions body conversion |
| `convert_sync_response` | ~1186–1230 | Chat Completion → Codex Responses body conversion |
| Streaming `sse` block | ~419+ | Chat Completions SSE → Codex Responses SSE |
| `responses_handler` | ~241–371 | Top-level: parse request, apply policies, call upstream, stream back |
| `extract_text_from_delta` | ~1312–1338 | Stream delta → text extraction |
| `extract_chat_message_text` | ~1340+ | Non-stream message → text extraction |

Current `apply_codex_provider_policy` (lines 1037–1051) does **only** strip and disable. Current `apply_codex_tool_call_mode` (lines 1002–1035) does **only** force `tool_choice = "required"`. Neither has platform-aware translation.

The v1.15.0 change (this release) was to drop `reasoning_content` fallback in `extract_text_from_delta` and `extract_chat_message_text` (lines 1313, 1341) — this is the starting point for Improvement 3 (now we need to redirect that content into a proper `reasoning` output item instead of dropping it).

The 5 improvements below are derived from reading `cc-switch`'s implementation. Each section has: **what / where / how (with code references) / tests / risks**.

---

## Improvement 5: tool_choice cleanup + stream_options.include_usage injection

**Why**: When Codex App sends a request with `tools: []` (or `tools` is filtered out by our `apply_codex_provider_policy.disable_tools`) but `tool_choice` is still set, vLLM and many enterprise gateways return `400 tool_choice requires tools` or `503 invalid request`. We currently let the stale `tool_choice` flow through. Also, without `stream_options.include_usage = true`, OpenAI-compatible streaming responses return zero token usage — our `request_logs` end up with `usage.prompt_tokens = 0`, breaking cost accounting.

### Where to change

File: `src-tauri/src/codex_gateway.rs`, function `convert_request` (around lines 990–999, the end of the conversion before return).

### How

Append two blocks at the end of `convert_request`, after the existing `apply_*` calls:

```rust
// Improvement 5a: strip dangling tool_choice / parallel_tool_calls when
// the request no longer has any tools. vLLM and many enterprise gateways
// return 400 / 503 if tool_choice is set without tools. Source:
// cc-switch transform_codex_chat.rs:322-334.
let has_tools_after_policy = chat_req
    .get("tools")
    .and_then(|v| v.as_array())
    .is_some_and(|a| !a.is_empty());
if !has_tools_after_policy {
    if let Some(obj) = chat_req.as_object_mut() {
        obj.remove("tool_choice");
        obj.remove("parallel_tool_calls");
    }
}

// Improvement 5b: inject stream_options.include_usage for streaming
// requests so OpenAI-compatible upstreams return token usage in the
// final chunk. Source: cc-switch transform_codex_chat.rs:335-340.
if is_stream {
    let stream_options = chat_req
        .entry("stream_options")
        .or_insert_with(|| json!({}));
    if let Some(obj) = stream_options.as_object_mut() {
        obj.insert("include_usage".into(), json!(true));
    }
}
```

The `is_stream` variable is already in scope inside `convert_request` (or you can pass it in — check current signature).

### Tests

Add to `mod tests` in `codex_gateway.rs`:

```rust
#[test]
fn test_convert_request_strips_tool_choice_when_no_tools() {
    // Build a Codex Responses request with tool_choice but empty tools.
    // Call convert_request.
    // Assert: forwarded body has no "tool_choice" or "parallel_tool_calls" keys.
}

#[test]
fn test_convert_request_preserves_tool_choice_when_tools_present() {
    // Build with tools: [one function] and tool_choice: "auto".
    // Assert: forwarded body still has tool_choice: "auto".
}

#[test]
fn test_convert_request_injects_include_usage_when_streaming() {
    // Build with stream: true. Assert: stream_options.include_usage == true.
}

#[test]
fn test_convert_request_skips_include_usage_when_not_streaming() {
    // Build with stream: false. Assert: no stream_options key (or include_usage is absent).
}
```

### Risks

**Very low** — pure addition at the end of `convert_request`. No existing behavior is changed. The only edge case is if a future caller relied on stale `tool_choice` surviving — there is no such caller today.

### Expected release

v1.16.0, immediately after v1.15.0 ships.

---

## Improvement 3: reasoning content split into Codex `reasoning` output item

**Why**: v1.15.0 dropped `reasoning_content` to prevent chain-of-thought leak. But Codex App expects reasoning models' CoT to land in a separate `reasoning` panel — it reads the `type: "reasoning"` output item from the Responses API. By dropping the content entirely, we lose the reasoning display feature. The fix: emit a proper `reasoning` output item while keeping the assistant `message` free of the CoT.

### Where to change

File: `src-tauri/src/codex_gateway.rs`, functions:
- `convert_sync_response` (lines ~1186–1230) — non-streaming path
- Streaming `sse` block (lines ~419+) — streaming path

### How

#### 3a. New helper for `chat_completion.reasoning_content` → Responses `reasoning` item

Add a new function in `codex_gateway.rs`:

```rust
fn chat_reasoning_to_response_output_item(
    message: &Value,
    msg_id: &str,
) -> Option<Value> {
    // Try 1: explicit reasoning_content field.
    let raw = message
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(String::from);
    // Try 2: reasoning field.
    let raw = raw.or_else(|| {
        message.get("reasoning")
            .and_then(|v| v.as_str())
            .map(String::from)
    });
    let text = raw?
        .trim()
        .to_string();
    if text.is_empty() {
        return None;
    }
    Some(json!({
        "type": "reasoning",
        "id": msg_id,
        "summary": [{
            "type": "summary_text",
            "text": text,
        }],
        "status": "completed",
    }))
}
```

#### 3b. Optional: split `<think>...</think>` blocks from the answer

Some reasoning models (DeepSeek R1 distill, etc.) emit a single `content` field that starts with `<think>...</think>\n`. Add a helper:

```rust
fn split_leading_think_block(content: &str) -> (Option<String>, String) {
    let trimmed_start = content.trim_start();
    if let Some(rest) = trimmed_start.strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            let think = rest[..end].trim().to_string();
            let after = rest[end + "</think>".len()..].trim_start().to_string();
            return (Some(think), after);
        }
    }
    (None, content.to_string())
}
```

#### 3c. Update `convert_sync_response`

In `convert_sync_response` (lines 1186–1230), after the existing `let content = extract_chat_message_text(message)` call:

```rust
let content = extract_chat_message_text(message).unwrap_or_default();

// Improvement 3: also try <think>...</think> split from the content.
let (think_text, content) = match split_leading_think_block(&content) {
    (Some(think), after) => (Some(think), after),
    (None, c) => (None, c),
};
// If no <think> block, also try the explicit reasoning_content field.
let think_text = think_text.or_else(|| chat_reasoning_to_response_output_item(message, msg_id)
    .and_then(|item| item["summary"][0]["text"].as_str().map(String::from)));

let mut output = Vec::new();
if let Some(think) = think_text {
    output.push(json!({
        "type": "reasoning",
        "id": format!("{msg_id}_r"),
        "summary": [{"type": "summary_text", "text": think}],
        "status": "completed",
    }));
}
// (existing message item generation follows, using `content` not `original_content`)
```

#### 3d. Update streaming `sse` block

In the streaming SSE handler, when a delta contains `reasoning_content`:

```rust
if let Some(think) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
    if !think.is_empty() {
        // Emit Codex Responses reasoning_summary_text.delta event.
        let evt = json!({
            "type": "response.reasoning_summary_text.delta",
            "item_id": reasoning_item_id,
            "summary_index": 0,
            "delta": think,
            "sequence_number": seq,
        });
        seq += 1;
        yield Ok(Bytes::from(format!(
            "event: response.reasoning_summary_text.delta\ndata: {}\n\n",
            serde_json::to_string(&evt).unwrap()
        )));
    }
}
```

And on stream completion, emit a `.done` event for the reasoning item before the final `response.completed`.

#### Tests

```rust
#[test]
fn test_convert_sync_response_emits_reasoning_item_from_explicit_field() {
    // Build a chat completion message with reasoning_content: "thinking" and content: "answer".
    // Call convert_sync_response.
    // Assert: output[0].type == "reasoning", output[1].type == "message",
    //         output[1].content[0].text == "answer".
}

#[test]
fn test_convert_sync_response_splits_think_block() {
    // Build a chat completion message with content: "<think>thought</think>\nfinal answer".
    // Assert: output[0].type == "reasoning", output[1].content[0].text == "final answer".
}

#[test]
fn test_convert_sync_response_no_reasoning_item_when_absent() {
    // Build with content: "just an answer" and no reasoning_content.
    // Assert: only one output item, type == "message". (Regression check vs v1.15.0.)
}
```

For streaming, write an integration test that pipes a known SSE sequence into the stream handler and asserts the emitted events include `response.reasoning_summary_text.delta` then `.done` then the message events.

### Risks

**Low**. We are re-adding content that v1.15.0 dropped. The only risk: if Codex App's parser is strict about `reasoning` item schema (it expects `summary` as an array of `{type, text}`), the format above matches the OpenAI Responses API spec.

### Expected release

v1.16.0, same release as Improvement 5.

---

## Improvement 4: Platform-aware reasoning translation

**Why**: Codex App sends `reasoning: { effort: "high" }` (or `reasoning_effort: "high"`). Each Chinese platform uses a different native field:

| Platform | Native reasoning field | Notes |
|---|---|---|
| DeepSeek (official) | `thinking: { type: "enabled" }` | `effort` ignored, on/off only |
| StepFun | `reasoning_split: true` | No effort, just split |
| Kimi (Moonshot) | `enable_thinking: true` | Boolean, no effort |
| GLM / Qwen | `thinking: { type: "enabled" }` | Same as DeepSeek |
| mimo (Xiaomi) | (none — handled internally) | Don't inject |
| OpenRouter | `reasoning: { effort: "high" }` | `max` → `xhigh` clamp |
| SiliconFlow | `enable_thinking: true` | Boolean |

Without translation, DeepSeek returns `400 Invalid option: reasoning_effort`. OpenRouter returns `400 reasoning_effort: Invalid option` for `max`. This is a class of production 400 errors that Improvement 4 fixes.

### Where to change

File: `src-tauri/src/codex_gateway.rs`, in `apply_codex_provider_policy` (lines 1037–1051) or a new sibling function called right after.

### How

#### 4a. Define `CodexChatReasoningConfig` in `gateway_strategy.rs` (or a new file)

```rust
#[derive(Debug, Clone)]
pub struct CodexChatReasoningConfig {
    pub thinking_param: ThinkingParam,    // Thinking | EnableThinking | ReasoningSplit | None
    pub effort_param: EffortParam,        // ReasoningEffort | ReasoningDotEffort | None
    pub max_effort_override: Option<String>,  // e.g. "max" → "xhigh" for OpenRouter
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThinkingParam {
    Thinking,            // DeepSeek/GLM/Qwen: {"type": "enabled"}
    EnableThinking,      // Kimi/SiliconFlow: true
    ReasoningSplit,      // StepFun: true
    ReasoningDotEffort,  // OpenRouter: {"effort": "..."}
    None,                // mimo: don't inject
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffortParam {
    None,                // platforms that don't take effort
    Passthrough,         // OpenRouter: use "high"/"low"/etc. directly
    ForceHigh,           // DeepSeek etc: enabled iff effort ∈ {low,medium,high,max}
}
```

#### 4b. Inference function

Add `infer_codex_chat_reasoning_config(provider: &Provider) -> CodexChatReasoningConfig` in `gateway_strategy.rs`. Source pattern: `cc-switch/src-tauri/src/proxy/providers/codex.rs:174-334` (`infer_codex_chat_reasoning_config` + `infer_aggregator_platform_config`).

Decision tree:
1. If `provider.base_url` or `provider.name` contains `openrouter` → OpenRouter config
2. Else if `provider.name` or `upstream_model` contains `siliconflow` → SiliconFlow
3. Else if `provider.name` contains `kimi` or `moonshot` → Kimi
4. Else if `upstream_model` contains `deepseek` → DeepSeek
5. Else if `upstream_model` contains `glm` → GLM
6. Else if `upstream_model` contains `qwen` or `dashscope` → Qwen
7. Else if `provider.id` or `upstream_model` contains `mimo` or `xiaomi` → None
8. Else → safe default (none, no injection)

#### 4c. Apply function

Add `apply_codex_reasoning_translation(chat_req: &mut Value, route: &Route)` in `codex_gateway.rs`. Call it after `apply_codex_provider_policy` in both `responses_handler` and the streaming path.

```rust
fn apply_codex_reasoning_translation(chat_req: &mut Value, route: &Route) {
    let cfg = gateway_strategy::infer_codex_chat_reasoning_config(&route.provider);
    if cfg.thinking_param == ThinkingParam::None { return; }

    let original_effort = chat_req
        .get("reasoning_effort")
        .and_then(|v| v.as_str())
        .or_else(|| {
            chat_req.get("reasoning")
                .and_then(|r| r.get("effort"))
                .and_then(|v| v.as_str())
        })
        .map(String::from);
    // Strip both Codex-native fields before re-injecting platform-native.
    if let Some(obj) = chat_req.as_object_mut() {
        obj.remove("reasoning");
        obj.remove("reasoning_effort");
    }

    let is_enabled = original_effort.as_deref().map_or(false, |e| {
        !matches!(e, "none" | "off" | "disabled" | "minimal")
    });
    let effort = original_effort.as_deref()
        .and_then(|e| cfg.max_effort_override.as_ref().map(|m| if e == m { /* clamp */ } else { e.to_string() }))
        .or(original_effort.clone());

    match (cfg.thinking_param, is_enabled) {
        (ThinkingParam::Thinking, true) => {
            chat_req["thinking"] = json!({ "type": "enabled" });
        }
        (ThinkingParam::EnableThinking, true) => {
            chat_req["enable_thinking"] = json!(true);
        }
        (ThinkingParam::ReasoningSplit, true) => {
            chat_req["reasoning_split"] = json!(true);
        }
        (ThinkingParam::ReasoningDotEffort, true) => {
            let mut reasoning = serde_json::Map::new();
            if let Some(eff) = effort {
                reasoning.insert("effort".into(), json!(eff));
            } else {
                reasoning.insert("effort".into(), json!("medium"));
            }
            chat_req["reasoning"] = json!(reasoning);
        }
        _ => {} // disabled or no param
    }
}
```

#### 4d. Update `codex_route` schema (optional but recommended)

Add a new column `codex_route.codex_reasoning_override TEXT` (nullable). If set, use it instead of the inference. Persist via existing `CreateCodexRoute` / `UpdateCodexRoute` types.

### Tests

```rust
#[test]
fn test_deepseek_translation() {
    // Build a chat req with reasoning_effort: "high".
    // Apply translation for a DeepSeek provider.
    // Assert: thinking: { type: "enabled" }, no reasoning_effort, no reasoning.
}

#[test]
fn test_openrouter_max_clamps_to_xhigh() {
    // Build with reasoning_effort: "max".
    // Apply translation for OpenRouter.
    // Assert: reasoning: { effort: "xhigh" }.
}

#[test]
fn test_openrouter_none_passes_through() {
    // Build with reasoning_effort: "none".
    // Assert: reasoning: { effort: "none" } (only this field set).
}

#[test]
fn test_mimo_no_injection() {
    // Build with reasoning_effort: "high".
    // Apply for a mimo provider.
    // Assert: no thinking / enable_thinking / reasoning field added.
}

#[test]
fn test_kimi_enable_thinking_boolean() {
    // Assert enable_thinking: true (not a struct).
}
```

### Risks

**Medium**. Translation logic adds a new mapping table. If the inference picks the wrong platform, the user sees 400s. Mitigation: start with the most common platforms (DeepSeek, Kimi, mimo, OpenRouter), leave the rest on the safe default (`None` = no injection), and add platforms incrementally.

### Expected release

v1.17.0.

---

## Improvement 2: Cross-request function_call history

**Why**: Codex CLI in multi-round sessions sends only `previous_response_id` + `function_call_output`. Chat Completions providers require the full `tool_calls` field on the assistant message. The current `convert_request` has no way to reconstruct the missing `tool_calls`. Result: DeepSeek, Kimi, and others return 400 in turn 2+.

### Where to change

New file: `src-tauri/src/codex_history.rs`.

Add to existing `src-tauri/src/codex_gateway.rs::Ctx` struct (lines ~30):
```rust
struct Ctx {
    db: PathBuf,
    client: Client,
    profile: GatewayProfile,
    history: Arc<CodexChatHistoryStore>,  // <-- new
}
```

Initialize in `gateway::start` (search for where `Ctx` is built):
```rust
let history = Arc::new(CodexChatHistoryStore::default());
let ctx = Ctx { db, client, profile, history };
```

### How

#### 2a. New file `src-tauri/src/codex_history.rs`

Mirror the API of `cc-switch/src-tauri/src/proxy/providers/codex_chat_history.rs` (864 lines), but slimmed to only the parts we need:

```rust
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

const MAX_RESPONSES: usize = 512;

#[derive(Default)]
pub struct CodexChatHistoryStore {
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    responses: HashMap<String, CachedResponse>,  // response_id → cached
    response_order: VecDeque<String>,            // LRU order (oldest first)
    call_index: HashMap<String, Vec<String>>,   // call_id → response_ids (for uniqueness check)
}

#[derive(Clone)]
struct CachedResponse {
    calls_by_id: HashMap<String, Value>,  // call_id → full function_call item
    call_order: Vec<String>,              // preserves sequence
}

impl CodexChatHistoryStore {
    pub fn record_response(&self, response_id: &str, output_items: &[Value]) {
        // Skip if no function_call items.
        let mut inner = self.inner.write().unwrap();
        let mut cached = CachedResponse::default();
        for item in output_items {
            if let Some(call_id) = item.get("call_id").and_then(|v| v.as_str()) {
                if !cached.calls_by_id.contains_key(call_id) {
                    cached.call_order.push(call_id.to_string());
                }
                cached.calls_by_id.insert(call_id.to_string(), item.clone());
                inner.call_index.entry(call_id.to_string()).or_default().push(response_id.to_string());
            }
        }
        if cached.calls_by_id.is_empty() { return; }
        // LRU eviction.
        if inner.responses.contains_key(response_id) {
            inner.response_order.retain(|id| id != response_id);
        } else {
            while inner.responses.len() >= MAX_RESPONSES {
                if let Some(oldest) = inner.response_order.pop_front() {
                    if let Some(removed) = inner.responses.remove(&oldest) {
                        for cid in &removed.call_order {
                            if let Some(list) = inner.call_index.get_mut(cid) {
                                list.retain(|rid| rid != &oldest);
                                if list.is_empty() { inner.call_index.remove(cid); }
                            }
                        }
                    }
                } else { break; }
            }
        }
        inner.responses.insert(response_id.to_string(), cached);
        inner.response_order.push_back(response_id.to_string());
    }

    pub fn enrich_request(&self, body: &mut Value) -> Result<(), String> {
        // Find previous_response_id and function_call_output blocks.
        // For each call_id, look up the cached function_call.
        // Insert it into the message array as an assistant tool_calls message
        // before the matching tool message.
        // Implementation: mirror cc-switch's `enrich_request` (codex_chat_history.rs:89-194).
        todo!()  // see cc-switch for full algorithm
    }
}
```

#### 2b. Wire into `responses_handler`

Before calling `ctx.client.post(...)` (search for the call site), insert:

```rust
ctx.history.enrich_request(&mut body)?;
```

#### 2c. Wire into streaming response recording

In the streaming SSE handler, when a `response.output_item.done` event arrives with `type: "function_call"`, call `ctx.history.record_call_item(item)`. When `response.completed` arrives, call `ctx.history.record_response(response_id, output_items)`.

### Tests

```rust
#[test]
fn test_history_enrich_reconstructs_missing_function_call() {
    // Step 1: record a response with a function_call item.
    // Step 2: send a new request with previous_response_id and a
    //         function_call_output referencing the same call_id but no
    //         tool_calls field in the assistant message.
    // Step 3: assert enrich_request inserts the missing tool_calls.
}

#[test]
fn test_history_lru_eviction_at_512() {
    // Record 513 responses, assert the first is evicted.
}

#[test]
fn test_history_rejects_ambiguous_call_id() {
    // Record the same call_id in two responses.
    // Try to enrich a request that references it. Assert error.
}
```

### Risks

**Low**. The history is in-memory and per-gateway-instance, so a gateway restart drops it (acceptable: Codex CLI's `previous_response_id` will be unknown after restart and the client will retry with full tool_calls). No persistence risk.

### Expected release

v1.18.0.

---

## Improvement 1: CodexToolContext + bidirectional restore

**Why**: Codex App supports 4 tool types: `function`, `custom` (e.g. `apply_patch`), `tool_search` (dynamic tool discovery), and `namespace` (server-prefixed). Chat Completions only knows `function`. Currently we flatten all 4 to `function` on the way out — but on the way back we can't restore the original type, so Codex App sees a `function_call` instead of a `custom_tool_call` and may misinterpret the arguments format.

### Where to change

New file: `src-tauri/src/codex_tools.rs` (or extend `codex_gateway.rs`).

### How

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CodexToolKind {
    Function,
    Custom,
    ToolSearch,
    Namespace,
}

#[derive(Debug, Clone)]
pub struct CodexToolSpec {
    pub kind: CodexToolKind,
    pub original_name: String,
    pub original_definition: Value,  // full original tool spec
    pub chat_name: String,          // sanitized name used in function spec
    pub namespace: Option<String>,  // for CodexToolKind::Namespace
}

pub struct CodexToolContext {
    pub specs: Vec<CodexToolSpec>,
    pub chat_name_to_spec: HashMap<String, usize>,  // chat_name → index in specs
    pub seen_chat_names: HashSet<String>,
}

impl CodexToolContext {
    pub fn from_request(tools: &[Value], input_items: &[Value]) -> Self { ... }
    pub fn downgrade_to_function(&self, tool: &CodexToolSpec) -> Value { ... }
    pub fn restore_response_item(&self, chat_name: &str, args: &Value) -> Value { ... }
}
```

#### Usage in `convert_request`

```rust
let tool_ctx = CodexToolContext::from_request(/* tools from body */, /* input items */);
for spec in &tool_ctx.specs {
    let function_tool = tool_ctx.downgrade_to_function(spec);
    chat_req["tools"].as_array_mut().unwrap().push(function_tool);
}
```

#### Usage in `convert_sync_response` and streaming

```rust
for chat_call in message["tool_calls"].as_array().unwrap_or(&[]) {
    let chat_name = chat_call["function"]["name"].as_str().unwrap_or("");
    if let Some(item) = tool_ctx.restore_response_item(chat_name, &chat_call["function"]["arguments"]) {
        output.push(item);
    }
}
```

### Tests

```rust
#[test]
fn test_downgrade_custom_tool_to_function() {
    // Build a Codex `custom` tool spec.
    // Downgrade, assert: type: "function", description contains original spec.
}

#[test]
fn test_restore_custom_tool_from_function_response() {
    // Build a function call with name "apply_patch".
    // Restore, assert: type: "custom_tool_call" with original spec.
}

#[test]
fn test_namespace_preserved_round_trip() {
    // Build a namespace tool "git:commit".
    // Downgrade, then restore. Assert name == "git:commit", type == original.
}
```

### Risks

**Medium**. Touches all tool paths. Mitigation: make downgrade idempotent (calling it twice produces the same result), so a future improvement that does partial downgrade still works.

### Expected release

v1.19.0 (after Improvements 5, 3, 4, 2 are stable).

---

## Test Strategy

All 5 improvements require unit tests in `codex_gateway.rs::mod tests` (or the new modules). The test fixtures are similar to existing tests at `codex_gateway.rs:1740+` (the existing test module already has helpers like `mock_chat_request`, `apply_codex_tool_call_mode` tests, etc.).

For Improvements 2 and 1 that introduce new modules (`codex_history.rs`, `codex_tools.rs`), create a separate `#[cfg(test)] mod tests` in each new file. Integration tests for the full `responses_handler` flow should live in `codex_gateway.rs::tests`.

A reasonable target: **each improvement lands with at least 5 new test cases** (3 unit + 2 integration). Total: **25 new test cases** across the 5 improvements.

## Risk Summary

| Improvement | Files Touched | New Files | LoC Estimate | Risk |
|---|---|---|---|---|
| 5 | `codex_gateway.rs` | — | ~30 | Very low |
| 3 | `codex_gateway.rs` | — | ~80 | Low |
| 4 | `codex_gateway.rs`, `gateway_strategy.rs` (or new `codex_strategy.rs`) | optional | ~250 | Medium |
| 2 | `codex_gateway.rs` | `codex_history.rs` | ~400 (incl. tests) | Low |
| 1 | `codex_gateway.rs` | `codex_tools.rs` | ~500 (incl. tests) | Medium |

**Total**: ~1260 LoC + 25 test cases across 5 releases.

## Implementation Order (recommended)

v1.16.0 → 5 + 3 (half day)
v1.17.0 → 4 (1-2 days)
v1.18.0 → 2 (1-2 days)
v1.19.0 → 1 (2-3 days)

Each release should:
1. Land with the test cases from its acceptance criteria.
2. Update `CHANGELOG.md` with the corresponding section.
3. Update `docs/project.md` if the user-facing behavior changes.
4. Build the DMG and update the GitHub Release.

## Reference

Source of all patterns: `/tmp/cc-switch` (cloned 2026-06-30). Key files:

- `src-tauri/src/proxy/providers/codex.rs` (965 lines) — platform-aware provider logic, especially `infer_codex_chat_reasoning_config` (lines 174-291) and `infer_aggregator_platform_config` (lines 297-334).
- `src-tauri/src/proxy/transform_codex_chat.rs` (3298 lines) — Responses ↔ Chat Conversions, especially the `tool_choice` cleanup at lines 322-334 and `include_usage` injection at lines 335-340.
- `src-tauri/src/proxy/providers/codex_chat_history.rs` (864 lines) — LRU function_call cache, especially `enrich_request` (lines 89-194) and `record_responses_sse_stream` (lines 367-441).
