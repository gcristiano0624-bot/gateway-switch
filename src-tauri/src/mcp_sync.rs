use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct McpServerEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTargetStatus {
    pub target: String,
    pub label: String,
    pub config_path: String,
    pub config_exists: bool,
    pub format: String,
    pub parse_status: String,
    pub server_count: usize,
    pub writable: bool,
    pub backup_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerPreview {
    pub name: String,
    pub server_type: String,
    pub sources: Vec<String>,
    pub completeness: u8,
    pub credential_keys: Vec<String>,
    pub action: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpSyncPreview {
    pub generated_at: String,
    pub targets: Vec<McpTargetStatus>,
    pub merged_count: usize,
    pub source_count: usize,
    pub conflict_count: usize,
    pub resolved_count: usize,
    pub servers: Vec<McpServerPreview>,
    pub warnings: Vec<String>,
    pub can_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpWriteResult {
    pub target: String,
    pub label: String,
    pub ok: bool,
    pub config_path: String,
    pub backup_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpSyncResult {
    pub generated_at: String,
    pub preview: McpSyncPreview,
    pub written_targets: Vec<McpWriteResult>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    ClaudeDesktop,
    ClaudeCode,
    Codex,
}

#[derive(Debug, Clone)]
struct TargetRead {
    kind: TargetKind,
    status: McpTargetStatus,
    json: Option<Value>,
    toml: Option<toml::Value>,
    servers: BTreeMap<String, McpServerEntry>,
}

#[derive(Debug, Clone)]
struct MergeResult {
    servers: BTreeMap<String, McpServerEntry>,
    sources: BTreeMap<String, Vec<String>>,
    conflicts: BTreeSet<String>,
}

pub fn inspect(home: &Path) -> Result<McpSyncPreview, String> {
    let targets = read_targets(home);
    Ok(build_preview(targets))
}

pub fn preview(home: &Path) -> Result<McpSyncPreview, String> {
    inspect(home)
}

pub fn sync(home: &Path) -> Result<McpSyncResult, String> {
    let mut targets = read_targets(home);
    let preview = build_preview(targets.clone());
    if !preview.can_sync {
        return Err(sync_block_reason(&preview));
    }

    let merged = merge_servers(targets.iter().map(|t| (&t.status.target, &t.servers)));
    let mut logs = vec![
        format!("Read {} MCP sync targets", targets.len()),
        format!("Merged {} unique MCP servers", merged.servers.len()),
    ];
    let mut written_targets = Vec::new();

    for target in targets.iter_mut() {
        let result = write_target(home, target, &merged.servers);
        match &result {
            Ok(w) => {
                logs.push(format!("{}: {}", w.label, w.message));
                written_targets.push(w.clone());
            }
            Err(message) => {
                logs.push(format!("{}: {}", target.status.label, message));
                written_targets.push(McpWriteResult {
                    target: target.status.target.clone(),
                    label: target.status.label.clone(),
                    ok: false,
                    config_path: target.status.config_path.clone(),
                    backup_path: None,
                    message: message.clone(),
                });
            }
        }
    }

    let refreshed_preview = build_preview(read_targets(home));
    Ok(McpSyncResult {
        generated_at: Utc::now().to_rfc3339(),
        preview: refreshed_preview,
        written_targets,
        logs,
    })
}

fn sync_block_reason(preview: &McpSyncPreview) -> String {
    if preview.merged_count == 0 {
        return "没有可同步的 MCP Servers".into();
    }
    if let Some(target) = preview
        .targets
        .iter()
        .find(|t| t.parse_status == "解析失败" || t.parse_status == "权限不足")
    {
        return format!(
            "{} 配置不可同步: {}",
            target.label,
            target
                .error
                .clone()
                .unwrap_or_else(|| target.parse_status.clone())
        );
    }
    "MCP 同步暂不可执行".into()
}

fn read_targets(home: &Path) -> Vec<TargetRead> {
    vec![
        read_json_target(
            TargetKind::ClaudeDesktop,
            "claude_desktop",
            "Claude Desktop",
            home.join("Library/Application Support/Claude/claude_desktop_config.json"),
        ),
        read_json_target(
            TargetKind::ClaudeCode,
            "claude_code",
            "Claude Code",
            home.join(".claude/settings.json"),
        ),
        read_toml_target(
            TargetKind::Codex,
            "codex",
            "Codex",
            home.join(".codex/config.toml"),
        ),
    ]
}

fn read_json_target(kind: TargetKind, target: &str, label: &str, path: PathBuf) -> TargetRead {
    let exists = path.exists();
    let writable = target_writable(&path);
    let latest = latest_backup(&path).map(|p| p.display().to_string());
    if !exists {
        return TargetRead {
            kind,
            status: McpTargetStatus {
                target: target.into(),
                label: label.into(),
                config_path: path.display().to_string(),
                config_exists: false,
                format: "JSON".into(),
                parse_status: if writable {
                    "文件不存在"
                } else {
                    "权限不足"
                }
                .into(),
                server_count: 0,
                writable,
                backup_path: latest,
                error: if writable {
                    None
                } else {
                    Some("Target directory is not writable".into())
                },
            },
            json: Some(json!({})),
            toml: None,
            servers: BTreeMap::new(),
        };
    }

    match fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|content| serde_json::from_str::<Value>(&content).map_err(|e| e.to_string()))
    {
        Ok(config) if config.is_object() => {
            let servers = extract_servers_json(&config);
            TargetRead {
                kind,
                status: McpTargetStatus {
                    target: target.into(),
                    label: label.into(),
                    config_path: path.display().to_string(),
                    config_exists: true,
                    format: "JSON".into(),
                    parse_status: if writable { "正常" } else { "权限不足" }.into(),
                    server_count: servers.len(),
                    writable,
                    backup_path: latest,
                    error: if writable {
                        None
                    } else {
                        Some("Target file is not writable".into())
                    },
                },
                json: Some(config),
                toml: None,
                servers,
            }
        }
        Ok(_) => TargetRead {
            kind,
            status: error_status(
                target,
                label,
                &path,
                true,
                "JSON",
                "解析失败",
                "Config root must be a JSON object",
                writable,
                latest,
            ),
            json: None,
            toml: None,
            servers: BTreeMap::new(),
        },
        Err(error) => TargetRead {
            kind,
            status: error_status(
                target,
                label,
                &path,
                true,
                "JSON",
                "解析失败",
                &error,
                writable,
                latest,
            ),
            json: None,
            toml: None,
            servers: BTreeMap::new(),
        },
    }
}

fn read_toml_target(kind: TargetKind, target: &str, label: &str, path: PathBuf) -> TargetRead {
    let exists = path.exists();
    let writable = target_writable(&path);
    let latest = latest_backup(&path).map(|p| p.display().to_string());
    if !exists {
        return TargetRead {
            kind,
            status: McpTargetStatus {
                target: target.into(),
                label: label.into(),
                config_path: path.display().to_string(),
                config_exists: false,
                format: "TOML".into(),
                parse_status: if writable {
                    "文件不存在"
                } else {
                    "权限不足"
                }
                .into(),
                server_count: 0,
                writable,
                backup_path: latest,
                error: if writable {
                    None
                } else {
                    Some("Target directory is not writable".into())
                },
            },
            json: None,
            toml: Some(toml::Value::Table(toml::map::Map::new())),
            servers: BTreeMap::new(),
        };
    }

    match fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|content| toml::from_str::<toml::Value>(&content).map_err(|e| e.to_string()))
    {
        Ok(config) => {
            let servers = extract_servers_toml(&config);
            TargetRead {
                kind,
                status: McpTargetStatus {
                    target: target.into(),
                    label: label.into(),
                    config_path: path.display().to_string(),
                    config_exists: true,
                    format: "TOML".into(),
                    parse_status: if writable { "正常" } else { "权限不足" }.into(),
                    server_count: servers.len(),
                    writable,
                    backup_path: latest,
                    error: if writable {
                        None
                    } else {
                        Some("Target file is not writable".into())
                    },
                },
                json: None,
                toml: Some(config),
                servers,
            }
        }
        Err(error) => TargetRead {
            kind,
            status: error_status(
                target,
                label,
                &path,
                true,
                "TOML",
                "解析失败",
                &error,
                writable,
                latest,
            ),
            json: None,
            toml: None,
            servers: BTreeMap::new(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn error_status(
    target: &str,
    label: &str,
    path: &Path,
    exists: bool,
    format: &str,
    parse_status: &str,
    error: &str,
    writable: bool,
    backup_path: Option<String>,
) -> McpTargetStatus {
    McpTargetStatus {
        target: target.into(),
        label: label.into(),
        config_path: path.display().to_string(),
        config_exists: exists,
        format: format.into(),
        parse_status: parse_status.into(),
        server_count: 0,
        writable,
        backup_path,
        error: Some(error.into()),
    }
}

fn build_preview(targets: Vec<TargetRead>) -> McpSyncPreview {
    let merged = merge_servers(targets.iter().map(|t| (&t.status.target, &t.servers)));
    let source_count = targets.iter().filter(|t| !t.servers.is_empty()).count();
    let mut warnings = Vec::new();
    for target in &targets {
        if target.status.parse_status == "解析失败" || target.status.parse_status == "权限不足"
        {
            warnings.push(format!(
                "{}: {}",
                target.status.label,
                target
                    .status
                    .error
                    .clone()
                    .unwrap_or_else(|| target.status.parse_status.clone())
            ));
        }
    }
    if merged.servers.is_empty() {
        warnings.push("没有可同步的 MCP Servers".into());
    }

    let servers = merged
        .servers
        .iter()
        .map(|(name, entry)| {
            let sources = merged.sources.get(name).cloned().unwrap_or_default();
            McpServerPreview {
                name: name.clone(),
                server_type: server_type(entry).into(),
                sources,
                completeness: completeness(entry),
                credential_keys: credential_keys(entry),
                action: if merged.conflicts.contains(name) {
                    "冲突合并"
                } else {
                    "同步"
                }
                .into(),
                command: entry.command.clone(),
                url: entry.url.clone(),
            }
        })
        .collect();

    let can_sync = !merged.servers.is_empty()
        && targets
            .iter()
            .all(|t| t.status.parse_status != "解析失败" && t.status.parse_status != "权限不足");

    McpSyncPreview {
        generated_at: Utc::now().to_rfc3339(),
        targets: targets.into_iter().map(|t| t.status).collect(),
        merged_count: merged.servers.len(),
        source_count,
        conflict_count: merged.conflicts.len(),
        resolved_count: merged.conflicts.len(),
        servers,
        warnings,
        can_sync,
    }
}

fn merge_servers<'a, I>(server_dicts: I) -> MergeResult
where
    I: IntoIterator<Item = (&'a String, &'a BTreeMap<String, McpServerEntry>)>,
{
    let mut servers = BTreeMap::new();
    let mut sources: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut conflicts = BTreeSet::new();

    for (source, dict) in server_dicts {
        for (name, entry) in dict {
            sources
                .entry(name.clone())
                .or_default()
                .push(source.clone());
            if let Some(existing) = servers.get_mut(name) {
                if existing != entry {
                    conflicts.insert(name.clone());
                }
                let existing_score = completeness(existing);
                let new_score = completeness(entry);
                if new_score > existing_score {
                    *existing = entry.clone();
                } else if new_score == existing_score {
                    if !entry.env.is_empty() {
                        existing.env.extend(entry.env.clone());
                    }
                    if !entry.headers.is_empty() {
                        existing.headers.extend(entry.headers.clone());
                    }
                    if entry.command.is_some() {
                        existing.command = entry.command.clone();
                        existing.args = entry.args.clone();
                    }
                    if entry.url.is_some() {
                        existing.url = entry.url.clone();
                    }
                }
            } else {
                servers.insert(name.clone(), entry.clone());
            }
        }
    }

    MergeResult {
        servers,
        sources,
        conflicts,
    }
}

fn extract_servers_json(config: &Value) -> BTreeMap<String, McpServerEntry> {
    let mut servers = BTreeMap::new();
    let Some(raw) = config.get("mcpServers").and_then(|v| v.as_object()) else {
        return servers;
    };
    for (name, value) in raw {
        if let Some(entry) = entry_from_json(value) {
            servers.insert(name.clone(), entry);
        }
    }
    servers
}

fn entry_from_json(value: &Value) -> Option<McpServerEntry> {
    let obj = value.as_object()?;
    Some(McpServerEntry {
        command: obj.get("command").and_then(|v| v.as_str()).map(Into::into),
        args: string_array_json(obj.get("args")),
        env: string_map_json(obj.get("env")),
        url: obj.get("url").and_then(|v| v.as_str()).map(Into::into),
        headers: string_map_json(obj.get("headers")),
    })
}

fn extract_servers_toml(config: &toml::Value) -> BTreeMap<String, McpServerEntry> {
    let mut servers = BTreeMap::new();
    let Some(raw) = config.get("mcp_servers").and_then(|v| v.as_table()) else {
        return servers;
    };
    for (name, value) in raw {
        if let Some(entry) = entry_from_toml(value) {
            servers.insert(name.clone(), entry);
        }
    }
    servers
}

fn entry_from_toml(value: &toml::Value) -> Option<McpServerEntry> {
    let table = value.as_table()?;
    Some(McpServerEntry {
        command: table
            .get("command")
            .and_then(|v| v.as_str())
            .map(Into::into),
        args: string_array_toml(table.get("args")),
        env: string_map_toml(table.get("env")),
        url: table.get("url").and_then(|v| v.as_str()).map(Into::into),
        headers: string_map_toml(table.get("headers")),
    })
}

fn write_target(
    home: &Path,
    target: &TargetRead,
    merged: &BTreeMap<String, McpServerEntry>,
) -> Result<McpWriteResult, String> {
    match target.kind {
        TargetKind::ClaudeDesktop => write_json_target(&desktop_path(home), target, merged),
        TargetKind::ClaudeCode => write_json_target(&claude_code_path(home), target, merged),
        TargetKind::Codex => write_toml_target(&codex_path(home), target, merged),
    }
}

fn write_json_target(
    path: &Path,
    target: &TargetRead,
    merged: &BTreeMap<String, McpServerEntry>,
) -> Result<McpWriteResult, String> {
    let backup = if path.exists() {
        Some(backup_file(path)?)
    } else {
        None
    };
    let mut root = target
        .json
        .as_ref()
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    root.insert("mcpServers".into(), Value::Object(json_servers(merged)));
    write_json(path, &Value::Object(root))?;
    Ok(McpWriteResult {
        target: target.status.target.clone(),
        label: target.status.label.clone(),
        ok: true,
        config_path: path.display().to_string(),
        backup_path: backup.map(|p| p.display().to_string()),
        message: format!("已写入 {} 个 MCP Servers", merged.len()),
    })
}

fn write_toml_target(
    path: &Path,
    target: &TargetRead,
    merged: &BTreeMap<String, McpServerEntry>,
) -> Result<McpWriteResult, String> {
    let backup = if path.exists() {
        Some(backup_file(path)?)
    } else {
        None
    };
    let mut root = target
        .toml
        .as_ref()
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    root.insert(
        "mcp_servers".into(),
        toml::Value::Table(toml_servers(merged)),
    );
    write_toml(path, &toml::Value::Table(root))?;
    Ok(McpWriteResult {
        target: target.status.target.clone(),
        label: target.status.label.clone(),
        ok: true,
        config_path: path.display().to_string(),
        backup_path: backup.map(|p| p.display().to_string()),
        message: format!("已写入 {} 个 MCP Servers", merged.len()),
    })
}

fn json_servers(servers: &BTreeMap<String, McpServerEntry>) -> Map<String, Value> {
    let mut out = Map::new();
    for (name, entry) in servers {
        let mut value = Map::new();
        if let Some(command) = &entry.command {
            value.insert("command".into(), json!(command));
            if !entry.args.is_empty() {
                value.insert("args".into(), json!(entry.args));
            }
            if !entry.env.is_empty() {
                value.insert("env".into(), json!(entry.env));
            }
        }
        if let Some(url) = &entry.url {
            value.insert("url".into(), json!(url));
            if !entry.headers.is_empty() {
                value.insert("headers".into(), json!(entry.headers));
            }
        }
        out.insert(name.clone(), Value::Object(value));
    }
    out
}

fn toml_servers(servers: &BTreeMap<String, McpServerEntry>) -> toml::map::Map<String, toml::Value> {
    let mut out = toml::map::Map::new();
    for (name, entry) in servers {
        let mut value = toml::map::Map::new();
        if let Some(url) = &entry.url {
            value.insert("url".into(), toml::Value::String(url.clone()));
            if !entry.headers.is_empty() {
                value.insert(
                    "headers".into(),
                    toml::Value::Table(string_map_to_toml(&entry.headers)),
                );
            }
        } else if let Some(command) = &entry.command {
            value.insert("command".into(), toml::Value::String(command.clone()));
            if !entry.args.is_empty() {
                value.insert(
                    "args".into(),
                    toml::Value::Array(
                        entry
                            .args
                            .iter()
                            .cloned()
                            .map(toml::Value::String)
                            .collect(),
                    ),
                );
            }
            if !entry.env.is_empty() {
                value.insert(
                    "env".into(),
                    toml::Value::Table(string_map_to_toml(&entry.env)),
                );
            }
        }
        out.insert(name.clone(), toml::Value::Table(value));
    }
    out
}

fn string_map_to_toml(values: &BTreeMap<String, String>) -> toml::map::Map<String, toml::Value> {
    values
        .iter()
        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
        .collect()
}

fn string_array_json(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(Into::into))
                .collect()
        })
        .unwrap_or_default()
}

fn string_array_toml(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(Into::into))
                .collect()
        })
        .unwrap_or_default()
}

