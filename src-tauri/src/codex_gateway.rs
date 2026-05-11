use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

use async_stream::stream;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use uuid::Uuid;

use crate::{
    compatibility, database,
    models::{GatewayProfile, Provider, RequestLog},
    state::{AppState, GatewayHandle, GatewayStatus},
};

#[derive(Clone)]
struct Ctx {
    db: PathBuf,
    client: Client,
    profile: GatewayProfile,
}

struct Route {
    display: String,
    provider_id: String,
    upstream_model: String,
    base_url: String,
    headers: Vec<(String, String)>,
}

pub fn start(st: &AppState) -> Result<String, String> {
    {
        let g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
        if g.is_some() { return Ok("already_running".into()); }
    }

    let profile = database::get_codex_profile(&st.db_path)?;
    let addr: SocketAddr = format!("{}:{}", profile.listen_host, profile.listen_port)
        .parse().map_err(|e: std::net::AddrParseError| e.to_string())?;

    let ctx = Ctx { db: st.db_path.clone(), client: Client::new(), profile: profile.clone() };
    let router = build_router(ctx);

    let (tx, rx) = oneshot::channel::<()>();
    let rt = Arc::clone(&st.runtime);
    {
        let mut s = rt.codex_gateway_status.lock().map_err(|_| "lock")?;
        *s = GatewayStatus { running: true, status: "starting".into(), error: None };
    }

    let handle: JoinHandle<()> = tokio::spawn(async move {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = true; s.status = "running".into(); s.error = None;
                }
                let server = axum::serve(listener, router).with_graceful_shutdown(async {
                    let _ = rx.await;
                });
                if let Err(e) = server.await {
                    if let Ok(mut s) = rt.codex_gateway_status.lock() {
                        s.running = false; s.status = "error".into(); s.error = Some(e.to_string());
                    }
                } else if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = false; s.status = "stopped".into();
                }
            }
            Err(e) => {
                if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = false; s.status = "error".into(); s.error = Some(e.to_string());
                }
            }
        }
    });

    let mut g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
    *g = Some(GatewayHandle { shutdown: Some(tx), _task: handle });
    Ok("started".into())
}

pub fn stop(st: &AppState) -> Result<String, String> {
    let mut g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
    if let Some(h) = g.as_mut() {
        if let Some(tx) = h.shutdown.take() { let _ = tx.send(()); }
    } else {
        return Ok("not_running".into());
    }
    *g = None;
    let mut s = st.runtime.codex_gateway_status.lock().map_err(|_| "lock")?;
    *s = GatewayStatus { running: false, status: "stopped".into(), error: None };
    Ok("stopped".into())
}

pub fn status(st: &AppState) -> Result<GatewayStatus, String> {
    st.runtime.codex_gateway_status.lock().map(|s| s.clone()).map_err(|_| "lock".into())
}

fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/responses", post(responses_handler))
        .with_state(ctx)
}

async fn health(State(ctx): State<Ctx>) -> impl IntoResponse {
    let providers = database::list_providers(&ctx.db).unwrap_or_default();
    let capabilities: Vec<Value> = providers
        .iter()
        .filter(|p| p.enabled)
        .map(compatibility::provider_capability_json)
        .collect();
    let routes = database::list_codex_routes(&ctx.db).unwrap_or_default();
    let models: Vec<String> = routes.into_iter().filter(|r| r.enabled).map(|r| r.codex_model).collect();
    Json(json!({ "ok": true, "gateway": "codex", "listen": format!("{}:{}", ctx.profile.listen_host, ctx.profile.listen_port), "models": models, "capabilities": capabilities }))
}

async fn list_models(State(ctx): State<Ctx>, headers: HeaderMap) -> Result<Json<Value>, Response> {
    verify_auth(&headers, &ctx)?;
    let routes = database::list_codex_routes(&ctx.db).map_err(internal)?;
    let providers = database::list_providers(&ctx.db).unwrap_or_default();
    let data: Vec<Value> = routes.into_iter().filter(|r| r.enabled).map(|r| {
        let provider_name = providers.iter().find(|p| p.id == r.provider_id).map(|p| p.name.as_str()).unwrap_or("unknown");
        json!({
            "id": r.codex_model,
            "object": "model",
            "created": 1700000000,
            "owned_by": provider_name,
        })
    }).collect();
    Ok(Json(json!({ "object": "list", "data": data })))
}

