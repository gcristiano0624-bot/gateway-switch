# Codex 对话中断问题 — 根因分析与修复方案

> 问题描述：通过 Gateway Switch 的 Codex Gateway 路由到中国大陆第三方模型（DeepSeek、XiaoMiMo、Qwen 等）时，模型经常"聊着聊着"就停了——它回复说"接下来我要做 XXX"，但对话进度就此终止，Codex 不再继续执行。
>
> 本文档基于对 `codex_gateway.rs`（1106 行）、`compatibility.rs`、`gateway.rs` 的逐行分析。

---

## 一、问题复现路径

```
用户在 Codex CLI 中输入任务
→ Codex 发送 /v1/responses 请求（带 tools 定义）
→ Gateway Switch 转为 Chat Completions 请求
→ 第三方模型生成回复（文本 + 可能的 tool_calls）
→ Gateway Switch 转回 Responses API 格式
→ Codex 接收并解析回复
→ 如果回复中没有 function_call，Codex 认为本轮结束
→ 用户看到模型说"我接下来要读取文件..."但实际什么都没做
```

---

## 二、根因分析

### 根因 1：`response.completed` 永远是 `status: "completed"`，即使没有 tool_calls

**代码位置：** `codex_gateway.rs` 第 431-447 行

```rust
// 5. Emit response.completed
let completed = json!({
    "type": "response.completed",
    "response": {
        "id": resp_id,
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": display,
        "output": output,
        "status": "completed",   // ← 永远是 "completed"
        "usage": usage
    },
    "sequence_number": seq
});
```

**问题：** 无论上游模型是否正确调用了 tool，gateway 都告诉 Codex "response completed"。Codex 看到一个 `status: "completed"` 的 response，其中只有 `type: "message"` 的文本输出（没有 `type: "function_call"`），就认为这一轮已经结束了。

**对比 OpenAI 原生行为：** OpenAI 的 GPT 模型在有 tools 可用时，如果需要执行操作，会返回 `function_call` output items。如果返回的是纯文本且 `status: "completed"`，说明模型确实决定不需要调用工具——这在 OpenAI 模型上是合理行为。但对第三方模型来说，模型**想**调用工具但**不知道怎么格式化**，所以只生成了描述性文本。

### 根因 2：`finish_reason` 被完全忽略

**代码位置：** `codex_gateway.rs` 第 270-324 行（streaming 处理）

```rust
while let Some(item) = body_stream.next().await {
    match item {
        Ok(chunk) => {
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].to_string();
                buf = buf[pos + 1..].to_string();
                if let Some(text) = extract_chat_delta(&line) {
                    // 只提取文本 delta
                    full_text.push_str(&text);
                    // ...
                }
                let tool_events = response_tool_delta_events(&line, &mut tool_items, ...);
                // 只提取 tool call delta
            }
        }
        // ...
    }
}
// 流结束后直接 emit completed，不检查 finish_reason
```

**问题：** Chat Completions 的 SSE 流中，最后一条 `data:` 包含 `choices[0].finish_reason`，值可以是：
- `"stop"` — 正常结束
- `"tool_calls"` — 模型请求调用工具
- `"length"` — 被 `max_tokens` 截断

Gateway 从未检查 `finish_reason`，所以：
- 当 `finish_reason: "length"` 时（模型输出被截断），gateway 不知道响应不完整
- 当 `finish_reason: "stop"` 但有 tools 可用时，gateway 不知道模型可能应该调用工具

**`extract_chat_delta` 函数（第 691-708 行）只提取文本，不提取 finish_reason：**

```rust
fn extract_chat_delta(line: &str) -> Option<String> {
    if !line.starts_with("data:") { return None; }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" { return None; }
    let v: Value = serde_json::from_str(payload).ok()?;
    // ...
    let delta = &v["choices"][0]["delta"];
    extract_text_from_delta(delta)  // 只返回文本
}
```

### 根因 3：`tool_choice: "auto"` 不够强制

**代码位置：** `codex_gateway.rs` 第 556-559 行

```rust
if !converted_tools.is_empty() {
    chat_req["tools"] = json!(converted_tools);
    if body.get("tool_choice").is_none() {
        chat_req["tool_choice"] = json!("auto");
    }
}
```

**问题：** `"auto"` 意味着模型**可以选择**不调用工具。对 OpenAI 的 GPT-4 来说，`auto` 通常能正确判断何时需要调用工具。但对 DeepSeek、MiMo 等模型来说：

1. 模型可能不理解 `tools` 字段的含义（尤其在长对话中 system prompt 被稀释时）
2. 模型可能生成 "我来读取文件" 这样的文本，但不生成 `tool_calls` 结构
3. 模型可能在第一轮正确调用了工具，但在后续轮次中"忘记"要调用工具

### 根因 4：系统提示不够强，且在长对话中容易被稀释

**代码位置：** `codex_gateway.rs` 第 475-481 行

```rust
if has_tools {
    messages.push(json!({
        "role": "system",
        "content": "Gateway Switch compatibility note: when the task requires reading files, \
                    running commands, editing code, or checking project state, you must call \
                    the provided tools using structured tool_calls. Do not merely say you will \
                    inspect, analyze, read, run, edit, or verify something unless you also \
                    emit the corresponding tool call."
    }));
}
```

