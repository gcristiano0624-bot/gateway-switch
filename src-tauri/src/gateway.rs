use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

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
    openai_base_url: String,
    anthropic_base_url: String,
    headers: Vec<(String, String)>,
}

pub fn start(st: &AppState) -> Result<String, String> {
    {
        let mut g = st.runtime.gateway_handle.lock().map_err(|_| "lock")?;
        if let Some(h) = g.as_ref() {
            let is_running = st.runtime.gateway_status.lock().map_err(|_| "lock")?.running;
            if is_running && !h._task.is_finished() {
                return Ok("already_running".into());
            }
        }
        if let Some(mut stale) = g.take() {
            if let Some(tx) = stale.shutdown.take() { let _ = tx.send(()); }
            stale._task.abort();
        }
    }

    let profile = database::get_profile(&st.db_path)?;
    let addr: SocketAddr = format!("{}:{}", profile.listen_host, profile.listen_port)
        .parse().map_err(|e: std::net::AddrParseError| e.to_string())?;

    let ctx = Ctx { db: st.db_path.clone(), client: Client::new(), profile: profile.clone() };
    let router = build_router(ctx);

    let (tx, rx) = oneshot::channel::<()>();
    let rt = Arc::clone(&st.runtime);
    {
        let mut s = rt.gateway_status.lock().map_err(|_| "lock")?;
        *s = GatewayStatus { running: true, status: "starting".into(), error: None };
    }

    let handle: JoinHandle<()> = tokio::spawn(async move {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Ok(mut s) = rt.gateway_status.lock() {
                    s.running = true; s.status = "running".into(); s.error = None;
                }
                let server = axum::serve(listener, router).with_graceful_shutdown(async {
                    let _ = rx.await;
                });
                if let Err(e) = server.await {
                    if let Ok(mut s) = rt.gateway_status.lock() {
                        s.running = false; s.status = "error".into(); s.error = Some(e.to_string());
                    }
                } else if let Ok(mut s) = rt.gateway_status.lock() {
                    s.running = false; s.status = "stopped".into();
                }
            }
            Err(e) => {
                if let Ok(mut s) = rt.gateway_status.lock() {
                    s.running = false; s.status = "error".into(); s.error = Some(e.to_string());
                }
            }
        }
    });

    let mut g = st.runtime.gateway_handle.lock().map_err(|_| "lock")?;
    *g = Some(GatewayHandle { shutdown: Some(tx), _task: handle });
    Ok("started".into())
}

pub fn stop(st: &AppState) -> Result<String, String> {
    let mut g = st.runtime.gateway_handle.lock().map_err(|_| "lock")?;
    if let Some(h) = g.as_mut() {
        if let Some(tx) = h.shutdown.take() { let _ = tx.send(()); }
    } else {
        return Ok("not_running".into());
    }
    *g = None;
    let mut s = st.runtime.gateway_status.lock().map_err(|_| "lock")?;
    *s = GatewayStatus { running: false, status: "stopped".into(), error: None };
    Ok("stopped".into())
}

pub fn status(st: &AppState) -> Result<GatewayStatus, String> {
    st.runtime.gateway_status.lock().map(|s| s.clone()).map_err(|_| "lock".into())
}

fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/messages/v1/messages/count_tokens", post(count_tokens))
        .with_state(ctx)
}

async fn health(State(ctx): State<Ctx>) -> impl IntoResponse {
    let providers = database::list_providers(&ctx.db).unwrap_or_default();
    let capabilities: Vec<Value> = providers
        .iter()
        .filter(|p| p.enabled)
        .map(compatibility::provider_capability_json)
        .collect();
    let models: Vec<String> = database::list_routes(&ctx.db)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| r.claude_alias)
        .collect();
    Json(json!({ "ok": true, "listen": format!("{}:{}", ctx.profile.listen_host, ctx.profile.listen_port), "models": models, "capabilities": capabilities }))
}

