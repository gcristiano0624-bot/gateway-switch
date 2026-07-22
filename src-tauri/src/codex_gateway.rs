use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

use async_stream::stream;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle, time::Duration};
use uuid::Uuid;

use crate::{
    compatibility, codex_history, codex_tools::CodexToolContext, database, gateway,
    gateway_diagnostics::{sanitize_payload_for_diagnostics, should_capture_diagnostic},
    gateway_strategy, loop_guard::{LoopGuard, TextGuardAction},
    models::{GatewayProfile, Provider, RequestDiagnosticSnapshot, RequestLog},
    state::{AppState, GatewayHandle, GatewayStatus},
};

#[derive(Clone)]
struct Ctx {
    db: PathBuf,
    client: Client,
    profile: GatewayProfile,
    history: Arc<codex_history::CodexChatHistoryStore>,
}

struct Route {
    display: String,
    provider_id: String,
    upstream_model: String,
    tool_call_mode: String,
    base_url: String,
    strategy: gateway::ProviderCompatibilityProfile,
    headers: Vec<(String, String)>,
    provider: Provider,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexRouteDiagnostic {
    pub route_id: String,
    pub codex_model: String,
    pub display_name: String,
    pub provider_id: String,
    pub provider_name: String,
    pub upstream_model: String,
    pub tool_call_mode: String,
    pub strategy: gateway::ProviderCompatibilityProfile,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

pub fn start(st: &AppState) -> Result<String, String> {
    {
        let mut g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
        if let Some(h) = g.as_ref() {
            let is_running = st
                .runtime
                .codex_gateway_status
                .lock()
                .map_err(|_| "lock")?
                .running;
            if is_running && !h._task.is_finished() {
                return Ok("already_running".into());
            }
        }
        if let Some(mut stale) = g.take() {
            if let Some(tx) = stale.shutdown.take() {
                let _ = tx.send(());
            }
            stale._task.abort();
        }
    }

    let profile = database::get_codex_profile(&st.db_path)?;
    let addr: SocketAddr = format!("{}:{}", profile.listen_host, profile.listen_port)
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;

    let ctx = Ctx {
        db: st.db_path.clone(),
        client: Client::new(),
        profile: profile.clone(),
        history: Arc::new(codex_history::CodexChatHistoryStore::default()),
    };
    let router = build_router(ctx);

    let (tx, rx) = oneshot::channel::<()>();
    let rt = Arc::clone(&st.runtime);
    {
        let mut s = rt.codex_gateway_status.lock().map_err(|_| "lock")?;
        *s = GatewayStatus {
            running: true,
            status: "starting".into(),
            error: None,
        };
    }

    let handle: JoinHandle<()> = tokio::spawn(async move {
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = true;
                    s.status = "running".into();
                    s.error = None;
                }
                let server = axum::serve(listener, router).with_graceful_shutdown(async {
                    let _ = rx.await;
                });
                if let Err(e) = server.await {
                    if let Ok(mut s) = rt.codex_gateway_status.lock() {
                        s.running = false;
                        s.status = "error".into();
                        s.error = Some(e.to_string());
                    }
                } else if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = false;
                    s.status = "stopped".into();
                }
            }
            Err(e) => {
                if let Ok(mut s) = rt.codex_gateway_status.lock() {
                    s.running = false;
                    s.status = "error".into();
                    s.error = Some(e.to_string());
                }
            }
        }
    });

    let mut g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
    *g = Some(GatewayHandle {
        shutdown: Some(tx),
        _task: handle,
    });
    Ok("started".into())
}

pub fn stop(st: &AppState) -> Result<String, String> {
    let mut g = st.runtime.codex_gateway_handle.lock().map_err(|_| "lock")?;
    if let Some(h) = g.as_mut() {
        if let Some(tx) = h.shutdown.take() {
            let _ = tx.send(());
        }
    } else {
        return Ok("not_running".into());
    }
    *g = None;
    let mut s = st.runtime.codex_gateway_status.lock().map_err(|_| "lock")?;
    *s = GatewayStatus {
        running: false,
        status: "stopped".into(),
        error: None,
    };
    Ok("stopped".into())
}

pub fn status(st: &AppState) -> Result<GatewayStatus, String> {
    st.runtime
        .codex_gateway_status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "lock".into())
}

const STREAM_TIMEOUT_SECS: u64 = 120;
const CODEX_REQUEST_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/responses", post(responses_handler))
        .layer(DefaultBodyLimit::max(CODEX_REQUEST_BODY_LIMIT_BYTES))
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
    let models: Vec<String> = routes
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| r.codex_model)
        .collect();
    Json(
        json!({ "ok": true, "gateway": "codex", "listen": format!("{}:{}", ctx.profile.listen_host, ctx.profile.listen_port), "models": models, "capabilities": capabilities }),
    )
}

async fn list_models(State(ctx): State<Ctx>, headers: HeaderMap) -> Result<Json<Value>, Response> {
    verify_auth(&headers, &ctx)?;
    let routes = database::list_codex_routes(&ctx.db).map_err(internal)?;
    let providers = database::list_providers(&ctx.db).unwrap_or_default();
    let data: Vec<Value> = routes
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| {
            let provider_name = providers
                .iter()
                .find(|p| p.id == r.provider_id)
                .map(|p| p.name.as_str())
                .unwrap_or("unknown");
            json!({
                "id": r.codex_model,
                "object": "model",
                "created": 1700000000,
                "owned_by": provider_name,
            })
        })
        .collect();
    Ok(Json(json!({ "object": "list", "data": data })))
}

