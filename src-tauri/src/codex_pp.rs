use flate2::read::GzDecoder;
use plist::{Dictionary as PlistDictionary, Value as PlistValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

const STORE_INDEX_URL: &str = "https://b-nnett.github.io/codex-plusplus/store/index.json";
const SOURCE_ARCHIVE_URL: &str =
    "https://codeload.github.com/b-nnett/codex-plusplus/tar.gz/refs/heads/main";
const LOADER_FILE_NAME: &str = "codex-plusplus-loader.cjs";
const ASAR_BLOCK_SIZE: usize = 4 * 1024 * 1024;
const NATIVE_BOOTSTRAP_STATE_FILE: &str = "native-bootstrap-state.json";
const DEFAULT_LOCAL_SIGNING_IDENTITY: &str = "Codex++ Local Signing";
const WATCHER_LABEL: &str = "com.codexplusplus.watcher";
const INSTALLED_GATEWAY_SWITCH_EXECUTABLE: &str =
    "/Applications/Gateway Switch.app/Contents/MacOS/gateway-switch";
const UI_IMPROVEMENTS_TWEAK_ID: &str = "co.bennett.ui-improvements";
const RECOMMENDED_SCRIPT_SOURCE: &str = "gateway-switch-recommended";
const RECOMMENDED_SCRIPTS: [(&str, &str, &str, &str); 4] = [
    (
        "codex-context-used-meter",
        "Codex Context Used Meter",
        "Shows Codex context usage directly in the app UI.",
        "market-codex-context-used-meter.js",
    ),
    (
        "hide-usage-alert",
        "Hide Usage Alert",
        "Hides repeated usage/quota warning banners when the Codex++ script host supports it.",
        "market-hide-usage-alert.js",
    ),
    (
        "codex-token-usage",
        "Codex Token Usage",
        "Displays token input/output/cache metrics for Codex conversations.",
        "market-codex-token-usage.js",
    ),
    (
        "codex-list-pagebuster",
        "Codex List Pagebuster",
        "Improves the Codex session list and sidebar navigation ergonomics.",
        "market-codex-list-pagebuster.js",
    ),
];
const LOADER_CJS: &str = r#"/* eslint-disable */
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const Module = require("node:module");

const pkg = require("./package.json");
const meta = pkg.__codexpp || {};
const originalMain = meta.originalMain;
const userRoot = meta.userRoot;

function appendLog(file, line) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.appendFileSync(file, line);
}

function safe(label, fn) {
  try {
    fn();
  } catch (error) {
    const text = `[${new Date().toISOString()}] ${label}: ${(error && error.stack) || error}\n`;
    try {
      if (userRoot) appendLog(path.join(userRoot, "log", "loader.log"), text);
      else process.stderr.write(text);
    } catch (_) {
      process.stderr.write(text);
    }
  }
}

safe("init", () => {
  if (!originalMain) throw new Error("package.json missing __codexpp.originalMain");
  if (!userRoot) throw new Error("package.json missing __codexpp.userRoot");

  const runtimeDir = path.join(userRoot, "runtime");
  if (!fs.existsSync(runtimeDir)) return;

  Module.globalPaths.push(path.join(runtimeDir, "node_modules"));
  process.env.CODEX_PLUSPLUS_USER_ROOT = userRoot;
  process.env.CODEX_PLUSPLUS_RUNTIME = runtimeDir;
  safe("runtime", () => require(path.join(runtimeDir, "main.js")));
});