async fn list_models(State(ctx): State<Ctx>, headers: HeaderMap) -> Result<Json<Value>, Response> {
    verify_auth(&headers, &ctx)?;
    let routes = database::list_routes(&ctx.db).map_err(internal)?;
    let data: Vec<Value> = routes.into_iter().filter(|r| r.enabled).map(|r| {
        json!({ "type": "model", "id": r.claude_alias, "display_name": r.display_name, "created_at": "2025-01-01T00:00:00Z" })
    }).collect();
    let first = data.first().map(|v| v["id"].as_str().unwrap_or("").to_string()).unwrap_or_default();
    let last = data.last().map(|v| v["id"].as_str().unwrap_or("").to_string()).unwrap_or_default();
    Ok(Json(json!({ "data": data, "has_more": false, "first_id": first, "last_id": last })))
}

async fn count_tokens(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<Value>) -> Result<Response, Response> {
    verify_auth(&headers, &ctx)?;
    let model = body.get("model").and_then(|v| v.as_str()).ok_or(bad_req("missing model"))?;
    let route = resolve(&ctx.db, model).map_err(bad_req)?;
    let mut upstream = body.clone();
    upstream["model"] = json!(route.upstream_model);
    let resp = ctx.client.post(upstream_url(&route.anthropic_base_url, "messages/count_tokens"))
        .headers(to_headers(&route.headers)?).json(&upstream)
        .send().await.map_err(upstream_err)?;
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(upstream_err)?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(json_body) => Ok((status, Json(json_body)).into_response()),
        Err(_) => {
            let input_tokens = estimate_tokens(&anthropic_messages_to_text(&body));
            Ok((StatusCode::OK, Json(json!({"input_tokens": input_tokens}))).into_response())
        }
    }
}

async fn messages(State(ctx): State<Ctx>, headers: HeaderMap, Json(body): Json<Value>) -> Result<Response, Response> {
    verify_auth(&headers, &ctx)?;
    let model = body.get("model").and_then(|v| v.as_str()).ok_or(bad_req("missing model"))?;
    let route = resolve(&ctx.db, model).map_err(bad_req)?;
    let req_id = Uuid::new_v4().to_string();
    let started = Instant::now();
    let is_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut upstream = body.clone();
    upstream["model"] = json!(route.upstream_model);

    let resp = ctx.client.post(upstream_url(&route.anthropic_base_url, "messages"))
        .headers(to_headers(&route.headers)?).json(&upstream)
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
        let bytes = resp.bytes().await.map_err(upstream_err)?;
        if status.is_success() {
            if let Ok(mut json_body) = serde_json::from_slice::<Value>(&bytes) {
                rewrite_model(&mut json_body, &route.display);
                let _ = database::insert_log(&ctx.db, &RequestLog {
                    request_id: req_id, claude_alias: route.display.clone(),
                    provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
                    status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
                    is_stream: false, error_summary: None, created_at: String::new(),
                });
                return Ok((status, Json(json_body)).into_response());
            }
        }
        return chat_completion_fallback(ctx, route, body, req_id, started, false, Some((status, bytes))).await;
    }

    let content_type = resp.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !status.is_success() || !content_type.contains("text/event-stream") {
        let bytes = resp.bytes().await.map_err(upstream_err)?;
        return chat_completion_fallback(ctx, route, body, req_id, started, true, Some((status, bytes))).await;
    }

    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model = route.upstream_model.clone();
    let log_req_id = req_id.clone();
    let db = ctx.db.clone();
    let body_stream = resp.bytes_stream();
    let sse = stream! {
        let mut buf = String::new();
        tokio::pin!(body_stream);
        while let Some(item) = body_stream.next().await {
            match item {
                Ok(chunk) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..=pos].to_string();
                        buf = buf[pos + 1..].to_string();
                        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(rewrite_sse(&line, &display)));
                    }
                }
                Err(_) => break,
            }
        }
        if !buf.is_empty() {
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(rewrite_sse(&buf, &display)));
        }
        let _ = database::insert_log(&db, &RequestLog {
            request_id: log_req_id, claude_alias: display,
            provider_id, upstream_model, status_code: Some(status.as_u16()),
            duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary: None, created_at: String::new(),
        });
    };

    let mut builder = Response::builder().status(status);
    builder = builder.header(header::CONTENT_TYPE, "text/event-stream");
    builder.body(Body::from_stream(sse)).map_err(internal)
}