async fn responses_handler(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response, Response> {
    verify_auth(&headers, &ctx)?;

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .ok_or(bad_req("missing model"))?;
    let route = resolve(&ctx.db, model).map_err(bad_req)?;
    let req_id = Uuid::new_v4().to_string();
    let resp_id = format!("resp_{}", Uuid::new_v4());
    let msg_id = format!("msg_{}", Uuid::new_v4());
    let started = Instant::now();
    let is_stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(true);

    let mut body = body;
    let _ = ctx.history.enrich_request(&mut body);

    // Build tool context from request for bidirectional tool type translation
    let tools = body
        .get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let input_items = body
        .get("input")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let tool_ctx = CodexToolContext::from_request(&tools, &input_items);

    // Convert Responses API request → Chat Completions request
    let body_ref = body.clone();
    let mut chat_req = convert_request(&body_ref, &route.upstream_model, &tool_ctx);
    apply_codex_tool_call_mode(&mut chat_req, &route);
    apply_xiaomi_mimo_codex_compat(&mut chat_req, &route);
    apply_codex_provider_policy(&mut chat_req, &route);
    apply_codex_reasoning_translation(&mut chat_req, &route);
    apply_codex_post_policy_cleanup(&mut chat_req, is_stream, &route);
    let request_tool_count = tool_count(&chat_req);
    let tool_choice = chat_req
        .get("tool_choice")
        .and_then(|v| v.as_str())
        .unwrap_or("none")
        .to_string();

    // Forward to upstream provider's Chat Completions endpoint.
    let resp = ctx
        .client
        .post(upstream_url(&route.base_url, "chat/completions"))
        .headers(to_headers(&route.headers)?)
        .json(&chat_req)
        .send()
        .await
        .map_err(|e| {
            let trace = tool_trace_summary(
                &route.tool_call_mode,
                &tool_choice,
                request_tool_count,
                0,
                None,
                false,
                Some(&e.to_string()),
            );
            record_codex_snapshot(
                &ctx.db,
                &req_id,
                &route,
                None,
                Some(&trace),
                &body_ref,
                &chat_req,
            );
            let _ = database::insert_log(
                &ctx.db,
                &RequestLog {
                    request_id: req_id.clone(),
                    claude_alias: route.display.clone(),
                    provider_id: route.provider_id.clone(),
                    upstream_model: route.upstream_model.clone(),
                    status_code: None,
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                    is_stream,
                    error_summary: Some(trace),
                    created_at: String::new(),
                },
            );
            upstream_err(e)
        })?;

    let status = resp.status();

    if !is_stream {
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let trace = tool_trace_summary(
                &route.tool_call_mode,
                &tool_choice,
                request_tool_count,
                0,
                None,
                false,
                Some(&text),
            );
            record_codex_snapshot(
                &ctx.db,
                &req_id,
                &route,
                Some(status.as_u16()),
                Some(&trace),
                &body_ref,
                &chat_req,
            );
            let _ = database::insert_log(
                &ctx.db,
                &RequestLog {
                    request_id: req_id,
                    claude_alias: route.display.clone(),
                    provider_id: route.provider_id.clone(),
                    upstream_model: route.upstream_model.clone(),
                    status_code: Some(status.as_u16()),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                    is_stream: false,
                    error_summary: Some(trace),
                    created_at: String::new(),
                },
            );
            return Err(upstream_status_err(status, text));
        }
        let chat_response = resp.json::<Value>().await.map_err(upstream_err)?;
        let response_tool_count = response_tool_call_count(&chat_response);
        let finish_reason = chat_response["choices"][0]["finish_reason"]
            .as_str()
            .map(String::from);
        let strict_violation = route.tool_call_mode == "strict_execution"
            && request_tool_count > 0
            && response_tool_count == 0;
        let trace = tool_trace_summary(
            &route.tool_call_mode,
            &tool_choice,
            request_tool_count,
            response_tool_count,
            finish_reason.as_deref(),
            strict_violation,
            None,
        );
        if let Err(err) = enforce_strict_tool_calls(&chat_response, &chat_req, &route) {
            let _ = database::insert_log(
                &ctx.db,
                &RequestLog {
                    request_id: req_id.clone(),
                    claude_alias: route.display.clone(),
                    provider_id: route.provider_id.clone(),
                    upstream_model: route.upstream_model.clone(),
                    status_code: Some(status.as_u16()),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                    is_stream: false,
                    error_summary: Some(trace),
                    created_at: String::new(),
                },
            );
            return Err(err);
        }
        let responses_response = convert_sync_response(
            &chat_response,
            &route.display,
            &resp_id,
            &msg_id,
            &tool_ctx,
        );
        if let Some(output) = responses_response.get("output").and_then(|v| v.as_array()) {
            ctx.history.record_response(&resp_id, output);
        }
        let _ = database::insert_log(
            &ctx.db,
            &RequestLog {
                request_id: req_id,
                claude_alias: route.display.clone(),
                provider_id: route.provider_id.clone(),
                upstream_model: route.upstream_model.clone(),
                status_code: Some(status.as_u16()),
                duration_ms: Some(started.elapsed().as_millis() as u64),
                is_stream: false,
                error_summary: Some(trace),
                created_at: String::new(),
            },
        );
        return Ok((status, Json(responses_response)).into_response());
    }

    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let trace = tool_trace_summary(
            &route.tool_call_mode,
            &tool_choice,
            request_tool_count,
            0,
            None,
            false,
            Some(&text),
        );
        record_codex_snapshot(
            &ctx.db,
            &req_id,
            &route,
            Some(status.as_u16()),
            Some(&trace),
            &body_ref,
            &chat_req,
        );
        let _ = database::insert_log(
            &ctx.db,
            &RequestLog {
                request_id: req_id,
                claude_alias: route.display.clone(),
                provider_id: route.provider_id.clone(),
                upstream_model: route.upstream_model.clone(),
                status_code: Some(status.as_u16()),
                duration_ms: Some(started.elapsed().as_millis() as u64),
                is_stream: true,
                error_summary: Some(trace),
                created_at: String::new(),
            },
        );
        return Err(upstream_status_err(status, text));
    }

    // Streaming: convert Chat Completions SSE → Responses API SSE
    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model_name = route.upstream_model.clone();
    let base_url = route.base_url.clone();
    let auth_headers = route.headers.clone();
    let log_req_id = req_id.clone();
    let db = ctx.db.clone();
    let client = ctx.client.clone();
    let history = ctx.history.clone();
    let tool_ctx = tool_ctx.clone();
    let body_stream = resp.bytes_stream();
    let has_tools_in_req = chat_req
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let strict_tool_calls = route.tool_call_mode == "strict_execution" && has_tools_in_req;
    let tool_call_mode = route.tool_call_mode.clone();
    let strategy_id = route.strategy.strategy_id.clone();

    let sse = stream! {
        let mut seq: i64 = 0;
        let timeout_dur = Duration::from_secs(STREAM_TIMEOUT_SECS);
        let reasoning_id = format!("{}_r", msg_id);

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

        // 2. Emit response.output_item.added for message
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
        let mut full_reasoning = String::new();
        let mut reasoning_started = false;
        let mut tool_items: std::collections::HashMap<i64, StreamingToolCall> = std::collections::HashMap::new();
        let mut next_output_index: i64 = 1;
        let mut finish_reason: Option<String> = None;
        let mut stream_error = false;
        let mut loop_guard = LoopGuard::default();

        // Process a single SSE stream with timeout and finish_reason tracking
        macro_rules! process_chat_stream {
            ($stream:expr) => {{
                let mut buf = String::new();
                let mut stream = Box::pin($stream);
                loop {
                    match tokio::time::timeout(timeout_dur, stream.next()).await {
                        Ok(Some(Ok(chunk))) => {
                            buf.push_str(&String::from_utf8_lossy(&chunk));
                            while let Some(pos) = buf.find('\n') {
                                let line = buf[..pos].to_string();
                                buf = buf[pos + 1..].to_string();
                                let (text, reasoning) = extract_chat_delta_with_reasoning(&line);

                                if let Some(reason_delta) = reasoning {
                                    if !reason_delta.is_empty() {
                                        if !reasoning_started {
                                            reasoning_started = true;
                                            let reasoning_item_added = json!({
                                                "type": "response.output_item.added",
                                                "output_index": next_output_index,
                                                "item": {
                                                    "type": "reasoning",
                                                    "id": reasoning_id,
                                                    "summary": [],
                                                    "status": "in_progress"
                                                },
                                                "sequence_number": seq
                                            });
                                            seq += 1;
                                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.output_item.added", &reasoning_item_added)));

                                            let reasoning_part_added = json!({
                                                "type": "response.content_part.added",
                                                "item_id": reasoning_id,
                                                "output_index": next_output_index,
                                                "content_index": 0,
                                                "part": {
                                                    "type": "summary_text",
                                                    "text": ""
                                                },
                                                "sequence_number": seq
                                            });
                                            seq += 1;
                                            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.content_part.added", &reasoning_part_added)));
                                            next_output_index += 1;
                                        }
                                        full_reasoning.push_str(&reason_delta);
                                        let reason_delta_event = json!({
                                            "type": "response.reasoning_summary_text.delta",
                                            "item_id": reasoning_id,
                                            "output_index": next_output_index - 1,
                                            "content_index": 0,
                                            "delta": reason_delta,
                                            "sequence_number": seq
                                        });
                                        seq += 1;
                                        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                                            "event: response.reasoning_summary_text.delta\ndata: {}\n\n",
                                            serde_json::to_string(&reason_delta_event).unwrap()
                                        )));
                                    }
                                }

                                if let Some(text) = text {
                                    let text = match loop_guard.observe_text(&text) {
                                        TextGuardAction::Pass(text) => text,
                                        TextGuardAction::Suppress => continue,
                                    };
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
                                if let Some(reason) = extract_finish_reason(&line) {
                                    finish_reason = Some(reason);
                                }
                                let tool_events = response_tool_delta_events(&line, &mut tool_items, &mut next_output_index, &mut seq);
                                for (event_name, event_body) in tool_events {
                                    yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event(event_name, &event_body)));
                                }
                            }
                        }
                        Ok(Some(Err(e))) => {
                            stream_error = true;
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
                        }
                        Ok(None) => break,
                        Err(_) => {
                            stream_error = true;
                            let text = "\n\n[Gateway: upstream stream timeout]";
                            full_text.push_str(text);
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
                        }
                    }
                }
            }};
        }

        // First attempt: process the initial upstream stream
        process_chat_stream!(body_stream);

        // Check if we should retry with tool_choice: "required"
        // This handles the case where the model described actions in text
        // but failed to emit structured tool_calls. Providers that loop under
        // forced `required` (MiMo, glm) are excluded so they can still produce
        // a final text answer instead of an empty tool-call spin.
        if should_retry_with_required(
            &strategy_id,
            &upstream_model_name,
            finish_reason.as_deref(),
            has_tools_in_req,
            tool_items.is_empty(),
            stream_error,
            &full_text,
        ) {
            {
                let mut retry_req = chat_req.clone();
                retry_req["tool_choice"] = json!("required");
                if let Some(msgs) = retry_req.get_mut("messages").and_then(|v| v.as_array_mut()) {
                    msgs.insert(0, json!({
                        "role": "system",
                        "content": "You MUST call the provided tools now. Do NOT describe planned actions in text. Emit tool_calls immediately."
                    }));
                }
                let retry_headers = to_headers(&auth_headers).unwrap_or_default();
                match client.post(upstream_url(&base_url, "chat/completions"))
                    .headers(retry_headers)
                    .json(&retry_req)
                    .send().await
                {
                    Ok(retry_resp) if retry_resp.status().is_success() => {
                        finish_reason = None;
                        let retry_stream = retry_resp.bytes_stream();
                        process_chat_stream!(retry_stream);
                    }
                    _ => {
                        full_text.push_str("\n\n[Gateway: tool-call retry failed]");
                    }
                }
            }
        }

        // 4. Emit reasoning done events if reasoning was present
        let reasoning_output_index = if reasoning_started {
            let idx = next_output_index - 1;
            let reason_text_done = json!({
                "type": "response.reasoning_summary_text.done",
                "item_id": reasoning_id,
                "output_index": idx,
                "content_index": 0,
                "text": full_reasoning,
                "sequence_number": seq
            });
            seq += 1;
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                "event: response.reasoning_summary_text.done\ndata: {}\n\n", serde_json::to_string(&reason_text_done).unwrap()
            )));

            let reason_part_done = json!({
                "type": "response.content_part.done",
                "item_id": reasoning_id,
                "output_index": idx,
                "content_index": 0,
                "part": {
                    "type": "summary_text",
                    "text": full_reasoning
                },
                "sequence_number": seq
            });
            seq += 1;
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
                "event: response.content_part.done\ndata: {}\n\n", serde_json::to_string(&reason_part_done).unwrap()
            )));
            Some(idx)
        } else {
            None
        };

        // 5. Emit text done and content part done
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

        // 6. Emit tool call events
        let mut tool_outputs = Vec::new();
        let mut sorted_tools: Vec<StreamingToolCall> = tool_items.into_values().collect();
        sorted_tools.sort_by_key(|t| t.output_index);
        let has_tool_calls = !sorted_tools.is_empty();
        for tool in sorted_tools {
            let arguments = compatibility::repair_json_object(&tool.arguments)
                .map(|v| v.to_string())
                .unwrap_or(tool.arguments);
            let _duplicate_tool_call = loop_guard.observe_tool_call(&tool.name, &arguments);

            let item = if let Some(restored) = tool_ctx.restore_response_item(&tool.name, &json!(arguments)) {
                let mut item = restored;
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("id".into(), json!(tool.item_id));
                    obj.insert("call_id".into(), json!(tool.call_id));
                    obj.insert("status".into(), json!("completed"));
                }
                item
            } else {
                json!({
                    "type": "function_call",
                    "id": tool.item_id,
                    "call_id": tool.call_id,
                    "name": tool.name,
                    "arguments": arguments,
                    "status": "completed"
                })
            };

            let args_done = json!({
                "type": "response.function_call_arguments.done",
                "item_id": tool.item_id,
                "output_index": tool.output_index,
                "arguments": arguments,
                "sequence_number": seq
            });
            seq += 1;
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.function_call_arguments.done", &args_done)));

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

        // 7. Emit reasoning item done if present
        let mut reasoning_output: Option<Value> = None;
        if let Some(r_idx) = reasoning_output_index {
            let reason_item = json!({
                "type": "reasoning",
                "id": reasoning_id,
                "summary": [{
                    "type": "summary_text",
                    "text": full_reasoning
                }],
                "status": "completed"
            });
            reasoning_output = Some(reason_item.clone());
            let reason_item_done = json!({
                "type": "response.output_item.done",
                "output_index": r_idx,
                "item": reason_item,
                "sequence_number": seq
            });
            seq += 1;
            yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(sse_event("response.output_item.done", &reason_item_done)));
        }

        // 8. Emit response.output_item.done for assistant message after tool calls
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

        // 9. Emit response.completed with proper status
        let reasoning_tokens = estimate_tokens(&full_reasoning);
        let output_tokens = estimate_tokens(&full_text) + reasoning_tokens;
        let mut usage = response_usage(0, output_tokens);
        if let Some(details) = usage.get_mut("output_tokens_details") {
            if let Some(obj) = details.as_object_mut() {
                obj.insert("reasoning_tokens".into(), json!(reasoning_tokens));
            }
        }
        let mut output = Vec::new();
        if let Some(reason) = reasoning_output {
            output.push(reason);
        }
        output.push(json!({
            "type": "message",
            "id": msg_id,
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": full_text,
                "annotations": []
            }],
            "status": "completed"
        }));
        let response_tool_count = tool_outputs.len();
        output.extend(tool_outputs);

        let strict_tool_error = strict_tool_calls && !has_tool_calls;
        let resp_status = if stream_error || strict_tool_error {
            "failed"
        } else if finish_reason.as_deref() == Some("length") && !has_tool_calls {
            "incomplete"
        } else {
            "completed"
        };

        let mut resp_body = json!({
            "id": resp_id,
            "object": "response",
            "created_at": chrono::Utc::now().timestamp(),
            "model": display,
            "output": output,
            "status": resp_status,
            "usage": usage
        });
        if resp_status == "incomplete" {
            resp_body["incomplete_details"] = json!({"reason": "max_output_tokens"});
        }
        if strict_tool_error {
            resp_body["error"] = json!({
                "code": "tool_call_required",
                "message": "Upstream model did not emit tool_calls while Codex strict execution mode was enabled."
            });
        }

        if resp_status == "completed" {
            history.record_response(&resp_id, &output);
        }

        let completed = json!({
            "type": "response.completed",
            "response": resp_body,
            "sequence_number": seq
        });
        yield Ok::<Bytes, std::convert::Infallible>(Bytes::from(format!(
            "event: response.completed\ndata: {}\n\n", serde_json::to_string(&completed).unwrap()
        )));

        let mut trace_error: Option<String> = None;
        if stream_error {
            trace_error = Some("stream_error".into());
        } else if strict_tool_error {
            trace_error = Some("strict_tool_call_missing".into());
        }
        let mut error_summary_text = tool_trace_summary(
            &tool_call_mode,
            &tool_choice,
            request_tool_count,
            response_tool_count,
            finish_reason.as_deref(),
            strict_tool_error,
            trace_error.as_deref(),
        );
        if let Some(loop_summary) = loop_guard.summary().to_log_summary() {
            error_summary_text.push_str("; ");
            error_summary_text.push_str(&loop_summary);
        }
        let error_summary = Some(error_summary_text);
        let _ = database::insert_log(&db, &RequestLog {
            request_id: log_req_id, claude_alias: display,
            provider_id, upstream_model: upstream_model_name, status_code: Some(status.as_u16()),
            duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary, created_at: String::new(),
        });
    };

    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-cache");
    builder.body(Body::from_stream(sse)).map_err(internal)
}