require("./" + originalMain);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpInstall {
    pub installed: bool,
    pub version: Option<String>,
    pub codex_version: Option<String>,
    pub app_root: Option<String>,
    pub user_root: String,
    pub runtime_dir: String,
    pub tweaks_dir: String,
    pub config_path: String,
    pub state_path: String,
    pub log_path: String,
    pub cli_path: Option<String>,
    pub auto_update: bool,
    pub safe_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexPpManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "githubRepo", default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub author: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(rename = "iconUrl", default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(rename = "minRuntime", default)]
    pub min_runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpTweak {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub scope: String,
    pub github_repo: Option<String>,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    pub tags: Vec<String>,
    pub permissions: Vec<String>,
    pub dir: String,
    pub manifest_path: String,
    pub entry_path: Option<String>,
    pub entry_exists: bool,
    pub enabled: bool,
    pub update_available: bool,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpHealth {
    pub checked_at: String,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub watcher: String,
    pub checks: Vec<CodexPpHealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpHealthCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpStoreIndex {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u8,
    #[serde(rename = "generatedAt", default)]
    pub generated_at: Option<String>,
    #[serde(rename = "sourceUrl", default)]
    pub source_url: Option<String>,
    #[serde(rename = "fetchedAt", default)]
    pub fetched_at: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(rename = "legacyRecommendations", default)]
    pub legacy_recommendations: Vec<CodexPpLegacyRecommendation>,
    pub entries: Vec<CodexPpStoreEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpStoreEntry {
    pub id: String,
    pub manifest: CodexPpManifest,
    pub repo: String,
    #[serde(rename = "approvedCommitSha")]
    pub approved_commit_sha: String,
    #[serde(rename = "approvedAt", default)]
    pub approved_at: Option<String>,
    #[serde(rename = "approvedBy", default)]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
    #[serde(rename = "releaseUrl", default)]
    pub release_url: Option<String>,
    #[serde(rename = "reviewUrl", default)]
    pub review_url: Option<String>,
    #[serde(rename = "archiveUrl", default)]
    pub archive_url: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub installed_version: Option<String>,
    #[serde(rename = "installedPath", default)]
    pub installed_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpLegacyRecommendation {
    pub name: String,
    #[serde(rename = "exactMatch")]
    pub exact_match: bool,
    #[serde(rename = "replacementEntryId", default)]
    pub replacement_entry_id: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpCliResult {
    pub action: String,
    pub command: String,
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpPreflight {
    pub ready: bool,
    pub install_mode: String,
    pub summary: String,
    pub app_path: Option<String>,
    pub checks: Vec<CodexPpPreflightCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpPreflightCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpRecommendedScriptsReport {
    pub storage_mode: String,
    pub storage_path: Option<String>,
    pub summary: String,
    pub scripts: Vec<CodexPpRecommendedScript>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPpRecommendedScript {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_name: String,
    pub status: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexPpLogEvent {
    pub session_id: String,
    pub stream: String,
    pub line: String,
}

pub fn detect() -> CodexPpInstall {
    let user_root = user_root();
    let state_path = user_root.join("state.json");
    let config_path = user_root.join("config.json");
    let runtime_dir = user_root.join("runtime");
    let tweaks_dir = user_root.join("tweaks");
    let log_path = user_root.join("log").join("main.log");
    let state = read_json(&state_path).unwrap_or(Value::Null);
    let config = read_json(&config_path).unwrap_or(Value::Null);
    let cli_path = find_cli_path(&user_root);

    CodexPpInstall {
        installed: state_path.exists()
            || runtime_dir.exists()
            || tweaks_dir.exists()
            || cli_path.is_some(),
        version: find_string(
            &state,
            &["version", "codexPlusPlusVersion", "installedVersion"],
        )
        .or_else(|| {
            find_nested_string(&config, &["codexPlusPlus", "updateCheck", "currentVersion"])
        }),
        codex_version: find_string(&state, &["codexVersion"]),
        app_root: find_string(&state, &["appRoot"]),
        user_root: user_root.display().to_string(),
        runtime_dir: runtime_dir.display().to_string(),
        tweaks_dir: tweaks_dir.display().to_string(),
        config_path: config_path.display().to_string(),
        state_path: state_path.display().to_string(),
        log_path: log_path.display().to_string(),
        cli_path,
        auto_update: find_nested_bool(&config, &["codexPlusPlus", "autoUpdate"]).unwrap_or(true),
        safe_mode: find_nested_bool(&config, &["codexPlusPlus", "safeMode"]).unwrap_or(false),
    }
}

pub fn list_tweaks() -> Result<Vec<CodexPpTweak>, String> {
    let install = detect();
    let tweaks_dir = PathBuf::from(&install.tweaks_dir);
    let config = read_json(Path::new(&install.config_path)).unwrap_or(Value::Null);
    let update_checks = config
        .get("tweakUpdateChecks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut tweaks = Vec::new();
    if !tweaks_dir.exists() {
        return Ok(tweaks);
    }

    for entry in fs::read_dir(&tweaks_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest: CodexPpManifest = match read_json_typed(&manifest_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if manifest.id.is_empty() || manifest.name.is_empty() || manifest.version.is_empty() {
            continue;
        }
        let entry_path = resolve_entry(&dir, &manifest);
        let update = update_checks
            .get(&manifest.id)
            .cloned()
            .unwrap_or(Value::Null);
        tweaks.push(CodexPpTweak {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            scope: manifest.scope.clone().unwrap_or_else(|| "renderer".into()),
            github_repo: manifest.github_repo.clone(),
            author: author_to_string(manifest.author.as_ref()),
            icon_url: manifest.icon_url.clone(),
            tags: manifest.tags.clone(),
            permissions: manifest.permissions.clone(),
            dir: dir.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            entry_path: entry_path.as_ref().map(|p| p.display().to_string()),
            entry_exists: entry_path.as_ref().is_some_and(|p| p.exists()),
            enabled: is_tweak_enabled(&config, &manifest.id),
            update_available: update
                .get("updateAvailable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            latest_version: update
                .get("latestVersion")
                .and_then(Value::as_str)
                .map(str::to_string),
            release_url: update
                .get("releaseUrl")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    tweaks.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(tweaks)
}

pub fn set_tweak_enabled(id: String, enabled: bool) -> Result<Vec<CodexPpTweak>, String> {
    let install = detect();
    let config_path = PathBuf::from(&install.config_path);
    let mut config = read_json(&config_path).unwrap_or_else(|| json!({}));
    if !config.is_object() {
        config = json!({});
    }
    set_tweak_enabled_in_config(&mut config, &id, enabled)?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    list_tweaks()
}

fn set_tweak_enabled_in_config(config: &mut Value, id: &str, enabled: bool) -> Result<(), String> {
    let root = config.as_object_mut().ok_or("invalid config")?;
    let tweaks = root.entry("tweaks").or_insert_with(|| json!({}));
    if !tweaks.is_object() {
        *tweaks = json!({});
    }
    let tweak_map = tweaks.as_object_mut().ok_or("invalid tweaks config")?;
    let existing = tweak_map.entry(id.to_string()).or_insert_with(|| json!({}));
    if !existing.is_object() {
        *existing = json!({});
    }
    existing
        .as_object_mut()
        .ok_or("invalid tweak config")?
        .insert("enabled".into(), Value::Bool(enabled));
    if id == UI_IMPROVEMENTS_TWEAK_ID {
        let codexpp = root.entry("codexPlusPlus").or_insert_with(|| json!({}));
        if !codexpp.is_object() {
            *codexpp = json!({});
        }
        codexpp
            .as_object_mut()
            .ok_or("invalid codexPlusPlus config")?
            .insert("uiSafeMode".into(), Value::Bool(!enabled));
    }
    Ok(())
}

pub async fn fetch_store() -> Result<CodexPpStoreIndex, String> {
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let mut index = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?
        .get(STORE_INDEX_URL)
        .header("User-Agent", "Gateway-Switch")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<CodexPpStoreIndex>()
        .await
        .map_err(|e| e.to_string())?;
    enrich_store_index(&mut index, &user_root(), fetched_at)?;
    Ok(index)
}

pub async fn install_from_store(
    repo: String,
    approved_commit_sha: String,
) -> Result<Vec<CodexPpTweak>, String> {
    if !valid_repo(&repo) || !valid_sha(&approved_commit_sha) {
        return Err("Invalid store entry repo or approved commit SHA".into());
    }
    let install = detect();
    let tweaks_dir = PathBuf::from(&install.tweaks_dir);
    fs::create_dir_all(&tweaks_dir).map_err(|e| e.to_string())?;
    let url = format!(
        "https://codeload.github.com/{}/tar.gz/{}",
        repo, approved_commit_sha
    );
    let tmp_root = std::env::temp_dir().join(format!("gateway-switch-codexpp-{}", now_millis()));
    let archive = tmp_root.join("tweak.tar.gz");
    let extract_dir = tmp_root.join("extract");
    fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    let bytes = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "Gateway-Switch")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    fs::write(&archive, &bytes).map_err(|e| e.to_string())?;
    let tar = Command::new("tar")
        .args([
            "-xzf",
            archive.to_string_lossy().as_ref(),
            "-C",
            extract_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !tar.status.success() {
        let _ = fs::remove_dir_all(&tmp_root);
        return Err(String::from_utf8_lossy(&tar.stderr).to_string());
    }
    let source = find_manifest_root(&extract_dir)
        .ok_or("Downloaded archive did not contain manifest.json")?;
    let manifest: CodexPpManifest = read_json_typed(&source.join("manifest.json"))?;
    if manifest.id.is_empty() {
        let _ = fs::remove_dir_all(&tmp_root);
        return Err("Downloaded tweak manifest is missing id".into());
    }
    let target = tweaks_dir.join(&manifest.id);
    if target.exists() {
        let backup = tweaks_dir.join(format!("{}.backup-{}", manifest.id, now_millis()));
        fs::rename(&target, backup).map_err(|e| e.to_string())?;
    }
    copy_dir(&source, &target)?;
    let _ = fs::remove_dir_all(&tmp_root);
    list_tweaks()
}

fn enrich_store_index(
    index: &mut CodexPpStoreIndex,
    root: &Path,
    fetched_at: String,
) -> Result<(), String> {
    if index.schema_version != 1 {
        return Err(format!(
            "Unsupported Codex++ store schema version: {}",
            index.schema_version
        ));
    }

    let installed = installed_tweaks_for_root(root)?;
    for entry in &mut index.entries {
        validate_store_entry(entry)?;
        entry.archive_url = Some(store_archive_url(&entry.repo, &entry.approved_commit_sha)?);
        if let Some(tweak) = installed.get(&entry.id) {
            entry.installed = true;
            entry.installed_version = Some(tweak.version.clone());
            entry.installed_path = Some(tweak.dir.clone());
        } else {
            entry.installed = false;
            entry.installed_version = None;
            entry.installed_path = None;
        }
    }

    index.entries.sort_by(|a, b| {
        a.manifest
            .name
            .to_lowercase()
            .cmp(&b.manifest.name.to_lowercase())
    });
    index.source_url = Some(STORE_INDEX_URL.into());
    index.fetched_at = Some(fetched_at);
    index.legacy_recommendations = legacy_recommendations(&index.entries);
    let exact_legacy_matches = index
        .legacy_recommendations
        .iter()
        .filter(|item| item.exact_match)
        .count();
    index.summary = Some(format!(
        "{} upstream tweaks loaded. {} of {} legacy requested scripts matched exact upstream entries.",
        index.entries.len(),
        exact_legacy_matches,
        RECOMMENDED_SCRIPTS.len()
    ));
    Ok(())
}

fn installed_tweaks_for_root(root: &Path) -> Result<HashMap<String, CodexPpTweak>, String> {
    let tweaks_dir = root.join("tweaks");
    let mut installed = HashMap::new();
    if !tweaks_dir.exists() {
        return Ok(installed);
    }
    let config = read_json(&root.join("config.json")).unwrap_or(Value::Null);
    for entry in fs::read_dir(&tweaks_dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest: CodexPpManifest = read_json_typed(&manifest_path)?;
        if manifest.id.is_empty() {
            continue;
        }
        let enabled = is_tweak_enabled(&config, &manifest.id);
        let entry_path = resolve_entry(&path, &manifest);
        installed.insert(
            manifest.id.clone(),
            CodexPpTweak {
                id: manifest.id.clone(),
                name: manifest.name,
                version: manifest.version,
                description: manifest.description,
                scope: manifest.scope.unwrap_or_else(|| "renderer".into()),
                github_repo: manifest.github_repo,
                author: author_to_string(manifest.author.as_ref()),
                icon_url: manifest.icon_url,
                tags: manifest.tags,
                permissions: manifest.permissions,
                dir: path.display().to_string(),
                manifest_path: manifest_path.display().to_string(),
                entry_path: entry_path.as_ref().map(|p| p.display().to_string()),
                entry_exists: entry_path.as_ref().map(|p| p.exists()).unwrap_or(false),
                enabled,
                update_available: false,
                latest_version: None,
                release_url: None,
            },
        );
    }
    Ok(installed)
}

fn validate_store_entry(entry: &CodexPpStoreEntry) -> Result<(), String> {
    if entry.id.trim().is_empty() {
        return Err("Codex++ store entry is missing id".into());
    }
    if entry.manifest.id.trim().is_empty()
        || entry.manifest.name.trim().is_empty()
        || entry.manifest.version.trim().is_empty()
    {
        return Err(format!(
            "Codex++ store entry {} is missing required manifest fields",
            entry.id
        ));
    }
    if entry.manifest.id != entry.id {
        return Err(format!(
            "Codex++ store entry {} id does not match manifest id {}",
            entry.id, entry.manifest.id
        ));
    }
    if !valid_repo(&entry.repo) {
        return Err(format!("Invalid Codex++ store repo: {}", entry.repo));
    }
    if let Some(repo) = &entry.manifest.github_repo {
        if repo != &entry.repo {
            return Err(format!(
                "Codex++ store entry {} repo does not match manifest githubRepo",
                entry.id
            ));
        }
    }
    if !valid_sha(&entry.approved_commit_sha) {
        return Err(format!(
            "Codex++ store entry {} must pin a full approved commit SHA",
            entry.id
        ));
    }
    Ok(())
}

fn store_archive_url(repo: &str, approved_commit_sha: &str) -> Result<String, String> {
    if !valid_repo(repo) || !valid_sha(approved_commit_sha) {
        return Err("Invalid store entry repo or approved commit SHA".into());
    }
    Ok(format!(
        "https://codeload.github.com/{repo}/tar.gz/{approved_commit_sha}"
    ))
}

fn legacy_recommendations(entries: &[CodexPpStoreEntry]) -> Vec<CodexPpLegacyRecommendation> {
    RECOMMENDED_SCRIPTS
        .iter()
        .map(|(_, name, _, _)| legacy_recommendation_for(name, entries))
        .collect()
}

fn legacy_recommendation_for(
    name: &str,
    entries: &[CodexPpStoreEntry],
) -> CodexPpLegacyRecommendation {
    let normalized = normalize_label(name);
    let exact = entries
        .iter()
        .find(|entry| normalize_label(&entry.manifest.name) == normalized);
    if let Some(entry) = exact {
        return CodexPpLegacyRecommendation {
            name: name.into(),
            exact_match: true,
            replacement_entry_id: Some(entry.id.clone()),
            note: format!("Exact upstream registry match: {}", entry.manifest.name),
        };
    }

    let replacement_entry_id = if matches!(
        name,
        "Codex Context Used Meter" | "Hide Usage Alert" | "Codex Token Usage"
    ) {
        entries
            .iter()
            .find(|entry| entry.id == UI_IMPROVEMENTS_TWEAK_ID)
            .map(|entry| entry.id.clone())
    } else {
        None
    };
    let note = match replacement_entry_id.as_deref() {
        Some(UI_IMPROVEMENTS_TWEAK_ID) => {
            "No exact upstream entry found. Bennett's UI Improvements is the closest approved tweak for hiding prompts and surfacing usage/message metrics."
        }
        _ => "No exact upstream registry entry found for this legacy script name.",
    };

    CodexPpLegacyRecommendation {
        name: name.into(),
        exact_match: false,
        replacement_entry_id,
        note: note.into(),
    }
}

fn normalize_label(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn uninstall_tweak(id: String) -> Result<Vec<CodexPpTweak>, String> {
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err("Invalid tweak id".into());
    }
    let install = detect();
    let target = PathBuf::from(&install.tweaks_dir).join(id);
    if target.exists() {
        fs::remove_dir_all(target).map_err(|e| e.to_string())?;
    }
    list_tweaks()
}

pub fn recommended_scripts_report() -> CodexPpRecommendedScriptsReport {
    recommended_scripts_report_for_root(&user_root())
}

pub fn install_recommended_scripts() -> Result<CodexPpRecommendedScriptsReport, String> {
    let root = user_root();
    let Some(storage_path) = detect_recommended_script_storage(&root) else {
        let report = recommended_scripts_report_for_root(&root);
        return Err(format!("{} No files were written.", report.summary));
    };

    fs::create_dir_all(&storage_path).map_err(|e| e.to_string())?;
    let backup_dir = storage_path.join(format!(".gateway-switch-backup-{}", now_millis()));
    let mut backed_up = false;
    for script in recommended_script_defs() {
        let target = storage_path.join(script.file_name);
        if target.exists() {
            fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
            fs::copy(&target, backup_dir.join(script.file_name)).map_err(|e| e.to_string())?;
            backed_up = true;
        }
        fs::write(&target, render_recommended_script_stub(&script)).map_err(|e| e.to_string())?;
    }
    if !backed_up {
        let _ = fs::remove_dir_all(&backup_dir);
    }
    append_line(
        &root.join("log").join("main.log"),
        &format!(
            "[{}] [info] Gateway Switch installed recommended scripts to {}",
            chrono::Utc::now().to_rfc3339(),
            storage_path.display()
        ),
    )?;
    Ok(recommended_scripts_report_for_root(&root))
}

fn recommended_scripts_report_for_root(root: &Path) -> CodexPpRecommendedScriptsReport {
    let storage_path = detect_recommended_script_storage(root);
    let scripts = recommended_script_defs()
        .into_iter()
        .map(|script| recommended_script_status(script, storage_path.as_deref()))
        .collect::<Vec<_>>();
    let installed = scripts
        .iter()
        .filter(|script| script.status == "installed")
        .count();
    let storage_mode = if storage_path.is_some() {
        "codex_user_scripts"
    } else {
        "unknown"
    }
    .to_string();
    let summary = if let Some(path) = &storage_path {
        format!(
            "{} of {} recommended scripts installed. Native script storage: {}",
            installed,
            RECOMMENDED_SCRIPTS.len(),
            path.display()
        )
    } else {
        "Codex++ native user-script storage was not detected in the installed runtime. Gateway Switch did not write arbitrary script files.".into()
    };
    CodexPpRecommendedScriptsReport {
        storage_mode,
        storage_path: storage_path.map(|path| path.display().to_string()),
        summary,
        scripts,
    }
}

#[derive(Debug, Clone)]
struct RecommendedScriptDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    file_name: &'static str,
}

fn recommended_script_defs() -> Vec<RecommendedScriptDef> {
    RECOMMENDED_SCRIPTS
        .iter()
        .map(|(id, name, description, file_name)| RecommendedScriptDef {
            id,
            name,
            description,
            file_name,
        })
        .collect()
}

fn recommended_script_status(
    script: RecommendedScriptDef,
    storage_path: Option<&Path>,
) -> CodexPpRecommendedScript {
    let path = storage_path.map(|storage| storage.join(script.file_name));
    let status = match path.as_deref() {
        Some(path) if path.is_file() => "installed",
        Some(_) => "missing",
        None => "unknown",
    }
    .to_string();
    CodexPpRecommendedScript {
        id: script.id.into(),
        name: script.name.into(),
        description: script.description.into(),
        file_name: script.file_name.into(),
        status,
        path: path.map(|path| path.display().to_string()),
    }
}

fn detect_recommended_script_storage(root: &Path) -> Option<PathBuf> {
    if !codex_runtime_exposes_user_scripts(root) {
        return None;
    }
    recommended_script_storage_candidates(root)
        .into_iter()
        .find(|path| path.is_dir())
}

fn recommended_script_storage_candidates(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("scripts"),
        root.join("user-scripts"),
        root.join("market-scripts"),
        root.join("runtime").join("scripts"),
    ]
}

fn codex_runtime_exposes_user_scripts(root: &Path) -> bool {
    let search_roots = [root.join("runtime"), root.join("source")];
    let needles = [
        "codexpp:list-scripts",
        "codexpp:get-scripts",
        "userScript",
        "user-script",
        "market-script",
        "market script",
        "not_loaded",
    ];
    search_roots
        .iter()
        .any(|path| path_contains_any(path, &needles))
}

fn path_contains_any(path: &Path, needles: &[&str]) -> bool {
    if path.is_file() {
        return fs::read_to_string(path)
            .map(|text| needles.iter().any(|needle| text.contains(needle)))
            .unwrap_or(false);
    }
    if !path.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if path_contains_any(&entry_path, needles) {
                return true;
            }
        } else if matches!(
            entry_path.extension().and_then(|ext| ext.to_str()),
            Some("js" | "ts" | "json")
        ) && path_contains_any(&entry_path, needles)
        {
            return true;
        }
    }
    false
}

fn render_recommended_script_stub(script: &RecommendedScriptDef) -> String {
    format!(
        r#"// {name}
// Installed by Gateway Switch.
// Source: {source}
// This file is managed only when Codex++ exposes a native user-script host.

export default {{
  id: "{id}",
  name: "{name}",
  source: "{source}",
  description: "{description}"
}};
"#,
        id = script.id,
        name = script.name,
        source = RECOMMENDED_SCRIPT_SOURCE,
        description = script.description.replace('"', "\\\""),
    )
}

pub fn health() -> CodexPpHealth {
    let install = detect();
    let mut checks = Vec::new();
    checks.push(check(
        "Install state",
        if install.installed { "ok" } else { "error" },
        install
            .version
            .clone()
            .unwrap_or_else(|| "state/runtime not found".into()),
    ));
    checks.push(check(
        "User root",
        if Path::new(&install.user_root).exists() {
            "ok"
        } else {
            "warn"
        },
        install.user_root.clone(),
    ));
    checks.push(check(
        "Runtime",
        if Path::new(&install.runtime_dir).exists() {
            "ok"
        } else {
            "error"
        },
        install.runtime_dir.clone(),
    ));
    checks.push(check(
        "Tweaks directory",
        if Path::new(&install.tweaks_dir).exists() {
            "ok"
        } else {
            "warn"
        },
        install.tweaks_dir.clone(),
    ));
    checks.push(check(
        "CLI",
        if install.cli_path.is_some() {
            "ok"
        } else {
            "warn"
        },
        install
            .cli_path
            .clone()
            .unwrap_or_else(|| "codexplusplus not found on known paths".into()),
    ));
    checks.push(check(
        "Automatic refresh",
        if install.auto_update { "ok" } else { "warn" },
        if install.auto_update {
            "enabled"
        } else {
            "disabled"
        }
        .into(),
    ));
    checks.push(check(
        "Safe mode",
        if install.safe_mode { "warn" } else { "ok" },
        if install.safe_mode {
            "all tweaks disabled"
        } else {
            "normal tweak loading"
        }
        .into(),
    ));
    checks.extend(platform_watcher_checks(&install));
    let has_error = checks.iter().any(|c| c.status == "error");
    let has_warn = checks.iter().any(|c| c.status == "warn");
    let status = if has_error {
        "error"
    } else if has_warn {
        "warn"
    } else {
        "ok"
    }
    .to_string();
    let title = match status.as_str() {
        "ok" => "Codex++ is ready",
        "warn" => "Codex++ needs review",
        _ => "Codex++ is not ready",
    }
    .to_string();
    let summary = if status == "ok" {
        "Runtime, tweak directory, and watcher checks passed.".into()
    } else {
        format!(
            "{} failing check(s), {} warning(s).",
            checks.iter().filter(|c| c.status == "error").count(),
            checks.iter().filter(|c| c.status == "warn").count()
        )
    };
    CodexPpHealth {
        checked_at: chrono::Utc::now().to_rfc3339(),
        status,
        title,
        summary,
        watcher: read_json(Path::new(&install.state_path))
            .and_then(|v| find_string(&v, &["watcher"]))
            .unwrap_or_else(|| "none".into()),
        checks,
    }
}

pub fn preflight() -> CodexPpPreflight {
    let install = detect();
    let node = detect_node();
    let npm = detect_tool("npm", &["--version"]);
    let app_path = detect_codex_app_path(install.app_root.as_deref());
    let mut checks = Vec::new();

    checks.push(CodexPpPreflightCheck {
        name: "codexplusplus CLI".into(),
        status: if install.cli_path.is_some() {
            "ok".into()
        } else {
            "warn".into()
        },
        detail: install
            .cli_path
            .clone()
            .unwrap_or_else(|| "not found, Gateway Switch will try bootstrap install".into()),
    });
    checks.push(CodexPpPreflightCheck {
        name: "Node.js".into(),
        status: node.status.clone(),
        detail: node.detail,
    });
    checks.push(CodexPpPreflightCheck {
        name: "npm".into(),
        status: npm.status.clone(),
        detail: npm.detail,
    });
    checks.push(CodexPpPreflightCheck {
        name: "Native bootstrap".into(),
        status: "ok".into(),
        detail: "Download and extract are handled by Gateway Switch; only npm install/build remain external.".into(),
    });
    checks.push(CodexPpPreflightCheck {
        name: "Codex.app".into(),
        status: if app_path.is_some() {
            "ok".into()
        } else {
            "error".into()
        },
        detail: app_path
            .clone()
            .unwrap_or_else(|| "Codex.app not found in standard locations".into()),
    });

    let ready = checks.iter().all(|check| check.status != "error");
    let install_mode = if ready {
        "native".to_string()
    } else {
        "blocked".to_string()
    };
    let summary = match install_mode.as_str() {
        "native" => {
            "Ready to run Gateway Switch native bootstrap, build codex++, and patch Codex.app without install.sh.".to_string()
        }
        _ => "Preflight failed. Fix the blocking checks before installing codex++.".to_string(),
    };

    CodexPpPreflight {
        ready,
        install_mode,
        summary,
        app_path,
        checks,
    }
}

pub fn run_cli(
    app: AppHandle,
    action: String,
    session_id: Option<String>,
) -> Result<CodexPpCliResult, String> {
    let session_id = session_id.unwrap_or_else(|| format!("codexpp-{}", now_millis()));
    if matches!(
        action.as_str(),
        "install" | "install-local" | "repair" | "repair-local"
    ) {
        return Ok(run_native_install(
            app,
            session_id,
            action.clone(),
            native_action_local_signing(&action),
        ));
    }
    let plan = resolve_cli_plan(&action)?;
    execute_command_plan(&app, &session_id, action, plan)
}

pub fn run_headless_cli(args: &[String]) -> Result<i32, String> {
    let Some(command) = args.first().map(|value| value.as_str()) else {
        eprintln!("Usage: gateway-switch codexpp <command> [args]");
        return Ok(1);
    };
    if matches!(command, "install" | "repair") {
        let local_signing =
            args.iter().any(|arg| arg == "--local") || native_action_local_signing(command);
        return Ok(run_native_headless(command, local_signing));
    }

    let plan = resolve_headless_cli_plan(args)?;
    execute_headless_command_plan(plan)
}

fn resolve_cli_plan(action: &str) -> Result<CommandPlan, String> {
    let (args, label) = match action {
        "status" => (vec!["status"], "status"),
        "doctor" => (vec!["doctor"], "doctor"),
        "debug" => (vec!["debug"], "debug"),
        "repair" => (vec!["repair"], "repair"),
        "repair-local" => (vec!["repair", "--local"], "repair --local"),
        "update" => (vec!["update"], "update"),
        "update-codex" => (vec!["update-codex"], "update-codex"),
        "safe-mode-on" => (vec!["safe-mode", "--on"], "safe-mode --on"),
        "safe-mode-off" => (vec!["safe-mode", "--off"], "safe-mode --off"),
        "safe-mode-status" => (vec!["safe-mode", "--status"], "safe-mode --status"),
        _ => return Err("Unsupported codex++ action".into()),
    };
    let install = detect();
    let cli = install.cli_path.ok_or("codexplusplus CLI not found")?;
    command_plan_from_cli(cli, args, label)
}

pub fn open_path(kind: String) -> Result<String, String> {
    let install = detect();
    let path = match kind.as_str() {
        "root" => install.user_root,
        "tweaks" => install.tweaks_dir,
        "config" => install.config_path,
        "state" => install.state_path,
        "log" => install.log_path,
        _ => return Err("Unsupported codex++ path kind".into()),
    };
    let status = Command::new("open")
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(path)
    } else {
        Err(format!("Failed to open {}", path))
    }
}

fn user_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("codex-plusplus")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("codex-plusplus")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"))
            .join("codex-plusplus")
    }
}

fn find_cli_path(user_root: &Path) -> Option<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let candidates = [
        "/opt/homebrew/bin/codexplusplus".into(),
        "/usr/local/bin/codexplusplus".into(),
        home.join(".bun").join("bin").join("codexplusplus"),
        home.join(".local").join("bin").join("codexplusplus"),
        user_root
            .join("source")
            .join("packages")
            .join("installer")
            .join("dist")
            .join("cli.js"),
    ];
    for path in candidates {
        if path.exists() {
            return Some(path.display().to_string());
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("codexplusplus"))
            .find(|p| p.exists())
            .map(|p| p.display().to_string())
    })
}

#[derive(Debug, Clone)]
struct CommandPlan {
    program: String,
    args: Vec<String>,
    display_command: String,
}

#[derive(Debug, Clone)]
struct ToolCheck {
    status: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct NativeCodexInstall {
    app_root: PathBuf,
    asar_path: PathBuf,
    asar_unpacked_path: PathBuf,
    meta_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct NativeBootstrapState {
    phase: String,
    status: String,
    detail: String,
    started_at: String,
    updated_at: String,
    source_dir: Option<String>,
    backup_source_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct NativeSigningResult {
    mode: String,
    identity: String,
    identity_hash: Option<String>,
}

struct NativeInstallContext {
    app: Option<AppHandle>,
    session_id: String,
    stdout: String,
    stderr: String,
    started_at: String,
    log_file: Option<PathBuf>,
    debug_file: Option<PathBuf>,
}

impl NativeInstallContext {
    fn new(app: Option<AppHandle>, session_id: String) -> Self {
        Self {
            app,
            session_id,
            stdout: String::new(),
            stderr: String::new(),
            started_at: chrono::Utc::now().to_rfc3339(),
            log_file: None,
            debug_file: None,
        }
    }

    fn set_log_file(&mut self, path: PathBuf) {
        self.log_file = Some(path);
    }

    fn set_debug_file(&mut self, path: PathBuf) {
        let _ = write_debug_header(&path, &self.started_at);
        self.debug_file = Some(path);
    }

    fn debug(&self, line: impl Into<String>) {
        if let Some(debug_file) = &self.debug_file {
            let line = line.into();
            let _ = append_line(debug_file, &format!("[debug] {}", line));
        }
    }

    fn log(&mut self, stream: &str, line: impl Into<String>) {
        let line = line.into();
        match stream {
            "stderr" => {
                self.stderr.push_str(&line);
                self.stderr.push('\n');
            }
            _ => {
                self.stdout.push_str(&line);
                self.stdout.push('\n');
            }
        }
        if self.app.is_none() {
            match stream {
                "stderr" => eprintln!("{}", line),
                _ => println!("{}", line),
            }
        }
        if let Some(path) = &self.log_file {
            let _ = append_line(path, &format!("[{}] {}", stream, line));
        }
        if let Some(app) = &self.app {
            emit_log(app, &self.session_id, stream, line);
        }
    }
}

fn native_action_local_signing(action: &str) -> bool {
    match action {
        "install-local" | "repair-local" => true,
        "repair" => preferred_local_signing(),
        _ => false,
    }
}

fn preferred_local_signing() -> bool {
    read_json(&user_root().join("state.json"))
        .map(|value| value.get("signingMode").and_then(Value::as_str) == Some("local-identity"))
        .unwrap_or(false)
}

fn native_command_display(action: &str, local_signing: bool) -> String {
    let base = match action {
        "repair" | "repair-local" => "gateway-switch native repair",
        _ => "gateway-switch native install",
    };
    if local_signing {
        format!("{} --local", base)
    } else {
        base.to_string()
    }
}

fn run_native_headless(action: &str, local_signing: bool) -> i32 {
    let mut ctx = NativeInstallContext::new(None, format!("headless-{}", now_millis()));
    let command = native_command_display(action, local_signing);
    ctx.log("system", format!("$ {}", command));
    let result = native_install_impl(&mut ctx, local_signing);
    let code = match result {
        Ok(_) => 0,
        Err(error) => {
            ctx.log("stderr", error);
            1
        }
    };
    ctx.log("system", format!("Process exited with code {}", code));
    code
}

fn run_native_install(
    app: AppHandle,
    session_id: String,
    action: String,
    local_signing: bool,
) -> CodexPpCliResult {
    let mut ctx = NativeInstallContext::new(Some(app), session_id);
    let command = native_command_display(&action, local_signing);
    ctx.log("system", format!("$ {}", command));
    let result = native_install_impl(&mut ctx, local_signing);
    let (success, code) = match result {
        Ok(_) => (true, Some(0)),
        Err(error) => {
            ctx.log("stderr", error);
            (false, Some(1))
        }
    };
    ctx.log(
        "system",
        format!("Process exited with code {}", code.unwrap_or_default()),
    );
    CodexPpCliResult {
        action,
        command,
        success,
        code,
        stdout: ctx.stdout,
        stderr: ctx.stderr,
    }
}

fn native_install_impl(ctx: &mut NativeInstallContext, local_signing: bool) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Gateway Switch native codex++ install currently targets macOS only.".into());
    }

    let root = user_root();
    let log_dir = root.join("log");
    fs::create_dir_all(&log_dir)
        .map_err(|e| format!("Failed to create {}: {}", log_dir.display(), e))?;
    ctx.set_log_file(log_dir.join("native-install.log"));
    ctx.set_debug_file(log_dir.join("native-debug.log"));
    ctx.debug(format!(
        "native action started_at={}, local_signing={}, pid={}",
        ctx.started_at,
        local_signing,
        std::process::id()
    ));
    let bootstrap_state_path = root.join(NATIVE_BOOTSTRAP_STATE_FILE);
    let source_dir = root.join("source");
    let backup_source_dir = root.join("source-prev");
    record_native_bootstrap_state(
        ctx,
        &bootstrap_state_path,
        &ctx.started_at,
        "preflight",
        "running",
        "Starting native codex++ bootstrap.",
        Some(&source_dir),
        Some(&backup_source_dir),
    );

    let codex = locate_native_codex_install()
        .map_err(|e| format!("Failed to locate Codex.app install: {}", e))?;
    ctx.log(
        "stdout",
        format!("Located Codex at {}", codex.app_root.display()),
    );
    ensure_unpacked_artifacts(ctx, &codex, &root)?;

    let node = detect_node();
    if node.status == "error" {
        return Err(format!("Node preflight failed: {}", node.detail));
    }
    ctx.log("stdout", format!("Node preflight ok: {}", node.detail));
    ctx.debug(format!(
        "Node preflight resolution detail: {}",
        command_resolution_debug("node")
    ));
    let npm = detect_tool("npm", &["--version"]);
    if npm.status == "error" {
        return Err(format!("npm preflight failed: {}", npm.detail));
    }
    ctx.log("stdout", format!("npm preflight ok: {}", npm.detail));
    ctx.debug(format!(
        "npm preflight resolution detail: {}",
        command_resolution_debug("npm")
    ));

    record_native_bootstrap_state(
        ctx,
        &bootstrap_state_path,
        &ctx.started_at,
        "download",
        "running",
        "Downloading codex++ source archive.",
        Some(&source_dir),
        Some(&backup_source_dir),
    );

    let mut should_restore_source = false;
    let mut should_restore_app = false;
    let result = (|| -> Result<(), String> {
        let extracted_root = download_and_extract_source(ctx, &root)?;
        switch_source_tree(ctx, &extracted_root, &source_dir, &backup_source_dir)?;
        should_restore_source = true;
        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "build",
            "running",
            "Running npm install/build inside the native source tree.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        run_streaming_command(ctx, "npm", &["install"], &source_dir, "npm install")?;
        run_streaming_command(ctx, "npm", &["run", "build"], &source_dir, "npm run build")?;
        maybe_fail_native_phase("after-build")?;

        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "stage-runtime",
            "running",
            "Copying runtime assets into the codex-plusplus user directory.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        stage_runtime_assets(ctx, &source_dir, &root)?;
        initialize_native_config(&root, &source_dir)?;
        fs::create_dir_all(root.join("tweaks")).map_err(|e| e.to_string())?;
        ctx.log(
            "stdout",
            "Codex++ 1.0 starts with a clean tweak directory. Install UI tweaks explicitly from the store.",
        );
        let cli_shim_summary = install_cli_shims_native(&root)?;
        ctx.log("stdout", cli_shim_summary);
        maybe_fail_native_phase("after-runtime")?;

        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "backup-app",
            "running",
            "Creating recoverable backups for Codex.app artifacts.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        backup_codex_artifacts(&codex, &root)?;
        should_restore_app = true;
        maybe_fail_native_phase("after-backup-app")?;

        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "patch-app",
            "running",
            "Patching app.asar and updating ElectronAsarIntegrity.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        let patch = patch_codex_asar(&codex, &root)?;
        if patch.already_patched {
            ctx.log(
                "stdout",
                format!(
                    "app.asar already points to {} (header hash {}...)",
                    patch.original_entry_point,
                    &patch.patched_asar_hash[..12]
                ),
            );
        } else {
            ctx.log(
                "stdout",
                format!(
                    "Patched app.asar (entry was {}, header hash {}...)",
                    patch.original_entry_point,
                    &patch.patched_asar_hash[..12]
                ),
            );
        }
        update_electron_asar_integrity(&codex.meta_path, &patch.patched_asar_hash)?;
        clear_quarantine_if_needed(&codex.app_root);
        maybe_fail_native_phase("after-patch")?;

        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "codesign",
            "running",
            "Re-signing Codex.app after native patching.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        let signing = sign_codex_app_native(ctx, &codex, local_signing)?;
        let watcher = install_watcher_native(&root, &codex)?;
        ctx.log("stdout", format!("Installed watcher: {}", watcher));

        write_native_installer_state(&root, &codex, &source_dir, &patch, &signing, &watcher)?;
        record_native_bootstrap_state(
            ctx,
            &bootstrap_state_path,
            &ctx.started_at,
            "complete",
            "ok",
            "Native bootstrap, build, and patch completed successfully.",
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        ctx.log("stdout", "Native codex++ install completed successfully.");
        Ok(())
    })();

    if let Err(error) = result {
        let _ = write_native_bootstrap_state(
            &bootstrap_state_path,
            &ctx.started_at,
            "rollback",
            "running",
            &format!("Install failed, starting rollback: {}", error),
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        if should_restore_app {
            let _ = restore_codex_artifacts(&codex, &root, ctx);
        }
        if should_restore_source {
            let _ = restore_previous_source_tree(&source_dir, &backup_source_dir, ctx);
        }
        let _ = write_native_bootstrap_state(
            &bootstrap_state_path,
            &ctx.started_at,
            "failed",
            "error",
            &error,
            Some(&source_dir),
            Some(&backup_source_dir),
        );
        return Err(error);
    }

    Ok(())
}

fn download_and_extract_source(
    ctx: &mut NativeInstallContext,
    root: &Path,
) -> Result<PathBuf, String> {
    let temp_root = root.join("tmp").join(format!("source-{}", now_millis()));
    fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    let archive_path = temp_root.join("codex-plusplus.tar.gz");
    ctx.log("stdout", format!("Downloading {}", SOURCE_ARCHIVE_URL));
    let response = reqwest::blocking::Client::new()
        .get(SOURCE_ARCHIVE_URL)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    fs::write(&archive_path, &bytes).map_err(|e| e.to_string())?;
    ctx.log(
        "stdout",
        format!(
            "Downloaded {} bytes to {}",
            bytes.len(),
            archive_path.display()
        ),
    );

    let extract_root = temp_root.join("extract");
    fs::create_dir_all(&extract_root).map_err(|e| e.to_string())?;
    let tar_gz = fs::File::open(&archive_path).map_err(|e| e.to_string())?;
    let decoder = GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(&extract_root).map_err(|e| e.to_string())?;
    let extracted_root = fs::read_dir(&extract_root)
        .map_err(|e| e.to_string())?
        .find_map(|entry| {
            let path = entry.ok()?.path();
            path.is_dir().then_some(path)
        })
        .ok_or("Failed to locate extracted codex++ source root")?;
    ctx.log(
        "stdout",
        format!("Extracted codex++ source into {}", extracted_root.display()),
    );
    Ok(extracted_root)
}

fn switch_source_tree(
    ctx: &mut NativeInstallContext,
    extracted_root: &Path,
    source_dir: &Path,
    backup_source_dir: &Path,
) -> Result<(), String> {
    if backup_source_dir.exists() {
        fs::remove_dir_all(backup_source_dir).map_err(|e| e.to_string())?;
    }
    if source_dir.exists() {
        fs::rename(source_dir, backup_source_dir).map_err(|e| e.to_string())?;
        ctx.log(
            "stdout",
            format!(
                "Moved previous source tree to {}",
                backup_source_dir.display()
            ),
        );
    }
    if let Some(parent) = source_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(extracted_root, source_dir).map_err(|e| e.to_string())?;
    ctx.log(
        "stdout",
        format!("Activated native source tree at {}", source_dir.display()),
    );
    Ok(())
}

fn restore_previous_source_tree(
    source_dir: &Path,
    backup_source_dir: &Path,
    ctx: &mut NativeInstallContext,
) -> Result<(), String> {
    if source_dir.exists() {
        fs::remove_dir_all(source_dir).map_err(|e| e.to_string())?;
    }
    if backup_source_dir.exists() {
        fs::rename(backup_source_dir, source_dir).map_err(|e| e.to_string())?;
        ctx.log(
            "stdout",
            format!(
                "Restored previous source tree from {}",
                source_dir.display()
            ),
        );
    }
    Ok(())
}

fn stage_runtime_assets(
    ctx: &mut NativeInstallContext,
    source_dir: &Path,
    root: &Path,
) -> Result<(), String> {
    let runtime_src = source_dir
        .join("packages")
        .join("installer")
        .join("assets")
        .join("runtime");
    if !runtime_src.exists() {
        return Err(format!(
            "Runtime assets not found at {}",
            runtime_src.display()
        ));
    }
    let runtime_dst = root.join("runtime");
    if runtime_dst.exists() {
        fs::remove_dir_all(&runtime_dst).map_err(|e| e.to_string())?;
    }
    copy_dir(&runtime_src, &runtime_dst)?;
    patch_runtime_health_compatibility(&runtime_dst)?;
    ctx.log(
        "stdout",
        format!("Staged runtime assets to {}", runtime_dst.display()),
    );
    Ok(())
}

fn patch_runtime_health_compatibility(runtime_dir: &Path) -> Result<(), String> {
    let old = r#"const loaded = commandSucceeds("launchctl", ["list", LAUNCHD_LABEL]);"#;
    let new = r#"const loaded = commandSucceeds("launchctl", ["list", LAUNCHD_LABEL]) || commandSucceeds("launchctl", ["print", `gui/${process.getuid?.() ?? ""}/${LAUNCHD_LABEL}`]);"#;
    for relative in ["watcher-health.js", "main.js"] {
        let path = runtime_dir.join(relative);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if content.contains(new) {
            continue;
        }
        if content.contains(old) {
            fs::write(&path, content.replace(old, new)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn locate_native_codex_install() -> Result<NativeCodexInstall, String> {
    let app_root = detect_codex_app_path(None)
        .map(PathBuf::from)
        .ok_or("Codex.app not found in /Applications or ~/Applications")?;
    let resources_dir = app_root.join("Contents").join("Resources");
    let asar_path = resources_dir.join("app.asar");
    let asar_unpacked_path = resources_dir.join("app.asar.unpacked");
    let meta_path = app_root.join("Contents").join("Info.plist");
    if !asar_path.exists() {
        return Err(format!("Missing app.asar at {}", asar_path.display()));
    }
    if !meta_path.exists() {
        return Err(format!("Missing Info.plist at {}", meta_path.display()));
    }
    Ok(NativeCodexInstall {
        app_root,
        asar_path,
        asar_unpacked_path,
        meta_path,
    })
}

fn backup_codex_artifacts(codex: &NativeCodexInstall, root: &Path) -> Result<(), String> {
    let backup_dir = root.join("backup").join("native-last");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    fs::copy(&codex.asar_path, backup_dir.join("app.asar")).map_err(|e| e.to_string())?;
    if codex.asar_unpacked_path.exists() {
        copy_dir(
            &codex.asar_unpacked_path,
            &backup_dir.join("app.asar.unpacked"),
        )?;
    }
    fs::copy(&codex.meta_path, backup_dir.join("Info.plist")).map_err(|e| e.to_string())?;
    Ok(())
}

fn restore_codex_artifacts(
    codex: &NativeCodexInstall,
    root: &Path,
    ctx: &mut NativeInstallContext,
) -> Result<(), String> {
    let backup_dir = root.join("backup").join("native-last");
    let backup_asar = backup_dir.join("app.asar");
    let backup_plist = backup_dir.join("Info.plist");
    if backup_asar.exists() {
        fs::copy(&backup_asar, &codex.asar_path).map_err(|e| e.to_string())?;
    }
    if backup_plist.exists() {
        fs::copy(&backup_plist, &codex.meta_path).map_err(|e| e.to_string())?;
    }
    let backup_unpacked = backup_dir.join("app.asar.unpacked");
    if backup_unpacked.exists() {
        if codex.asar_unpacked_path.exists() {
            fs::remove_dir_all(&codex.asar_unpacked_path).map_err(|e| e.to_string())?;
        }
        copy_dir(&backup_unpacked, &codex.asar_unpacked_path)?;
    }
    ctx.log(
        "stdout",
        "Restored Codex.app artifacts from the native backup set.",
    );
    Ok(())
}

fn ensure_unpacked_artifacts(
    ctx: &mut NativeInstallContext,
    codex: &NativeCodexInstall,
    root: &Path,
) -> Result<(), String> {
    if is_valid_asar_unpacked(&codex.asar_unpacked_path) {
        ctx.log(
            "stdout",
            format!(
                "app.asar.unpacked native modules look healthy at {}",
                codex.asar_unpacked_path.display()
            ),
        );
        ctx.debug(format!(
            "app.asar.unpacked healthy diagnostic: {}",
            asar_unpacked_debug_summary(&codex.asar_unpacked_path)
        ));
        return Ok(());
    }
    ctx.log(
        "stderr",
        format!(
            "app.asar.unpacked native modules are missing or incomplete at {}; checking backups.",
            codex.asar_unpacked_path.display()
        ),
    );
    ctx.log(
        "stderr",
        format!(
            "Current app.asar.unpacked diagnostic was written to {}",
            ctx.debug_file
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "native-debug.log".into())
        ),
    );
    ctx.debug(format!(
        "app.asar.unpacked current diagnostic: {}",
        asar_unpacked_debug_summary(&codex.asar_unpacked_path)
    ));

    let candidates = [
        root.join("backup").join("app.asar.unpacked"),
        root.join("backup")
            .join("Codex.app")
            .join("Contents")
            .join("Resources")
            .join("app.asar.unpacked"),
        root.join("backup")
            .join("native-last")
            .join("app.asar.unpacked"),
    ];
    for candidate in &candidates {
        ctx.debug(format!(
            "app.asar.unpacked backup candidate {} => {}",
            candidate.display(),
            asar_unpacked_debug_summary(candidate)
        ));
        ctx.log(
            "stdout",
            format!(
                "Checking app.asar.unpacked backup candidate: {}",
                candidate.display()
            ),
        );
    }
    let Some(source) = candidates
        .iter()
        .find(|candidate| is_valid_asar_unpacked(candidate))
    else {
        return Err(format!(
            "Codex.app is missing valid app.asar.unpacked native modules, and no valid backup was found under {}",
            root.join("backup").display()
        ));
    };

    ctx.log(
        "stdout",
        format!(
            "Restoring missing app.asar.unpacked native modules from {}",
            source.display()
        ),
    );
    if codex.asar_unpacked_path.exists() {
        fs::remove_dir_all(&codex.asar_unpacked_path).map_err(|e| e.to_string())?;
    }
    copy_dir(source, &codex.asar_unpacked_path)?;
    ctx.log(
        "stdout",
        format!(
            "Restored app.asar.unpacked native modules to {}",
            codex.asar_unpacked_path.display()
        ),
    );
    ctx.debug(format!(
        "app.asar.unpacked restored diagnostic: {}",
        asar_unpacked_debug_summary(&codex.asar_unpacked_path)
    ));
    Ok(())
}

fn is_valid_asar_unpacked(path: &Path) -> bool {
    required_unpacked_files()
        .iter()
        .all(|relative| path.join(relative).is_file())
}

fn required_unpacked_files() -> [&'static str; 2] {
    [
        "node_modules/better-sqlite3/build/Release/better_sqlite3.node",
        "node_modules/better-sqlite3/lib/index.js",
    ]
}

fn asar_unpacked_debug_summary(path: &Path) -> String {
    let mut required = Vec::new();
    for relative in required_unpacked_files() {
        let candidate = path.join(relative);
        let status = match fs::metadata(&candidate) {
            Ok(meta) if meta.is_file() => format!("ok:{}B", meta.len()),
            Ok(meta) if meta.is_dir() => "is-dir".into(),
            Ok(_) => "exists-not-file".into(),
            Err(error) => format!("missing:{}", error.kind()),
        };
        required.push(format!("{}={}", relative, status));
    }
    let (node_count, node_samples) = count_node_binaries(path, 12);
    format!(
        "exists={}, valid={}, required=[{}], node_binaries={}, node_samples=[{}]",
        path.exists(),
        is_valid_asar_unpacked(path),
        required.join("; "),
        node_count,
        node_samples.join(", ")
    )
}

fn count_node_binaries(path: &Path, max_samples: usize) -> (usize, Vec<String>) {
    if !path.exists() {
        return (0, Vec::new());
    }
    let mut count = 0;
    let mut samples = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&entry_path) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("node") {
                count += 1;
                if samples.len() < max_samples {
                    samples.push(
                        entry_path
                            .strip_prefix(path)
                            .unwrap_or(&entry_path)
                            .display()
                            .to_string(),
                    );
                }
            }
        }
    }
    (count, samples)
}

struct NativePatchOutcome {
    original_entry_point: String,
    original_asar_hash: String,
    patched_asar_hash: String,
    already_patched: bool,
}

fn patch_codex_asar(codex: &NativeCodexInstall, root: &Path) -> Result<NativePatchOutcome, String> {
    let parsed = parse_asar_archive(&codex.asar_path)?;
    let original_asar_hash = sha256_hex(&parsed.header_json);
    let package_json = read_inline_asar_file(&parsed, "package.json")?;
    let mut package: Value = serde_json::from_slice(&package_json)
        .map_err(|e| format!("Invalid package.json: {}", e))?;
    let original_entry_point = package
        .get("__codexpp")
        .and_then(|value| value.get("originalMain"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            package
                .get("main")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or("Codex package.json has no main entry")?;
    let existing_user_root = package
        .get("__codexpp")
        .and_then(|value| value.get("userRoot"))
        .and_then(Value::as_str);
    let current_main = package.get("main").and_then(Value::as_str);
    let loader_matches = read_inline_asar_file(&parsed, LOADER_FILE_NAME)
        .ok()
        .map(|bytes| bytes == LOADER_CJS.as_bytes())
        .unwrap_or(false);
    if current_main == Some(LOADER_FILE_NAME)
        && existing_user_root == Some(root.display().to_string().as_str())
        && loader_matches
    {
        return Ok(NativePatchOutcome {
            original_entry_point,
            original_asar_hash: original_asar_hash.clone(),
            patched_asar_hash: original_asar_hash,
            already_patched: true,
        });
    }

    let meta = json!({
        "originalMain": original_entry_point,
        "userRoot": root.display().to_string(),
        "loader": LOADER_FILE_NAME,
    });
    let pkg = package
        .as_object_mut()
        .ok_or("Codex package.json must be a JSON object")?;
    pkg.insert("__codexpp".into(), meta);
    pkg.insert("main".into(), Value::String(LOADER_FILE_NAME.into()));

    let mut modified_files = HashMap::new();
    modified_files.insert(
        "package.json".to_string(),
        serde_json::to_vec_pretty(&package).map_err(|e| e.to_string())?,
    );
    modified_files.insert(LOADER_FILE_NAME.to_string(), LOADER_CJS.as_bytes().to_vec());

    let mut out_data = Vec::new();
    let rebuilt_header = rebuild_asar_header(
        &parsed.header,
        "",
        &parsed.bytes,
        parsed.data_offset,
        &modified_files,
        &mut out_data,
    )?;
    let rebuilt_header_json = serde_json::to_vec(&rebuilt_header).map_err(|e| e.to_string())?;
    let patched_asar_hash = sha256_hex(&rebuilt_header_json);
    write_asar_archive(&codex.asar_path, &rebuilt_header_json, &out_data)?;

    Ok(NativePatchOutcome {
        original_entry_point,
        original_asar_hash,
        patched_asar_hash,
        already_patched: false,
    })
}

struct ParsedAsarArchive {
    bytes: Vec<u8>,
    header: Value,
    header_json: Vec<u8>,
    data_offset: usize,
}

fn parse_asar_archive(path: &Path) -> Result<ParsedAsarArchive, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 16 {
        return Err(format!(
            "{} is too small to be a valid asar",
            path.display()
        ));
    }
    let header_size = le_u32(&bytes[4..8]) as usize;
    let json_size = le_u32(&bytes[12..16]) as usize;
    let json_start = 16;
    let json_end = json_start + json_size;
    if bytes.len() < json_end {
        return Err("asar header JSON is truncated".into());
    }
    let data_offset = 8 + header_size;
    if bytes.len() < data_offset {
        return Err("asar data section is truncated".into());
    }
    let header_json = bytes[json_start..json_end].to_vec();
    let header =
        serde_json::from_slice(&header_json).map_err(|e| format!("Invalid asar header: {}", e))?;
    Ok(ParsedAsarArchive {
        bytes,
        header,
        header_json,
        data_offset,
    })
}

fn rebuild_asar_header(
    node: &Value,
    current_path: &str,
    archive: &[u8],
    data_offset: usize,
    modified_files: &HashMap<String, Vec<u8>>,
    out_data: &mut Vec<u8>,
) -> Result<Value, String> {
    if let Some(files) = node.get("files").and_then(Value::as_object) {
        let mut new_files = serde_json::Map::new();
        for (name, child) in files {
            let child_path = join_archive_path(current_path, name);
            new_files.insert(
                name.clone(),
                rebuild_asar_header(
                    child,
                    &child_path,
                    archive,
                    data_offset,
                    modified_files,
                    out_data,
                )?,
            );
        }
        if current_path.is_empty() && !new_files.contains_key(LOADER_FILE_NAME) {
            let loader_bytes = modified_files
                .get(LOADER_FILE_NAME)
                .ok_or("Native loader bytes are missing")?;
            new_files.insert(
                LOADER_FILE_NAME.into(),
                build_inline_asar_entry(loader_bytes, false, out_data)?,
            );
        }
        Ok(json!({ "files": new_files }))
    } else if node.get("link").is_some() {
        Ok(node.clone())
    } else {
        let executable = node
            .get("executable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if node
            .get("unpacked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            if modified_files.contains_key(current_path) {
                return Err(format!(
                    "Cannot replace unpacked asar file {} in native patch mode",
                    current_path
                ));
            }
            Ok(node.clone())
        } else {
            let bytes = if let Some(modified) = modified_files.get(current_path) {
                modified.clone()
            } else {
                extract_inline_asar_bytes(node, archive, data_offset)?
            };
            build_inline_asar_entry(&bytes, executable, out_data)
        }
    }
}

fn read_inline_asar_file(archive: &ParsedAsarArchive, path: &str) -> Result<Vec<u8>, String> {
    let node = find_asar_node(&archive.header, path)
        .ok_or_else(|| format!("{} not found in app.asar", path))?;
    if node
        .get("unpacked")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!(
            "{} is marked unpacked and cannot be read from inline asar data",
            path
        ));
    }
    extract_inline_asar_bytes(node, &archive.bytes, archive.data_offset)
}

fn build_inline_asar_entry(
    bytes: &[u8],
    executable: bool,
    out_data: &mut Vec<u8>,
) -> Result<Value, String> {
    let offset = out_data.len();
    out_data.extend_from_slice(bytes);
    let mut file = serde_json::Map::new();
    file.insert("offset".into(), Value::String(offset.to_string()));
    file.insert("size".into(), Value::Number((bytes.len() as u64).into()));
    if executable {
        file.insert("executable".into(), Value::Bool(true));
    }
    file.insert("integrity".into(), build_integrity_value(bytes));
    Ok(Value::Object(file))
}

fn build_integrity_value(bytes: &[u8]) -> Value {
    let block_hashes = bytes
        .chunks(ASAR_BLOCK_SIZE)
        .map(|chunk| Value::String(sha256_hex(chunk)))
        .collect::<Vec<_>>();
    json!({
        "algorithm": "SHA256",
        "hash": sha256_hex(bytes),
        "blockSize": ASAR_BLOCK_SIZE,
        "blocks": block_hashes,
    })
}

fn extract_inline_asar_bytes(
    node: &Value,
    archive: &[u8],
    data_offset: usize,
) -> Result<Vec<u8>, String> {
    let offset = node
        .get("offset")
        .and_then(Value::as_str)
        .ok_or("asar file entry missing offset")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let size = node
        .get("size")
        .and_then(Value::as_u64)
        .ok_or("asar file entry missing size")? as usize;
    let start = data_offset + offset;
    let end = start + size;
    if archive.len() < end {
        return Err("asar file payload is truncated".into());
    }
    Ok(archive[start..end].to_vec())
}

fn find_asar_node<'a>(node: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = node;
    for part in path.split('/').filter(|segment| !segment.is_empty()) {
        current = current.get("files")?.get(part)?;
    }
    Some(current)
}

fn join_archive_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", base, name)
    }
}

fn write_asar_archive(path: &Path, header_json: &[u8], data: &[u8]) -> Result<(), String> {
    let aligned_json_size = header_json.len() + ((4 - (header_json.len() % 4)) % 4);
    let mut output = Vec::with_capacity(16 + aligned_json_size + data.len());
    output.extend_from_slice(&4u32.to_le_bytes());
    output.extend_from_slice(&((aligned_json_size as u32) + 8).to_le_bytes());
    output.extend_from_slice(&((aligned_json_size as u32) + 4).to_le_bytes());
    output.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
    output.extend_from_slice(header_json);
    output.resize(16 + aligned_json_size, 0);
    output.extend_from_slice(data);

    let tmp_path = PathBuf::from(format!("{}.codexpp-new", path.display()));
    fs::write(&tmp_path, output).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, path).map_err(|e| e.to_string())?;
    Ok(())
}

fn update_electron_asar_integrity(meta_path: &Path, hash: &str) -> Result<(), String> {
    let mut plist = PlistValue::from_file(meta_path).map_err(|e| e.to_string())?;
    let dict = plist
        .as_dictionary_mut()
        .ok_or("Info.plist root is not a dictionary")?;
    if !dict.contains_key("ElectronAsarIntegrity") {
        dict.insert(
            "ElectronAsarIntegrity".into(),
            PlistValue::Dictionary(PlistDictionary::new()),
        );
    }
    let integrity = dict
        .get_mut("ElectronAsarIntegrity")
        .ok_or("Failed to get ElectronAsarIntegrity")?;
    let integrity_dict = integrity
        .as_dictionary_mut()
        .ok_or("ElectronAsarIntegrity is not a dictionary")?;
    let mut entry = PlistDictionary::new();
    entry.insert("algorithm".into(), PlistValue::String("SHA256".into()));
    entry.insert("hash".into(), PlistValue::String(hash.into()));
    integrity_dict.insert("Resources/app.asar".into(), PlistValue::Dictionary(entry));
    plist.to_file_xml(meta_path).map_err(|e| e.to_string())
}

fn write_native_installer_state(
    root: &Path,
    codex: &NativeCodexInstall,
    source_dir: &Path,
    patch: &NativePatchOutcome,
    signing: &NativeSigningResult,
    watcher: &str,
) -> Result<(), String> {
    let state = json!({
        "version": read_source_version(source_dir),
        "installedAt": chrono::Utc::now().to_rfc3339(),
        "appRoot": codex.app_root.display().to_string(),
        "codexVersion": read_codex_version(&codex.meta_path),
        "originalAsarHash": patch.original_asar_hash,
        "patchedAsarHash": patch.patched_asar_hash,
        "originalEntryPoint": patch.original_entry_point,
        "watcher": watcher,
        "sourceRoot": source_dir.display().to_string(),
        "resigned": true,
        "signingMode": signing.mode,
        "signingIdentity": signing.identity,
        "signingIdentityHash": signing.identity_hash,
        "fuseFlipped": false,
    });
    write_json_pretty(&root.join("state.json"), &state)
}

fn read_source_version(source_dir: &Path) -> String {
    read_json(&source_dir.join("package.json"))
        .and_then(|value| {
            value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "native-bootstrap".into())
}

fn read_codex_version(meta_path: &Path) -> Option<String> {
    let plist = PlistValue::from_file(meta_path).ok()?;
    plist
        .as_dictionary()?
        .get("CFBundleShortVersionString")?
        .as_string()
        .map(str::to_string)
}

fn record_native_bootstrap_state(
    ctx: &NativeInstallContext,
    path: &Path,
    started_at: &str,
    phase: &str,
    status: &str,
    detail: &str,
    source_dir: Option<&Path>,
    backup_source_dir: Option<&Path>,
) {
    if let Err(error) = write_native_bootstrap_state(
        path,
        started_at,
        phase,
        status,
        detail,
        source_dir,
        backup_source_dir,
    ) {
        let line = format!(
            "Unable to write native bootstrap state at {}: {}",
            path.display(),
            error
        );
        if let Some(log_file) = &ctx.log_file {
            let _ = append_line(log_file, &format!("[stderr] {}", line));
        }
        if let Some(app) = &ctx.app {
            emit_log(app, &ctx.session_id, "stderr", line);
        } else {
            eprintln!("{}", line);
        }
    }
}

fn write_native_bootstrap_state(
    path: &Path,
    started_at: &str,
    phase: &str,
    status: &str,
    detail: &str,
    source_dir: Option<&Path>,
    backup_source_dir: Option<&Path>,
) -> Result<(), String> {
    let state = NativeBootstrapState {
        phase: phase.into(),
        status: status.into(),
        detail: detail.into(),
        started_at: started_at.into(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        source_dir: source_dir.map(|value| value.display().to_string()),
        backup_source_dir: backup_source_dir.map(|value| value.display().to_string()),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    write_json_pretty(
        path,
        &serde_json::to_value(state).map_err(|e| e.to_string())?,
    )
}

fn maybe_fail_native_phase(phase: &str) -> Result<(), String> {
    match std::env::var("GATEWAY_SWITCH_NATIVE_FAIL_PHASE") {
        Ok(value) if value == phase => Err(format!("Injected native install failure at {}", phase)),
        _ => Ok(()),
    }
}

fn initialize_native_config(root: &Path, source_dir: &Path) -> Result<(), String> {
    let config_path = root.join("config.json");
    let mut config = read_json(&config_path).unwrap_or_else(|| json!({}));
    let root_obj = config
        .as_object_mut()
        .ok_or("codex++ config root must be a JSON object")?;
    let codexpp = root_obj.entry("codexPlusPlus").or_insert_with(|| json!({}));
    if !codexpp.is_object() {
        *codexpp = json!({});
    }
    let codexpp_obj = codexpp
        .as_object_mut()
        .ok_or("codexPlusPlus config must be an object")?;
    codexpp_obj
        .entry("autoUpdate")
        .or_insert_with(|| Value::Bool(true));
    codexpp_obj
        .entry("safeMode")
        .or_insert_with(|| Value::Bool(false));
    codexpp_obj
        .entry("uiSafeMode")
        .or_insert_with(|| Value::Bool(false));
    let update = codexpp_obj
        .entry("updateCheck")
        .or_insert_with(|| json!({}));
    if !update.is_object() {
        *update = json!({});
    }
    update
        .as_object_mut()
        .ok_or("updateCheck must be an object")?
        .insert(
            "currentVersion".into(),
            Value::String(read_source_version(source_dir)),
        );
    root_obj.entry("tweaks").or_insert_with(|| json!({}));
    write_json_pretty(&config_path, &config)
}

fn install_cli_shims_native(root: &Path) -> Result<String, String> {
    let shim_dir = root.join("bin");
    fs::create_dir_all(&shim_dir).map_err(|e| e.to_string())?;
    let gateway_switch = native_gateway_switch_executable()?;
    for command in ["codexplusplus", "codex-plusplus"] {
        let shim_path = shim_dir.join(command);
        let script = format!(
            "#!/bin/sh\nexec \"{}\" codexpp \"$@\"\n",
            gateway_switch.display()
        );
        fs::write(&shim_path, script).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        fs::set_permissions(&shim_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    let path_target = select_cli_path_dir();
    if let Some(dir) = &path_target {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        for command in ["codexplusplus", "codex-plusplus"] {
            let source = shim_dir.join(command);
            let target = dir.join(command);
            let _ = fs::remove_file(&target);
            #[cfg(unix)]
            {
                if symlink(&source, &target).is_err() {
                    fs::copy(&source, &target).map_err(|e| e.to_string())?;
                }
            }
        }
    }

    Ok(match path_target {
        Some(dir) => format!(
            "Installed CLI shims to {} and linked into {}",
            shim_dir.display(),
            dir.display()
        ),
        None => format!("Installed CLI shims to {}", shim_dir.display()),
    })
}

fn select_cli_path_dir() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let path_dirs = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    let preferred = [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local").join("bin"),
        home.join("bin"),
    ];
    for dir in preferred {
        if path_dirs.iter().any(|item| item == &dir) && ensure_writable_dir(&dir) {
            return Some(dir);
        }
    }
    let fallback = home.join(".local").join("bin");
    ensure_writable_dir(&fallback).then_some(fallback)
}

fn ensure_writable_dir(path: &Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(format!(".codexpp-probe-{}", now_millis()));
    match fs::write(&probe, b"") {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn sign_codex_app_native(
    ctx: &mut NativeInstallContext,
    codex: &NativeCodexInstall,
    local_signing: bool,
) -> Result<NativeSigningResult, String> {
    let signing = if local_signing {
        let prepared = ensure_local_signing_identity(DEFAULT_LOCAL_SIGNING_IDENTITY)?;
        ctx.log(
            "stdout",
            format!(
                "Using local signing identity {} ({})",
                prepared.identity,
                prepared.identity_hash.as_deref().unwrap_or("?")
            ),
        );
        prepared
    } else {
        NativeSigningResult {
            mode: "adhoc".into(),
            identity: "-".into(),
            identity_hash: None,
        }
    };

    let identity_arg = signing
        .identity_hash
        .as_deref()
        .unwrap_or(signing.identity.as_str());
    sign_unpacked_macho_tree(ctx, &codex.asar_unpacked_path, identity_arg)?;
    run_streaming_command(
        ctx,
        "/usr/bin/codesign",
        &[
            "--force",
            "--deep",
            "--sign",
            identity_arg,
            &codex.app_root.display().to_string(),
        ],
        codex.app_root.parent().unwrap_or(Path::new("/")),
        "codesign bundle",
    )?;
    Ok(signing)
}

fn ensure_local_signing_identity(name: &str) -> Result<NativeSigningResult, String> {
    if let Some(existing) = find_local_signing_identity(name)? {
        return Ok(existing);
    }
    let temp_root = std::env::temp_dir().join(format!("gateway-switch-sign-{}", now_millis()));
    fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    let config_path = temp_root.join("openssl.cnf");
    let key_path = temp_root.join("identity.key");
    let cert_path = temp_root.join("identity.crt");
    let p12_path = temp_root.join("identity.p12");
    let password = uuid::Uuid::new_v4().simple().to_string();
    fs::write(
        &config_path,
        format!(
            "[req]\ndistinguished_name=req_distinguished_name\nx509_extensions=v3_req\nprompt=no\n\n[req_distinguished_name]\nCN={}\n\n[v3_req]\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=codeSigning\n",
            name
        ),
    )
    .map_err(|e| e.to_string())?;
    let keychain = default_user_keychain()?;
    run_quiet_command(
        "openssl",
        &[
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-x509",
            "-sha256",
            "-days",
            "3650",
            "-nodes",
            "-config",
            &config_path.display().to_string(),
            "-keyout",
            &key_path.display().to_string(),
            "-out",
            &cert_path.display().to_string(),
        ],
    )?;
    run_quiet_command(
        "openssl",
        &[
            "pkcs12",
            "-export",
            "-inkey",
            &key_path.display().to_string(),
            "-in",
            &cert_path.display().to_string(),
            "-name",
            name,
            "-out",
            &p12_path.display().to_string(),
            "-keypbe",
            "PBE-SHA1-3DES",
            "-certpbe",
            "PBE-SHA1-3DES",
            "-macalg",
            "sha1",
            "-passout",
            &format!("pass:{}", password),
        ],
    )?;
    run_quiet_command(
        "security",
        &[
            "import",
            &p12_path.display().to_string(),
            "-k",
            &keychain,
            "-P",
            &password,
            "-T",
            "/usr/bin/codesign",
        ],
    )?;
    run_quiet_command(
        "security",
        &[
            "add-trusted-cert",
            "-r",
            "trustRoot",
            "-p",
            "codeSign",
            "-k",
            &keychain,
            &cert_path.display().to_string(),
        ],
    )?;
    let created = find_local_signing_identity(name)?
        .ok_or_else(|| format!("Created signing identity {} was not found", name))?;
    let _ = fs::remove_dir_all(&temp_root);
    Ok(created)
}

fn find_local_signing_identity(name: &str) -> Result<Option<NativeSigningResult>, String> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .map_err(|e| e.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.contains(&format!("\"{}\"", name)) {
            continue;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 3 {
            continue;
        }
        let hash = parts[1].to_string();
        return Ok(Some(NativeSigningResult {
            mode: "local-identity".into(),
            identity: name.to_string(),
            identity_hash: Some(hash),
        }));
    }
    Ok(None)
}

fn default_user_keychain() -> Result<String, String> {
    let output = Command::new("security")
        .args(["default-keychain", "-d", "user"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_string())
}

fn sign_unpacked_macho_tree(
    ctx: &mut NativeInstallContext,
    root: &Path,
    identity: &str,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    for entry in walk_files(root)? {
        if !is_macho_file(&entry) {
            continue;
        }
        run_streaming_command(
            ctx,
            "/usr/bin/codesign",
            &[
                "--force",
                "--sign",
                identity,
                "--preserve-metadata=entitlements,flags",
                &entry.display().to_string(),
            ],
            root,
            "codesign unpacked Mach-O",
        )?;
    }
    Ok(())
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            files.extend(walk_files(&path)?);
        } else if meta.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn is_macho_file(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    if bytes.len() < 4 {
        return false;
    }
    matches!(
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        0xfeedface | 0xfeedfacf | 0xcafebabe | 0xcffaedfe | 0xcefaedfe
    )
}

fn install_watcher_native(root: &Path, codex: &NativeCodexInstall) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Unable to resolve home directory")?;
    let launch_agents = home.join("Library").join("LaunchAgents");
    fs::create_dir_all(&launch_agents).map_err(|e| e.to_string())?;
    let plist_path = launch_agents.join(format!("{}.plist", WATCHER_LABEL));
    let cli_shim = root.join("bin").join("codexplusplus");
    if !cli_shim.exists() {
        return Err(format!(
            "codexplusplus CLI shim not found at {}; run repair again",
            cli_shim.display()
        ));
    }
    let watcher_command = format!(
        "CODEX_PLUSPLUS_WATCHER=1 {} update --watcher --quiet",
        shell_quote(&cli_shim.display().to_string())
    );
    let watcher_log = home
        .join("Library")
        .join("Logs")
        .join("codex-plusplus-watcher.log");
    if let Some(parent) = watcher_log.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        &watcher_log,
        format!(
            "[{}] Gateway Switch installed Codex++ watcher.\n",
            chrono::Utc::now().to_rfc3339()
        ),
    )
    .map_err(|e| e.to_string())?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{label}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>CODEX_PLUSPLUS_WATCHER</key><string>1</string>
  </dict>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>-lc</string>
    <string>{watcher_command}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>WatchPaths</key>
  <array>
    <string>{asar}</string>
  </array>
  <key>WorkingDirectory</key><string>{cwd}</string>
  <key>StandardOutPath</key><string>{stdout}</string>
  <key>StandardErrorPath</key><string>{stderr}</string>
</dict>
</plist>
"#,
        label = WATCHER_LABEL,
        watcher_command = xml_escape(&watcher_command),
        asar = xml_escape(&codex.asar_path.display().to_string()),
        cwd = xml_escape(&root.display().to_string()),
        stdout = xml_escape(&watcher_log.display().to_string()),
        stderr = xml_escape(&watcher_log.display().to_string()),
    );
    fs::write(&plist_path, plist).map_err(|e| e.to_string())?;
    let _ = Command::new("launchctl")
        .args(["unload", &plist_path.display().to_string()])
        .output();
    let load = Command::new("launchctl")
        .args(["load", "-w", &plist_path.display().to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    if !load.status.success() {
        return Err(String::from_utf8_lossy(&load.stderr).trim().to_string());
    }
    Ok("launchd".into())
}

fn clear_quarantine_if_needed(app_root: &Path) {
    let _ = Command::new("xattr")
        .args([
            "-dr",
            "com.apple.quarantine",
            &app_root.display().to_string(),
        ])
        .output();
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn run_quiet_command(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn run_streaming_command(
    ctx: &mut NativeInstallContext,
    program: &str,
    args: &[&str],
    cwd: &Path,
    label: &str,
) -> Result<(), String> {
    let resolved_program = find_command_on_path(program).unwrap_or_else(|| program.to_string());
    let command_path = augmented_command_path();
    let display = format!("{} {}", resolved_program, args.join(" "));
    ctx.log(
        "system",
        format!("{} [{}] (cwd: {})", display, label, cwd.display()),
    );
    ctx.debug(format!("{} uses PATH={}", label, command_path));
    ctx.debug(format!(
        "{} command resolution detail: {}",
        label,
        command_resolution_debug(program)
    ));
    let output = Command::new(&resolved_program)
        .args(args)
        .current_dir(cwd)
        .env("PATH", command_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        ctx.log("stdout", line.to_string());
    }
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        ctx.log("stderr", line.to_string());
    }
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed with exit code {:?}",
            label,
            output.status.code()
        ))
    }
}

fn le_u32(bytes: &[u8]) -> u32 {
    let mut slice = [0_u8; 4];
    slice.copy_from_slice(bytes);
    u32::from_le_bytes(slice)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn command_plan_from_cli(cli: String, args: Vec<&str>, label: &str) -> Result<CommandPlan, String> {
    let (program, argv, display_command) = normalize_cli_command(&cli, &args)?;
    Ok(CommandPlan {
        program,
        args: argv,
        display_command: if display_command.is_empty() {
            format!("{} {}", cli, label)
        } else {
            display_command
        },
    })
}

fn execute_command_plan(
    app: &AppHandle,
    session_id: &str,
    action: String,
    plan: CommandPlan,
) -> Result<CodexPpCliResult, String> {
    emit_log(
        app,
        session_id,
        "system",
        format!("$ {}", plan.display_command),
    );
    let mut child = Command::new(&plan.program)
        .args(&plan.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));

    let stdout_handle = spawn_log_reader(
        app.clone(),
        session_id.to_string(),
        "stdout",
        stdout,
        stdout_buf.clone(),
    );
    let stderr_handle = spawn_log_reader(
        app.clone(),
        session_id.to_string(),
        "stderr",
        stderr,
        stderr_buf.clone(),
    );

    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();
    let code = status.code();
    emit_log(
        app,
        session_id,
        "system",
        match code {
            Some(value) => format!("Process exited with code {}", value),
            None => "Process terminated by signal".into(),
        },
    );
    let stdout_text = stdout_buf
        .lock()
        .map_err(|_| "stdout buffer poisoned")?
        .clone();
    let stderr_text = stderr_buf
        .lock()
        .map_err(|_| "stderr buffer poisoned")?
        .clone();

    Ok(CodexPpCliResult {
        action,
        command: plan.display_command,
        success: status.success(),
        code,
        stdout: stdout_text,
        stderr: stderr_text,
    })
}

fn spawn_log_reader(
    app: AppHandle,
    session_id: String,
    stream: &'static str,
    reader: impl std::io::Read + Send + 'static,
    buffer: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if let Ok(mut buf) = buffer.lock() {
                        buf.push_str(&line);
                        buf.push('\n');
                    }
                    emit_log(&app, &session_id, stream, line);
                }
                Err(error) => {
                    emit_log(
                        &app,
                        &session_id,
                        "system",
                        format!("Failed to read {}: {}", stream, error),
                    );
                    break;
                }
            }
        }
    })
}

fn emit_log(app: &AppHandle, session_id: &str, stream: &str, line: String) {
    let _ = app.emit(
        "codex-pp-cli-log",
        CodexPpLogEvent {
            session_id: session_id.to_string(),
            stream: stream.to_string(),
            line,
        },
    );
}

fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{}", line).map_err(|e| e.to_string())
}

fn write_debug_header(path: &Path, started_at: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        format!(
            "# Gateway Switch codex++ native debug log\n# started_at={}\n",
            started_at
        ),
    )
    .map_err(|e| e.to_string())
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).truncate(true);
    if path.exists() {
        options.create(false);
    } else {
        options.create(true);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())
}

fn normalize_cli_command(
    cli: &str,
    args: &[&str],
) -> Result<(String, Vec<String>, String), String> {
    if cli.ends_with(".js") {
        let node =
            find_command_on_path("node").ok_or("Node.js 20+ is required to run the codex++ CLI")?;
        let mut argv = vec![cli.to_string()];
        argv.extend(args.iter().map(|arg| arg.to_string()));
        let display = std::iter::once(node.clone())
            .chain(argv.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        return Ok((node, argv, display));
    }
    let argv = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    let display = std::iter::once(cli.to_string())
        .chain(argv.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    Ok((cli.to_string(), argv, display))
}

fn resolve_headless_cli_plan(args: &[String]) -> Result<CommandPlan, String> {
    let cli = source_cli_entry_path();
    if !cli.exists() {
        return Err(format!("codex++ CLI source not found at {}", cli.display()));
    }
    let argv = args.iter().map(|value| value.as_str()).collect::<Vec<_>>();
    command_plan_from_cli(cli.display().to_string(), argv, &args.join(" "))
}

fn execute_headless_command_plan(plan: CommandPlan) -> Result<i32, String> {
    let status = Command::new(&plan.program)
        .args(&plan.args)
        .status()
        .map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(1))
}

fn source_cli_entry_path() -> PathBuf {
    user_root()
        .join("source")
        .join("packages")
        .join("installer")
        .join("dist")
        .join("cli.js")
}

fn native_gateway_switch_executable() -> Result<PathBuf, String> {
    if let Ok(override_path) = std::env::var("GATEWAY_SWITCH_EXECUTABLE_OVERRIDE") {
        let path = PathBuf::from(override_path);
        if path.exists() {
            return Ok(path);
        }
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let installed = PathBuf::from(INSTALLED_GATEWAY_SWITCH_EXECUTABLE);
    if installed.exists() && current.starts_with("/Volumes") {
        return Ok(installed);
    }
    if current
        .components()
        .any(|component| component.as_os_str() == "deps")
    {
        if let Some(debug_dir) = current.parent().and_then(Path::parent) {
            let candidate = debug_dir.join("gateway-switch");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Ok(current)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn find_command_on_path(command: &str) -> Option<String> {
    command_search_paths()
        .into_iter()
        .map(|p| p.join(command))
        .find(|p| p.exists())
        .map(|p| p.display().to_string())
}

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

fn augmented_command_path() -> String {
    std::env::join_paths(command_search_paths())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            "/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
                .into()
        })
}

fn command_resolution_debug(command: &str) -> String {
    let raw_path = std::env::var("PATH").unwrap_or_default();
    let augmented_path = augmented_command_path();
    let searched = command_search_paths()
        .into_iter()
        .map(|path| {
            let candidate = path.join(command);
            format!(
                "{}:{}",
                candidate.display(),
                if candidate.exists() {
                    "exists"
                } else {
                    "missing"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "raw_PATH={}, augmented_PATH={}, searched=[{}]",
        raw_path, augmented_path, searched
    )
}

fn detect_tool(command: &str, version_args: &[&str]) -> ToolCheck {
    let Some(path) = find_command_on_path(command) else {
        return ToolCheck {
            status: "error".into(),
            detail: format!("{} not found on PATH", command),
        };
    };
    let detail = Command::new(&path)
        .args(version_args)
        .env("PATH", augmented_command_path())
        .output()
        .ok()
        .and_then(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                (!stderr.is_empty()).then_some(stderr)
            } else {
                Some(stdout)
            }
        })
        .map(|version| format!("{} at {}", version, path))
        .unwrap_or_else(|| format!("found at {}", path));
    ToolCheck {
        status: "ok".into(),
        detail,
    }
}

fn detect_node() -> ToolCheck {
    let Some(path) = find_command_on_path("node") else {
        return ToolCheck {
            status: "error".into(),
            detail: "node not found on PATH; codex++ bootstrap requires Node.js 20+".into(),
        };
    };
    let output = match Command::new(&path)
        .arg("--version")
        .env("PATH", augmented_command_path())
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return ToolCheck {
                status: "error".into(),
                detail: format!("failed to execute node: {}", error),
            };
        }
    };
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let major = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok());
    match major {
        Some(value) if value >= 20 => ToolCheck {
            status: "ok".into(),
            detail: format!("{} at {}", version, path),
        },
        Some(value) => ToolCheck {
            status: "error".into(),
            detail: format!(
                "{} detected at {}; codex++ requires Node.js 20+",
                value, path
            ),
        },
        None => ToolCheck {
            status: "warn".into(),
            detail: format!(
                "unable to parse Node.js version from {} at {}",
                version, path
            ),
        },
    }
}

fn detect_codex_app_path(installed_path: Option<&str>) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(path) = installed_path {
        candidates.push(PathBuf::from(path));
    }
    let home = dirs::home_dir().unwrap_or_default();
    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/Applications/Codex.app"));
        candidates.push(home.join("Applications").join("Codex.app"));
    } else if cfg!(target_os = "windows") {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local_app_data)
                    .join("codex-plusplus")
                    .join("store-apps")
                    .join("Codex"),
            );
        }
    } else {
        candidates.push(home.join(".local").join("share").join("Codex"));
    }
    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
}