async fn responses_handler(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<Value>) -> Result<Response, Response> {
    verify_auth(&headers, &ctx)?;

    let model = body.get("model").and_then(|v| v.as_str()).ok_or(bad_req("missing model"))?;
    let route = resolve(&ctx.db, model).map_err(bad_req)?;
    let req_id = Uuid::new_v4().to_string();
    let resp_id = format!("resp_{}", Uuid::new_v4());
    let msg_id = format!("msg_{}", Uuid::new_v4());
    let started = Instant::now();
    let is_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(true);

    // Convert Responses API request → Chat Completions request
    let chat_req = convert_request(&body, &route.upstream_model);

    // Forward to upstream provider's Chat Completions endpoint.
    let resp = ctx.client.post(upstream_url(&route.base_url, "chat/completions"))
        .headers(to_headers(&route.headers)?)
        .json(&chat_req)
        .send().await.map_err(|e| {
            let _ = database::insert_log(&ctx.db, &RequestLog {
                request_id: req_id.clone(), claude_alias: route.display.clone(),
                provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
                status_code: None, duration_ms: Some(started.elapsed().as_millis() as u64),
                is_stream, error_summary: Some(e.to_string()), created_at: String::new(),
            });
            upstream_err(e)
        })?;

    let status = resp.status();

    if !is_stream {
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let _ = database::insert_log(&ctx.db, &RequestLog {
                request_id: req_id, claude_alias: route.display.clone(),
                provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
                status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
                is_stream: false, error_summary: Some(text.clone()), created_at: String::new(),
            });
            return Err(upstream_status_err(status, text));
        }
        let chat_response = resp.json::<Value>().await.map_err(upstream_err)?;
        let responses_response = convert_sync_response(&chat_response, &route.display, &resp_id, &msg_id);
        let _ = database::insert_log(&ctx.db, &RequestLog {
            request_id: req_id, claude_alias: route.display.clone(),
            provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
            status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: false, error_summary: None, created_at: String::new(),
        });
        return Ok((status, Json(responses_response)).into_response());
    }

    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let _ = database::insert_log(&ctx.db, &RequestLog {
            request_id: req_id, claude_alias: route.display.clone(),
            provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
            status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary: Some(text.clone()), created_at: String::new(),
        });
        return Err(upstream_status_err(status, text));
    }

    // Streaming: convert Chat Completions SSE → Responses API SSE
    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model = route.upstream_model.clone();
    let log_req_id = req_id.clone();
    let db = ctx.db.clone();
    let body_stream = resp.bytes_stream();

    let sse = stream! {
        let mut seq: i64 = 0;
        // 1. Emit response.created
        let created_event = json!({
            "type": "response.created",
            "response": {
                "id": resp_id,
                "object": "response",
                "created_at": chrono::Utc::now().timestamp(),
                "model": display,
                "output": [],
                "status": "in_progress",
                "usage": null
            },
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.created\ndata: {}\n\n", serde_json::to_string(&created_event).unwrap()
        )));

        // 2. Emit response.output_item.added
        let item_added = json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {
                "type": "message",
                "id": msg_id,
                "role": "assistant",
                "content": [],
                "status": "in_progress"
            },
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.output_item.added\ndata: {}\n\n", serde_json::to_string(&item_added).unwrap()
        )));

        let part_added = json!({
            "type": "response.content_part.added",
            "item_id": msg_id,
            "output_index": 0,
            "content_index": 0,
            "part": {
                "type": "output_text",
                "text": "",
                "annotations": []
            },
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.content_part.added\ndata: {}\n\n", serde_json::to_string(&part_added).unwrap()
        )));

        // 3. Stream content deltas from Chat Completions format
        let mut full_text = String::new();
        let mut tool_items: std::collections::HashMap<i64, StreamingToolCall> = std::collections::HashMap::new();
        let mut next_output_index: i64 = 1;
        let mut buf = String::new();
        tokio::pin!(body_stream);
        while let Some(item) = body_stream.next().await {
            match item {
                Ok(chunk) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].to_string();
                        buf = buf[pos + 1..].to_string();
                        if let Some(text) = extract_chat_delta(&line) {
                            full_text.push_str(&text);
                            let delta_event = json!({
                                "type": "response.output_text.delta",
                                "item_id": msg_id,
                                "output_index": 0,
                                "content_index": 0,
                                "delta": text,
                                "sequence_number": seq
                            });
                            seq += 1;
                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                                "event: response.output_text.delta\ndata: {}\n\n",
                                serde_json::to_string(&delta_event).unwrap()
                            )));
                        }
                        let tool_events = response_tool_delta_events(&line, &mut tool_items, &mut next_output_index, &mut seq);
                        for (event_name, event_body) in tool_events {
                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event(event_name, &event_body)));
                        }
                    }
                }
                Err(e) => {
                    let text = format!("\n\n[Gateway stream error: {e}]");
                    full_text.push_str(&text);
                    let delta_event = json!({
                        "type": "response.output_text.delta",
                        "item_id": msg_id,
                        "output_index": 0,
                        "content_index": 0,
                        "delta": text,
                        "sequence_number": seq
                    });
                    seq += 1;
                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                        "event: response.output_text.delta\ndata: {}\n\n",
                        serde_json::to_string(&delta_event).unwrap()
                    )));
                    break;
                },
            }
        }

        let text_done = json!({
            "type": "response.output_text.done",
            "item_id": msg_id,
            "output_index": 0,
            "content_index": 0,
            "text": full_text,
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.output_text.done\ndata: {}\n\n", serde_json::to_string(&text_done).unwrap()
        )));

        let part_done = json!({
            "type": "response.content_part.done",
            "item_id": msg_id,
            "output_index": 0,
            "content_index": 0,
            "part": {
                "type": "output_text",
                "text": full_text,
                "annotations": []
            },
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.content_part.done\ndata: {}\n\n", serde_json::to_string(&part_done).unwrap()
        )));

        // 4. Emit response.output_item.done
        let item_done = json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": {
                "type": "message",
                "id": msg_id,
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": full_text,
                    "annotations": []
                }],
                "status": "completed"
            },
            "sequence_number": seq
        });
        seq += 1;
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.output_item.done\ndata: {}\n\n", serde_json::to_string(&item_done).unwrap()
        )));

        let mut tool_outputs = Vec::new();
        let mut sorted_tools: Vec<StreamingToolCall> = tool_items.into_values().collect();
        sorted_tools.sort_by_key(|t| t.output_index);
        for tool in sorted_tools {
            let args_done = json!({
                "type": "response.function_call_arguments.done",
                "item_id": tool.item_id,
                "output_index": tool.output_index,
                "arguments": tool.arguments,
                "sequence_number": seq
            });
            seq += 1;
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.function_call_arguments.done", &args_done)));

            let item = json!({
                "type": "function_call",
                "id": tool.item_id,
                "call_id": tool.call_id,
                "name": tool.name,
                "arguments": tool.arguments,
                "status": "completed"
            });
            let item_done = json!({
                "type": "response.output_item.done",
                "output_index": tool.output_index,
                "item": item,
                "sequence_number": seq
            });
            seq += 1;
            tool_outputs.push(item);
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.output_item.done", &item_done)));
        }

        let output_tokens = estimate_tokens(&full_text);
        let usage = response_usage(0, output_tokens);
        let mut output = vec![json!({
            "type": "message",
            "id": msg_id,
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": full_text,
                "annotations": []
            }],
            "status": "completed"
        })];
        output.extend(tool_outputs);

        // 5. Emit response.completed
        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": resp_id,
                "object": "response",
                "created_at": chrono::Utc::now().timestamp(),
                "model": display,
                "output": output,
                "status": "completed",
                "usage": usage
            },
            "sequence_number": seq
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.completed\ndata: {}\n\n", serde_json::to_string(&completed).unwrap()
        )));

        let _ = database::insert_log(&db, &RequestLog {
            request_id: log_req_id, claude_alias: display,
            provider_id, upstream_model, status_code: Some(status.as_u16()),
            duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary: None, created_at: String::new(),
        });
    };

    let builder = Response::builder().status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-cache");
    builder.body(Body::from_stream(sse)).map_err(internal)
}