async fn chat_completion_fallback(
    ctx: Ctx,
    route: Route,
    body: Value,
    req_id: String,
    started: Instant,
    is_stream: bool,
    prior: Option<(reqwest::StatusCode, Bytes)>,
) -> Result<Response, Response> {
    let chat_req = anthropic_to_chat_request(&body, &route.upstream_model, is_stream);
    let resp = ctx.client.post(upstream_url(&route.openai_base_url, "chat/completions"))
        .headers(to_headers(&route.headers)?)
        .json(&chat_req)
        .send().await.map_err(|e| {
            let prior_message = prior.as_ref()
                .map(|(status, bytes)| format!("Anthropic endpoint HTTP {}: {}", status.as_u16(), body_preview(bytes)))
                .unwrap_or_default();
            let error = if prior_message.is_empty() { e.to_string() } else { format!("{prior_message}; Chat fallback error: {e}") };
            let _ = database::insert_log(&ctx.db, &RequestLog {
                request_id: req_id.clone(), claude_alias: route.display.clone(),
                provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
                status_code: None, duration_ms: Some(started.elapsed().as_millis() as u64),
                is_stream, error_summary: Some(error), created_at: String::new(),
            });
            upstream_err(e)
        })?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let _ = database::insert_log(&ctx.db, &RequestLog {
            request_id: req_id, claude_alias: route.display.clone(),
            provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
            status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream, error_summary: Some(text.clone()), created_at: String::new(),
        });
        return Err(upstream_status_err(status, text));
    }

    if !is_stream {
        let chat_body = resp.json::<Value>().await.map_err(upstream_err)?;
        let message = chat_to_anthropic_message(&chat_body, &route.display);
        let _ = database::insert_log(&ctx.db, &RequestLog {
            request_id: req_id, claude_alias: route.display.clone(),
            provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
            status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: false, error_summary: None, created_at: String::new(),
        });
        return Ok((StatusCode::OK, Json(message)).into_response());
    }

    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model = route.upstream_model.clone();
    let log_req_id = req_id.clone();
    let db = ctx.db.clone();
    let body_stream = resp.bytes_stream();
    let sse = stream! {
        let message_id = format!("msg_{}", Uuid::new_v4());
        let mut full_text = String::new();
        let mut text_started = false;
        let mut text_stopped = false;
        let mut text_index: i64 = 0;
        let mut next_content_index: i64 = 0;
        let mut tool_blocks: HashMap<i64, (i64, String, String, String)> = HashMap::new();
        let start_event = json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "model": display,
                "content": [],
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            }
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap())));

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
                            if !text_started {
                                text_index = next_content_index;
                                let block_start = json!({"type":"content_block_start","index":text_index,"content_block":{"type":"text","text":""}});
                                next_content_index += 1;
                                text_started = true;
                                yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&block_start).unwrap())));
                            }
                            full_text.push_str(&text);
                            let delta = json!({"type":"content_block_delta","index":text_index,"delta":{"type":"text_delta","text":text}});
                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&delta).unwrap())));
                        }
                        if let Some(events) = chat_tool_delta_events(&line, &mut tool_blocks, &mut next_content_index) {
                            if !events.is_empty() && text_started && !text_stopped {
                                let block_stop = json!({"type":"content_block_stop","index":text_index});
                                text_stopped = true;
                                yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&block_stop).unwrap())));
                            }
                            for event in events {
                                yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(event));
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        if text_started && !text_stopped {
            let block_stop = json!({"type":"content_block_stop","index":text_index});
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&block_stop).unwrap())));
        }
        let has_tools = !tool_blocks.is_empty();
        for (_, (content_index, _, _, _)) in tool_blocks.iter() {
            let block_stop = json!({"type":"content_block_stop","index":content_index});
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&block_stop).unwrap())));
        }
        let message_delta = json!({
            "type": "message_delta",
            "delta": {"stop_reason": if has_tools { "tool_use" } else { "end_turn" },"stop_sequence":null},
            "usage": {"output_tokens": estimate_tokens(&full_text)}
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: message_delta\ndata: {}\n\n", serde_json::to_string(&message_delta).unwrap())));
        let message_stop = json!({"type":"message_stop"});
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&message_stop).unwrap())));

        let _ = database::insert_log(&db, &RequestLog {
            request_id: log_req_id, claude_alias: display,
            provider_id, upstream_model, status_code: Some(status.as_u16()),
            duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary: None, created_at: String::new(),
        });
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(sse))
        .map_err(internal)
}

