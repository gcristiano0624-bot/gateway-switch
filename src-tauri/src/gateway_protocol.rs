use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    compatibility,
    loop_guard::{LoopGuard, LoopGuardSummary, TextGuardAction, ToolLoopHint},
};

pub(crate) struct ChatConversion {
    pub(crate) payload: Value,
    pub(crate) loop_summary: LoopGuardSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ChatRoleMode {
    Standard,
    UserAssistantOnly,
}

pub(crate) fn anthropic_to_chat_request(
    body: &Value,
    upstream_model: &str,
    stream: bool,
    role_mode: ChatRoleMode,
) -> Value {
    anthropic_to_chat_conversion(body, upstream_model, stream, role_mode).payload
}

pub(crate) fn guard_anthropic_request_payload(body: &Value) -> ChatConversion {
    let mut payload = body.clone();
    let mut loop_guard = LoopGuard::default();
    let mut tool_loop_hints: Vec<ToolLoopHint> = Vec::new();

    if let Some(messages) = payload.get_mut("messages").and_then(|v| v.as_array_mut()) {
        for item in messages.iter_mut() {
            let is_assistant = item.get("role").and_then(|v| v.as_str()) == Some("assistant");
            let Some(content_value) = item.get_mut("content") else {
                continue;
            };

            if is_assistant {
                observe_anthropic_tool_uses(content_value, &mut loop_guard, &mut tool_loop_hints);
            } else {
                compress_anthropic_tool_results(content_value, &mut loop_guard);
            }
        }

        if let Some(hint) = loop_guard_context_hint(&tool_loop_hints) {
            messages.push(json!({
                "role": "user",
                "content": [{"type": "text", "text": hint}]
            }));
        }
    }

    ChatConversion {
        payload,
        loop_summary: loop_guard.summary(),
    }
}

pub(crate) fn anthropic_to_chat_conversion(
    body: &Value,
    upstream_model: &str,
    stream: bool,
    role_mode: ChatRoleMode,
) -> ChatConversion {
    let mut messages: Vec<Value> = Vec::new();
    let mut loop_guard = LoopGuard::default();
    let mut tool_loop_hints: Vec<ToolLoopHint> = Vec::new();
    let mut pending_system = body
        .get("system")
        .map(anthropic_content_to_text)
        .filter(|content| !content.is_empty());

    if role_mode == ChatRoleMode::Standard {
        if let Some(content) = pending_system.take() {
            messages.push(json!({"role":"system","content":content}));
        }
    }

    if let Some(items) = body.get("messages").and_then(|v| v.as_array()) {
        for item in items {
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let Some(content_value) = item.get("content") else {
                continue;
            };

            if role == "assistant" {
                let text = anthropic_content_to_text(content_value);
                let tool_calls = anthropic_tool_uses_to_chat_guarded(
                    content_value,
                    &mut loop_guard,
                    &mut tool_loop_hints,
                );
                if !tool_calls.is_empty() {
                    messages.push(
                        json!({"role": "assistant", "content": text, "tool_calls": tool_calls}),
                    );
                } else if !text.is_empty() {
                    messages.push(json!({"role": "assistant", "content": text}));
                }
                continue;
            }

            let tool_results =
                anthropic_tool_results_to_chat_guarded(content_value, &mut loop_guard);
            let mut content = anthropic_content_to_text(content_value);
            if let Some(system) = pending_system.take() {
                content = merge_system_into_user_content(&system, &content);
            }
            if !content.is_empty() {
                messages.push(json!({"role": role, "content": content}));
            }
            for tool_result in tool_results {
                if role_mode == ChatRoleMode::UserAssistantOnly {
                    messages.push(json!({
                        "role": "user",
                        "content": format!(
                            "[tool_result:{}]\n{}",
                            tool_result
                                .get("tool_call_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown"),
                            tool_result
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                        )
                    }));
                } else {
                    messages.push(tool_result);
                }
            }
        }
    }

    if let Some(system) = pending_system.take() {
        messages
            .push(json!({"role":"user","content": merge_system_into_user_content(&system, "")}));
    }

    if messages.is_empty() {
        messages.push(json!({"role":"user","content":""}));
    }

    if let Some(hint) = loop_guard_context_hint(&tool_loop_hints) {
        messages.push(json!({"role":"user","content":hint}));
    }

    let mut req = json!({
        "model": upstream_model,
        "messages": messages,
        "stream": stream,
    });

    if let Some(max_tokens) = body.get("max_tokens") {
        req["max_tokens"] = max_tokens.clone();
    }
    if let Some(temperature) = body.get("temperature") {
        req["temperature"] = temperature.clone();
    }
    if let Some(top_p) = body.get("top_p") {
        req["top_p"] = top_p.clone();
    }
    if let Some(stop) = body.get("stop_sequences") {
        req["stop"] = stop.clone();
    }
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let converted: Vec<Value> = tools.iter().filter_map(|tool| {
            let name = tool.get("name").and_then(|v| v.as_str())?;
            Some(json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": tool.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    "parameters": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
                }
            }))
        }).collect();
        if !converted.is_empty() {
            req["tools"] = json!(converted);
        }
    }

    ChatConversion {
        payload: req,
        loop_summary: loop_guard.summary(),
    }
}