// ── Request/Response Conversion ──

fn convert_request(body: &Value, upstream_model: &str) -> Value {
    let mut messages: Vec<Value> = Vec::new();

    // 1. instructions → system message
    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            messages.push(json!({"role": "system", "content": instructions}));
        }
    }

    // 2. input → messages
    if let Some(input) = body.get("input").and_then(|v| v.as_str()) {
        if !input.is_empty() {
            messages.push(json!({"role": "user", "content": input}));
        }
    } else if let Some(input) = body.get("input").and_then(|v| v.as_array()) {
        for item in input {
            match item.get("type").and_then(|v| v.as_str()) {
                Some("message") => {
                    let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                    let content = extract_content_from_item(item);
                    messages.push(json!({"role": role, "content": content}));
                }
                Some("function_call_output") => {
                    let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                    let output = item.get("output").and_then(|v| v.as_str()).unwrap_or("");
                    messages.push(json!({"role": "tool", "tool_call_id": call_id, "content": output}));
                }
                Some("function_call") => {
                    // Convert function_call to assistant message with tool_calls
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                    let arguments = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                    messages.push(json!({
                        "role": "assistant",
                        "tool_calls": [{
                            "id": call_id,
                            "type": "function",
                            "function": {"name": name, "arguments": arguments}
                        }]
                    }));
                }
                _ => {
                    // Try to extract as a simple message
                    let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                    let content = extract_content_from_item(item);
                    if !content.is_empty() {
                        messages.push(json!({"role": role, "content": content}));
                    }
                }
            }
        }
    }

    // If no messages were extracted, add a placeholder
    if messages.is_empty() {
        messages.push(json!({"role": "user", "content": ""}));
    }

    let mut chat_req = json!({
        "model": upstream_model,
        "messages": messages,
        "stream": body.get("stream").and_then(|v| v.as_bool()).unwrap_or(true),
    });

    // Pass tools if present
    if let Some(tools) = body.get("tools") {
        if let Some(tools_array) = tools.as_array() {
            let converted_tools: Vec<Value> = tools_array.iter().filter_map(|t| {
                if t.get("type").and_then(|v| v.as_str()) == Some("function") {
                    Some(json!({
                        "type": "function",
                        "function": {
                            "name": t.get("name").and_then(|v| v.as_str())?,
                            "description": t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "parameters": t.get("parameters").cloned().unwrap_or(json!({})),
                        }
                    }))
                } else {
                    None
                }
            }).collect();
            if !converted_tools.is_empty() {
                chat_req["tools"] = json!(converted_tools);
            }
        }
    }

    // Pass temperature
    if let Some(temp) = body.get("temperature") {
        chat_req["temperature"] = temp.clone();
    }

    // Pass max_output_tokens → max_tokens
    if let Some(max) = body.get("max_output_tokens") {
        chat_req["max_tokens"] = max.clone();
    }

    // Pass top_p
    if let Some(top_p) = body.get("top_p") {
        chat_req["top_p"] = top_p.clone();
    }

    // Pass tool_choice
    if let Some(tc) = body.get("tool_choice") {
        chat_req["tool_choice"] = tc.clone();
    }

    chat_req
}