fn resolve(db: &PathBuf, model: &str) -> Result<Route, String> {
    let routes = database::list_routes(db)?;
    let providers = database::list_providers(db)?;
    let route = routes.into_iter()
        .find(|r| r.enabled && r.claude_alias == model)
        .ok_or_else(|| format!("Unknown model: {model}"))?;
    let provider = providers.into_iter()
        .find(|p| p.enabled && p.id == route.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", route.provider_id))?;
    Ok(Route {
        display: route.claude_alias,
        provider_id: provider.id.clone(),
        upstream_model: route.upstream_model.trim().to_string(),
        openai_base_url: provider.openai_base_url.trim_end_matches('/').to_string(),
        anthropic_base_url: provider.anthropic_base_url.as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&provider.openai_base_url)
            .trim_end_matches('/')
            .to_string(),
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
        ("anthropic-version".into(), "2023-06-01".into()),
        ("content-type".into(), "application/json".into()),
    ]
}

fn anthropic_to_chat_request(body: &Value, upstream_model: &str, stream: bool) -> Value {
    let mut messages: Vec<Value> = Vec::new();

    if let Some(system) = body.get("system") {
        let content = anthropic_content_to_text(system);
        if !content.is_empty() {
            messages.push(json!({"role":"system","content":content}));
        }
    }

    if let Some(items) = body.get("messages").and_then(|v| v.as_array()) {
        for item in items {
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let Some(content_value) = item.get("content") else { continue; };

            if role == "assistant" {
                let text = anthropic_content_to_text(content_value);
                let tool_calls = anthropic_tool_uses_to_chat(content_value);
                if !tool_calls.is_empty() {
                    messages.push(json!({"role": "assistant", "content": text, "tool_calls": tool_calls}));
                } else if !text.is_empty() {
                    messages.push(json!({"role": "assistant", "content": text}));
                }
                continue;
            }

            let tool_results = anthropic_tool_results_to_chat(content_value);
            let content = anthropic_content_to_text(content_value);
            if !content.is_empty() {
                messages.push(json!({"role": role, "content": content}));
            }
            for tool_result in tool_results {
                messages.push(tool_result);
            }
        }
    }

    if messages.is_empty() {
        messages.push(json!({"role":"user","content":""}));
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

    req
}

fn anthropic_tool_uses_to_chat(value: &Value) -> Vec<Value> {
    value.as_array()
        .map(|parts| {
            parts.iter().filter_map(|part| {
                if part.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                    return None;
                }
                let id = part.get("id").and_then(|v| v.as_str()).unwrap_or_else(|| "toolu_unknown");
                let name = part.get("name").and_then(|v| v.as_str())?;
                let input = part.get("input").cloned().unwrap_or_else(|| json!({}));
                Some(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".into())
                    }
                }))
            }).collect()
        })
        .unwrap_or_default()
}

fn anthropic_tool_results_to_chat(value: &Value) -> Vec<Value> {
    value.as_array()
        .map(|parts| {
            parts.iter().filter_map(|part| {
                if part.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                    return None;
                }
                let id = part.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
                let content = part.get("content").map(anthropic_content_to_text).unwrap_or_default();
                Some(json!({"role":"tool","tool_call_id":id,"content":content}))
            }).collect()
        })
        .unwrap_or_default()
}

fn anthropic_content_to_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(parts) = value.as_array() {
        return parts.iter().filter_map(|part| {
            match part.get("type").and_then(|v| v.as_str()) {
                Some("text") => part.get("text").and_then(|v| v.as_str()).map(String::from),
                Some("tool_result") | Some("tool_use") => None,
                _ => part.get("text").and_then(|v| v.as_str()).map(String::from),
            }
        }).collect::<Vec<_>>().join("");
    }
    String::new()
}