// ── Request/Response Conversion ──

fn tools_array_from_body(body: &Value) -> Vec<Value> {
    body.get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn has_action_description(text: &str) -> bool {
    let patterns = [
        "我来",
        "我将",
        "让我",
        "接下来",
        "现在我",
        "我需要",
        "我应该",
        "我去",
        "我要",
        "先让我",
        "先我",
        "我先",
        "I'll ",
        "I'll\n",
        "Let me ",
        "I will ",
        "Now I'll ",
        "I'm going to ",
        "I need to ",
        "I should ",
        "Going to ",
        "I'll now ",
        "I'll first ",
        "Let's ",
    ];
    let lower = text.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

/// Decide whether to re-issue a streaming request with `tool_choice="required"`
/// after the model produced only an action description but no structured
/// tool_calls. Some providers loop forever under forced `required` and never
/// emit final text (observed with Xiaomi MiMo and Volcengine glm), so those are
/// excluded. Pure function to keep it unit-testable.
fn should_retry_with_required(
    strategy_id: &str,
    upstream_model: &str,
    finish_reason: Option<&str>,
    has_tools: bool,
    tool_items_empty: bool,
    stream_error: bool,
    text: &str,
) -> bool {
    if !has_tools || !tool_items_empty || stream_error {
        return false;
    }
    // Providers prone to infinite tool-call loops under forced `required`.
    let model = upstream_model.to_lowercase();
    let loops_on_required = matches!(strategy_id, "xiaomi_mimo_chat")
        || model.contains("mimo")
        || model.contains("glm");
    if loops_on_required {
        return false;
    }
    match finish_reason {
        Some("stop") | Some("length") | None => has_action_description(text),
        _ => false,
    }
}

fn extract_finish_reason(line: &str) -> Option<String> {
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    v["choices"][0]["finish_reason"]
        .as_str()
        .or_else(|| v["choices"][0]["delta"]["finish_reason"].as_str())
        .map(String::from)
}

fn convert_request(body: &Value, upstream_model: &str, tool_ctx: &CodexToolContext) -> Value {
    let mut messages: Vec<Value> = Vec::new();

    // 1. instructions → system message
    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            messages.push(json!({"role": "system", "content": instructions}));
        }
    }

    let has_tools = !tool_ctx.is_empty();
    if has_tools {
        let tool_names: Vec<String> = tool_ctx
            .specs
            .iter()
            .map(|s| s.original_name.clone())
            .collect();
        let tool_list = if tool_names.is_empty() {
            String::from("the provided tools")
        } else {
            format!("[{}]", tool_names.join(", "))
        };
        messages.push(json!({
            "role": "system",
            "content": format!(
                "CRITICAL: You have access to tools: {tool_list}. \
                 When you need to perform any action — read a file, run a command, edit code, \
                 search files, check project state, write content, or interact with the system — \
                 you MUST emit structured tool_calls. \
                 NEVER say 'I will ...', 'Let me ...', 'I need to ...' without also emitting \
                 the corresponding tool_call in the same response. \
                 Text descriptions of planned actions without tool_calls are ALWAYS wrong. \
                 Call the tool first, then explain what you did afterward."
            )
        }));
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
                    let role = normalize_chat_role(
                        item.get("role").and_then(|v| v.as_str()).unwrap_or("user"),
                    );
                    let content = extract_content_from_item(item);
                    messages.push(json!({"role": role, "content": content}));
                }
                Some("function_call_output") => {
                    let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                    let output = item.get("output").and_then(|v| v.as_str()).unwrap_or("");
                    messages
                        .push(json!({"role": "tool", "tool_call_id": call_id, "content": output}));
                }
                Some("function_call") => {
                    // Convert function_call to assistant message with tool_calls
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                    let arguments = item
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    messages.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [{
                            "id": call_id,
                            "type": "function",
                            "function": {"name": name, "arguments": arguments}
                        }]
                    }));
                }
                _ => {
                    // Try to extract as a simple message
                    let role = normalize_chat_role(
                        item.get("role").and_then(|v| v.as_str()).unwrap_or("user"),
                    );
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

    // Pass tools if present — use CodexToolContext to downgrade all tool types to function
    if !tool_ctx.is_empty() {
        let converted_tools = tool_ctx.downgrade_all_to_functions();
        if !converted_tools.is_empty() {
            chat_req["tools"] = json!(converted_tools);
            if body.get("tool_choice").is_none() {
                chat_req["tool_choice"] = json!("auto");
            }
        }
    }

    // Pass temperature
    if let Some(temp) = body.get("temperature") {
        chat_req["temperature"] = temp.clone();
    }

    // Pass max_output_tokens; provider-specific compatibility may rename it later.
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

    // Provider-specific clients may pass through thinking controls.
    if let Some(thinking) = body.get("thinking") {
        chat_req["thinking"] = thinking.clone();
    }

    chat_req
}