fn extract_content_from_item(item: &Value) -> String {
    // Try "content" field first (string)
    if let Some(s) = item.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    // Try "content" field (array of content parts)
    if let Some(arr) = item.get("content").and_then(|v| v.as_array()) {
        let parts: Vec<String> = arr.iter().filter_map(|part| {
            if part.get("type").and_then(|v| v.as_str()) == Some("input_text") {
                part.get("text").and_then(|v| v.as_str()).map(String::from)
            } else if part.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                part.get("text").and_then(|v| v.as_str()).map(String::from)
            } else {
                part.get("text").and_then(|v| v.as_str()).map(String::from)
            }
        }).collect();
        return parts.join("");
    }
    String::new()
}

fn convert_sync_response(chat: &Value, model: &str, resp_id: &str, msg_id: &str) -> Value {
    let message = &chat["choices"][0]["message"];
    let content = extract_chat_message_text(message).unwrap_or_default();
    let tool_calls = chat_tool_calls_to_response_items(message);
    let input_tokens = chat["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = chat["usage"]["completion_tokens"].as_u64().unwrap_or_else(|| estimate_tokens(&content));
    let total_tokens = chat["usage"]["total_tokens"].as_u64().unwrap_or(input_tokens + output_tokens);
    let usage = response_usage(input_tokens, output_tokens).as_object().cloned().map(|mut usage| {
        usage.insert("total_tokens".into(), json!(total_tokens));
        Value::Object(usage)
    }).unwrap_or_else(|| response_usage(input_tokens, output_tokens));
    let mut output = Vec::new();
    if !content.is_empty() || tool_calls.is_empty() {
        output.push(json!({
            "type": "message",
            "id": msg_id,
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": content,
                "annotations": []
            }],
            "status": "completed"
        }));
    }
    output.extend(tool_calls);

    json!({
        "id": resp_id,
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": model,
        "output": output,
        "usage": usage,
        "status": "completed"
    })
}

