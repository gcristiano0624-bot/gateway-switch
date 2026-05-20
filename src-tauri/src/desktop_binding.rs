use std::{fs, path::{Path, PathBuf}};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::ModelRoute;

const APP_NAME: &str = "Gateway Switch";

#[derive(Debug, Clone, serde::Serialize)]
pub struct DesktopModelConfig {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "supports1m")]
    pub supports_1m: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DesktopInfo {
    pub config_path: String,
    pub config_exists: bool,
    pub managed: bool,
    pub base_url: Option<String>,
    pub auth_scheme: Option<String>,
    pub models: Vec<String>,
    pub backup_path: Option<String>,
}

pub fn gateway_base_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

pub fn model_configs_from_routes(routes: &[ModelRoute]) -> Vec<DesktopModelConfig> {
    routes
        .iter()
        .filter(|r| r.enabled)
        .map(|r| {
            let display_name = if r.display_name.trim().is_empty() {
                if r.upstream_model.trim().is_empty() {
                    r.claude_alias.clone()
                } else {
                    r.upstream_model.clone()
                }
            } else {
                r.display_name.clone()
            };
            DesktopModelConfig {
                name: r.claude_alias.clone(),
                display_name,
                supports_1m: true,
            }
        })
        .collect()
}

pub fn inspect(home: &Path) -> Result<DesktopInfo, String> {
    let config_dir = home.join("Library/Application Support/Claude-3p/configLibrary");
    let entry = active_entry(&config_dir)?;
    let config = read_json(&entry);

    Ok(DesktopInfo {
        config_path: entry.display().to_string(),
        config_exists: entry.exists(),
        managed: config.get("managedBy").and_then(|v| v.as_str()) == Some(APP_NAME),
        base_url: config.get("inferenceGatewayBaseUrl").and_then(|v| v.as_str()).map(Into::into),
        auth_scheme: config.get("inferenceGatewayAuthScheme").and_then(|v| v.as_str()).map(Into::into),
        models: config.get("inferenceModels").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|i| i.get("name").and_then(|v| v.as_str()).map(Into::into)).collect())
            .unwrap_or_default(),
        backup_path: latest_backup(&config_dir, entry.file_stem()).map(|p| p.display().to_string()),
    })
}

pub fn apply(home: &Path, base_url: &str, auth_scheme: &str, api_key: &str, models: &[DesktopModelConfig]) -> Result<DesktopInfo, String> {
    let config_dir = home.join("Library/Application Support/Claude-3p/configLibrary");
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let entry = active_entry(&config_dir)?;
    let config = read_json(&entry);

    // backup
    let backups = config_dir.join("backups");
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let stem = entry.file_stem().and_then(|s| s.to_str()).unwrap_or("default");
    let backup = backups.join(format!("{stem}-{}.json", Utc::now().timestamp_millis()));
    write_json(&backup, &config)?;

    // merge
    let mut c = config.as_object().cloned().unwrap_or_default();
    c.insert("inferenceProvider".into(), json!("gateway"));
    c.insert("inferenceGatewayBaseUrl".into(), json!(base_url));
    c.insert("inferenceGatewayApiKey".into(), json!(api_key));
    c.insert("inferenceGatewayAuthScheme".into(), json!(auth_scheme));
    c.insert("inferenceModels".into(), json!(models));
    c.insert("managedBy".into(), json!(APP_NAME));
    c.insert("managedAt".into(), json!(Utc::now().to_rfc3339()));
    write_json(&entry, &Value::Object(c))?;

    inspect(home)
}

pub fn restore(home: &Path) -> Result<DesktopInfo, String> {
    let config_dir = home.join("Library/Application Support/Claude-3p/configLibrary");
    let entry = active_entry(&config_dir)?;
    let backup = latest_backup(&config_dir, entry.file_stem())
        .ok_or("No backup found")?;
    let content = fs::read_to_string(&backup).map_err(|e| e.to_string())?;
    fs::write(&entry, content).map_err(|e| e.to_string())?;
    inspect(home)
}