fn read_json(path: &Path) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn read_json_typed<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_string))
}

fn find_nested_string(value: &Value, keys: &[&str]) -> Option<String> {
    let mut cur = value;
    for key in keys {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::to_string)
}

fn find_nested_bool(value: &Value, keys: &[&str]) -> Option<bool> {
    let mut cur = value;
    for key in keys {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn is_tweak_enabled(config: &Value, id: &str) -> bool {
    if find_nested_bool(config, &["codexPlusPlus", "safeMode"]).unwrap_or(false) {
        return false;
    }
    if id == UI_IMPROVEMENTS_TWEAK_ID
        && find_nested_bool(config, &["codexPlusPlus", "uiSafeMode"]).unwrap_or(false)
    {
        return false;
    }
    config
        .get("tweaks")
        .and_then(|v| v.get(id))
        .and_then(|v| v.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn resolve_entry(dir: &Path, manifest: &CodexPpManifest) -> Option<PathBuf> {
    if let Some(main) = &manifest.main {
        let path = dir.join(main);
        return path.exists().then_some(path);
    }
    ["index.js", "index.cjs", "index.mjs"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}

fn author_to_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn valid_repo(repo: &str) -> bool {
    let parts: Vec<&str> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
}

fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn find_manifest_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("manifest.json").exists() {
        return Some(dir.to_path_buf());
    }
    for entry in fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            if let Some(found) = find_manifest_root(&path) {
                return Some(found);
            }
        }
    }
    None
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let meta = fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
        if src
            .file_name()
            .is_some_and(|n| n == ".git" || n == "node_modules")
        {
            continue;
        }
        if meta.file_type().is_symlink() {
            #[cfg(unix)]
            {
                let target = fs::read_link(&src).map_err(|e| e.to_string())?;
                let _ = fs::remove_file(&dst);
                symlink(&target, &dst).map_err(|e| e.to_string())?;
            }
            #[cfg(not(unix))]
            {
                let target = fs::canonicalize(&src).map_err(|e| e.to_string())?;
                fs::copy(&target, &dst).map_err(|e| e.to_string())?;
            }
        } else if meta.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn check(name: &str, status: &str, detail: String) -> CodexPpHealthCheck {
    CodexPpHealthCheck {
        name: name.into(),
        status: status.into(),
        detail,
    }
}

fn platform_watcher_checks(install: &CodexPpInstall) -> Vec<CodexPpHealthCheck> {
    let mut checks = Vec::new();
    if cfg!(target_os = "macos") {
        let home = dirs::home_dir().unwrap_or_default();
        let plist = home
            .join("Library")
            .join("LaunchAgents")
            .join("com.codexplusplus.watcher.plist");
        let plist_body = fs::read_to_string(&plist).unwrap_or_default();
        checks.push(check(
            "launchd plist",
            if plist.exists() { "ok" } else { "error" },
            plist.display().to_string(),
        ));
        checks.push(check(
            "launchd label",
            if plist_body.contains("com.codexplusplus.watcher") {
                "ok"
            } else {
                "warn"
            },
            "com.codexplusplus.watcher".into(),
        ));
        if let Some(app_root) = &install.app_root {
            checks.push(check(
                "Codex app",
                if Path::new(app_root).exists() {
                    "ok"
                } else {
                    "error"
                },
                app_root.clone(),
            ));
        }
    }
    checks
}

#[cfg(test)]
mod native_install_acceptance_tests {
    use super::*;

    fn has_real_codex_app() -> bool {
        Path::new("/Applications/Codex.app").exists()
            || dirs::home_dir()
                .map(|home| home.join("Applications").join("Codex.app").exists())
                .unwrap_or(false)
    }

    fn cargo_binary() -> PathBuf {
        find_command_on_path("cargo")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".cargo").join("bin").join("cargo")))
            .expect("cargo should exist")
    }

    fn ensure_gateway_switch_binary() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let binary = manifest_dir
            .join("target")
            .join("debug")
            .join("gateway-switch");
        let status = Command::new(cargo_binary())
            .args(["build", "--bin", "gateway-switch"])
            .current_dir(&manifest_dir)
            .status()
            .expect("cargo build should run");
        assert!(
            status.success(),
            "cargo build --bin gateway-switch should succeed"
        );
        assert!(binary.exists(), "built gateway-switch binary should exist");
        binary
    }

    #[test]
    fn ui_safe_mode_only_disables_page_enhancement() {
        let mut config = json!({
            "codexPlusPlus": {
                "safeMode": false,
                "uiSafeMode": false
            },
            "tweaks": {
                UI_IMPROVEMENTS_TWEAK_ID: { "enabled": true },
                "co.bennett.custom-keyboard-shortcuts": { "enabled": true }
            }
        });

        set_tweak_enabled_in_config(&mut config, UI_IMPROVEMENTS_TWEAK_ID, false)
            .expect("ui safe mode config update should succeed");

        assert_eq!(
            find_nested_bool(&config, &["codexPlusPlus", "uiSafeMode"]),
            Some(true)
        );
        assert!(!is_tweak_enabled(&config, UI_IMPROVEMENTS_TWEAK_ID));
        assert!(is_tweak_enabled(
            &config,
            "co.bennett.custom-keyboard-shortcuts"
        ));

        set_tweak_enabled_in_config(&mut config, UI_IMPROVEMENTS_TWEAK_ID, true)
            .expect("ui safe mode config reset should succeed");

        assert_eq!(
            find_nested_bool(&config, &["codexPlusPlus", "uiSafeMode"]),
            Some(false)
        );
        assert!(is_tweak_enabled(&config, UI_IMPROVEMENTS_TWEAK_ID));
    }

    #[test]
    fn recommended_scripts_report_stays_unknown_without_native_storage() {
        let root =
            std::env::temp_dir().join(format!("gateway-switch-script-report-{}", now_millis()));
        fs::create_dir_all(root.join("runtime")).expect("create runtime");
        let report = recommended_scripts_report_for_root(&root);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(report.storage_mode, "unknown");
        assert!(report.storage_path.is_none());
        assert_eq!(report.scripts.len(), RECOMMENDED_SCRIPTS.len());
        assert!(report
            .scripts
            .iter()
            .all(|script| script.status == "unknown"));
    }

    #[test]
    fn recommended_scripts_report_detects_native_storage_when_runtime_exposes_it() {
        let root =
            std::env::temp_dir().join(format!("gateway-switch-script-storage-{}", now_millis()));
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("create scripts");
        fs::create_dir_all(root.join("runtime")).expect("create runtime");
        fs::write(
            root.join("runtime").join("script-host.js"),
            "ipcMain.handle('codexpp:list-scripts', () => [])",
        )
        .expect("write runtime marker");
        fs::write(scripts.join(RECOMMENDED_SCRIPTS[0].3), "// installed")
            .expect("write installed script");

        let report = recommended_scripts_report_for_root(&root);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(report.storage_mode, "codex_user_scripts");
        assert_eq!(
            report.storage_path.as_deref(),
            Some(scripts.to_str().unwrap())
        );
        assert_eq!(report.scripts[0].status, "installed");
        assert!(report.scripts[1..]
            .iter()
            .all(|script| script.status == "missing"));
    }

    fn sample_store_entry(id: &str, name: &str, repo: &str, sha: &str) -> CodexPpStoreEntry {
        CodexPpStoreEntry {
            id: id.into(),
            manifest: CodexPpManifest {
                id: id.into(),
                name: name.into(),
                version: "1.0.0".into(),
                github_repo: Some(repo.into()),
                description: Some("sample".into()),
                ..Default::default()
            },
            repo: repo.into(),
            approved_commit_sha: sha.into(),
            approved_at: None,
            approved_by: None,
            platforms: None,
            release_url: None,
            review_url: None,
            archive_url: None,
            installed: false,
            installed_version: None,
            installed_path: None,
        }
    }

    #[test]
    fn store_archive_url_requires_full_approved_sha() {
        assert_eq!(
            store_archive_url(
                "b-nnett/codex-plusplus-bennett-ui",
                "17156ac0cc3402284b09c13c74754eda70388f50",
            )
            .unwrap(),
            "https://codeload.github.com/b-nnett/codex-plusplus-bennett-ui/tar.gz/17156ac0cc3402284b09c13c74754eda70388f50"
        );
        assert!(store_archive_url("b-nnett/codex-plusplus-bennett-ui", "17156ac").is_err());
        assert!(store_archive_url("../bad", "17156ac0cc3402284b09c13c74754eda70388f50").is_err());
    }

    #[test]
    fn enrich_store_index_sets_archive_status_and_legacy_mapping() {
        let root =
            std::env::temp_dir().join(format!("gateway-switch-store-index-{}", now_millis()));
        let tweak_dir = root.join("tweaks").join(UI_IMPROVEMENTS_TWEAK_ID);
        fs::create_dir_all(&tweak_dir).expect("create installed tweak");
        fs::write(
            tweak_dir.join("manifest.json"),
            r#"{
              "id": "co.bennett.ui-improvements",
              "name": "Bennett's UI Improvements",
              "version": "1.0.3",
              "githubRepo": "b-nnett/codex-plusplus-bennett-ui",
              "scope": "both"
            }"#,
        )
        .expect("write manifest");

        let mut index = CodexPpStoreIndex {
            schema_version: 1,
            generated_at: Some("test".into()),
            source_url: None,
            fetched_at: None,
            summary: None,
            legacy_recommendations: vec![],
            entries: vec![sample_store_entry(
                UI_IMPROVEMENTS_TWEAK_ID,
                "Bennett's UI Improvements",
                "b-nnett/codex-plusplus-bennett-ui",
                "17156ac0cc3402284b09c13c74754eda70388f50",
            )],
        };
        enrich_store_index(&mut index, &root, "now".into()).expect("enrich store");
        let _ = fs::remove_dir_all(&root);

        assert_eq!(index.source_url.as_deref(), Some(STORE_INDEX_URL));
        assert_eq!(index.fetched_at.as_deref(), Some("now"));
        assert_eq!(index.entries[0].archive_url.as_deref(), Some("https://codeload.github.com/b-nnett/codex-plusplus-bennett-ui/tar.gz/17156ac0cc3402284b09c13c74754eda70388f50"));
        assert!(index.entries[0].installed);
        assert_eq!(index.entries[0].installed_version.as_deref(), Some("1.0.3"));
        assert!(index
            .legacy_recommendations
            .iter()
            .any(|item| item.name == "Codex Token Usage"
                && item.replacement_entry_id.as_deref() == Some(UI_IMPROVEMENTS_TWEAK_ID)));
    }

    #[test]
    fn validate_store_entry_rejects_unpinned_registry_entries() {
        let entry = sample_store_entry("co.example.bad", "Bad", "example/bad", "short-sha");
        assert!(validate_store_entry(&entry).is_err());
    }

    #[test]
    #[ignore]
    fn native_real_install_local_signing_smoke() {
        if !has_real_codex_app() {
            return;
        }
        let binary = ensure_gateway_switch_binary();
        std::env::set_var(
            "GATEWAY_SWITCH_EXECUTABLE_OVERRIDE",
            binary.display().to_string(),
        );
        std::env::remove_var("GATEWAY_SWITCH_NATIVE_FAIL_PHASE");
        let mut ctx = NativeInstallContext::new(None, "native-real-install".into());
        native_install_impl(&mut ctx, true).expect("native install-local should succeed");

        let state = read_json(&user_root().join("state.json")).expect("state.json should exist");
        assert_eq!(
            state.get("signingMode").and_then(Value::as_str),
            Some("local-identity")
        );
        assert_eq!(
            state.get("watcher").and_then(Value::as_str),
            Some("launchd")
        );
        assert!(user_root().join("tweaks").exists());
        assert!(user_root().join("bin").join("codexplusplus").exists());
        let shim = fs::read_to_string(user_root().join("bin").join("codexplusplus"))
            .expect("native CLI shim should be readable");
        assert!(shim.contains("codexpp"));
        assert!(!shim.contains("cli.js"));
        let watcher = fs::read_to_string(
            dirs::home_dir()
                .expect("home directory")
                .join("Library")
                .join("LaunchAgents")
                .join(format!("{}.plist", WATCHER_LABEL)),
        )
        .expect("watcher plist should exist");
        assert!(watcher.contains("CODEX_PLUSPLUS_WATCHER=1"));
        assert!(watcher.contains(" update --watcher --quiet"));
        assert!(watcher.contains("codexplusplus"));
        assert!(!watcher.contains("cli.js"));
        std::env::remove_var("GATEWAY_SWITCH_EXECUTABLE_OVERRIDE");
    }

    #[test]
    #[ignore]
    fn native_real_install_failure_rolls_back_app_asar() {
        if !has_real_codex_app() {
            return;
        }
        let binary = ensure_gateway_switch_binary();
        std::env::set_var(
            "GATEWAY_SWITCH_EXECUTABLE_OVERRIDE",
            binary.display().to_string(),
        );
        let codex = locate_native_codex_install().expect("real Codex.app should exist");
        let before = sha256_hex(&fs::read(&codex.asar_path).expect("read app.asar before install"));
        std::env::set_var("GATEWAY_SWITCH_NATIVE_FAIL_PHASE", "after-backup-app");
        let mut ctx = NativeInstallContext::new(None, "native-real-rollback".into());
        let result = native_install_impl(&mut ctx, false);
        std::env::remove_var("GATEWAY_SWITCH_NATIVE_FAIL_PHASE");
        std::env::remove_var("GATEWAY_SWITCH_EXECUTABLE_OVERRIDE");
        assert!(result.is_err(), "failure injection should abort install");
        let after = sha256_hex(&fs::read(&codex.asar_path).expect("read app.asar after rollback"));
        assert_eq!(before, after, "rollback should restore original app.asar");
    }

    #[test]
    #[ignore]
    fn native_real_repair_smoke() {
        if !has_real_codex_app() {
            return;
        }
        let binary = ensure_gateway_switch_binary();
        let output = Command::new(&binary)
            .args(["codexpp", "repair"])
            .output()
            .expect("native repair binary should run");
        assert!(
            output.status.success(),
            "repair should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let state = read_json(&user_root().join("state.json")).expect("state.json should exist");
        assert_eq!(
            state.get("signingMode").and_then(Value::as_str),
            Some("local-identity")
        );
    }
}