fn chat_tool_calls_to_response_items(message: &Value) -> Vec<Value> {
    message.get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            calls.iter().filter_map(|call| {
                let function = call.get("function")?;
                let name = function.get("name").and_then(|v| v.as_str())?;
                let arguments = function.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                let arguments = compatibility::repair_json_object(arguments)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| arguments.to_string());
                let call_id = call.get("id").and_then(|v| v.as_str()).unwrap_or_else(|| "call_unknown");
                Some(json!({
                    "type": "function_call",
                    "id": format!("fc_{}", Uuid::new_v4()),
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments,
                    "status": "completed"
                }))
            }).collect()
        })
        .unwrap_or_default()
}

fn response_usage(input_tokens: u64, output_tokens: u64) -> Value {
    json!({
        "input_tokens": input_tokens,
        "input_tokens_details": {
            "cached_tokens": 0
        },
        "output_tokens": output_tokens,
        "output_tokens_details": {
            "reasoning_tokens": 0
        },
        "total_tokens": input_tokens + output_tokens
    })
}

fn estimate_tokens(text: &str) -> u64 {
    let words = text.split_whitespace().count() as u64;
    words.max((text.chars().count() as u64 + 3) / 4)
}

fn extract_chat_delta(line: &str) -> Option<String> {
    if !line.starts_with("data:") { return None; }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" { return None; }
    let v: Value = serde_json::from_str(payload).ok()?;
    if let Some(message) = v.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .filter(|s| !s.is_empty())
    {
        return Some(format!("\n\n[Upstream error: {message}]"));
    }

    let delta = &v["choices"][0]["delta"];
    extract_text_from_delta(delta).or_else(|| {
        v["choices"][0]["message"]["content"].as_str().map(String::from)
    })
}

fn extract_text_from_delta(delta: &Value) -> Option<String> {
    for key in ["content", "reasoning_content", "reasoning", "text"] {
        if let Some(text) = delta.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return Some(text.to_string());
        }
    }

    delta.get("content")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts.iter()
                .filter_map(|part| {
                    part.get("text")
                        .or_else(|| part.get("content"))
                        .and_then(|v| v.as_str())
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|text| !text.is_empty())
}

fn extract_chat_message_text(message: &Value) -> Option<String> {
    for key in ["content", "reasoning_content", "reasoning", "text"] {
        if let Some(text) = message.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return Some(text.to_string());
        }
    }

    message.get("content")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts.iter()
                .filter_map(|part| {
                    part.get("text")
                        .or_else(|| part.get("content"))
                        .and_then(|v| v.as_str())
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|text| !text.is_empty())
}

#[derive(Debug, Clone)]
struct StreamingToolCall {
    output_index: i64,
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
}