fn anthropic_messages_to_text(body: &Value) -> String {
    body.get("messages")
        .and_then(|v| v.as_array())
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("content").map(anthropic_content_to_text))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn chat_to_anthropic_message(chat: &Value, model: &str) -> Value {
    let message = &chat["choices"][0]["message"];
    let content = extract_chat_message_text(message);
    let tool_uses = chat_tool_calls_to_anthropic(message);
    let finish_reason = chat["choices"][0]["finish_reason"].as_str().unwrap_or("stop");
    let stop_reason = match finish_reason {
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        _ => "end_turn",
    };
    let input_tokens = chat["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = chat["usage"]["completion_tokens"].as_u64().unwrap_or_else(|| estimate_tokens(&content));
    let mut content_blocks = Vec::new();
    if !content.is_empty() {
        content_blocks.push(json!({"type":"text","text":content}));
    }
    content_blocks.extend(tool_uses);
    let has_tools = content_blocks.iter().any(|v| v.get("type").and_then(|t| t.as_str()) == Some("tool_use"));

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
    message.get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            calls.iter().filter_map(|call| {
                let id = call.get("id").and_then(|v| v.as_str()).unwrap_or_else(|| "toolu_unknown");
                let function = call.get("function")?;
                let name = function.get("name").and_then(|v| v.as_str())?;
                let arguments = function.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                let input = compatibility::repair_json_object(arguments).unwrap_or_else(|_| json!({}));
                Some(json!({"type":"tool_use","id":id,"name":name,"input":input}))
            }).collect()
        })
        .unwrap_or_default()
}

fn extract_chat_message_text(message: &Value) -> String {
    for key in ["content", "reasoning_content", "reasoning", "text"] {
        if let Some(text) = message.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return text.to_string();
        }
    }
    message.get("content")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts.iter()
                .filter_map(|part| part.get("text").or_else(|| part.get("content")).and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
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
    extract_chat_message_text(delta).split_once('\0')
        .map(|(s, _)| s.to_string())
        .or_else(|| {
            let text = extract_chat_message_text(delta);
            if text.is_empty() {
                v["choices"][0]["message"].as_object().map(|_| extract_chat_message_text(&v["choices"][0]["message"])).filter(|s| !s.is_empty())
            } else {
                Some(text)
            }
        })
}

fn chat_tool_delta_events(
    line: &str,
    tool_blocks: &mut HashMap<i64, (i64, String, String, String)>,
    next_content_index: &mut i64,
) -> Option<Vec<String>> {
    if !line.starts_with("data:") { return None; }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" { return None; }
    let v: Value = serde_json::from_str(payload).ok()?;
    let calls = v["choices"][0]["delta"]["tool_calls"].as_array()?;
    let mut events = Vec::new();

    for call in calls {
        let openai_index = call.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
        let id_delta = call.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let function = call.get("function").unwrap_or(&Value::Null);
        let name_delta = function.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args_delta = function.get("arguments").and_then(|v| v.as_str()).unwrap_or("");

        if !tool_blocks.contains_key(&openai_index) {
            let content_index = *next_content_index;
            *next_content_index += 1;
            let id = if id_delta.is_empty() { format!("toolu_{}", Uuid::new_v4()) } else { id_delta.to_string() };
            let name = if name_delta.is_empty() { "tool".to_string() } else { name_delta.to_string() };
            tool_blocks.insert(openai_index, (content_index, id.clone(), name.clone(), String::new()));
            let block_start = json!({
                "type":"content_block_start",
                "index": content_index,
                "content_block": {"type":"tool_use","id":id,"name":name,"input":{}}
            });
            events.push(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&block_start).unwrap()));
        }

        if let Some((content_index, id, name, arguments)) = tool_blocks.get_mut(&openai_index) {
            if !id_delta.is_empty() { *id = id_delta.to_string(); }
            if !name_delta.is_empty() { *name = name_delta.to_string(); }
            if !args_delta.is_empty() {
                arguments.push_str(args_delta);
                let delta = json!({
                    "type":"content_block_delta",
                    "index": *content_index,
                    "delta": {"type":"input_json_delta","partial_json":args_delta}
                });
                events.push(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&delta).unwrap()));
            }
        }
    }

    Some(events)
}

fn estimate_tokens(text: &str) -> u64 {
    let words = text.split_whitespace().count() as u64;
    words.max((text.chars().count() as u64 + 3) / 4)
}

fn body_preview(bytes: &Bytes) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    let redacted = compatibility::redact_secrets(&text);
    if redacted.chars().count() > 300 {
        format!("{}...", redacted.chars().take(300).collect::<String>())
    } else {
        redacted
    }
}

