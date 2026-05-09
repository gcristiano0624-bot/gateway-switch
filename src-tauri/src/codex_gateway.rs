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
    database,
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
    let routes = database::list_codex_routes(&ctx.db).unwrap_or_default();
    let models: Vec<String> = routes.into_iter().filter(|r| r.enabled).map(|r| r.codex_model).collect();
    Json(json!({ "ok": true, "gateway": "codex", "listen": format!("{}:{}", ctx.profile.listen_host, ctx.profile.listen_port), "models": models }))
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

    // Forward to upstream provider's /v1/chat/completions
    let resp = ctx.client.post(format!("{}/v1/chat/completions", route.base_url))
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

    // Streaming: convert Chat Completions SSE → Responses API SSE
    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model = route.upstream_model.clone();
    let db = ctx.db.clone();
    let body_stream = resp.bytes_stream();

    let sse = stream! {
        // 1. Emit response.created
        let created_event = json!({
            "type": "response.created",
            "response": {
                "id": resp_id,
                "object": "response",
                "created_at": chrono::Utc::now().timestamp(),
                "model": display,
                "output": [],
                "status": "in_progress"
            }
        });
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
            }
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.output_item.added\ndata: {}\n\n", serde_json::to_string(&item_added).unwrap()
        )));

        // 3. Stream content deltas from Chat Completions format
        let mut full_text = String::new();
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
                                "type": "response.content_part.delta",
                                "output_index": 0,
                                "content_index": 0,
                                "delta": text,
                            });
                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                                "event: response.content_part.delta\ndata: {}\n\n",
                                serde_json::to_string(&delta_event).unwrap()
                            )));
                        }
                    }
                }
                Err(_) => break,
            }
        }

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
            }
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.output_item.done\ndata: {}\n\n", serde_json::to_string(&item_done).unwrap()
        )));

        // 5. Emit response.completed
        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": resp_id,
                "object": "response",
                "model": display,
                "output": [{
                    "type": "message",
                    "id": msg_id,
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": full_text,
                        "annotations": []
                    }],
                    "status": "completed"
                }],
                "status": "completed",
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 0
                }
            }
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.completed\ndata: {}\n\n", serde_json::to_string(&completed).unwrap()
        )));

        let _ = database::insert_log(&db, &RequestLog {
            request_id: Uuid::new_v4().to_string(), claude_alias: display,
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

    // 2. input array → messages
    if let Some(input) = body.get("input").and_then(|v| v.as_array()) {
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
    let content = chat["choices"][0]["message"]["content"].as_str().unwrap_or("");
    let input_tokens = chat["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = chat["usage"]["completion_tokens"].as_u64().unwrap_or(0);

    json!({
        "id": resp_id,
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": model,
        "output": [{
            "type": "message",
            "id": msg_id,
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": content,
                "annotations": []
            }],
            "status": "completed"
        }],
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        },
        "status": "completed"
    })
}

fn extract_chat_delta(line: &str) -> Option<String> {
    if !line.starts_with("data: ") { return None; }
    let payload = line[6..].trim();
    if payload.is_empty() || payload == "[DONE]" { return None; }
    let v: Value = serde_json::from_str(payload).ok()?;
    v["choices"][0]["delta"]["content"].as_str().map(String::from)
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
        upstream_model: route.upstream_model,
        base_url: provider.base_url.trim_end_matches('/').to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http, Router, routing::post};
    use tower::ServiceExt;
    use crate::{database, models::{CreateProvider, CreateModelRoute}};

    #[tokio::test]
    async fn test_codex_models_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();
        database::create_provider(&db, &CreateProvider {
            id: "p1".into(), name: "P".into(), base_url: "http://x".into(),
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
        assert_eq!(resp["status"], "completed");
    }
}
