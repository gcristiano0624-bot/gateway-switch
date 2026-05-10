use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde_json::{json, Map, Value};

use crate::models::ClaudeCodeInfo;

const MANAGED_BY: &str = "Gateway Switch";
const ENV_BASE_URL: &str = "ANTHROPIC_BASE_URL";
const ENV_AUTH_TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";
const ENV_API_KEY: &str = "ANTHROPIC_API_KEY";
const ENV_MODEL: &str = "ANTHROPIC_MODEL";

pub fn inspect(home: &Path) -> Result<ClaudeCodeInfo, String> {
    let path = settings_path(home);
    let config = read_json(&path);
    let env = config.get("env").and_then(|v| v.as_object());
    let managed = config
        .get("gatewaySwitchClaudeCode")
        .and_then(|v| v.get("managedBy"))
        .and_then(|v| v.as_str())
        == Some(MANAGED_BY);
    let auth_env = env.and_then(|e| {
        if e.get(ENV_AUTH_TOKEN).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).is_some() {
            Some(ENV_AUTH_TOKEN.to_string())
        } else if e.get(ENV_API_KEY).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).is_some() {
            Some(ENV_API_KEY.to_string())
        } else {
            None
        }
    });

    Ok(ClaudeCodeInfo {
        config_path: path.display().to_string(),
        config_exists: path.exists(),
        managed,
        base_url: env.and_then(|e| e.get(ENV_BASE_URL)).and_then(|v| v.as_str()).map(Into::into),
        model: env.and_then(|e| e.get(ENV_MODEL)).and_then(|v| v.as_str()).map(Into::into),
        auth_env,
        backup_path: latest_backup(&path).map(|p| p.display().to_string()),
    })
}

pub fn apply_gateway(home: &Path, base_url: &str, auth_token: &str, model: &str) -> Result<ClaudeCodeInfo, String> {
    if model.trim().is_empty() {
        return Err("Choose a Claude Code model before binding".into());
    }
    let mut env = Map::new();
    env.insert(ENV_BASE_URL.into(), json!(claude_code_base_url(base_url)));
    env.insert(ENV_AUTH_TOKEN.into(), json!(auth_token));
    env.insert(ENV_MODEL.into(), json!(model.trim()));
    apply_env(home, env, "gateway")
}

fn claude_code_base_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let lower = base.to_ascii_lowercase();
    if lower.contains("xiaomimimo.com") || lower.contains("xiaomimo.com") {
        if let Some(root) = base.strip_suffix("/v1") {
            return format!("{root}/anthropic");
        }
        if !lower.ends_with("/anthropic") {
            return format!("{base}/anthropic");
        }
    }
    base.to_string()
}

pub fn apply_provider(
    home: &Path,
    anthropic_base_url: &str,
    auth_header: &str,
    _auth_scheme: Option<&str>,
    api_key: &str,
    model: &str,
) -> Result<ClaudeCodeInfo, String> {
    if anthropic_base_url.trim().is_empty() {
        return Err("Provider Anthropic Base URL is empty".into());
    }
    if api_key.trim().is_empty() {
        return Err("Provider API key is empty".into());
    }
    if model.trim().is_empty() {
        return Err("Choose an upstream model before binding".into());
    }

    let mut env = Map::new();
    env.insert(ENV_BASE_URL.into(), json!(anthropic_base_url.trim_end_matches('/')));
    env.insert(ENV_MODEL.into(), json!(model.trim()));

    let header = auth_header.trim().to_ascii_lowercase();
    if header == "authorization" {
        env.insert(ENV_AUTH_TOKEN.into(), json!(api_key.trim()));
    } else {
        env.insert(ENV_API_KEY.into(), json!(api_key.trim()));
    }

    apply_env(home, env, "direct-provider")
}

pub fn restore(home: &Path) -> Result<ClaudeCodeInfo, String> {
    let path = settings_path(home);
    let backup = latest_backup(&path).ok_or("No Claude Code backup found")?;
    let content = fs::read_to_string(&backup).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())?;
    inspect(home)
}

fn apply_env(home: &Path, env_values: Map<String, Value>, mode: &str) -> Result<ClaudeCodeInfo, String> {
    let path = settings_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let config = read_json(&path);
    backup(&path, &config)?;

    let mut root = config.as_object().cloned().unwrap_or_default();
    let mut env = root
        .get("env")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    for key in [ENV_BASE_URL, ENV_AUTH_TOKEN, ENV_API_KEY, ENV_MODEL] {
        env.remove(key);
    }
    for (key, value) in env_values {
        env.insert(key, value);
    }

    root.insert("env".into(), Value::Object(env));
    root.insert(
        "gatewaySwitchClaudeCode".into(),
        json!({
            "managedBy": MANAGED_BY,
            "managedAt": Utc::now().to_rfc3339(),
            "mode": mode
        }),
    );

    write_json(&path, &Value::Object(root))?;
    inspect(home)
}

fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude/settings.json")
}

fn backup(path: &Path, config: &Value) -> Result<(), String> {
    let backup_dir = path
        .parent()
        .ok_or("Cannot find Claude Code settings directory")?
        .join("gateway-switch-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup_path = backup_dir.join(format!("settings-{}.json", Utc::now().timestamp_millis()));
    write_json(&backup_path, config)
}

fn latest_backup(path: &Path) -> Option<PathBuf> {
    let backup_dir = path.parent()?.join("gateway-switch-backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backup_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("settings-")).unwrap_or(false))
        .collect();
    entries.sort();
    entries.pop()
}

fn read_json(path: &Path) -> Value {
    if path.exists() {
        fs::read_to_string(path).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .filter(|v: &Value| v.is_object())
            .unwrap_or_else(|| json!({}))
    } else {
        json!({})
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_gateway_preserves_other_settings_and_restores() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let path = settings_path(home);
        write_json(&path, &json!({"theme":"dark","env":{"FOO":"bar"}})).unwrap();

        let info = apply_gateway(home, "http://127.0.0.1:3456/", "tok", "claude-sonnet-4-6").unwrap();
        assert!(info.managed);
        assert_eq!(info.base_url.as_deref(), Some("http://127.0.0.1:3456"));
        assert_eq!(info.model.as_deref(), Some("claude-sonnet-4-6"));

        let config = read_json(&path);
        assert_eq!(config["theme"], "dark");
        assert_eq!(config["env"]["FOO"], "bar");
        assert_eq!(config["env"][ENV_AUTH_TOKEN], "tok");

        let restored = restore(home).unwrap();
        assert!(!restored.managed);
        let config = read_json(&path);
        assert_eq!(config["env"]["FOO"], "bar");
        assert!(config["env"].get(ENV_AUTH_TOKEN).is_none());
    }

    #[test]
    fn direct_provider_writes_explicit_anthropic_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let info = apply_provider(
            home,
            "https://token-plan-sgp.xiaomimimo.com/anthropic/",
            "Authorization",
            Some("Bearer"),
            "test-key",
            "mimo-v2.5",
        ).unwrap();

        assert_eq!(info.base_url.as_deref(), Some("https://token-plan-sgp.xiaomimimo.com/anthropic"));
        let config = read_json(&settings_path(home));
        assert_eq!(config["env"][ENV_AUTH_TOKEN], "test-key");
        assert_eq!(config["env"][ENV_MODEL], "mimo-v2.5");
    }
}