fn active_entry(dir: &Path) -> Result<PathBuf, String> {
    let meta = dir.join("_meta.json");
    if meta.exists() {
        let m: Value = serde_json::from_str(&fs::read_to_string(&meta).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if let Some(id) = m.get("appliedId").and_then(|v| v.as_str()) {
            return Ok(dir.join(format!("{id}.json")));
        }
    }
    let id = Uuid::new_v4().to_string();
    write_json(&meta, &json!({"appliedId": id, "entries": [{"id": id, "name": "Default"}]}))?;
    Ok(dir.join(format!("{id}.json")))
}

fn read_json(path: &Path) -> Value {
    if path.exists() {
        fs::read_to_string(path).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .filter(|v: &Value| v.is_object())
            .unwrap_or(json!({}))
    } else {
        json!({})
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(p) = path.parent() { fs::create_dir_all(p).map_err(|e| e.to_string())?; }
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn latest_backup(dir: &Path, stem: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let prefix = stem?.to_str()?;
    let backups = dir.join("backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backups).ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(prefix)).unwrap_or(false))
        .collect();
    entries.sort();
    entries.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_restore() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dir = home.join("Library/Application Support/Claude-3p/configLibrary");
        fs::create_dir_all(&dir).unwrap();

        let meta = dir.join("_meta.json");
        write_json(&meta, &json!({"appliedId": "test", "entries": [{"id": "test", "name": "D"}]})).unwrap();
        write_json(&dir.join("test.json"), &json!({
            "inferenceProvider": "gateway",
            "inferenceGatewayBaseUrl": "https://old/v1/messages",
            "inferenceGatewayApiKey": "old-key",
            "inferenceGatewayAuthScheme": "x-api-key",
            "inferenceModels": [{"name": "old-model"}]
        })).unwrap();

        let applied = apply(home, "http://127.0.0.1:3456", "x-api-key", "tok", &[DesktopModelConfig {
            name: "claude-sonnet-4-6".into(),
            display_name: "MiMO Sonnet".into(),
            supports_1m: true,
        }]).unwrap();
        assert!(applied.managed);
        assert_eq!(applied.base_url.as_deref(), Some("http://127.0.0.1:3456"));
        assert_eq!(applied.models, vec!["claude-sonnet-4-6"]);
        let config = read_json(&dir.join("test.json"));
        assert_eq!(config["inferenceModels"][0]["name"], "claude-sonnet-4-6");
        assert_eq!(config["inferenceModels"][0]["displayName"], "MiMO Sonnet");
        assert_eq!(config["inferenceModels"][0]["supports1m"], true);

        let restored = restore(home).unwrap();
        assert_eq!(restored.base_url.as_deref(), Some("https://old/v1/messages"));
    }

    #[test]
    fn model_configs_use_route_display_name_and_enable_1m() {
        let routes = vec![
            ModelRoute {
                id: "r1".into(),
                claude_alias: "claude-sonnet-4-6".into(),
                display_name: "Xiaomi MiMO Sonnet".into(),
                provider_id: "xiaomi".into(),
                upstream_model: "mimo-v2.5".into(),
                enabled: true,
            },
            ModelRoute {
                id: "r2".into(),
                claude_alias: "claude-opus-4-7".into(),
                display_name: "".into(),
                provider_id: "volcengine".into(),
                upstream_model: "DeepSeek-V4-Pro".into(),
                enabled: true,
            },
        ];

        let configs = model_configs_from_routes(&routes);
        assert_eq!(configs[0].name, "claude-sonnet-4-6");
        assert_eq!(configs[0].display_name, "Xiaomi MiMO Sonnet");
        assert!(configs[0].supports_1m);
        assert_eq!(configs[1].display_name, "DeepSeek-V4-Pro");
        assert!(configs[1].supports_1m);
    }
}