fn upstream_url(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');
    if base.ends_with(endpoint) {
        base.to_string()
    } else if base.ends_with("/v1") || base.ends_with("/v2") || base.ends_with("/v3") {
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

fn rewrite_model(v: &mut Value, model: &str) {
    match v {
        Value::Object(m) => {
            if let Some(x) = m.get_mut("model") { *x = json!(model); }
            for x in m.values_mut() { rewrite_model(x, model); }
        }
        Value::Array(a) => { for x in a { rewrite_model(x, model); } }
        _ => {}
    }
}

fn rewrite_sse(line: &str, model: &str) -> String {
    if !line.starts_with("data: ") { return line.to_string(); }
    let payload = line[6..].trim();
    if payload.is_empty() || payload == "[DONE]" { return line.to_string(); }
    match serde_json::from_str::<Value>(payload) {
        Ok(mut v) => {
            rewrite_model(&mut v, model);
            if let Some(text) = v.pointer("/delta/text").and_then(|t| t.as_str()) {
                if compatibility::detect_fake_tool_call(text) {
                    v["gateway_warning"] = json!("Possible fake tool call text without a tool_use block");
                }
            }
            format!("data: {}\n", serde_json::to_string(&v).unwrap_or_else(|_| payload.to_string()))
        }
        Err(_) => line.to_string(),
    }
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
    (StatusCode::UNAUTHORIZED, Json(json!({"error":{"type":"auth_error","message":msg}}))).into_response()
}
fn bad_req(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error":{"type":"invalid_request","message":msg.into()}}))).into_response()
}
fn internal<E: std::fmt::Display>(e: E) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":{"type":"internal","message":e.to_string()}}))).into_response()
}
fn upstream_err<E: std::fmt::Display>(e: E) -> Response {
    (StatusCode::BAD_GATEWAY, Json(json!({"error":{"type":"upstream","message":e.to_string()}}))).into_response()
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
                body.chars().take(500).collect()
            }
        });
    (StatusCode::BAD_GATEWAY, Json(json!({
        "error": {
            "type": "upstream",
            "message": message,
            "upstream_status": status.as_u16()
        }
    }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http, Router, routing::post};
    use tower::ServiceExt;
    use crate::{database, models::{CreateProvider, CreateModelRoute}};

    #[tokio::test]
    async fn test_list_models_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();
        database::create_provider(&db, &CreateProvider {
            id: "p1".into(), name: "P".into(), base_url: "http://x".into(),
            openai_base_url: None, anthropic_base_url: None,
            auth_header: "Authorization".into(), auth_scheme: Some("Bearer".into()), api_key: Some("k".into()),
        }).unwrap();
        database::create_route(&db, &CreateModelRoute {
            id: "r1".into(), claude_alias: "claude-sonnet-4-6".into(),
            display_name: "S".into(), provider_id: "p1".into(), upstream_model: "m".into(),
        }).unwrap();

        let app = build_router(Ctx {
            db, client: Client::new(),
            profile: GatewayProfile { listen_host: "127.0.0.1".into(), listen_port: 3456, auth_token: "tok".into() },
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
        assert_eq!(body["data"][0]["id"], "claude-sonnet-4-6");
    }

    #[tokio::test]
    async fn test_messages_model_rewrite() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();

        let upstream = Router::new().route("/v1/messages", post(|| async {
            Json(json!({"id":"m1","type":"message","role":"assistant","model":"real-model",
                "content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn",
                "usage":{"input_tokens":1,"output_tokens":1}}))
        }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap(); });

        database::create_provider(&db, &CreateProvider {
            id: "p1".into(), name: "P".into(), base_url: format!("http://{addr}"),
            openai_base_url: None, anthropic_base_url: None,
            auth_header: "Authorization".into(), auth_scheme: Some("Bearer".into()), api_key: Some("k".into()),
        }).unwrap();
        database::create_route(&db, &CreateModelRoute {
            id: "r1".into(), claude_alias: "claude-sonnet-4-6".into(),
            display_name: "S".into(), provider_id: "p1".into(), upstream_model: "m".into(),
        }).unwrap();

        let app = build_router(Ctx {
            db, client: Client::new(),
            profile: GatewayProfile { listen_host: "127.0.0.1".into(), listen_port: 3456, auth_token: "tok".into() },
        });

        let resp = app.clone().oneshot(
            http::Request::builder().method(http::Method::POST).uri("/v1/messages")
                .header("x-api-key", "tok")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"hi"}],"max_tokens":32}).to_string()))
                .unwrap()
        ).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["model"], "claude-sonnet-4-6");

        let resp = app.oneshot(
            http::Request::builder().method(http::Method::POST).uri("/v1/messages/v1/messages")
                .header("x-api-key", "tok")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"hi"}],"max_tokens":32}).to_string()))
                .unwrap()
        ).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_messages_falls_back_to_chat_completions() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();

        let upstream = Router::new()
            .route("/v1/messages", post(|| async {
                (StatusCode::NOT_FOUND, "not found")
            }))
            .route("/v1/chat/completions", post(|| async {
                Json(json!({
                    "id":"chatcmpl-1",
                    "object":"chat.completion",
                    "choices":[{
                        "index":0,
                        "message":{"role":"assistant","content":"hello from chat"},
                        "finish_reason":"stop"
                    }],
                    "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}
                }))
            }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap(); });

        database::create_provider(&db, &CreateProvider {
            id: "p1".into(), name: "P".into(), base_url: format!("http://{addr}"),
            openai_base_url: None, anthropic_base_url: None,
            auth_header: "Authorization".into(), auth_scheme: Some("Bearer".into()), api_key: Some("k".into()),
        }).unwrap();
        database::create_route(&db, &CreateModelRoute {
            id: "r1".into(), claude_alias: "claude-sonnet-4-6".into(),
            display_name: "S".into(), provider_id: "p1".into(), upstream_model: "m".into(),
        }).unwrap();

        let app = build_router(Ctx {
            db, client: Client::new(),
            profile: GatewayProfile { listen_host: "127.0.0.1".into(), listen_port: 3456, auth_token: "tok".into() },
        });

        let resp = app.oneshot(
            http::Request::builder().method(http::Method::POST).uri("/v1/messages")
                .header("x-api-key", "tok")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"hi"}],"max_tokens":32}).to_string()))
                .unwrap()
        ).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["content"][0]["text"], "hello from chat");
        assert_eq!(body["usage"]["output_tokens"], 3);
    }

    #[test]
    fn test_tool_use_conversion_between_anthropic_and_chat() {
        let req = anthropic_to_chat_request(&json!({
            "model":"claude-sonnet-4-6",
            "messages":[{
                "role":"assistant",
                "content":[{
                    "type":"tool_use",
                    "id":"toolu_1",
                    "name":"web_search",
                    "input":{"query":"Manchester United transfers"}
                }]
            },{
                "role":"user",
                "content":[{
                    "type":"tool_result",
                    "tool_use_id":"toolu_1",
                    "content":"Search result text"
                }]
            }],
            "tools":[{
                "name":"web_search",
                "description":"Search the web",
                "input_schema":{"type":"object","properties":{"query":{"type":"string"}}}
            }]
        }), "mimo-v2.5", false);

        assert_eq!(req["tools"][0]["function"]["name"], "web_search");
        assert_eq!(req["messages"][0]["tool_calls"][0]["function"]["name"], "web_search");
        assert_eq!(req["messages"][1]["role"], "tool");
        assert_eq!(req["messages"][1]["tool_call_id"], "toolu_1");

        let resp = chat_to_anthropic_message(&json!({
            "choices":[{
                "message":{
                    "role":"assistant",
                    "content":"",
                    "tool_calls":[{
                        "id":"call_1",
                        "type":"function",
                        "function":{
                            "name":"web_search",
                            "arguments":"{\"query\":\"Manchester United transfers\"}"
                        }
                    }]
                },
                "finish_reason":"tool_calls"
            }],
            "usage":{"prompt_tokens":10,"completion_tokens":4}
        }), "claude-sonnet-4-6");

        assert_eq!(resp["stop_reason"], "tool_use");
        assert_eq!(resp["content"][0]["type"], "tool_use");
        assert_eq!(resp["content"][0]["name"], "web_search");
        assert_eq!(resp["content"][0]["input"]["query"], "Manchester United transfers");
    }
}
