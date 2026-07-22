use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use serde_json::Value;

use crate::models::CodexBindingInfo;

fn command_search_paths() -> Vec<PathBuf> {
    let mut paths = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    paths.extend(
        [
            "/opt/homebrew/bin",
            "/opt/homebrew/sbin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]
        .iter()
        .map(PathBuf::from),
    );
    paths.sort();
    paths.dedup();
    paths
}

fn find_command_on_path(command: &str) -> Option<String> {
    command_search_paths()
        .into_iter()
        .map(|p| p.join(command))
        .find(|p| p.exists())
        .map(|p| p.display().to_string())
}

fn augmented_command_path() -> String {
    std::env::join_paths(command_search_paths())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            "/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".into()
        })
}

const PROVIDER_ID: &str = "gateway-switch";
const PROVIDER_NAME: &str = "Gateway Switch";
const OPENAI_AUTH_METHOD: &str = "chatgpt";

/// First Codex CLI minor version known to move `preferred_auth_method` into the
/// `[auth]` table (older builds read it at the top level). Codex 0.133.0 still
/// parses the top-level form, so we only switch to the `[auth]` table once the
/// CLI is at or beyond this version. Tunable single source of truth.
const MODERN_AUTH_TABLE_MIN: (u32, u32, u32) = (0, 140, 0);

/// Where `preferred_auth_method` is written in `config.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthLayout {
    /// Legacy: `preferred_auth_method` at the document root.
    TopLevel,
    /// Modern: `preferred_auth_method` inside an `[auth]` table.
    AuthTable,
}

/// Model metadata Codex needs so it does not fall back to generic defaults
/// (which degrade tool-call reliability and context-window management). Emitted
/// as top-level `model_context_window` / `model_max_output_tokens` keys.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelMeta {
    pub context_window: u32,
    pub max_output_tokens: u32,
}

/// Parse a `codex --version` line such as `codex-cli 0.133.0` into a version
/// tuple. Kept pure (no process spawn) so it is unit-testable.
pub(crate) fn parse_codex_cli_version(output: &str) -> Option<(u32, u32, u32)> {
    let token = output
        .split_whitespace()
        .find(|t| t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))?;
    let mut parts = token.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

/// Detect the installed Codex CLI version by running `codex --version`.
/// Returns `None` when the binary can't be found or run.
fn detect_codex_cli_version() -> Option<(u32, u32, u32)> {
    let codex = find_command_on_path("codex").unwrap_or_else(|| "codex".to_string());
    let output = Command::new(codex)
        .arg("--version")
        .env("PATH", augmented_command_path())
        .output()
        .ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_codex_cli_version(&text)
}

/// Choose the config layout for the installed CLI. Defaults to the proven
/// top-level form when detection fails, so binding never breaks on a missing
/// or unreadable `codex` binary.
fn detect_auth_layout() -> AuthLayout {
    match detect_codex_cli_version() {
        Some(version) if version >= MODERN_AUTH_TABLE_MIN => AuthLayout::AuthTable,
        _ => AuthLayout::TopLevel,
    }
}

/// Conservative-but-correct model metadata keyed by model-name prefix. Unknown
/// models still get sensible values so the "fallback model metadata" warning is
/// suppressed for every bound model.
pub(crate) fn model_metadata_defaults(model: &str) -> ModelMeta {
    let m = model.to_ascii_lowercase();
    if m.starts_with("gpt-5.6") {
        ModelMeta { context_window: 400_000, max_output_tokens: 128_000 }
    } else if m.starts_with("gpt-5.5") || m.starts_with("gpt-5.4") {
        ModelMeta { context_window: 400_000, max_output_tokens: 128_000 }
    } else if m.contains("codex") || m.starts_with("gpt-5.1") || m.starts_with("gpt-5.3") {
        ModelMeta { context_window: 272_000, max_output_tokens: 128_000 }
    } else {
        ModelMeta { context_window: 272_000, max_output_tokens: 64_000 }
    }
}

/// Build the managed portion of `config.toml` (everything Gateway Switch owns).
pub(crate) fn build_managed_header(
    base_url: &str,
    auth_token: &str,
    model: &str,
    layout: AuthLayout,
    meta: ModelMeta,
) -> String {
    let auth_root = match layout {
        AuthLayout::TopLevel => "preferred_auth_method = \"apikey\"\n".to_string(),
        AuthLayout::AuthTable => String::new(),
    };
    let auth_table = match layout {
        AuthLayout::TopLevel => String::new(),
        AuthLayout::AuthTable => "[auth]\npreferred_auth_method = \"apikey\"\n\n".to_string(),
    };
    format!(
        r#"model_provider = "{PROVIDER_ID}"
model = "{model}"
model_context_window = {context_window}
model_max_output_tokens = {max_output_tokens}
{auth_root}{auth_table}[model_providers.{PROVIDER_ID}]
name = "{PROVIDER_NAME}"
base_url = "{base_url}"
wire_api = "responses"
experimental_bearer_token = "{auth_token}"

"#,
        model = toml_escape(model),
        base_url = toml_escape(base_url),
        auth_token = toml_escape(auth_token),
        context_window = meta.context_window,
        max_output_tokens = meta.max_output_tokens,
    )
}

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
    apply_with_layout(home, base_url, auth_token, model, detect_auth_layout())
}

