use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;

use crate::models::CodexBindingInfo;

const PROVIDER_ID: &str = "gateway-switch";
const PROVIDER_NAME: &str = "Gateway Switch";

pub fn inspect(home: &Path) -> Result<CodexBindingInfo, String> {
    let config = config_path(home);
    let content = read_config(&config)?;
    Ok(CodexBindingInfo {
        config_path: config.display().to_string(),
        config_exists: config.exists(),
        managed: top_level_value(&content, "model_provider").as_deref() == Some(PROVIDER_ID),
        model_provider: top_level_value(&content, "model_provider"),
        model: top_level_value(&content, "model"),
        base_url: table_value(&content, &format!("model_providers.{PROVIDER_ID}"), "base_url"),
        backup_path: latest_backup(&config).map(|p| p.display().to_string()),
    })
}

pub fn apply(home: &Path, base_url: &str, auth_token: &str, model: &str) -> Result<CodexBindingInfo, String> {
    if model.trim().is_empty() {
        return Err("Choose a Codex model before binding".into());
    }

    let config = config_path(home);
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let original = read_config(&config)?;
    if !is_managed_config(&original) {
        write_backup(&config, &original)?;
    }

    let cleaned = remove_managed_codex_config(&original);
    let header = format!(
        r#"model_provider = "{PROVIDER_ID}"
model = "{model}"
preferred_auth_method = "apikey"

[model_providers.{PROVIDER_ID}]
name = "{PROVIDER_NAME}"
base_url = "{base_url}"
wire_api = "responses"
requires_openai_auth = false
experimental_bearer_token = "{auth_token}"

"#,
        model = toml_escape(model),
        base_url = toml_escape(base_url),
        auth_token = toml_escape(auth_token),
    );

    fs::write(&config, format!("{header}{}", cleaned.trim_start())).map_err(|e| e.to_string())?;
    inspect(home)
}

pub fn restore(home: &Path) -> Result<CodexBindingInfo, String> {
    let config = config_path(home);
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Some(backup) = latest_unmanaged_backup(&config) {
        let content = fs::read_to_string(&backup).map_err(|e| e.to_string())?;
        fs::write(&config, content).map_err(|e| e.to_string())?;
    } else {
        let current = read_config(&config)?;
        fs::write(&config, remove_managed_codex_config(&current)).map_err(|e| e.to_string())?;
    }
    inspect(home)
}

fn config_path(home: &Path) -> PathBuf {
    home.join(".codex/config.toml")
}

fn read_config(config: &Path) -> Result<String, String> {
    if config.exists() {
        fs::read_to_string(config).map_err(|e| e.to_string())
    } else {
        Ok(String::new())
    }
}

fn write_backup(config: &Path, content: &str) -> Result<(), String> {
    let backup_dir = config
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("gateway-switch-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup = backup_dir.join(format!("config-{}.toml", Utc::now().timestamp_millis()));
    fs::write(backup, content).map_err(|e| e.to_string())
}

fn latest_backup(config: &Path) -> Option<PathBuf> {
    let backup_dir = config.parent()?.join("gateway-switch-backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backup_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.starts_with("config-")).unwrap_or(false))
        .collect();
    entries.sort();
    entries.pop()
}

fn latest_unmanaged_backup(config: &Path) -> Option<PathBuf> {
    let backup_dir = config.parent()?.join("gateway-switch-backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backup_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.starts_with("config-")).unwrap_or(false))
        .collect();
    entries.sort();
    entries.into_iter().rev().find(|path| {
        fs::read_to_string(path)
            .map(|content| !is_managed_config(&content))
            .unwrap_or(false)
    })
}

fn is_managed_config(content: &str) -> bool {
    top_level_value(content, "model_provider").as_deref() == Some(PROVIDER_ID)
}

fn remove_managed_codex_config(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_gateway_table = false;
    let mut in_root = true;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_root = false;
            in_gateway_table = trimmed == format!("[model_providers.{PROVIDER_ID}]");
            if in_gateway_table {
                continue;
            }
        }

        if in_gateway_table {
            continue;
        }

        if in_root && (is_key(trimmed, "model_provider") || is_key(trimmed, "model") || is_key(trimmed, "preferred_auth_method")) {
            continue;
        }

        out.push(line);
    }

    out.join("\n")
}

fn top_level_value(content: &str, key: &str) -> Option<String> {
    let mut in_root = true;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_root = false;
        }
        if in_root && is_key(trimmed, key) {
            return parse_string_value(trimmed);
        }
    }
    None
}

fn table_value(content: &str, table: &str, key: &str) -> Option<String> {
    let header = format!("[{table}]");
    let mut in_table = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_table = trimmed == header;
            continue;
        }
        if in_table && is_key(trimmed, key) {
            return parse_string_value(trimmed);
        }
    }
    None
}

fn is_key(line: &str, key: &str) -> bool {
    line.strip_prefix(key)
        .map(|rest| rest.trim_start().starts_with('='))
        .unwrap_or(false)
}

fn parse_string_value(line: &str) -> Option<String> {
    let (_, raw) = line.split_once('=')?;
    let raw = raw.trim().split('#').next()?.trim();
    raw.strip_prefix('"')?.strip_suffix('"').map(unescape_basic)
}

fn unescape_basic(value: &str) -> String {
    value.replace("\\\"", "\"").replace("\\\\", "\\")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_restore_codex_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "[projects.foo]\ntrust_level = \"trusted\"\n").unwrap();

        let applied = apply(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5").unwrap();
        assert!(applied.managed);
        assert_eq!(applied.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(applied.base_url.as_deref(), Some("http://127.0.0.1:3457/v1"));
        let applied_config = fs::read_to_string(&config).unwrap();
        assert!(applied_config.contains("requires_openai_auth = false"));
        assert!(applied_config.contains("experimental_bearer_token = \"tok\""));
        assert!(applied_config.contains("preferred_auth_method = \"apikey\""));

        apply(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5").unwrap();

        let restored = restore(tmp.path()).unwrap();
        assert!(!restored.managed);
        assert!(fs::read_to_string(config).unwrap().contains("[projects.foo]"));
    }

    #[test]
    fn restore_without_clean_backup_removes_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, r#"model_provider = "gateway-switch"
model = "gpt-5.5"
preferred_auth_method = "apikey"

[model_providers.gateway-switch]
name = "Gateway Switch"
base_url = "http://127.0.0.1:3457/v1"
wire_api = "responses"
requires_openai_auth = false
experimental_bearer_token = "tok"

[projects.foo]
trust_level = "trusted"
"#).unwrap();

        let restored = restore(tmp.path()).unwrap();
        let content = fs::read_to_string(config).unwrap();
        assert!(!restored.managed);
        assert!(!content.contains("gateway-switch"));
        assert!(content.contains("[projects.foo]"));
    }
}