fn string_map_json(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn string_map_toml(value: Option<&toml::Value>) -> BTreeMap<String, String> {
    value
        .and_then(|v| v.as_table())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn completeness(entry: &McpServerEntry) -> u8 {
    let mut score = 0;
    if entry.command.is_some() || entry.url.is_some() {
        score += 1;
    }
    if !entry.args.is_empty() {
        score += 1;
    }
    if !entry.env.is_empty() {
        score += 1;
    }
    if !entry.headers.is_empty() {
        score += 1;
    }
    score
}

fn credential_keys(entry: &McpServerEntry) -> Vec<String> {
    entry
        .env
        .keys()
        .chain(entry.headers.keys())
        .cloned()
        .collect()
}

fn server_type(entry: &McpServerEntry) -> &'static str {
    if entry.url.is_some() {
        "SSE / HTTP"
    } else if entry.command.is_some() {
        "STDIO"
    } else {
        "未知"
    }
}

fn target_writable(path: &Path) -> bool {
    if path.exists() {
        return fs::OpenOptions::new().append(true).open(path).is_ok();
    }
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent.exists() {
            return parent
                .metadata()
                .map(|meta| !meta.permissions().readonly())
                .unwrap_or(false);
        }
        current = parent.parent();
    }
    false
}

fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let content = fs::read(path).map_err(|e| e.to_string())?;
    let backup_dir = path
        .parent()
        .ok_or("Cannot find config parent directory")?
        .join("gateway-switch-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("bak");
    let backup = backup_dir.join(format!("{stem}-{}.{}", Utc::now().timestamp_millis(), ext));
    fs::write(&backup, content).map_err(|e| e.to_string())?;
    Ok(backup)
}