fn apply_codex_tool_call_mode(chat_req: &mut Value, route: &Route) {
    let has_tools = chat_req
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_tools {
        return;
    }

    if route.strategy.codex_strict_tool_calls {
        chat_req["tool_choice"] = json!("required");
        if let Some(messages) = chat_req.get_mut("messages").and_then(|v| v.as_array_mut()) {
            messages.insert(0, json!({
                "role": "system",
                "content": "Codex compatibility policy requires structured tool_calls whenever tools are available. Call tools instead of describing planned actions."
            }));
        }
        return;
    }

    match route.tool_call_mode.as_str() {
        "strict_execution" => {
            chat_req["tool_choice"] = json!("required");
            if let Some(messages) = chat_req.get_mut("messages").and_then(|v| v.as_array_mut()) {
                messages.insert(0, json!({
                    "role": "system",
                    "content": "Codex execution mode is active. You MUST emit structured tool_calls whenever tools are available. Do not answer with a plan before calling a tool."
                }));
            }
        }
        "force_when_tools_present" => {
            if let Some(messages) = chat_req.get_mut("messages").and_then(|v| v.as_array_mut()) {
                messages.insert(0, json!({
                    "role": "system",
                    "content": "Codex execution mode is active. Prefer structured tool_calls when an action is needed, but answer normally when no tool is required."
                }));
            }
        }
        _ => {}
    }
}

fn apply_codex_provider_policy(chat_req: &mut Value, route: &Route) {
    if route.strategy.strip_unsupported_params {
        if let Some(obj) = chat_req.as_object_mut() {
            obj.remove("thinking");
            obj.remove("reasoning");
            obj.remove("reasoning_effort");
        }
    }
    if route.strategy.disable_tools {
        if let Some(obj) = chat_req.as_object_mut() {
            obj.remove("tools");
            obj.remove("tool_choice");
        }
    }
}