fn observe_anthropic_tool_uses(
    value: &Value,
    loop_guard: &mut LoopGuard,
    tool_loop_hints: &mut Vec<ToolLoopHint>,
) {
    if let Some(parts) = value.as_array() {
        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                continue;
            }
            let Some(name) = part.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let input = part.get("input").cloned().unwrap_or_else(|| json!({}));
            let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "{}".into());
            if let Some(hint) = loop_guard.observe_tool_call_pattern(name, &arguments) {
                tool_loop_hints.push(hint);
            }
        }
    }
}

fn compress_anthropic_tool_results(value: &mut Value, loop_guard: &mut LoopGuard) {
    if let Some(parts) = value.as_array_mut() {
        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                continue;
            }
            let id = part
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = part
                .get("content")
                .map(anthropic_content_to_text)
                .unwrap_or_default();
            let compressed = loop_guard.compress_tool_result(&id, &content);
            if compressed != content {
                part["content"] = json!(compressed);
            }
        }
    }
}

fn merge_system_into_user_content(system: &str, user: &str) -> String {
    if user.trim().is_empty() {
        format!("[system]\n{system}")
    } else {
        format!("[system]\n{system}\n\n[user]\n{user}")
    }
}

fn anthropic_tool_uses_to_chat_guarded(
    value: &Value,
    loop_guard: &mut LoopGuard,
    tool_loop_hints: &mut Vec<ToolLoopHint>,
) -> Vec<Value> {
    value
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    if part.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                        return None;
                    }
                    let id = part
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| "toolu_unknown");
                    let name = part.get("name").and_then(|v| v.as_str())?;
                    let input = part.get("input").cloned().unwrap_or_else(|| json!({}));
                    let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "{}".into());
                    if let Some(hint) = loop_guard.observe_tool_call_pattern(name, &arguments) {
                        tool_loop_hints.push(hint);
                    }
                    Some(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments
                        }
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn anthropic_tool_results_to_chat_guarded(value: &Value, loop_guard: &mut LoopGuard) -> Vec<Value> {
    value
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    if part.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                        return None;
                    }
                    let id = part
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let content = part
                        .get("content")
                        .map(anthropic_content_to_text)
                        .unwrap_or_default();
                    let content = loop_guard.compress_tool_result(id, &content);
                    Some(json!({"role":"tool","tool_call_id":id,"content":content}))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn loop_guard_context_hint(hints: &[ToolLoopHint]) -> Option<String> {
    let hint = hints.first()?;
    Some(format!(
        "Gateway Switch LoopGuard note: the tool `{}` with similar arguments appears {} times in the recent context. This approach may be looping. Before calling it again, consider a different strategy, summarize what failed, or ask the user for guidance.",
        hint.tool_name, hint.repeats
    ))
}

fn anthropic_content_to_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .filter_map(|part| match part.get("type").and_then(|v| v.as_str()) {
                Some("text") => part.get("text").and_then(|v| v.as_str()).map(String::from),
                Some("tool_result") | Some("tool_use") => None,
                _ => part.get("text").and_then(|v| v.as_str()).map(String::from),
            })
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}