pub(crate) fn apply_with_layout(
    home: &Path,
    base_url: &str,
    auth_token: &str,
    model: &str,
    layout: AuthLayout,
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
    let header = build_managed_header(
        base_url,
        auth_token,
        model,
        layout,
        model_metadata_defaults(model),
    );

    fs::write(&config, format!("{header}{}", cleaned.trim_start())).map_err(|e| e.to_string())?;
    inspect(home)
}

pub fn restore(home: &Path) -> Result<CodexBindingInfo, String> {
    restore_with_layout(home, detect_auth_layout())
}

pub(crate) fn restore_with_layout(
    home: &Path,
    layout: AuthLayout,
) -> Result<CodexBindingInfo, String> {
    let config = config_path(home);
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let current = read_config(&config)?;
    fs::write(&config, restore_openai_auth_config(&current, layout))
        .map_err(|e| e.to_string())?;
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

fn restore_openai_auth_config(content: &str, layout: AuthLayout) -> String {
    let cleaned = remove_gateway_managed_and_codexpp_config(content);
    let trimmed = cleaned.trim_start();
    let auth_block = match layout {
        AuthLayout::TopLevel => format!("preferred_auth_method = \"{OPENAI_AUTH_METHOD}\"\n"),
        AuthLayout::AuthTable => {
            format!("[auth]\npreferred_auth_method = \"{OPENAI_AUTH_METHOD}\"\n")
        }
    };
    if trimmed.is_empty() {
        auth_block
    } else {
        format!("{auth_block}\n{trimmed}")
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
    let mut out: Vec<String> = Vec::new();
    let mut in_removed_provider_table = false;
    let mut in_root = true;

    // The `[auth]` table is buffered: we only keep `preferred_auth_method` out
    // of it, and we drop the whole table (header included) if nothing else
    // remains — but we must preserve any user-owned keys in that table.
    let mut in_auth_table = false;
    let mut auth_header: Option<String> = None;
    let mut auth_body: Vec<String> = Vec::new();

    let flush_auth = |header: &mut Option<String>,
                      body: &mut Vec<String>,
                      out: &mut Vec<String>| {
        if let Some(h) = header.take() {
            let has_content = body.iter().any(|line| !line.trim().is_empty());
            if has_content {
                out.push(h);
                out.append(body);
            }
        }
        body.clear();
    };

    for line in content.lines() {
        let trimmed = line.trim();
        let is_header = trimmed.starts_with('[') && trimmed.ends_with(']');

        if is_header {
            // Leaving any previously open buffered `[auth]` table.
            if in_auth_table {
                flush_auth(&mut auth_header, &mut auth_body, &mut out);
                in_auth_table = false;
            }

            in_root = false;
            in_removed_provider_table = is_removed_provider_table(trimmed);
            if in_removed_provider_table {
                continue;
            }
            if trimmed == "[auth]" {
                in_auth_table = true;
                auth_header = Some(line.to_string());
                continue;
            }
        }

        if in_removed_provider_table {
            continue;
        }

        if in_auth_table {
            // Drop only the managed `preferred_auth_method`; keep everything else.
            if !is_key(trimmed, "preferred_auth_method") {
                auth_body.push(line.to_string());
            }
            continue;
        }

        if in_root
            && (is_key(trimmed, "model_provider")
                || is_key(trimmed, "model")
                || is_key(trimmed, "model_context_window")
                || is_key(trimmed, "model_max_output_tokens")
                || is_key(trimmed, "preferred_auth_method"))
        {
            continue;
        }

        out.push(line.to_string());
    }

    // Flush a trailing `[auth]` table if the file ended inside it.
    flush_auth(&mut auth_header, &mut auth_body, &mut out);

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
    fn parse_codex_cli_version_variants() {
        assert_eq!(parse_codex_cli_version("codex-cli 0.133.0"), Some((0, 133, 0)));
        assert_eq!(parse_codex_cli_version("codex 1.2.3\n"), Some((1, 2, 3)));
        assert_eq!(parse_codex_cli_version("codex-cli 0.140"), Some((0, 140, 0)));
        assert_eq!(parse_codex_cli_version("no version here"), None);
    }

    #[test]
    fn model_metadata_defaults_known_and_unknown() {
        assert_eq!(model_metadata_defaults("gpt-5.6-sol").context_window, 400_000);
        assert_eq!(model_metadata_defaults("gpt-5.1-codex").context_window, 272_000);
        // Unknown models still get non-zero metadata so the warning is suppressed.
        let unknown = model_metadata_defaults("some-random-model");
        assert!(unknown.context_window > 0);
        assert!(unknown.max_output_tokens > 0);
    }

    #[test]
    fn apply_and_restore_codex_config() {
        for layout in [AuthLayout::TopLevel, AuthLayout::AuthTable] {
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

            let applied =
                apply_with_layout(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5", layout)
                    .unwrap();
            assert!(applied.managed);
            assert_eq!(applied.model.as_deref(), Some("gpt-5.5"));
            assert_eq!(applied.base_url.as_deref(), Some("http://127.0.0.1:3457/v1"));
            let applied_config = fs::read_to_string(&config).unwrap();
            // requires_openai_auth is intentionally dropped now.
            assert!(!applied_config.contains("requires_openai_auth"));
            assert!(applied_config.contains("experimental_bearer_token = \"tok\""));
            assert!(applied_config.contains("preferred_auth_method = \"apikey\""));
            assert!(applied_config.contains("wire_api = \"responses\""));
            // B1: model metadata keys present.
            assert!(applied_config.contains("model_context_window ="));
            assert!(applied_config.contains("model_max_output_tokens ="));
            if layout == AuthLayout::AuthTable {
                assert!(applied_config.contains("[auth]"));
            }

            // Idempotent re-apply.
            apply_with_layout(tmp.path(), "http://127.0.0.1:3457/v1", "tok", "gpt-5.5", layout)
                .unwrap();

            let restored = restore_with_layout(tmp.path(), layout).unwrap();
            let restored_config = fs::read_to_string(&config).unwrap();
            let restored_auth = fs::read_to_string(&auth).unwrap();
            assert!(!restored.managed);
            assert!(restored_config.contains("preferred_auth_method = \"chatgpt\""));
            assert!(restored_config.contains("[projects.foo]"));
            // Managed metadata keys are stripped on restore.
            assert!(!restored_config.contains("model_context_window"));
            assert!(!restored_config.contains("model_max_output_tokens"));
            assert!(!restored_auth.contains("OPENAI_API_KEY"));
            assert!(restored_auth.contains("tokens"));
        }
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

        apply_with_layout(
            tmp.path(),
            "http://127.0.0.1:3457/v1",
            "tok",
            "gpt-5.5",
            AuthLayout::AuthTable,
        )
        .unwrap();

        let restored = restore_with_layout(tmp.path(), AuthLayout::AuthTable).unwrap();
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
        assert!(!content.contains("model_context_window"));
        assert!(!fs::read_to_string(auth).unwrap().contains("OPENAI_API_KEY"));
        assert!(content.contains("[projects.foo]"));
    }

    #[test]
    fn restore_without_clean_backup_removes_binding() {
        // Cover the modern [auth]-table managed shape too.
        let tmp = tempfile::tempdir().unwrap();
        let config = config_path(tmp.path());
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(
            &config,
            r#"model_provider = "gateway-switch"
model = "gpt-5.5"
model_context_window = 400000
model_max_output_tokens = 128000

[auth]
preferred_auth_method = "apikey"

[model_providers.gateway-switch]
name = "Gateway Switch"
base_url = "http://127.0.0.1:3457/v1"
wire_api = "responses"
experimental_bearer_token = "tok"

[projects.foo]
trust_level = "trusted"
"#,
        )
        .unwrap();

        let restored = restore_with_layout(tmp.path(), AuthLayout::AuthTable).unwrap();
        let content = fs::read_to_string(config).unwrap();
        assert!(!restored.managed);
        assert!(content.contains("preferred_auth_method = \"chatgpt\""));
        assert!(!content.contains("gateway-switch"));
        assert!(!content.contains("model_context_window"));
        assert!(content.contains("[projects.foo]"));
    }

    #[test]
    fn strip_auth_table_preserves_user_keys() {
        // A user-owned [auth] table with an extra key must keep that key and the
        // header; only the managed preferred_auth_method is removed.
        let input = r#"model_provider = "gateway-switch"
model = "gpt-5.5"

[auth]
preferred_auth_method = "apikey"
cli_auth_credentials_store = "keyring"

[projects.foo]
trust_level = "trusted"
"#;
        let cleaned = remove_gateway_managed_and_codexpp_config(input);
        assert!(cleaned.contains("[auth]"));
        assert!(cleaned.contains("cli_auth_credentials_store = \"keyring\""));
        assert!(!cleaned.contains("preferred_auth_method"));
        assert!(!cleaned.contains("model_provider"));
        assert!(cleaned.contains("[projects.foo]"));
    }

    #[test]
    fn strip_auth_table_drops_empty_header() {
        // An [auth] table that only held preferred_auth_method is removed whole.
        let input = "[auth]\npreferred_auth_method = \"apikey\"\n\n[projects.foo]\ntrust_level = \"trusted\"\n";
        let cleaned = remove_gateway_managed_and_codexpp_config(input);
        assert!(!cleaned.contains("[auth]"));
        assert!(cleaned.contains("[projects.foo]"));
    }
}
