use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde_json::Value;

use crate::models::CodexBindingInfo;

const PROVIDER_ID: &str = "gateway-switch";
const PROVIDER_NAME: &str = "Gateway Switch";
const OPENAI_AUTH_METHOD: &str = "chatgpt";

pub fn inspect(home: &Path) -> Result<CodexBindingInfo, String> {
    let config = config_path(home);
    let content = read_config(&config)?;
    Ok(CodexBindingInfo {
        config_path: config.display().to_string(),
        config_exists: config.exists(),
        managed: top_level_value(&content, "model_provider").as_deref() == Some(PROVIDER_ID),
        model_provider: top_level_value(&content, "model_provider"),
        model: top_level_value(&content, "model"),
        base_url: table_value(
            &content,
            &format!("model_providers.{PROVIDER_ID}"),
            "base_url",
        ),
        backup_path: latest_backup(&config).map(|p| p.display().to_string()),
    })
}

pub fn apply(
    home: &Path,
    base_url: &str,
    auth_token: &str,
    model: &str,
) -> Result<CodexBindingInfo, String> {
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

    let cleaned = remove_gateway_managed_and_codexpp_config(&original);
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
    let current = read_config(&config)?;
    fs::write(&config, restore_openai_auth_config(&current)).map_err(|e| e.to_string())?;
    remove_openai_api_key_from_auth(home)?;
    inspect(home)
}

fn config_path(home: &Path) -> PathBuf {
    home.join(".codex/config.toml")
}

fn auth_path(home: &Path) -> PathBuf {
    home.join(".codex/auth.json")
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

fn write_named_backup(path: &Path, content: &str, label: &str) -> Result<(), String> {
    let backup_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("gateway-switch-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup = backup_dir.join(format!("{label}-{}.bak", Utc::now().timestamp_millis()));
    fs::write(backup, content).map_err(|e| e.to_string())
}

fn latest_backup(config: &Path) -> Option<PathBuf> {
    let backup_dir = config.parent()?.join("gateway-switch-backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backup_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("config-"))
                .unwrap_or(false)
        })
        .collect();
    entries.sort();
    entries.pop()
}

fn is_managed_config(content: &str) -> bool {
    top_level_value(content, "model_provider").as_deref() == Some(PROVIDER_ID)
}

fn restore_openai_auth_config(content: &str) -> String {
    let cleaned = remove_gateway_managed_and_codexpp_config(content);
    let trimmed = cleaned.trim_start();
    if trimmed.is_empty() {
        format!("preferred_auth_method = \"{OPENAI_AUTH_METHOD}\"\n")
    } else {
        format!("preferred_auth_method = \"{OPENAI_AUTH_METHOD}\"\n\n{trimmed}")
    }
}

fn remove_openai_api_key_from_auth(home: &Path) -> Result<(), String> {
    let auth = auth_path(home);
    if !auth.exists() {
        return Ok(());
    }

    let original = fs::read_to_string(&auth).map_err(|e| e.to_string())?;
    let Ok(mut value) = serde_json::from_str::<Value>(&original) else {
        return Ok(());
    };

    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };

    let mut changed = false;
    for key in ["OPENAI_API_KEY", "openai_api_key", "api_key"] {
        changed |= object.remove(key).is_some();
    }

    if changed {
        write_named_backup(&auth, &original, "auth")?;
        let next = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        fs::write(&auth, format!("{next}\n")).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn remove_gateway_managed_and_codexpp_config(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_removed_provider_table = false;
    let mut in_root = true;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_root = false;
            in_removed_provider_table = is_removed_provider_table(trimmed);
            if in_removed_provider_table {
                continue;
            }
        }

        if in_removed_provider_table {
            continue;
        }

        if in_root
            && (is_key(trimmed, "model_provider")
                || is_key(trimmed, "model")
                || is_key(trimmed, "preferred_auth_method"))
        {
            continue;
        }

        out.push(line);
    }

    out.join("\n")
}

fn is_removed_provider_table(trimmed: &str) -> bool {
    let Some(table) = trimmed
        .strip_prefix("[model_providers.")
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return false;
    };
    is_gateway_or_codexpp_provider_id(table)
}

fn is_gateway_or_codexpp_provider_id(provider_id: &str) -> bool {
    let normalized: String = provider_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect();
    normalized == "gatewayswitch" || normalized == "codexplusplus"
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
        let auth = auth_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "[projects.foo]\ntrust_level = \"trusted\"\n").unwrap();
        fs::write(
            &auth,
            r#"{"OPENAI_API_KEY":"tp-test","tokens":{"id":"keep"}}"#,
        )
        .unwrap();

        let applied = apply(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5").unwrap();
        assert!(applied.managed);
        assert_eq!(applied.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(
            applied.base_url.as_deref(),
            Some("http://127.0.0.1:3457/v1")
        );
        let applied_config = fs::read_to_string(&config).unwrap();
        assert!(applied_config.contains("requires_openai_auth = false"));
        assert!(applied_config.contains("experimental_bearer_token = \"tok\""));
        assert!(applied_config.contains("preferred_auth_method = \"apikey\""));

        apply(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5").unwrap();

        let restored = restore(tmp.path()).unwrap();
        let restored_config = fs::read_to_string(config).unwrap();
        let restored_auth = fs::read_to_string(auth).unwrap();
        assert!(!restored.managed);
        assert!(restored_config.contains("preferred_auth_method = \"chatgpt\""));
        assert!(restored_config.contains("[projects.foo]"));
        assert!(!restored_auth.contains("OPENAI_API_KEY"));
        assert!(restored_auth.contains("tokens"));
    }

    #[test]
    fn restore_openai_login_deactivates_codexplusplus_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_path(tmp.path());
        let auth = auth_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(
            &config,
            r#"model_provider = "CodexPlusPlus"
model = "gpt-4.1"
preferred_auth_method = "apikey"

[model_providers.CodexPlusPlus]
name = "codex++"
base_url = "http://127.0.0.1:3000/v1"
requires_openai_auth = true
experimental_bearer_token = "token"

[projects.foo]
trust_level = "trusted"
"#,
        )
        .unwrap();
        fs::write(&auth, r#"{"OPENAI_API_KEY":"tp-test"}"#).unwrap();

        apply(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5").unwrap();

        let restored = restore(tmp.path()).unwrap();
        let content = fs::read_to_string(config).unwrap();
        assert!(!restored.managed);
        assert_eq!(restored.model_provider, None);
        assert_eq!(restored.model, None);
        assert!(content.contains("preferred_auth_method = \"chatgpt\""));
        assert!(!content.contains("model_provider = \"CodexPlusPlus\""));
        assert!(!content.contains("model = \"gpt-4.1\""));
        assert!(!content.contains("preferred_auth_method = \"apikey\""));
        assert!(!content.contains("[model_providers.gateway-switch]"));
        assert!(!content.contains("[model_providers.CodexPlusPlus]"));
        assert!(!fs::read_to_string(auth).unwrap().contains("OPENAI_API_KEY"));
        assert!(content.contains("[projects.foo]"));
    }

    #[test]
    fn restore_without_clean_backup_removes_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(
            &config,
            r#"model_provider = "gateway-switch"
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
"#,
        )
        .unwrap();

        let restored = restore(tmp.path()).unwrap();
        let content = fs::read_to_string(config).unwrap();
        assert!(!restored.managed);
        assert!(content.contains("preferred_auth_method = \"chatgpt\""));
        assert!(!content.contains("gateway-switch"));
        assert!(content.contains("[projects.foo]"));
    }
}