fn response_tool_delta_events(
    line: &str,
    tool_items: &mut std::collections::HashMap<i64, StreamingToolCall>,
    next_output_index: &mut i64,
    seq: &mut i64,
) -> Vec<(&'static str, Value)> {
    if !line.starts_with("data:") { return Vec::new(); }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" { return Vec::new(); }
    let Ok(v) = serde_json::from_str::<Value>(payload) else { return Vec::new(); };
    let Some(calls) = v["choices"][0]["delta"]["tool_calls"].as_array() else { return Vec::new(); };
    let mut events = Vec::new();

    for call in calls {
        let openai_index = call.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
        let id_delta = call.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let function = call.get("function").unwrap_or(&Value::Null);
        let name_delta = function.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args_delta = function.get("arguments").and_then(|v| v.as_str()).unwrap_or("");

        if !tool_items.contains_key(&openai_index) {
            let output_index = *next_output_index;
            *next_output_index += 1;
            let item = StreamingToolCall {
                output_index,
                item_id: format!("fc_{}", Uuid::new_v4()),
                call_id: if id_delta.is_empty() { format!("call_{}", Uuid::new_v4()) } else { id_delta.to_string() },
                name: if name_delta.is_empty() { "function".into() } else { name_delta.to_string() },
                arguments: String::new(),
            };
            let body = json!({
                "type": "response.output_item.added",
                "output_index": output_index,
                "item": {
                    "type": "function_call",
                    "id": item.item_id,
                    "call_id": item.call_id,
                    "name": item.name,
                    "arguments": "",
                    "status": "in_progress"
                },
                "sequence_number": *seq
            });
            *seq += 1;
            events.push(("response.output_item.added", body));
            tool_items.insert(openai_index, item);
        }

        if let Some(item) = tool_items.get_mut(&openai_index) {
            if !id_delta.is_empty() { item.call_id = id_delta.to_string(); }
            if !name_delta.is_empty() { item.name = name_delta.to_string(); }
            if !args_delta.is_empty() {
                item.arguments.push_str(args_delta);
                let body = json!({
                    "type": "response.function_call_arguments.delta",
                    "item_id": item.item_id,
                    "output_index": item.output_index,
                    "delta": args_delta,
                    "sequence_number": *seq
                });
                *seq += 1;
                events.push(("response.function_call_arguments.delta", body));
            }
        }
    }

    events
}

fn sse_event(event_name: &str, body: &Value) -> String {
    format!("event: {event_name}\ndata: {}\n\n", serde_json::to_string(body).unwrap())
}

// ── Shared utilities ──

fn resolve(db: &PathBuf, model: &str) -> Result<Route, String> {
    let routes = database::list_codex_routes(db)?;
    let providers = database::list_providers(db)?;
    let route = routes.into_iter()
        .find(|r| r.enabled && r.codex_model == model)
        .ok_or_else(|| format!("Unknown model: {model}"))?;
    let provider = providers.into_iter()
        .find(|p| p.enabled && p.id == route.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", route.provider_id))?;
    Ok(Route {
        display: route.codex_model,
        provider_id: provider.id.clone(),
        upstream_model: route.upstream_model.trim().to_string(),
        base_url: provider.openai_base_url.trim_end_matches('/').to_string(),
        headers: auth_headers(&provider),
    })
}

fn auth_headers(p: &Provider) -> Vec<(String, String)> {
    let mut val = p.api_key.clone().unwrap_or_default();
    if let Some(scheme) = p.auth_scheme.as_deref().filter(|s| !s.is_empty()) {
        val = format!("{scheme} {val}");
    }
    vec![
        (p.auth_header.clone(), val),
        ("content-type".into(), "application/json".into()),
    ]
}

fn upstream_url(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{endpoint}")
    } else {
        format!("{base}/v1/{endpoint}")
    }
}

fn verify_auth(headers: &HeaderMap, ctx: &Ctx) -> Result<(), Response> {
    if let Some(v) = headers.get("x-api-key") {
        if v.to_str().unwrap_or("") == ctx.profile.auth_token { return Ok(()); }
        return Err(auth_err("Invalid token"));
    }
    if let Some(v) = headers.get(header::AUTHORIZATION) {
        let s = v.to_str().unwrap_or("");
        if s == format!("Bearer {}", ctx.profile.auth_token) { return Ok(()); }
    }
    Err(auth_err("Missing x-api-key or Authorization header"))
}

fn to_headers(pairs: &[(String, String)]) -> Result<reqwest::header::HeaderMap, Response> {
    let mut h = reqwest::header::HeaderMap::new();
    for (k, v) in pairs {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).map_err(internal)?;
        let val = reqwest::header::HeaderValue::from_str(v).map_err(internal)?;
        h.insert(name, val);
    }
    Ok(h)
}

fn auth_err(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({"error":{"message":msg,"type":"invalid_request_error","code":"unauthorized"}}))).into_response()
}
fn bad_req(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error":{"message":msg.into(),"type":"invalid_request_error"}}))).into_response()
}
fn internal<E: std::fmt::Display>(e: E) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":{"message":e.to_string(),"type":"internal_error"}}))).into_response()
}
fn upstream_err<E: std::fmt::Display>(e: E) -> Response {
    (StatusCode::BAD_GATEWAY, Json(json!({"error":{"message":e.to_string(),"type":"upstream_error"}}))).into_response()
}