fn apply_codex_reasoning_translation(chat_req: &mut Value, route: &Route) {
    if route.strategy.codex_strip_reasoning {
        if let Some(obj) = chat_req.as_object_mut() {
            obj.remove("thinking");
            obj.remove("reasoning");
            obj.remove("reasoning_effort");
        }
        return;
    }

    let cfg = gateway_strategy::infer_codex_chat_reasoning_config(&route.provider, &route.upstream_model);

    if cfg.thinking_param == gateway_strategy::CodexThinkingParam::None {
        return;
    }

    let original_effort = chat_req
        .get("reasoning_effort")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            chat_req
                .get("reasoning")
                .and_then(|r| r.get("effort"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    if let Some(obj) = chat_req.as_object_mut() {
        obj.remove("reasoning");
        obj.remove("reasoning_effort");
    }

    let is_enabled = original_effort.as_deref().map_or(false, |e| {
        !matches!(e, "none" | "off" | "disabled" | "minimal")
    });

    let effort = original_effort.as_deref().map(|e| {
        if let Some(max_override) = cfg.max_effort_override {
            if e == "max" {
                return max_override.to_string();
            }
        }
        e.to_string()
    });

    match cfg.thinking_param {
        gateway_strategy::CodexThinkingParam::Thinking => {
            if is_enabled {
                if let Some(obj) = chat_req.as_object_mut() {
                    obj.insert("thinking".into(), json!({ "type": "enabled" }));
                }
            }
        }
        gateway_strategy::CodexThinkingParam::EnableThinking => {
            if is_enabled {
                if let Some(obj) = chat_req.as_object_mut() {
                    obj.insert("enable_thinking".into(), json!(true));
                }
            }
        }
        gateway_strategy::CodexThinkingParam::ReasoningSplit => {
            if is_enabled {
                if let Some(obj) = chat_req.as_object_mut() {
                    obj.insert("reasoning_split".into(), json!(true));
                }
            }
        }
        gateway_strategy::CodexThinkingParam::ReasoningDotEffort => {
            if is_enabled {
                let eff = effort.unwrap_or_else(|| "medium".to_string());
                if let Some(obj) = chat_req.as_object_mut() {
                    obj.insert("reasoning".into(), json!({ "effort": eff }));
                }
            } else if original_effort.is_some() {
                if let Some(obj) = chat_req.as_object_mut() {
                    obj.insert("reasoning".into(), json!({ "effort": "none" }));
                }
            }
        }
        gateway_strategy::CodexThinkingParam::None => {}
    }
}

fn apply_codex_post_policy_cleanup(chat_req: &mut Value, is_stream: bool, route: &Route) {
    let has_tools = chat_req
        .get("tools")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    if !has_tools {
        if let Some(obj) = chat_req.as_object_mut() {
            obj.remove("tool_choice");
            obj.remove("parallel_tool_calls");
        }
    }

    if is_stream && !route.strategy.strip_unsupported_params {
        if let Some(obj) = chat_req.as_object_mut() {
            let stream_options = obj
                .get_mut("stream_options")
                .and_then(|v| v.as_object_mut());
            match stream_options {
                Some(so_obj) => {
                    so_obj.insert("include_usage".into(), json!(true));
                }
                None => {
                    obj.insert("stream_options".into(), json!({"include_usage": true}));
                }
            }
        }
    }
}

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

fn extract_reasoning_from_message(message: &Value) -> Option<String> {
    let raw = message
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(String::from);
    let raw = raw.or_else(|| {
        message
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(String::from)
    });
    raw.and_then(|t| {
        let trimmed = t.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn extract_reasoning_from_delta(delta: &Value) -> Option<String> {
    let raw = delta
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(String::from);
    let raw = raw.or_else(|| {
        delta
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(String::from)
    });
    raw.and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

fn tool_count(chat_req: &Value) -> usize {
    chat_req
        .get("tools")
        .and_then(|v| v.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn response_tool_call_count(chat_response: &Value) -> usize {
    chat_response["choices"][0]["message"]
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn tool_trace_summary(
    mode: &str,
    tool_choice: &str,
    request_tool_count: usize,
    response_tool_call_count: usize,
    finish_reason: Option<&str>,
    strict_violation: bool,
    error: Option<&str>,
) -> String {
    let mut trace = json!({
        "type": "tool_trace",
        "mode": normalize_tool_call_mode(mode),
        "tool_choice": tool_choice,
        "request_tools": request_tool_count,
        "response_tool_calls": response_tool_call_count,
        "finish_reason": finish_reason.unwrap_or("unknown"),
        "forced_required": tool_choice == "required",
        "strict_violation": strict_violation,
    });
    if let Some(error) = error {
        trace["error"] = json!(error.chars().take(500).collect::<String>());
    }
    format!(
        "tool_trace: {}",
        serde_json::to_string(&trace).unwrap_or_else(|_| "{}".into())
    )
}

// #region debug-point codex-route-failures
fn record_codex_snapshot(
    db: &PathBuf,
    request_id: &str,
    route: &Route,
    status_code: Option<u16>,
    error_summary: Option<&str>,
    original_payload: &Value,
    converted_payload: &Value,
) {
    if !should_capture_diagnostic(status_code, error_summary) {
        return;
    }
    let (sanitized_original, original_count) = sanitize_payload_for_diagnostics(original_payload);
    let (sanitized_converted, converted_count) =
        sanitize_payload_for_diagnostics(converted_payload);
    let redactions = original_count + converted_count;
    let snapshot = RequestDiagnosticSnapshot {
        request_id: request_id.to_string(),
        surface: "codex_responses".into(),
        claude_alias: Some(route.display.clone()),
        provider_id: Some(route.provider_id.clone()),
        upstream_model: Some(route.upstream_model.clone()),
        status_code,
        error_summary: error_summary.map(compatibility::redact_log_summary),
        original_payload_json: serde_json::to_string(&sanitized_original)
            .unwrap_or_else(|_| "{}".into()),
        converted_payload_json: Some(
            serde_json::to_string(&sanitized_converted).unwrap_or_else(|_| "{}".into()),
        ),
        redaction_summary: format!(
            "{redactions} field(s) redacted or truncated; replay preview is local-only."
        ),
        created_at: None,
    };
    let _ = database::insert_request_snapshot(db, &snapshot);
}
// #endregion debug-point codex-route-failures

fn enforce_strict_tool_calls(
    chat_response: &Value,
    chat_req: &Value,
    route: &Route,
) -> Result<(), Response> {
    let has_tools = chat_req
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if route.tool_call_mode != "strict_execution" || !has_tools {
        return Ok(());
    }

    let has_tool_calls = chat_response["choices"][0]["message"]
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if has_tool_calls {
        Ok(())
    } else {
        Err(upstream_err(
            "Upstream model did not emit tool_calls while Codex strict execution mode was enabled.",
        ))
    }
}

fn normalize_chat_role(role: &str) -> &str {
    match role {
        "developer" => "system",
        "system" | "assistant" | "user" | "tool" => role,
        _ => "user",
    }
}

fn apply_xiaomi_mimo_codex_compat(chat_req: &mut Value, route: &Route) {
    if !is_xiaomi_mimo_route(route) {
        return;
    }

    if chat_req.get("thinking").is_none() {
        chat_req["thinking"] = json!({ "type": "disabled" });
    }

    if let Some(max_tokens) = chat_req.get("max_tokens").cloned() {
        if chat_req.get("max_completion_tokens").is_none() {
            chat_req["max_completion_tokens"] = max_tokens;
        }
        if let Some(obj) = chat_req.as_object_mut() {
            obj.remove("max_tokens");
        }
    }
}

fn is_xiaomi_mimo_route(route: &Route) -> bool {
    let provider = route.provider_id.to_lowercase();
    let model = route.upstream_model.to_lowercase();
    let base = route.base_url.to_lowercase();
    provider.contains("xiaomi")
        || provider.contains("mimo")
        || model.contains("mimo")
        || base.contains("xiaomimimo.com")
}

fn extract_content_from_item(item: &Value) -> String {
    // Try "content" field first (string)
    if let Some(s) = item.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    // Try "content" field (array of content parts)
    if let Some(arr) = item.get("content").and_then(|v| v.as_array()) {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|part| {
                if part.get("type").and_then(|v| v.as_str()) == Some("input_text") {
                    part.get("text").and_then(|v| v.as_str()).map(String::from)
                } else if part.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                    part.get("text").and_then(|v| v.as_str()).map(String::from)
                } else {
                    part.get("text").and_then(|v| v.as_str()).map(String::from)
                }
            })
            .collect();
        return parts.join("");
    }
    String::new()
}

fn convert_sync_response(
    chat: &Value,
    model: &str,
    resp_id: &str,
    msg_id: &str,
    tool_ctx: &CodexToolContext,
) -> Value {
    let message = &chat["choices"][0]["message"];
    let mut content = extract_chat_message_text(message).unwrap_or_default();
    let mut reasoning_text = extract_reasoning_from_message(message);

    if let (Some(think), after) = split_leading_think_block(&content) {
        if reasoning_text.is_none() {
            reasoning_text = Some(think);
        }
        content = after;
    }

    let tool_calls = chat_tool_calls_to_response_items(message, tool_ctx);
    let input_tokens = chat["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let reasoning_tokens = reasoning_text.as_ref().map(|t| estimate_tokens(t)).unwrap_or(0);
    let output_tokens = chat["usage"]["completion_tokens"]
        .as_u64()
        .unwrap_or_else(|| estimate_tokens(&content) + reasoning_tokens);
    let total_tokens = chat["usage"]["total_tokens"]
        .as_u64()
        .unwrap_or(input_tokens + output_tokens);
    let usage = response_usage(input_tokens, output_tokens)
        .as_object()
        .cloned()
        .map(|mut usage| {
            usage.insert("total_tokens".into(), json!(total_tokens));
            if let Some(details) = usage.get_mut("output_tokens_details") {
                if let Some(obj) = details.as_object_mut() {
                    obj.insert("reasoning_tokens".into(), json!(reasoning_tokens));
                }
            }
            Value::Object(usage)
        })
        .unwrap_or_else(|| {
            let mut u = response_usage(input_tokens, output_tokens);
            if let Some(details) = u.get_mut("output_tokens_details") {
                if let Some(obj) = details.as_object_mut() {
                    obj.insert("reasoning_tokens".into(), json!(reasoning_tokens));
                }
            }
            u
        });
    let mut output = Vec::new();

    if let Some(think) = reasoning_text {
        let reasoning_id = format!("{}_r", msg_id);
        output.push(json!({
            "type": "reasoning",
            "id": reasoning_id,
            "summary": [{
                "type": "summary_text",
                "text": think
            }],
            "status": "completed"
        }));
    }

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

fn chat_tool_calls_to_response_items(message: &Value, tool_ctx: &CodexToolContext) -> Vec<Value> {
    message
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let function = call.get("function")?;
                    let name = function.get("name").and_then(|v| v.as_str())?;
                    let arguments = function
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    let arguments = compatibility::repair_json_object(arguments)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|_| arguments.to_string());
                    let call_id = call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| "call_unknown");

                    if let Some(restored) = tool_ctx.restore_response_item(name, &json!(arguments)) {
                        let mut item = restored;
                        if let Some(obj) = item.as_object_mut() {
                            obj.insert("id".into(), json!(format!("fc_{}", Uuid::new_v4())));
                            obj.insert("call_id".into(), json!(call_id));
                            obj.insert("status".into(), json!("completed"));
                        }
                        Some(item)
                    } else {
                        Some(json!({
                            "type": "function_call",
                            "id": format!("fc_{}", Uuid::new_v4()),
                            "call_id": call_id,
                            "name": name,
                            "arguments": arguments,
                            "status": "completed"
                        }))
                    }
                })
                .collect()
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
    let (text, _reasoning) = extract_chat_delta_with_reasoning(line);
    text
}

fn extract_chat_delta_with_reasoning(line: &str) -> (Option<String>, Option<String>) {
    if !line.starts_with("data:") {
        return (None, None);
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return (None, None);
    }
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return (None, None);
    };
    if let Some(message) = v
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .filter(|s| !s.is_empty())
    {
        return (Some(format!("\n\n[Upstream error: {message}]")), None);
    }

    let delta = &v["choices"][0]["delta"];
    let text = extract_text_from_delta(delta).or_else(|| {
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(String::from)
    });
    let reasoning = extract_reasoning_from_delta(delta);
    (text, reasoning)
}

fn extract_text_from_delta(delta: &Value) -> Option<String> {
    // Only look at user-facing text fields. `reasoning_content` is the model's
    // internal monologue (e.g. "User is just saying hello. No tools needed.")
    // and must NOT be exposed as the assistant's text reply — that would leak
    // private chain-of-thought into the Codex App chat UI.
    for key in ["content", "text"] {
        if let Some(text) = delta
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(text.to_string());
        }
    }

    delta
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
        .filter(|text| !text.is_empty())
}

fn extract_chat_message_text(message: &Value) -> Option<String> {
    // See extract_text_from_delta: do not fall back to `reasoning_content`.
    for key in ["content", "text"] {
        if let Some(text) = message
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(text.to_string());
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
    if !line.starts_with("data:") {
        return Vec::new();
    }
    let payload = line[5..].trim();
    if payload.is_empty() || payload == "[DONE]" {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return Vec::new();
    };
    let Some(calls) = v["choices"][0]["delta"]["tool_calls"].as_array() else {
        return Vec::new();
    };
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

        if !tool_items.contains_key(&openai_index) {
            let output_index = *next_output_index;
            *next_output_index += 1;
            let item = StreamingToolCall {
                output_index,
                item_id: format!("fc_{}", Uuid::new_v4()),
                call_id: if id_delta.is_empty() {
                    format!("call_{}", Uuid::new_v4())
                } else {
                    id_delta.to_string()
                },
                name: if name_delta.is_empty() {
                    "function".into()
                } else {
                    name_delta.to_string()
                },
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
            if !id_delta.is_empty() {
                item.call_id = id_delta.to_string();
            }
            if !name_delta.is_empty() {
                item.name = name_delta.to_string();
            }
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
    format!(
        "event: {event_name}\ndata: {}\n\n",
        serde_json::to_string(body).unwrap()
    )
}

// ── Shared utilities ──

fn resolve(db: &PathBuf, model: &str) -> Result<Route, String> {
    let routes = database::list_codex_routes(db)?;
    let providers = database::list_providers(db)?;
    let route = routes
        .into_iter()
        .find(|r| r.enabled && r.codex_model == model)
        .ok_or_else(|| format!("Unknown model: {model}"))?;
    let provider = providers
        .into_iter()
        .find(|p| p.enabled && p.id == route.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", route.provider_id))?;
    let strategy =
        gateway::effective_provider_compatibility_profile(db, &provider, &route.upstream_model);
    let tool_call_mode =
        effective_codex_tool_call_mode(&strategy, Some(route.tool_call_mode.as_str()));
    Ok(Route {
        display: route.codex_model,
        provider_id: provider.id.clone(),
        upstream_model: route.upstream_model.trim().to_string(),
        tool_call_mode,
        base_url: provider.openai_base_url.trim_end_matches('/').to_string(),
        strategy,
        headers: auth_headers(&provider),
        provider,
    })
}

pub fn route_diagnostics(db: &PathBuf) -> Result<Vec<CodexRouteDiagnostic>, String> {
    let routes = database::list_codex_routes(db)?;
    let providers = database::list_providers(db)?;
    Ok(routes
        .into_iter()
        .filter_map(|route| {
            let provider = providers.iter().find(|p| p.id == route.provider_id)?;
            let strategy =
                gateway::effective_provider_compatibility_profile(db, provider, &route.upstream_model);
            let effective_mode =
                effective_codex_tool_call_mode(&strategy, Some(route.tool_call_mode.as_str()));
            let mut warnings = Vec::new();
            let mut recommendations = Vec::new();
            if strategy.codex_disable_responses {
                warnings.push("Codex Responses API is converted through Chat Completions fallback for this provider.".into());
            }
            if strategy.codex_strip_reasoning {
                warnings.push("Reasoning/thinking parameters are stripped for provider compatibility.".into());
            }
            if strategy.codex_strict_tool_calls {
                warnings.push("Strict tool-call policy is enforced when tools are present.".into());
            }
            if strategy.gateway_route_recommended {
                recommendations.push("Keep Codex Gateway running before using this route.".into());
            }
            Some(CodexRouteDiagnostic {
                route_id: route.id,
                codex_model: route.codex_model,
                display_name: route.display_name,
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                upstream_model: route.upstream_model,
                tool_call_mode: effective_mode,
                strategy,
                warnings,
                recommendations,
            })
        })
        .collect())
}

fn normalize_tool_call_mode(mode: &str) -> &'static str {
    match mode.trim() {
        "auto" => "auto",
        "strict_execution" => "strict_execution",
        _ => "force_when_tools_present",
    }
}

fn effective_codex_tool_call_mode(
    strategy: &gateway::ProviderCompatibilityProfile,
    configured_mode: Option<&str>,
) -> String {
    if strategy.codex_strict_tool_calls {
        return "strict_execution".into();
    }
    let configured = configured_mode.unwrap_or("force_when_tools_present");
    let normalized = normalize_tool_call_mode(configured);
    if strategy.strategy_id == "xiaomi_mimo_chat" && normalized == "strict_execution" {
        "auto".into()
    } else {
        normalized.into()
    }
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
        if v.to_str().unwrap_or("") == ctx.profile.auth_token {
            return Ok(());
        }
        return Err(auth_err("Invalid token"));
    }
    if let Some(v) = headers.get(header::AUTHORIZATION) {
        let s = v.to_str().unwrap_or("");
        if s == format!("Bearer {}", ctx.profile.auth_token) {
            return Ok(());
        }
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
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":{"message":msg,"type":"invalid_request_error","code":"unauthorized"}})),
    )
        .into_response()
}
fn bad_req(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":{"message":msg.into(),"type":"invalid_request_error"}})),
    )
        .into_response()
}
fn internal<E: std::fmt::Display>(e: E) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error":{"message":e.to_string(),"type":"internal_error"}})),
    )
        .into_response()
}
fn upstream_err<E: std::fmt::Display>(e: E) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({"error":{"message":e.to_string(),"type":"upstream_error"}})),
    )
        .into_response()
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
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({"error":{"message":message,"type":"upstream_error","status":status.as_u16()}})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database, models::CreateProvider};
    use axum::{body::to_bytes, http};
    use tower::ServiceExt;

    #[test]
    fn retry_with_required_skipped_for_mimo_and_glm() {
        // Generic provider that described an action but emitted no tool_calls: retry.
        assert!(should_retry_with_required(
            "openai_chat_fallback",
            "some-model",
            Some("stop"),
            true,
            true,
            false,
            "I'll run the tests now.",
        ));
        // MiMo (by strategy) loops on forced required: never retry.
        assert!(!should_retry_with_required(
            "xiaomi_mimo_chat",
            "mimo-v2.5-pro",
            Some("stop"),
            true,
            true,
            false,
            "I'll run the tests now.",
        ));
        // glm (by upstream model name under Volcengine) also excluded.
        assert!(!should_retry_with_required(
            "volcengine_deepseek_coding",
            "glm-latest",
            Some("stop"),
            true,
            true,
            false,
            "Let me start.",
        ));
        // No tools, or tool items already present, or stream error: never retry.
        assert!(!should_retry_with_required(
            "openai_chat_fallback", "m", Some("stop"), false, true, false, "I'll go."
        ));
        assert!(!should_retry_with_required(
            "openai_chat_fallback", "m", Some("stop"), true, false, false, "I'll go."
        ));
        assert!(!should_retry_with_required(
            "openai_chat_fallback", "m", Some("stop"), true, true, true, "I'll go."
        ));
    }

    fn standard_strategy() -> gateway::ProviderCompatibilityProfile {
        gateway::ProviderCompatibilityProfile {
            strategy_id: "openai_chat_fallback".into(),
            system_to_user: false,
            tool_to_user: false,
            disable_tools: false,
            strip_unsupported_params: false,
            direct_provider_safe: false,
            gateway_route_recommended: true,
            codex_disable_responses: true,
            codex_strict_tool_calls: false,
            codex_strip_reasoning: false,
            summary: "test".into(),
        }
    }

    fn test_provider(id: &str, name: &str, base_url: &str) -> Provider {
        Provider {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            openai_base_url: base_url.into(),
            anthropic_base_url: None,
            auth_header: "Authorization".into(),
            auth_scheme: Some("Bearer".into()),
            api_key: Some("test-key".into()),
            enabled: true,
            priority: 0,
        }
    }

    #[test]
    fn test_xiaomi_codex_strategy_downgrades_configured_strict_mode() {
        let mut strategy = standard_strategy();
        strategy.strategy_id = "xiaomi_mimo_chat".into();

        assert_eq!(
            effective_codex_tool_call_mode(&strategy, Some("strict_execution")),
            "auto"
        );

        strategy.codex_strict_tool_calls = true;
        assert_eq!(
            effective_codex_tool_call_mode(&strategy, Some("auto")),
            "strict_execution"
        );
    }

    #[test]
    fn test_codex_route_diagnostics_use_effective_provider_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();
        database::create_provider(
            &db,
            &CreateProvider {
                id: "openrouter".into(),
                name: "OpenRouter".into(),
                base_url: "https://openrouter.ai/api/v1".into(),
                openai_base_url: Some("https://openrouter.ai/api/v1".into()),
                anthropic_base_url: None,
                auth_header: "Authorization".into(),
                auth_scheme: Some("Bearer".into()),
                api_key: Some("k".into()),
            },
        )
        .unwrap();
        database::create_codex_route(
            &db,
            &crate::models::CreateCodexRoute {
                id: "r1".into(),
                codex_model: "codex-openrouter".into(),
                display_name: "Codex OpenRouter".into(),
                provider_id: "openrouter".into(),
                upstream_model: "anthropic/claude-sonnet".into(),
                tool_call_mode: Some("auto".into()),
            },
        )
        .unwrap();
        database::upsert_provider_policy(
            &db,
            &crate::models::ProviderCompatibilityPolicy {
                provider_id: "openrouter".into(),
                system_to_user: None,
                tool_to_user: None,
                disable_tools: None,
                strip_unsupported_params: None,
                direct_provider_safe: None,
                gateway_route_recommended: None,
                codex_disable_responses: Some(true),
                codex_strict_tool_calls: Some(true),
                codex_strip_reasoning: Some(true),
                notes: Some("codex policy test".into()),
                updated_by: "test".into(),
                updated_at: None,
            },
        )
        .unwrap();

        let diagnostics = route_diagnostics(&db).unwrap();
        assert_eq!(diagnostics.len(), 1);
        let diag = &diagnostics[0];
        assert_eq!(diag.tool_call_mode, "strict_execution");
        assert!(diag.strategy.codex_strict_tool_calls);
        assert!(diag.strategy.codex_strip_reasoning);
        assert!(diag
            .warnings
            .iter()
            .any(|warning| warning.contains("Strict tool-call policy")));
    }

    #[tokio::test]
    async fn test_codex_models_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();
        database::create_provider(
            &db,
            &CreateProvider {
                id: "p1".into(),
                name: "P".into(),
                base_url: "http://x".into(),
                openai_base_url: None,
                anthropic_base_url: None,
                auth_header: "Authorization".into(),
                auth_scheme: Some("Bearer".into()),
                api_key: Some("k".into()),
            },
        )
        .unwrap();
        database::create_codex_route(
            &db,
            &crate::models::CreateCodexRoute {
                id: "r1".into(),
                codex_model: "gpt-4o".into(),
                display_name: "GPT-4o".into(),
                provider_id: "p1".into(),
                upstream_model: "deepseek-chat".into(),
                tool_call_mode: None,
            },
        )
        .unwrap();

        let app = build_router(Ctx {
            db: db.clone(),
            client: Client::new(),
            profile: GatewayProfile {
                listen_host: "127.0.0.1".into(),
                listen_port: 3457,
                auth_token: "tok".into(),
            },
            history: Arc::new(codex_history::CodexChatHistoryStore::default()),
        });

        let resp = app
            .clone()
            .oneshot(
                http::Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = app
            .oneshot(
                http::Request::builder()
                    .uri("/v1/models")
                    .header("x-api-key", "tok")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["data"][0]["id"], "gpt-4o");
    }

    #[tokio::test]
    async fn test_codex_responses_accepts_large_payloads_above_axum_default() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("t.db");
        database::initialize(&db).unwrap();

        let app = build_router(Ctx {
            db: db.clone(),
            client: Client::new(),
            profile: GatewayProfile {
                listen_host: "127.0.0.1".into(),
                listen_port: 3457,
                auth_token: "tok".into(),
            },
            history: Arc::new(codex_history::CodexChatHistoryStore::default()),
        });
        let large_body = json!({
            "input": "x".repeat(3 * 1024 * 1024),
            "stream": false
        })
        .to_string();

        let resp = app
            .oneshot(
                http::Request::builder()
                    .method("POST")
                    .uri("/v1/responses")
                    .header(header::AUTHORIZATION, "Bearer tok")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(large_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["message"], "missing model");
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

        let chat_req = convert_request(&responses_req, "deepseek-chat", &CodexToolContext::default());
        assert_eq!(chat_req["model"], "deepseek-chat");
        assert_eq!(chat_req["messages"][0]["role"], "system");
        assert_eq!(
            chat_req["messages"][0]["content"],
            "You are a helpful assistant."
        );
        assert_eq!(chat_req["messages"][1]["role"], "user");
        assert_eq!(chat_req["messages"][1]["content"], "Hello");
        assert_eq!(chat_req["stream"], false);
    }

    #[tokio::test]
    async fn test_codex_developer_role_maps_to_system_for_chat_compat() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": [
                {
                    "type": "message",
                    "role": "developer",
                    "content": [{"type": "input_text", "text": "Follow coding-plan constraints."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Implement the task."}]
                }
            ],
            "stream": true
        });

        let chat_req = convert_request(&responses_req, "DeepSeek-V4-Pro", &CodexToolContext::default());

        assert_eq!(chat_req["messages"][0]["role"], "system");
        assert_eq!(
            chat_req["messages"][0]["content"],
            "Follow coding-plan constraints."
        );
        assert_eq!(chat_req["messages"][1]["role"], "user");
        assert_eq!(chat_req["messages"][1]["content"], "Implement the task.");
    }

    #[tokio::test]
    async fn test_codex_string_input_conversion() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": "Edit this file carefully",
            "stream": false
        });

        let chat_req = convert_request(&responses_req, "deepseek-chat", &CodexToolContext::default());
        assert_eq!(chat_req["messages"][0]["role"], "user");
        assert_eq!(
            chat_req["messages"][0]["content"],
            "Edit this file carefully"
        );
    }

    #[tokio::test]
    async fn test_codex_sync_response_conversion() {
        let chat_resp = json!({
            "choices": [{"message": {"content": "Hi there!"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test", &CodexToolContext::default());
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

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test", &CodexToolContext::default());
        assert_eq!(resp["output"][0]["type"], "function_call");
        assert_eq!(resp["output"][0]["call_id"], "call_1");
        assert_eq!(resp["output"][0]["name"], "apply_patch");
    }

    #[test]
    fn test_extracts_provider_delta_variants() {
        let content = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
        // `reasoning_content` is the model's internal monologue and must NOT
        // be exposed as the assistant's text reply; strip it from the text
        // stream so the Codex App chat UI never shows chain-of-thought.
        let reasoning = r#"data: {"choices":[{"delta":{"reasoning_content":"thinking"}}]}"#;
        let text = r#"data: {"choices":[{"delta":{"text":"plain"}}]}"#;

        assert_eq!(extract_chat_delta(content).as_deref(), Some("hello"));
        assert_eq!(extract_chat_delta(reasoning).as_deref(), None);
        assert_eq!(extract_chat_delta(text).as_deref(), Some("plain"));
        assert_eq!(extract_chat_delta("data: [DONE]"), None);
    }

    #[test]
    fn test_codex_request_adds_tool_call_guardrail() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": "Inspect the repository",
            "tools": [{
                "type": "function",
                "name": "exec_command",
                "description": "Run a command",
                "parameters": {"type": "object"}
            }],
            "stream": true
        });

        let tools = responses_req
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let tool_ctx = CodexToolContext::from_request(&tools, &[]);
        let chat_req = convert_request(&responses_req, "mimo-v2.5-pro", &tool_ctx);
        assert_eq!(chat_req["tool_choice"], "auto");
        assert_eq!(chat_req["messages"][0]["role"], "system");
        assert!(chat_req["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("structured tool_calls"));
        assert_eq!(chat_req["tools"][0]["function"]["name"], "exec_command");
    }

    #[test]
    fn test_force_tool_call_mode_keeps_auto_tool_choice_when_tools_are_present() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": "Inspect the repository",
            "tools": [{
                "type": "function",
                "name": "exec_command",
                "description": "Run a command",
                "parameters": {"type": "object"}
            }],
            "stream": true
        });
        let route = Route {
            display: "gpt-4o".into(),
            provider_id: "OpenAI".into(),
            upstream_model: "gpt-4o".into(),
            tool_call_mode: "force_when_tools_present".into(),
            base_url: "https://api.openai.com/v1".into(),
            strategy: standard_strategy(),
            headers: vec![],
            provider: test_provider("OpenAI", "OpenAI", "https://api.openai.com/v1"),
        };

        let tools = responses_req
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let tool_ctx = CodexToolContext::from_request(&tools, &[]);
        let mut chat_req = convert_request(&responses_req, "gpt-4o", &tool_ctx);
        apply_codex_tool_call_mode(&mut chat_req, &route);

        assert_eq!(chat_req["tool_choice"], "auto");
        assert!(chat_req["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Codex execution mode is active"));
        assert!(chat_req["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("answer normally"));
    }

    #[test]
    fn test_xiaomi_mimo_codex_compat_disables_thinking_and_renames_max_tokens() {
        let responses_req = json!({
            "model": "gpt-5.4",
            "input": "Inspect the repository",
            "max_output_tokens": 2048,
            "stream": true
        });
        let route = Route {
            display: "gpt-5.4".into(),
            provider_id: "Xiaomi".into(),
            upstream_model: "mimo-v2.5".into(),
            tool_call_mode: "force_when_tools_present".into(),
            base_url: "https://token-plan-sgp.xiaomimimo.com/v1".into(),
            strategy: standard_strategy(),
            headers: vec![],
            provider: test_provider("Xiaomi", "Xiaomi MiMo", "https://token-plan-sgp.xiaomimimo.com/v1"),
        };

        let mut chat_req = convert_request(&responses_req, "mimo-v2.5", &CodexToolContext::default());
        apply_xiaomi_mimo_codex_compat(&mut chat_req, &route);

        assert_eq!(chat_req["thinking"]["type"], "disabled");
        assert_eq!(chat_req["max_completion_tokens"], 2048);
        assert!(chat_req.get("max_tokens").is_none());
    }

    #[test]
    fn test_xiaomi_mimo_codex_compat_keeps_explicit_thinking() {
        let route = Route {
            display: "gpt-5.4".into(),
            provider_id: "Xiaomi".into(),
            upstream_model: "mimo-v2.5".into(),
            tool_call_mode: "force_when_tools_present".into(),
            base_url: "https://api.xiaomimimo.com/v1".into(),
            strategy: standard_strategy(),
            headers: vec![],
            provider: test_provider("Xiaomi", "Xiaomi MiMo", "https://api.xiaomimimo.com/v1"),
        };
        let mut chat_req = json!({
            "model": "mimo-v2.5",
            "messages": [{"role": "user", "content": "hi"}],
            "thinking": {"type": "enabled"}
        });

        apply_xiaomi_mimo_codex_compat(&mut chat_req, &route);

        assert_eq!(chat_req["thinking"]["type"], "enabled");
    }

    #[test]
    fn test_non_xiaomi_routes_keep_standard_chat_completion_shape() {
        let responses_req = json!({
            "model": "gpt-4o",
            "input": "Inspect the repository",
            "max_output_tokens": 1024,
            "stream": true
        });
        let route = Route {
            display: "gpt-4o".into(),
            provider_id: "OpenAI".into(),
            upstream_model: "gpt-4o".into(),
            tool_call_mode: "force_when_tools_present".into(),
            base_url: "https://api.openai.com/v1".into(),
            strategy: standard_strategy(),
            headers: vec![],
            provider: test_provider("OpenAI", "OpenAI", "https://api.openai.com/v1"),
        };

        let mut chat_req = convert_request(&responses_req, "gpt-4o", &CodexToolContext::default());
        apply_xiaomi_mimo_codex_compat(&mut chat_req, &route);

        assert!(chat_req.get("thinking").is_none());
        assert_eq!(chat_req["max_tokens"], 1024);
        assert!(chat_req.get("max_completion_tokens").is_none());
    }

    #[test]
    fn test_streaming_tool_arguments_are_repaired() {
        let mut tool_items = std::collections::HashMap::new();
        let mut output_index = 1;
        let mut seq = 0;
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"apply_patch","arguments":"{path:\"src/main.rs\",}"}}]}}]}"#;

        let events = response_tool_delta_events(line, &mut tool_items, &mut output_index, &mut seq);
        assert_eq!(events.len(), 2);
        let tool = tool_items.get(&0).unwrap();
        let repaired = compatibility::repair_json_object(&tool.arguments).unwrap();
        assert_eq!(repaired["path"], "src/main.rs");
    }

    #[test]
    fn test_upstream_url_accepts_root_versioned_or_full_endpoint_base() {
        assert_eq!(
            upstream_url("https://api.example.com", "chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            upstream_url("https://api.example.com/v1", "chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            upstream_url(
                "https://ark.cn-beijing.volces.com/api/coding/v3",
                "chat/completions"
            ),
            "https://ark.cn-beijing.volces.com/api/coding/v3/chat/completions"
        );
        assert_eq!(
            upstream_url(
                "https://ark.cn-beijing.volces.com/api/coding/v3/chat/completions",
                "chat/completions"
            ),
            "https://ark.cn-beijing.volces.com/api/coding/v3/chat/completions"
        );
    }

    #[test]
    fn test_post_policy_cleanup_strips_tool_choice_when_no_tools() {
        let route = build_route_with_provider("test", "Test", "https://test.com/v1", "test");
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "tool_choice": "auto",
            "parallel_tool_calls": true
        });
        apply_codex_post_policy_cleanup(&mut chat_req, false, &route);
        assert!(chat_req.get("tool_choice").is_none());
        assert!(chat_req.get("parallel_tool_calls").is_none());
    }

    #[test]
    fn test_post_policy_cleanup_preserves_tool_choice_when_tools_present() {
        let route = build_route_with_provider("test", "Test", "https://test.com/v1", "test");
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "tools": [{"type": "function", "function": {"name": "test"}}],
            "tool_choice": "auto"
        });
        apply_codex_post_policy_cleanup(&mut chat_req, false, &route);
        assert_eq!(chat_req["tool_choice"], "auto");
    }

    #[test]
    fn test_post_policy_cleanup_injects_include_usage_when_streaming() {
        let route = build_route_with_provider("test", "Test", "https://test.com/v1", "test");
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "stream": true
        });
        apply_codex_post_policy_cleanup(&mut chat_req, true, &route);
        assert_eq!(chat_req["stream_options"]["include_usage"], true);
    }

    #[test]
    fn test_post_policy_cleanup_skips_include_usage_when_not_streaming() {
        let route = build_route_with_provider("test", "Test", "https://test.com/v1", "test");
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "stream": false
        });
        apply_codex_post_policy_cleanup(&mut chat_req, false, &route);
        assert!(chat_req.get("stream_options").is_none());
    }

    #[test]
    fn test_split_leading_think_block_extracts_reasoning() {
        let (think, after) = split_leading_think_block("<think>Let me think about this...</think>\nHello!");
        assert_eq!(think, Some("Let me think about this...".to_string()));
        assert_eq!(after, "Hello!");
    }

    #[test]
    fn test_split_leading_think_block_no_think_tag() {
        let (think, after) = split_leading_think_block("Just a normal answer");
        assert_eq!(think, None);
        assert_eq!(after, "Just a normal answer");
    }

    #[test]
    fn test_convert_sync_response_emits_reasoning_item_from_explicit_field() {
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "reasoning_content": "I need to think carefully here",
                    "content": "The answer is 42"
                }
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 8}
        });

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test", &CodexToolContext::default());
        assert_eq!(resp["output"][0]["type"], "reasoning");
        assert_eq!(resp["output"][0]["summary"][0]["text"], "I need to think carefully here");
        assert_eq!(resp["output"][1]["type"], "message");
        assert_eq!(resp["output"][1]["content"][0]["text"], "The answer is 42");
        assert!(resp["usage"]["output_tokens_details"]["reasoning_tokens"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_convert_sync_response_splits_think_block() {
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "content": "<think>Hmm, let me calculate...</think>\n42 is the answer"
                }
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 10}
        });

        let resp = convert_sync_response(&chat_resp, "deepseek", "resp_test", "msg_test", &CodexToolContext::default());
        assert_eq!(resp["output"][0]["type"], "reasoning");
        assert_eq!(resp["output"][0]["summary"][0]["text"], "Hmm, let me calculate...");
        assert_eq!(resp["output"][1]["content"][0]["text"], "42 is the answer");
    }

    #[test]
    fn test_convert_sync_response_no_reasoning_item_when_absent() {
        let chat_resp = json!({
            "choices": [{
                "message": {
                    "content": "Just a simple answer"
                }
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 4}
        });

        let resp = convert_sync_response(&chat_resp, "gpt-4o", "resp_test", "msg_test", &CodexToolContext::default());
        assert_eq!(resp["output"].as_array().unwrap().len(), 1);
        assert_eq!(resp["output"][0]["type"], "message");
        assert_eq!(resp["output"][0]["content"][0]["text"], "Just a simple answer");
    }

    #[test]
    fn test_extract_reasoning_from_delta_reads_reasoning_content() {
        let delta = json!({"reasoning_content": "thinking...", "content": ""});
        assert_eq!(extract_reasoning_from_delta(&delta), Some("thinking...".to_string()));
    }

    #[test]
    fn test_extract_chat_delta_with_reasoning_separates_both() {
        let line = r#"data: {"choices":[{"delta":{"reasoning_content":"thinking","content":"hello"}}]}"#;
        let (text, reasoning) = extract_chat_delta_with_reasoning(line);
        assert_eq!(text, Some("hello".to_string()));
        assert_eq!(reasoning, Some("thinking".to_string()));
    }

    #[test]
    fn test_reasoning_only_delta_returns_no_text() {
        let line = r#"data: {"choices":[{"delta":{"reasoning_content":"processing..."}}]}"#;
        let (text, reasoning) = extract_chat_delta_with_reasoning(line);
        assert_eq!(text, None);
        assert_eq!(reasoning, Some("processing...".to_string()));
    }

    fn build_route_with_provider(id: &str, name: &str, base_url: &str, model: &str) -> Route {
        Route {
            display: model.into(),
            provider_id: id.into(),
            upstream_model: model.into(),
            tool_call_mode: "auto".into(),
            base_url: base_url.into(),
            strategy: standard_strategy(),
            headers: vec![],
            provider: test_provider(id, name, base_url),
        }
    }

    #[test]
    fn test_deepseek_reasoning_translates_to_thinking_enabled() {
        let route = build_route_with_provider(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com/v1",
            "deepseek-reasoner",
        );
        let mut chat_req = json!({
            "model": "deepseek-reasoner",
            "messages": [],
            "reasoning_effort": "high"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["thinking"]["type"], "enabled");
        assert!(chat_req.get("reasoning_effort").is_none());
        assert!(chat_req.get("reasoning").is_none());
    }

    #[test]
    fn test_kimi_reasoning_translates_to_enable_thinking_bool() {
        let route = build_route_with_provider(
            "moonshot",
            "Moonshot Kimi",
            "https://api.moonshot.cn/v1",
            "kimi-k2.7",
        );
        let mut chat_req = json!({
            "model": "kimi-k2.7",
            "messages": [],
            "reasoning_effort": "high"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["enable_thinking"], true);
        assert!(chat_req.get("reasoning_effort").is_none());
    }

    #[test]
    fn test_openrouter_max_effort_clamps_to_xhigh() {
        let route = build_route_with_provider(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "anthropic/claude-sonnet",
        );
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "reasoning": { "effort": "max" }
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["reasoning"]["effort"], "xhigh");
    }

    #[test]
    fn test_openrouter_none_effort_passes_through() {
        let route = build_route_with_provider(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "anthropic/claude-sonnet",
        );
        let mut chat_req = json!({
            "model": "test",
            "messages": [],
            "reasoning_effort": "none"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["reasoning"]["effort"], "none");
        assert!(chat_req.get("reasoning_effort").is_none());
        assert!(chat_req.get("thinking").is_none());
        assert!(chat_req.get("enable_thinking").is_none());
    }

    #[test]
    fn test_mimo_no_reasoning_injection() {
        let route = build_route_with_provider(
            "xiaomi",
            "Xiaomi MiMo",
            "https://api.xiaomimimo.com/v1",
            "mimo-v2.5-pro",
        );
        let mut chat_req = json!({
            "model": "mimo-v2.5-pro",
            "messages": [],
            "reasoning_effort": "high"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert!(chat_req.get("thinking").is_none());
        assert!(chat_req.get("enable_thinking").is_none());
        assert!(chat_req.get("reasoning").is_none());
        assert!(chat_req.get("reasoning_effort").is_some());
    }

    #[test]
    fn test_strip_reasoning_removes_all_reasoning_fields() {
        let mut strategy = standard_strategy();
        strategy.codex_strip_reasoning = true;
        let route = Route {
            display: "test".into(),
            provider_id: "deepseek".into(),
            upstream_model: "deepseek-r1".into(),
            tool_call_mode: "auto".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            strategy,
            headers: vec![],
            provider: test_provider("deepseek", "DeepSeek", "https://api.deepseek.com/v1"),
        };
        let mut chat_req = json!({
            "model": "deepseek-r1",
            "messages": [],
            "reasoning_effort": "high",
            "thinking": { "type": "enabled" }
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert!(chat_req.get("thinking").is_none());
        assert!(chat_req.get("reasoning").is_none());
        assert!(chat_req.get("reasoning_effort").is_none());
    }

    #[test]
    fn test_stepfun_reasoning_split() {
        let route = build_route_with_provider(
            "stepfun",
            "StepFun",
            "https://api.stepfun.com/v1",
            "step-1r",
        );
        let mut chat_req = json!({
            "model": "step-1r",
            "messages": [],
            "reasoning_effort": "high"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["reasoning_split"], true);
        assert!(chat_req.get("reasoning_effort").is_none());
    }

    #[test]
    fn test_siliconflow_enable_thinking() {
        let route = build_route_with_provider(
            "siliconflow",
            "SiliconFlow",
            "https://api.siliconflow.cn/v1",
            "deepseek-ai/DeepSeek-R1",
        );
        let mut chat_req = json!({
            "model": "deepseek-ai/DeepSeek-R1",
            "messages": [],
            "reasoning_effort": "medium"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["enable_thinking"], true);
    }

    #[test]
    fn test_qwen_thinking_enabled() {
        let route = build_route_with_provider(
            "qwen",
            "Qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen3-reasoning",
        );
        let mut chat_req = json!({
            "model": "qwen3-reasoning",
            "messages": [],
            "reasoning_effort": "high"
        });
        apply_codex_reasoning_translation(&mut chat_req, &route);
        assert_eq!(chat_req["thinking"]["type"], "enabled");
    }
}