**问题：**
1. 这个 system message 被插入在 `instructions` 之后、用户消息之前。但在长对话中（Codex 会发送整个对话历史），这条提示只出现一次，位于消息列表的前部，容易被模型忽略
2. 对某些中文模型来说，英文系统提示的效果不如中文
3. 模型可能已经在之前的对话轮次中建立了"用文本描述行动"的习惯模式

### 根因 5：上游流意外中断时，gateway 仍发送 `response.completed`

**代码位置：** `codex_gateway.rs` 第 304-322 行

```rust
Err(e) => {
    let text = format!("\n\n[Gateway stream error: {e}]");
    full_text.push_str(&text);
    let delta_event = json!({
        "type": "response.output_text.delta",
        // ...
        "delta": text,
        // ...
    });
    // ...
    break;  // ← break 出循环
},
// 循环结束后，仍然执行后续的 output_text.done → content_part.done → completed
```

**问题：** 当上游流因网络错误、超时、Provider 限流等原因中断时，gateway 在文本中追加一条错误消息，然后 `break` 出循环。但循环之后的代码仍然会发送 `response.output_text.done`、`response.content_part.done`、`response.output_item.done` 和 `response.completed`。

这意味着：
- Codex 收到一个带错误消息的 "completed" response
- Codex 可能把错误消息当成模型的正常回复
- Codex 不知道这个 response 实际上是不完整的

### 根因 6：没有流超时机制

**问题：** `codex_gateway.rs` 中没有任何超时设置。如果上游 Provider：
- 连接后长时间不返回数据（挂起）
- 发送部分数据后停止
- 每隔很长时间才发送一个 SSE chunk

Gateway 会无限等待，Codex 也会无限等待。用户看到的就是"���话卡住了"。

### 根因 7：`convert_request` 中 `max_tokens` 的传递可能不够

**代码位置：** `codex_gateway.rs` 第 570-573 行

```rust
if let Some(max) = body.get("max_output_tokens") {
    chat_req["max_tokens"] = max.clone();
}
```

**问题：** Codex 发送的 `max_output_tokens` 可能是一个较大的值（如 16384），但某些第三方 Provider 对 `max_tokens` 有自己的上限限制（如 4096）。当模型输出被 Provider 截断时，`finish_reason` 会是 `"length"`，但 gateway 不检测这个（根因 2），所以 Codex 收到的是一个不完整的、被截断的回复。

---

## 三、完整的问题链路图

```
Codex 发送请求（带 tools）
    ↓
Gateway convert_request()
    ↓ 注入 system prompt（根因 4：可能被忽略）
    ↓ 设置 tool_choice: "auto"（根因 3：不够强制）
    ↓
上游模型生成回复
    ↓
模型选择用文本描述而不是调用 tool（第三方模型常见行为）
    ↓
Gateway streaming 处理
    ↓ 不检查 finish_reason（根因 2）
    ↓ 不检测"文本描述了行动但没有 tool_calls"（根因 1 的前置条件）
    ↓
Gateway 发送 response.completed, status: "completed"（根因 1）
    ↓ 没有 function_call output items
    ↓
Codex 收到 completed response
    ↓ 没有 function_call → 本轮结束
    ↓
用户看到：模型说"我接下来要..."，但什么都没发生
```

---

## 四、修复方案

### 方案 A：检测 "should have called tool but didn't" 并重试（推荐）

**原理：** 在 streaming 结束后、发送 `response.completed` 之前，检查：
1. 请求中是否携带了 tools
2. 响应中是否包含 function_call
3. 如果有 tools 但没有 function_call，且文本中包含行动描述关键词

如果检测到"应该调用工具但没有调用"，可以：
- 不发送 `response.completed`
- 而是自动重试一次请求，使用更强的 `tool_choice: "required"`
- 或者在 `response.completed` 中设置 `status: "incomplete"` + `incomplete_details: {"reason": "tool_calls_missing"}`

**实现位置：** `codex_gateway.rs` streaming 结束后（第 324 行之后）

```rust
// 伪代码
if has_tools && tool_items.is_empty() && looks_like_action_description(&full_text) {
    // 方案 A1: 用 tool_choice: "required" 重试
    // 方案 A2: 发送 incomplete response 让 Codex 重试
}
```

**需要新增的函数：**

```rust
fn looks_like_action_description(text: &str) -> bool {
    // 中文+英文关键词检测
    let patterns = [
        "我来", "我将", "让我", "接下来", "现在我",
        "I'll", "Let me", "I will", "Now I'll", "I'm going to",
        "I need to", "I should", "Going to",
        "读取", "查看", "修改", "执行", "运行", "创建", "打开",
        "read", "check", "modify", "run", "execute", "create", "open",
        "edit", "write", "install", "update", "delete", "remove",
    ];
    let lower = text.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}
```

### 方案 B：检测 `finish_reason` 并设置正确的 response status

**原理：** 解析最后一个 SSE chunk 中的 `finish_reason`，根据其值决定 response status。