fn upstream_status_err(status: reqwest::StatusCode, body: String) -> Response {
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(String::from)
                .or_else(|| v.get("message").and_then(|m| m.as_str()).map(String::from))
        })
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("Upstream returned HTTP {}", status.as_u16())
            } else {
                body
            }
        });
    (StatusCode::BAD_GATEWAY, Json(json!({"error":{"message":message,"type":"upstream_error","status":status.as_u16()}}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http};
    use tower::ServiceExt;
    use crate::{database, models::CreateProvider};

    #[tokio::test]
    async fn test_codex_models_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();
        database::create_provider(&db, &CreateProvider {
            id: "p1".into(), name: "P".into(), base_url: "http://x".into(),
            openai_base_url: None, anthropic_base_url: None,
            auth_header: "Authorization".into(), auth_scheme: Some("Bearer".into()), api_key: Some("k".into()),
        }).unwrap();
        database::create_codex_route(&db, &crate::models::CreateCodexRoute {
            id: "r1".into(), codex_model: "gpt-4o".into(), display_name: "GPT-4o".into(),
            provider_id: "p1".into(), upstream_model: "deepseek-chat".into(),
        }).unwrap();

        let app = build_router(Ctx {
            db, client: Client::new(),
            profile: GatewayProfile { listen_host: "127.0.0.1".into(), listen_port: 3457, auth_token: "tok".into() },
        });

        let resp = app.clone().oneshot(
            http::Request::builder().uri("/v1/models").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = app.oneshot(
            http::Request::builder().uri("/v1/models")
                .header("x-api-key", "tok").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["data"][0]["id"], "gpt-4o");
    }

    #[tokio::test]
    async fn test_codex_request_conversion() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Hello"}]}
            ],
            "instructions": "You are a helpful assistant.",
            "stream": false
        });

        let chat_req = convert_request(&responses_req, "deepseek-chat");
        assert_eq!(chat_req["model"], "deepseek-chat");
        assert_eq!(chat_req["messages"][0]["role"], "system");
        assert_eq!(chat_req["messages"][0]["content"], "You are a helpful assistant.");
        assert_eq!(chat_req["messages"][1]["role"], "user");
        assert_eq!(chat_req["messages"][1]["content"], "Hello");
        assert_eq!(chat_req["stream"], false);
    }

    #[tokio::test]
    async fn test_codex_string_input_conversion() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": "Edit this file carefully",
            "stream": false
        });

        let chat_req = convert_request(&responses_req, "deepseek-chat");
        assert_eq!(chat_req["messages"][0]["role"], "user");
        assert_eq!(chat_req["messages"][0]["content"], "Edit this file carefully");
    }

    #[tokio::test]
    async fn test_codex_sync_response_conversion() {
        let chat_resp = json!({
            "choices": [{"message": {"content": "Hi there!"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["model"], "gpt-4o");
        assert_eq!(resp["output"][0]["type"], "message");
        assert_eq!(resp["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(resp["output"][0]["content"][0]["text"], "Hi there!");
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 5);
        assert_eq!(resp["usage"]["total_tokens"], 15);
        assert_eq!(resp["status"], "completed");
    }

    #[tokio::test]
    async fn test_codex_sync_tool_call_conversion() {
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "apply_patch", "arguments": "{\"path\":\"src/main.rs\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test");
        assert_eq!(resp["output"][0]["type"], "function_call");
        assert_eq!(resp["output"][0]["call_id"], "call_1");
        assert_eq!(resp["output"][0]["name"], "apply_patch");
    }

    #[test]
    fn test_extracts_provider_delta_variants() {
        let content = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
        let reasoning = r#"data: {"choices":[{"delta":{"reasoning_content":"thinking"}}]}"#;
        let text = r#"data: {"choices":[{"delta":{"text":"plain"}}]}"#;

        assert_eq!(extract_chat_delta(content).as_deref(), Some("hello"));
        assert_eq!(extract_chat_delta(reasoning).as_deref(), Some("thinking"));
        assert_eq!(extract_chat_delta(text).as_deref(), Some("plain"));
        assert_eq!(extract_chat_delta("data: [DONE]"), None);
    }

    #[test]
    fn test_upstream_url_accepts_root_or_v1_base() {
        assert_eq!(
            upstream_url("https://api.example.com", "chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            upstream_url("https://api.example.com/v1", "chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }
}