fn latest_backup(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let backup_dir = path.parent()?.join("gateway-switch-backups");
    let mut entries: Vec<PathBuf> = fs::read_dir(backup_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(stem))
                .unwrap_or(false)
        })
        .collect();
    entries.sort();
    entries.pop()
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn write_toml(path: &Path, value: &toml::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        toml::to_string_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn desktop_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Claude/claude_desktop_config.json")
}

fn claude_code_path(home: &Path) -> PathBuf {
    home.join(".claude/settings.json")
}

fn codex_path(home: &Path) -> PathBuf {
    home.join(".codex/config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prefers_more_complete_entry_and_merges_equal_env() {
        let mut a = BTreeMap::new();
        a.insert(
            "github".into(),
            McpServerEntry {
                command: Some("npx".into()),
                args: vec![],
                env: BTreeMap::from([("A".into(), "1".into())]),
                url: None,
                headers: BTreeMap::new(),
            },
        );
        let mut b = BTreeMap::new();
        b.insert(
            "github".into(),
            McpServerEntry {
                command: Some("npx".into()),
                args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
                env: BTreeMap::from([("B".into(), "2".into())]),
                url: None,
                headers: BTreeMap::new(),
            },
        );
        let source_a = "desktop".to_string();
        let source_b = "codex".to_string();
        let merged = merge_servers(vec![(&source_a, &a), (&source_b, &b)]);
        let entry = merged.servers.get("github").unwrap();
        assert_eq!(
            entry.args,
            vec!["-y", "@modelcontextprotocol/server-github"]
        );
        assert_eq!(entry.env.get("A").map(String::as_str), None);
        assert_eq!(entry.env.get("B").map(String::as_str), Some("2"));
        assert!(merged.conflicts.contains("github"));

        let mut c = BTreeMap::new();
        c.insert(
            "fetch".into(),
            McpServerEntry {
                command: Some("uvx".into()),
                args: vec!["mcp-server-fetch".into()],
                env: BTreeMap::from([("A".into(), "1".into())]),
                url: None,
                headers: BTreeMap::new(),
            },
        );
        let mut d = BTreeMap::new();
        d.insert(
            "fetch".into(),
            McpServerEntry {
                command: Some("uvx".into()),
                args: vec!["mcp-server-fetch".into()],
                env: BTreeMap::from([("B".into(), "2".into())]),
                url: None,
                headers: BTreeMap::new(),
            },
        );
        let merged_equal = merge_servers(vec![(&source_a, &c), (&source_b, &d)]);
        let entry = merged_equal.servers.get("fetch").unwrap();
        assert_eq!(entry.env.get("A").map(String::as_str), Some("1"));
        assert_eq!(entry.env.get("B").map(String::as_str), Some("2"));
    }

    #[test]
    fn sync_writes_three_targets_and_preserves_non_mcp_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let desktop = desktop_path(home);
        let code = claude_code_path(home);
        let codex = codex_path(home);
        write_json(&desktop, &json!({"theme":"dark","mcpServers":{"filesystem":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"]}}})).unwrap();
        write_json(&code, &json!({"env":{"FOO":"bar"},"mcpServers":{"fetch":{"command":"uvx","args":["mcp-server-fetch"]}}})).unwrap();
        write_toml(
            &codex,
            &toml::from_str(
                r#"
model = "gpt-4o"

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]

[mcp_servers.github.env]
GITHUB_PERSONAL_ACCESS_TOKEN = "token"
"#,
            )
            .unwrap(),
        )
        .unwrap();

        let result = sync(home).unwrap();
        assert_eq!(result.preview.merged_count, 3);
        assert_eq!(result.written_targets.iter().filter(|r| r.ok).count(), 3);

        let desktop_after: Value =
            serde_json::from_str(&fs::read_to_string(desktop).unwrap()).unwrap();
        assert_eq!(desktop_after["theme"], "dark");
        assert_eq!(desktop_after["mcpServers"].as_object().unwrap().len(), 3);

        let code_after: Value = serde_json::from_str(&fs::read_to_string(code).unwrap()).unwrap();
        assert_eq!(code_after["env"]["FOO"], "bar");
        assert_eq!(code_after["mcpServers"].as_object().unwrap().len(), 3);

        let codex_after: toml::Value = toml::from_str(&fs::read_to_string(codex).unwrap()).unwrap();
        assert_eq!(
            codex_after.get("model").and_then(|v| v.as_str()),
            Some("gpt-4o")
        );
        assert_eq!(
            codex_after
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .unwrap()
                .len(),
            3
        );
    }
}