**实现位置：** `codex_gateway.rs` 的 `extract_chat_delta` 函数需要额外返回 `finish_reason` 信息。

```rust
// 修改 streaming 循环
let mut finish_reason: Option<String> = None;

// 在循环中解析 finish_reason
if let Some(reason) = v["choices"][0]["delta"]["finish_reason"].as_str()
    .or_else(|| v["choices"][0]["finish_reason"].as_str()) 
{
    finish_reason = Some(reason.to_string());
}

// 在 response.completed 中
let status = match finish_reason.as_deref() {
    Some("length") => "incomplete",
    Some("tool_calls") => "completed", // 有 tool calls 是正常情况
    _ => "completed",
};

let completed = json!({
    "type": "response.completed",
    "response": {
        // ...
        "status": status,
        "incomplete_details": if status == "incomplete" {
            Some(json!({"reason": "max_output_tokens"}))
        } else {
            None
        },
    }
});
```

### 方案 C：使用 `tool_choice: "required"` 代替 `"auto"`

**原理：** 当请求包含 tools 时，强制模型必须调用至少一个工具。

**优点：** 简单直接，确保模型不会只生成文本
**缺点：** 有些第三方 Provider 不支持 `tool_choice: "required"`，会返回 400 错误

**实现：** 在 `convert_request` 中修改默认值：

```rust
// 当前代码
if body.get("tool_choice").is_none() {
    chat_req["tool_choice"] = json!("auto");
}

// 改为
if body.get("tool_choice").is_none() {
    // 先尝试 required，如果上游返回 400 则 fallback 到 auto
    chat_req["tool_choice"] = json!("required");
}
```

**更好的实现：** 在 Provider Capability Profile 中记录是否支持 `tool_choice: "required"`，根据能力选择策略。

### 方案 D：添加流超时机制

**实现位置：** streaming 循环中

```rust
use tokio::time::{timeout, Duration};

// 包装 body_stream 的读取
let stream_timeout = Duration::from_secs(120); // 2 分钟无数据则超时

while let Ok(Some(item)) = timeout(stream_timeout, body_stream.next()).await {
    // 处理 chunk
}
```

### 方案 E：增强系统提示

**原理：** 使用更强的系统提示，特别是对中文模型使用中文提示。

**实现位置：** `convert_request` 函数

```rust
if has_tools {
    let tool_names: Vec<String> = tools_array.iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();
    
    let guardrail = format!(
        "CRITICAL INSTRUCTION: You have access to tools: [{}]. \
         When you need to perform any action (read a file, run a command, edit code, \
         check project state, search files, etc.), you MUST use the provided tools \
         by emitting structured tool_calls in your response. \
         NEVER just describe what you would do in text without actually calling the tool. \
         Saying 'I will read the file' without a tool_call is ALWAYS wrong. \
         Always emit tool_calls first, then explain what you did after.",
        tool_names.join(", ")
    );
    
    messages.push(json!({
        "role": "system",
        "content": guardrail
    }));
}
```

---

## 五、推荐修复优先级

| 优先级 | 方案 | 效果 | 风险 |
|--------|------|------|------|
| **P0** | B: 检测 finish_reason | 识别截断响应，设置 incomplete status | 低 |
| **P0** | A: 检测 "should call tool but didn't" | 最大程度减少对话中断 | 中（需要平衡重试次数） |
| **P1** | C: tool_choice: "required" | 强制工具调用 | 高（部分 Provider 不支持） |
| **P1** | E: 增强系统提示 | 提高工具调用概率 | 低 |
| **P2** | D: 流超时机制 | 防止无限等待 | 低 |

建议的组合策略：**B + A + E**（检测 finish_reason + 检测缺失 tool calls 并重试 + 增强提示），这三者组合能覆盖绝大多数场景，且风险可控。

---

## 六、测试验证方法

1. **Unit test：** 为 `looks_like_action_description` 编写测试用例
2. **Integration test：** 模拟上游返回纯文本（无 tool_calls）的 SSE 流，验证 gateway 的处理逻辑
3. **手动验证：** 使用 DeepSeek/MiMo 作为上游，在 Codex 中执行 "帮我看看这个项目的代码结构" 类任务，观察是否还会中断
4. **Log 验证：** 在 `request_logs` 中记录 `finish_reason` 和 `tool_calls_count`，便于事后分析

---

## 七、相关代码文件

| 文件 | 行号 | 说明 |
|------|------|------|
| `src-tauri/src/codex_gateway.rs` | 270-324 | Streaming 处理循环 |
| `src-tauri/src/codex_gateway.rs` | 326-460 | 流结束后事件发射 |
| `src-tauri/src/codex_gateway.rs` | 465-586 | 请求转换 |
| `src-tauri/src/codex_gateway.rs` | 691-708 | `extract_chat_delta` |
| `src-tauri/src/codex_gateway.rs` | 710-730 | `extract_text_from_delta` |
| `src-tauri/src/codex_gateway.rs` | 763-830 | `response_tool_delta_events` |
| `src-tauri/src/compatibility.rs` | 全文 | 工具调用修复、能力画像 |
