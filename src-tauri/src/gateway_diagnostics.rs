use bytes::Bytes;
use serde_json::{json, Value};

use crate::compatibility;

pub(crate) fn should_capture_diagnostic(status_code: Option<u16>, error_summary: Option<&str>) -> bool {
    matches!(status_code, Some(400 | 413 | 429 | 500 | 502 | 503 | 504))
        || status_code.is_none()
        || error_summary
            .map(|s| {
                let lower = s.to_ascii_lowercase();
                lower.contains("timeout")
                    || lower.contains("too large")
                    || lower.contains("messages.role")
                    || lower.contains("bad gateway")
            })
            .unwrap_or(false)
}

pub(crate) fn sanitize_payload_for_diagnostics(value: &Value) -> (Value, usize) {
    match value {
        Value::Object(map) => {
            let mut redactions = 0;
            let mut output = serde_json::Map::new();
            for (key, item) in map {
                let lower = key.to_ascii_lowercase();
                if [
                    "authorization",
                    "api_key",
                    "apikey",
                    "token",
                    "auth_token",
                    "x-api-key",
                    "key",
                    "image",
                    "source",
                    "data",
                    "attachment",
                ]
                .iter()
                .any(|needle| lower.contains(needle))
                {
                    output.insert(key.clone(), json!("[redacted]"));
                    redactions += 1;
                    continue;
                }
                let (clean, count) = sanitize_payload_for_diagnostics(item);
                output.insert(key.clone(), clean);
                redactions += count;
            }
            (Value::Object(output), redactions)
        }
        Value::Array(items) => {
            let mut redactions = 0;
            let cleaned = items
                .iter()
                .take(20)
                .map(|item| {
                    let (clean, count) = sanitize_payload_for_diagnostics(item);
                    redactions += count;
                    clean
                })
                .collect::<Vec<_>>();
            if items.len() > 20 {
                redactions += 1;
            }
            (Value::Array(cleaned), redactions)
        }
        Value::String(text) => {
            let redacted = compatibility::redact_secrets(text);
            if redacted.chars().count() > 1200 {
                (
                    Value::String(format!(
                        "{}...[truncated {} chars]",
                        redacted.chars().take(1200).collect::<String>(),
                        redacted.chars().count().saturating_sub(1200)
                    )),
                    1,
                )
            } else if redacted != *text {
                (Value::String(redacted), 1)
            } else {
                (Value::String(text.clone()), 0)
            }
        }
        _ => (value.clone(), 0),
    }
}

pub(crate) fn likely_failure_cause(status_code: Option<u16>, error_summary: Option<&str>) -> String {
    let lower = error_summary.unwrap_or("").to_ascii_lowercase();
    if status_code == Some(413) || lower.contains("too large") {
        "Request or attachment exceeded provider/body limits.".into()
    } else if status_code == Some(400) && lower.contains("messages.role") {
        "Provider rejected Anthropic-style roles; enable Gateway Route and user/assistant-only conversion.".into()
    } else if matches!(status_code, Some(502 | 503 | 504)) || lower.contains("bad gateway") {
        "Upstream provider returned a server-side gateway error or timed out.".into()
    } else if status_code == Some(429) {
        "Provider rate limit or quota was reached.".into()
    } else if status_code.is_none() || lower.contains("timeout") {
        "Network timeout or connection failure before an HTTP status was returned.".into()
    } else {
        "Provider rejected the request; inspect the converted payload and selected compatibility strategy.".into()
    }
}


pub(crate) fn body_preview(bytes: &Bytes) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    let redacted = compatibility::redact_secrets(&text);
    if redacted.chars().count() > 300 {
        format!("{}...", redacted.chars().take(300).collect::<String>())
    } else {
        redacted
    }
}

pub(crate) fn should_fallback_from_anthropic_status(status: reqwest::StatusCode, bytes: &Bytes) -> bool {
    if status.is_success() {
        return true;
    }

    // Only fall back when the Anthropic-shaped endpoint is probably not implemented.
    // Retrying validation/size/auth errors through Chat Completions duplicates the
    // request and can make Claude Desktop enter repeated "request too large" loops.
    if matches!(status.as_u16(), 404 | 405 | 406 | 415 | 501) {
        return true;
    }

    let preview = body_preview(bytes).to_ascii_lowercase();
    if preview.contains("not found")
        || preview.contains("unsupported endpoint")
        || preview.contains("unknown endpoint")
        || preview.contains("method not allowed")
    {
        return true;
    }

    false
}