pub(crate) fn anthropic_messages_to_text(body: &Value) -> String {
    body.get("messages")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("content").map(anthropic_content_to_text))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub(crate) fn chat_to_anthropic_message(chat: &Value, model: &str) -> Value {
    let message = &chat["choices"][0]["message"];
    let content = extract_chat_message_text(message);
    let tool_uses = chat_tool_calls_to_anthropic(message);
    let finish_reason = chat["choices"][0]["finish_reason"]
        .as_str()
        .unwrap_or("stop");
    let stop_reason = match finish_reason {
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        _ => "end_turn",
    };
    let input_tokens = chat["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = chat["usage"]["completion_tokens"]
        .as_u64()
        .unwrap_or_else(|| estimate_tokens(&content));
    let mut content_blocks = Vec::new();
    if !content.is_empty() {
        content_blocks.push(json!({"type":"text","text":content}));
    }
    content_blocks.extend(tool_uses);
    let has_tools = content_blocks
        .iter()
        .any(|v| v.get("type").and_then(|t| t.as_str()) == Some("tool_use"));

    json!({
        "id": format!("msg_{}", Uuid::new_v4()),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content_blocks,
        "stop_reason": if has_tools { "tool_use" } else { stop_reason },
        "stop_sequence": null,
        "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens}
    })
}

fn chat_tool_calls_to_anthropic(message: &Value) -> Vec<Value> {
    message
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let id = call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| "toolu_unknown");
                    let function = call.get("function")?;
                    let name = function.get("name").and_then(|v| v.as_str())?;
                    let arguments = function
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    let input =
                        compatibility::repair_json_object(arguments).unwrap_or_else(|_| json!({}));
                    Some(json!({"type":"tool_use","id":id,"name":name,"input":input}))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_chat_message_text(message: &Value) -> String {
    for key in ["content", "reasoning_content", "reasoning", "text"] {
        if let Some(text) = message
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return text.to_string();
        }
    }
    message
        .get("content")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    part.get("text")
                        .or_else(|| part.get("content"))
                        .and_then(|v| v.as_str())
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

pub(crate) fn extract_chat_delta(line: &str) -> Option<String> {
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    let delta = &v["choices"][0]["delta"];
    extract_chat_message_text(delta)
        .split_once('\0')
        .map(|(s, _)| s.to_string())
        .or_else(|| {
            let text = extract_chat_message_text(delta);
            if text.is_empty() {
                v["choices"][0]["message"]
                    .as_object()
                    .map(|_| extract_chat_message_text(&v["choices"][0]["message"]))
                    .filter(|s| !s.is_empty())
            } else {
                Some(text)
            }
        })
}

pub(crate) fn extract_chat_stream_error(line: &str) -> Option<String> {
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    v.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| {
            v.get("message")
                .and_then(|m| m.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
}

pub(crate) fn chat_tool_delta_events(
    line: &str,
    tool_blocks: &mut HashMap<i64, (i64, String, String, String)>,
    next_content_index: &mut i64,
) -> Option<Vec<String>> {
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    let calls = v["choices"][0]["delta"]["tool_calls"].as_array()?;
    let mut events = Vec::new();

    for call in calls {
        let openai_index = call.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
        let id_delta = call.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let function = call.get("function").unwrap_or(&Value::Null);
        let name_delta = function.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args_delta = function
            .get("arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !tool_blocks.contains_key(&openai_index) {
            let content_index = *next_content_index;
            *next_content_index += 1;
            let id = if id_delta.is_empty() {
                format!("toolu_{}", Uuid::new_v4())
            } else {
                id_delta.to_string()
            };
            let name = if name_delta.is_empty() {
                "tool".to_string()
            } else {
                name_delta.to_string()
            };
            tool_blocks.insert(
                openai_index,
                (content_index, id.clone(), name.clone(), String::new()),
            );
            let block_start = json!({
                "type":"content_block_start",
                "index": content_index,
                "content_block": {"type":"tool_use","id":id,"name":name,"input":{}}
            });
            events.push(format!(
                "event: content_block_start\ndata: {}\n\n",
                serde_json::to_string(&block_start).unwrap()
            ));
        }

        if let Some((content_index, id, name, arguments)) = tool_blocks.get_mut(&openai_index) {
            if !id_delta.is_empty() {
                *id = id_delta.to_string();
            }
            if !name_delta.is_empty() {
                *name = name_delta.to_string();
            }
            if !args_delta.is_empty() {
                arguments.push_str(args_delta);
                let delta = json!({
                    "type":"content_block_delta",
                    "index": *content_index,
                    "delta": {"type":"input_json_delta","partial_json":args_delta}
                });
                events.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::to_string(&delta).unwrap()
                ));
            }
        }
    }

    Some(events)
}

pub(crate) fn estimate_tokens(text: &str) -> u64 {
    let words = text.split_whitespace().count() as u64;
    words.max((text.chars().count() as u64 + 3) / 4)
}
