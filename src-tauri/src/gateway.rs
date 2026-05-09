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
        let g = st.runtime.gateway_handle.lock().map_err(|_| "lock")?;
        if g.is_some() { return Ok("already_running".into()); }
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
        .route("/v1/messages/count_tokens", post(count_tokens))
        .with_state(ctx)
}

async fn health(State(ctx): State<Ctx>) -> impl IntoResponse {
    let models: Vec<String> = database::list_routes(&ctx.db)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| r.claude_alias)
        .collect();
    Json(json!({ "ok": true, "listen": format!("{}:{}", ctx.profile.listen_host, ctx.profile.listen_port), "models": models }))
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
    let resp = ctx.client.post(upstream_url(&route.base_url, "messages/count_tokens"))
        .headers(to_headers(&route.headers)?).json(&upstream)
        .send().await.map_err(upstream_err)?;
    let status = resp.status();
    let json_body = resp.json::<Value>().await.map_err(upstream_err)?;
    Ok((status, Json(json_body)).into_response())
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

    let resp = ctx.client.post(upstream_url(&route.base_url, "messages"))
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
        let mut json_body = resp.json::<Value>().await.map_err(upstream_err)?;
        rewrite_model(&mut json_body, &route.display);
        let _ = database::insert_log(&ctx.db, &RequestLog {
            request_id: req_id, claude_alias: route.display.clone(),
            provider_id: route.provider_id.clone(), upstream_model: route.upstream_model.clone(),
            status_code: Some(status.as_u16()), duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: false, error_summary: None, created_at: String::new(),
        });
        return Ok((status, Json(json_body)).into_response());
    }

    let display = route.display.clone();
    let provider_id = route.provider_id.clone();
    let upstream_model = route.upstream_model.clone();
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
            request_id: Uuid::new_v4().to_string(), claude_alias: display,
            provider_id, upstream_model, status_code: Some(status.as_u16()),
            duration_ms: Some(started.elapsed().as_millis() as u64),
            is_stream: true, error_summary: None, created_at: String::new(),
        });
    };

    let mut builder = Response::builder().status(status);
    builder = builder.header(header::CONTENT_TYPE, "text/event-stream");
    builder.body(Body::from_stream(sse)).map_err(internal)
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
        ("anthropic-version".into(), "2023-06-01".into()),
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
    }
}
