use crate::compatibility;
use crate::models::*;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn initialize(db: &Path) -> Result<(), String> {
    if let Some(p) = db.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(db).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            openai_base_url TEXT,
            anthropic_base_url TEXT,
            auth_header TEXT NOT NULL DEFAULT 'x-api-key',
            auth_scheme TEXT,
            api_key TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS model_routes (
            id TEXT PRIMARY KEY,
            claude_alias TEXT NOT NULL,
            display_name TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            upstream_model TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS gateway_profile (
            id TEXT PRIMARY KEY CHECK (id = 'default'),
            listen_host TEXT NOT NULL,
            listen_port INTEGER NOT NULL,
            auth_token TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS request_logs (
            id TEXT PRIMARY KEY,
            request_id TEXT NOT NULL,
            claude_alias TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            upstream_model TEXT NOT NULL,
            status_code INTEGER,
            duration_ms INTEGER,
            is_stream INTEGER NOT NULL DEFAULT 0,
            error_summary TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS provider_compatibility_policies (
            provider_id TEXT PRIMARY KEY,
            system_to_user INTEGER,
            tool_to_user INTEGER,
            disable_tools INTEGER,
            strip_unsupported_params INTEGER,
            direct_provider_safe INTEGER,
            gateway_route_recommended INTEGER,
            codex_disable_responses INTEGER,
            codex_strict_tool_calls INTEGER,
            codex_strip_reasoning INTEGER,
            notes TEXT,
            updated_by TEXT NOT NULL DEFAULT 'user',
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS request_diagnostic_snapshots (
            request_id TEXT PRIMARY KEY,
            surface TEXT NOT NULL,
            claude_alias TEXT,
            provider_id TEXT,
            upstream_model TEXT,
            status_code INTEGER,
            error_summary TEXT,
            original_payload_json TEXT NOT NULL,
            converted_payload_json TEXT,
            redaction_summary TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO gateway_profile (id, listen_host, listen_port, auth_token)
        VALUES ('default', '127.0.0.1', 3456, 'gateway-switch-token')
        ON CONFLICT(id) DO NOTHING;
        CREATE TABLE IF NOT EXISTS codex_routes (
            id TEXT PRIMARY KEY,
            codex_model TEXT NOT NULL,
            display_name TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            upstream_model TEXT NOT NULL,
            tool_call_mode TEXT NOT NULL DEFAULT 'force_when_tools_present',
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS codex_profile (
            id TEXT PRIMARY KEY CHECK (id = 'default'),
            listen_host TEXT NOT NULL,
            listen_port INTEGER NOT NULL,
            auth_token TEXT NOT NULL
        );
        INSERT INTO codex_profile (id, listen_host, listen_port, auth_token)
        VALUES ('default', '127.0.0.1', 3457, 'gateway-switch-token')
        ON CONFLICT(id) DO NOTHING;
        CREATE TABLE IF NOT EXISTS model_aliases (
            id TEXT PRIMARY KEY,
            alias TEXT NOT NULL,
            alias_type TEXT NOT NULL CHECK (alias_type IN ('claude', 'codex')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
    "#,
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute("ALTER TABLE providers ADD COLUMN openai_base_url TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE providers ADD COLUMN anthropic_base_url TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE codex_routes ADD COLUMN tool_call_mode TEXT NOT NULL DEFAULT 'force_when_tools_present'",
        [],
    );
    let _ = conn.execute("ALTER TABLE providers ADD COLUMN priority INTEGER NOT NULL DEFAULT 100", []);
    let _ = conn.execute("ALTER TABLE model_routes ADD COLUMN failover_enabled INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE model_routes ADD COLUMN failover_provider_id TEXT", []);
    let _ = conn.execute("ALTER TABLE codex_routes ADD COLUMN failover_enabled INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE codex_routes ADD COLUMN failover_provider_id TEXT", []);
    // C3: persisted Codex binding mode — 'relay' (route through the local
    // gateway) or 'official' (restore ChatGPT login). 'api-mix' is intentionally
    // not supported (no Codex mechanism under single-binding).
    let _ = conn.execute(
        "ALTER TABLE codex_profile ADD COLUMN bind_mode TEXT NOT NULL DEFAULT 'relay'",
        [],
    );
    conn.execute(
        "UPDATE providers SET openai_base_url = COALESCE(NULLIF(openai_base_url, ''), base_url)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE providers SET anthropic_base_url = 'https://token-plan-sgp.xiaomimimo.com/anthropic' WHERE (lower(id) IN ('xiaomi', 'xiaomimo') OR lower(name) LIKE '%xiaomimo%' OR lower(name) LIKE '%mimo%') AND (anthropic_base_url IS NULL OR anthropic_base_url = '')",
        [],
    ).map_err(|e| e.to_string())?;
    // Seed default claude aliases if table is empty
    {
        let conn = open(db)?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM model_aliases WHERE alias_type='claude'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if count == 0 {
            let defaults = [
                "claude-opus-4-7",
                "claude-opus-4-20250514",
                "claude-opus-4-0",
                "claude-sonnet-4-6",
                "claude-sonnet-4-20250514",
                "claude-sonnet-4-5",
                "claude-sonnet-4-0",
                "claude-haiku-4-5",
                "claude-haiku-4-20250414",
                "claude-sonnet-3-7",
                "claude-sonnet-3-5-v2",
                "claude-haiku-3-5",
            ];
            for alias in defaults {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, 'claude')",
                    params![id, alias],
                );
            }
        }
        let codex_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM model_aliases WHERE alias_type='codex'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if codex_count == 0 {
            let defaults = [
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.6-luna",
                "gpt-5.5",
                "gpt-5.3-codex",
                "gpt-5.1-codex",
                "gpt-5.1-codex-mini",
            ];
            for alias in defaults {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, 'codex')",
                    params![id, alias],
                );
            }
        } else {
            // Idempotently backfill the 2026 Codex catalog for existing installs
            // that predate the ChatGPT-merge model rename, without disturbing any
            // user-created aliases.
            let backfill = [
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.6-luna",
                "gpt-5.5",
                "gpt-5.3-codex",
                "gpt-5.1-codex",
                "gpt-5.1-codex-mini",
            ];
            for alias in backfill {
                let exists: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM model_aliases WHERE alias_type='codex' AND alias=?1",
                        params![alias],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if exists == 0 {
                    let id = uuid::Uuid::new_v4().to_string();
                    let _ = conn.execute(
                        "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, 'codex')",
                        params![id, alias],
                    );
                }
            }
        }
    }
    Ok(())
}

fn open(db: &Path) -> Result<Connection, String> {
    Connection::open(db).map_err(|e| e.to_string())
}

fn normalize_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn bool_to_i64(value: Option<bool>) -> Option<i64> {
    value.map(|v| if v { 1 } else { 0 })
}

fn i64_to_bool(value: Option<i64>) -> Option<bool> {
    value.map(|v| v == 1)
}

// ---- Providers ----

pub fn list_providers(db: &Path) -> Result<Vec<Provider>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, base_url, COALESCE(NULLIF(openai_base_url, ''), base_url), NULLIF(anthropic_base_url, ''), auth_header, auth_scheme, api_key, enabled, COALESCE(priority, 100) FROM providers ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Provider {
                id: r.get(0)?,
                name: r.get(1)?,
                base_url: r.get(2)?,
                openai_base_url: r.get(3)?,
                anthropic_base_url: r.get(4)?,
                auth_header: r.get(5)?,
                auth_scheme: r.get(6)?,
                api_key: r.get(7)?,
                enabled: r.get::<_, i64>(8)? == 1,
                priority: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn create_provider(db: &Path, p: &CreateProvider) -> Result<(), String> {
    let conn = open(db)?;
    let openai_base_url = p
        .openai_base_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&p.base_url);
    let anthropic_base_url = normalize_empty(p.anthropic_base_url.as_deref());
    conn.execute(
        "INSERT INTO providers (id, name, base_url, openai_base_url, anthropic_base_url, auth_header, auth_scheme, api_key) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![p.id.trim(), p.name.trim(), p.base_url.trim(), openai_base_url.trim(), anthropic_base_url, p.auth_header.trim(), p.auth_scheme, p.api_key],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_provider(db: &Path, p: &UpdateProvider) -> Result<(), String> {
    let conn = open(db)?;
    let openai_base_url = p
        .openai_base_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&p.base_url);
    let anthropic_base_url = normalize_empty(p.anthropic_base_url.as_deref());
    conn.execute(
        "UPDATE providers SET name=?2, base_url=?3, openai_base_url=?4, anthropic_base_url=?5, auth_header=?6, auth_scheme=?7, api_key=?8, enabled=?9 WHERE id=?1",
        params![p.id.trim(), p.name.trim(), p.base_url.trim(), openai_base_url.trim(), anthropic_base_url, p.auth_header.trim(), p.auth_scheme, p.api_key, if p.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_provider(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    let refs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM model_routes WHERE provider_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if refs > 0 {
        return Err("Provider is still referenced by model routes".into());
    }
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Model Routes ----

pub fn list_routes(db: &Path) -> Result<Vec<ModelRoute>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, claude_alias, display_name, provider_id, upstream_model, enabled, COALESCE(failover_enabled, 0), failover_provider_id FROM model_routes ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ModelRoute {
                id: r.get(0)?,
                claude_alias: r.get(1)?,
                display_name: r.get(2)?,
                provider_id: r.get(3)?,
                upstream_model: r.get(4)?,
                enabled: r.get::<_, i64>(5)? == 1,
                failover_enabled: r.get::<_, i64>(6)? == 1,
                failover_provider_id: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn create_route(db: &Path, r: &CreateModelRoute) -> Result<(), String> {
    let conn = open(db)?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM providers WHERE id = ?1",
            params![r.provider_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("Provider '{}' not found", r.provider_id));
    }
    conn.execute(
        "INSERT INTO model_routes (id, claude_alias, display_name, provider_id, upstream_model) VALUES (?1,?2,?3,?4,?5)",
        params![r.id.trim(), r.claude_alias.trim(), r.display_name.trim(), r.provider_id.trim(), r.upstream_model.trim()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_route(db: &Path, r: &UpdateModelRoute) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE model_routes SET claude_alias=?2, display_name=?3, provider_id=?4, upstream_model=?5, enabled=?6 WHERE id=?1",
        params![r.id.trim(), r.claude_alias.trim(), r.display_name.trim(), r.provider_id.trim(), r.upstream_model.trim(), if r.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_route(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM model_routes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Gateway Profile ----

pub fn get_profile(db: &Path) -> Result<GatewayProfile, String> {
    let conn = open(db)?;
    conn.query_row(
        "SELECT listen_host, listen_port, auth_token FROM gateway_profile WHERE id = 'default'",
        [],
        |r| {
            Ok(GatewayProfile {
                listen_host: r.get(0)?,
                listen_port: r.get(1)?,
                auth_token: r.get(2)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn save_profile(db: &Path, p: &GatewayProfile) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE gateway_profile SET listen_host=?1, listen_port=?2, auth_token=?3 WHERE id='default'",
        params![p.listen_host, p.listen_port, p.auth_token],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Request Logs ----

pub fn insert_log(db: &Path, log: &RequestLog) -> Result<(), String> {
    let conn = open(db)?;
    let error_summary = log
        .error_summary
        .as_deref()
        .map(compatibility::redact_log_summary);
    conn.execute(
        "INSERT INTO request_logs (id, request_id, claude_alias, provider_id, upstream_model, status_code, duration_ms, is_stream, error_summary, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            log.request_id, log.request_id, log.claude_alias, log.provider_id,
            log.upstream_model, log.status_code.map(|v| v as i64),
            log.duration_ms.map(|v| v as i64), if log.is_stream {1} else {0},
            error_summary, Utc::now().to_rfc3339()
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_logs(db: &Path, limit: usize) -> Result<Vec<RequestLog>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT request_id, claude_alias, provider_id, upstream_model, status_code, duration_ms, is_stream, error_summary, created_at FROM request_logs ORDER BY created_at DESC LIMIT ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(RequestLog {
                request_id: r.get(0)?,
                claude_alias: r.get(1)?,
                provider_id: r.get(2)?,
                upstream_model: r.get(3)?,
                status_code: r.get::<_, Option<i64>>(4)?.map(|v| v as u16),
                duration_ms: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                is_stream: r.get::<_, i64>(6)? == 1,
                error_summary: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

// ---- Compatibility Policies ----

pub fn list_provider_policies(db: &Path) -> Result<Vec<ProviderCompatibilityPolicy>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT provider_id, system_to_user, tool_to_user, disable_tools, strip_unsupported_params, direct_provider_safe, gateway_route_recommended, codex_disable_responses, codex_strict_tool_calls, codex_strip_reasoning, notes, updated_by, updated_at FROM provider_compatibility_policies ORDER BY updated_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ProviderCompatibilityPolicy {
                provider_id: r.get(0)?,
                system_to_user: i64_to_bool(r.get(1)?),
                tool_to_user: i64_to_bool(r.get(2)?),
                disable_tools: i64_to_bool(r.get(3)?),
                strip_unsupported_params: i64_to_bool(r.get(4)?),
                direct_provider_safe: i64_to_bool(r.get(5)?),
                gateway_route_recommended: i64_to_bool(r.get(6)?),
                codex_disable_responses: i64_to_bool(r.get(7)?),
                codex_strict_tool_calls: i64_to_bool(r.get(8)?),
                codex_strip_reasoning: i64_to_bool(r.get(9)?),
                notes: r.get(10)?,
                updated_by: r.get(11)?,
                updated_at: r.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_provider_policy(
    db: &Path,
    provider_id: &str,
) -> Result<Option<ProviderCompatibilityPolicy>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT provider_id, system_to_user, tool_to_user, disable_tools, strip_unsupported_params, direct_provider_safe, gateway_route_recommended, codex_disable_responses, codex_strict_tool_calls, codex_strip_reasoning, notes, updated_by, updated_at FROM provider_compatibility_policies WHERE provider_id = ?1"
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query(params![provider_id])
        .map_err(|e| e.to_string())?;
    if let Some(r) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(ProviderCompatibilityPolicy {
            provider_id: r.get(0).map_err(|e| e.to_string())?,
            system_to_user: i64_to_bool(r.get(1).map_err(|e| e.to_string())?),
            tool_to_user: i64_to_bool(r.get(2).map_err(|e| e.to_string())?),
            disable_tools: i64_to_bool(r.get(3).map_err(|e| e.to_string())?),
            strip_unsupported_params: i64_to_bool(r.get(4).map_err(|e| e.to_string())?),
            direct_provider_safe: i64_to_bool(r.get(5).map_err(|e| e.to_string())?),
            gateway_route_recommended: i64_to_bool(r.get(6).map_err(|e| e.to_string())?),
            codex_disable_responses: i64_to_bool(r.get(7).map_err(|e| e.to_string())?),
            codex_strict_tool_calls: i64_to_bool(r.get(8).map_err(|e| e.to_string())?),
            codex_strip_reasoning: i64_to_bool(r.get(9).map_err(|e| e.to_string())?),
            notes: r.get(10).map_err(|e| e.to_string())?,
            updated_by: r.get(11).map_err(|e| e.to_string())?,
            updated_at: r.get(12).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

pub fn upsert_provider_policy(
    db: &Path,
    policy: &ProviderCompatibilityPolicy,
) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "INSERT INTO provider_compatibility_policies (provider_id, system_to_user, tool_to_user, disable_tools, strip_unsupported_params, direct_provider_safe, gateway_route_recommended, codex_disable_responses, codex_strict_tool_calls, codex_strip_reasoning, notes, updated_by, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(provider_id) DO UPDATE SET
            system_to_user=excluded.system_to_user,
            tool_to_user=excluded.tool_to_user,
            disable_tools=excluded.disable_tools,
            strip_unsupported_params=excluded.strip_unsupported_params,
            direct_provider_safe=excluded.direct_provider_safe,
            gateway_route_recommended=excluded.gateway_route_recommended,
            codex_disable_responses=excluded.codex_disable_responses,
            codex_strict_tool_calls=excluded.codex_strict_tool_calls,
            codex_strip_reasoning=excluded.codex_strip_reasoning,
            notes=excluded.notes,
            updated_by=excluded.updated_by,
            updated_at=excluded.updated_at",
        params![
            policy.provider_id.trim(),
            bool_to_i64(policy.system_to_user),
            bool_to_i64(policy.tool_to_user),
            bool_to_i64(policy.disable_tools),
            bool_to_i64(policy.strip_unsupported_params),
            bool_to_i64(policy.direct_provider_safe),
            bool_to_i64(policy.gateway_route_recommended),
            bool_to_i64(policy.codex_disable_responses),
            bool_to_i64(policy.codex_strict_tool_calls),
            bool_to_i64(policy.codex_strip_reasoning),
            policy.notes,
            policy.updated_by,
            Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reset_provider_policy(db: &Path, provider_id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "DELETE FROM provider_compatibility_policies WHERE provider_id = ?1",
        params![provider_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Request Diagnostic Snapshots ----

pub fn insert_request_snapshot(
    db: &Path,
    snapshot: &RequestDiagnosticSnapshot,
) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "INSERT OR REPLACE INTO request_diagnostic_snapshots (request_id, surface, claude_alias, provider_id, upstream_model, status_code, error_summary, original_payload_json, converted_payload_json, redaction_summary, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            snapshot.request_id,
            snapshot.surface,
            snapshot.claude_alias,
            snapshot.provider_id,
            snapshot.upstream_model,
            snapshot.status_code.map(|v| v as i64),
            snapshot.error_summary.as_deref().map(compatibility::redact_log_summary),
            snapshot.original_payload_json,
            snapshot.converted_payload_json,
            snapshot.redaction_summary,
            Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_failed_request_snapshots(
    db: &Path,
    limit: usize,
) -> Result<Vec<FailedRequestDiagnosticCandidate>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT request_id, surface, claude_alias, provider_id, upstream_model, status_code, error_summary, redaction_summary, created_at FROM request_diagnostic_snapshots ORDER BY created_at DESC LIMIT ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(FailedRequestDiagnosticCandidate {
                request_id: r.get(0)?,
                surface: r.get(1)?,
                claude_alias: r.get(2)?,
                provider_id: r.get(3)?,
                upstream_model: r.get(4)?,
                status_code: r.get::<_, Option<i64>>(5)?.map(|v| v as u16),
                error_summary: r.get(6)?,
                redaction_summary: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_request_snapshot(
    db: &Path,
    request_id: &str,
) -> Result<Option<RequestDiagnosticSnapshot>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT request_id, surface, claude_alias, provider_id, upstream_model, status_code, error_summary, original_payload_json, converted_payload_json, redaction_summary, created_at FROM request_diagnostic_snapshots WHERE request_id = ?1"
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![request_id]).map_err(|e| e.to_string())?;
    if let Some(r) = rows.next().map_err(|e| e.to_string())? {
        Ok(Some(RequestDiagnosticSnapshot {
            request_id: r.get(0).map_err(|e| e.to_string())?,
            surface: r.get(1).map_err(|e| e.to_string())?,
            claude_alias: r.get(2).map_err(|e| e.to_string())?,
            provider_id: r.get(3).map_err(|e| e.to_string())?,
            upstream_model: r.get(4).map_err(|e| e.to_string())?,
            status_code: r
                .get::<_, Option<i64>>(5)
                .map_err(|e| e.to_string())?
                .map(|v| v as u16),
            error_summary: r.get(6).map_err(|e| e.to_string())?,
            original_payload_json: r.get(7).map_err(|e| e.to_string())?,
            converted_payload_json: r.get(8).map_err(|e| e.to_string())?,
            redaction_summary: r.get(9).map_err(|e| e.to_string())?,
            created_at: r.get(10).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn count_rows(db: &Path, table: &str) -> Result<i64, String> {
    let conn = open(db)?;
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

// ---- Codex Routes ----

pub fn list_codex_routes(db: &Path) -> Result<Vec<CodexRoute>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, codex_model, display_name, provider_id, upstream_model, COALESCE(NULLIF(tool_call_mode, ''), 'force_when_tools_present'), enabled, COALESCE(failover_enabled, 0), failover_provider_id FROM codex_routes ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CodexRoute {
                id: r.get(0)?,
                codex_model: r.get(1)?,
                display_name: r.get(2)?,
                provider_id: r.get(3)?,
                upstream_model: r.get(4)?,
                tool_call_mode: r.get(5)?,
                enabled: r.get::<_, i64>(6)? == 1,
                failover_enabled: r.get::<_, i64>(7)? == 1,
                failover_provider_id: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn create_codex_route(db: &Path, r: &CreateCodexRoute) -> Result<(), String> {
    let conn = open(db)?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM providers WHERE id = ?1",
            params![r.provider_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("Provider '{}' not found", r.provider_id));
    }
    let tool_call_mode = normalize_tool_call_mode(r.tool_call_mode.as_deref());
    conn.execute(
        "INSERT INTO codex_routes (id, codex_model, display_name, provider_id, upstream_model, tool_call_mode) VALUES (?1,?2,?3,?4,?5,?6)",
        params![r.id.trim(), r.codex_model.trim(), r.display_name.trim(), r.provider_id.trim(), r.upstream_model.trim(), tool_call_mode],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_codex_route(db: &Path, r: &UpdateCodexRoute) -> Result<(), String> {
    let conn = open(db)?;
    let tool_call_mode = normalize_tool_call_mode(r.tool_call_mode.as_deref());
    conn.execute(
        "UPDATE codex_routes SET codex_model=?2, display_name=?3, provider_id=?4, upstream_model=?5, tool_call_mode=?6, enabled=?7 WHERE id=?1",
        params![r.id.trim(), r.codex_model.trim(), r.display_name.trim(), r.provider_id.trim(), r.upstream_model.trim(), tool_call_mode, if r.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn normalize_tool_call_mode(mode: Option<&str>) -> &'static str {
    match mode.unwrap_or("").trim() {
        "auto" => "auto",
        "strict_execution" => "strict_execution",
        _ => "force_when_tools_present",
    }
}

pub fn delete_codex_route(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM codex_routes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Codex Profile ----

pub fn get_codex_profile(db: &Path) -> Result<GatewayProfile, String> {
    let conn = open(db)?;
    conn.query_row(
        "SELECT listen_host, listen_port, auth_token FROM codex_profile WHERE id = 'default'",
        [],
        |r| {
            Ok(GatewayProfile {
                listen_host: r.get(0)?,
                listen_port: r.get(1)?,
                auth_token: r.get(2)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn save_codex_profile(db: &Path, p: &GatewayProfile) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE codex_profile SET listen_host=?1, listen_port=?2, auth_token=?3 WHERE id='default'",
        params![p.listen_host, p.listen_port, p.auth_token],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the persisted Codex binding mode ('relay' or 'official'). Defaults to
/// 'relay' when the column is missing or empty.
pub fn get_codex_bind_mode(db: &Path) -> Result<String, String> {
    let conn = open(db)?;
    let mode: String = conn
        .query_row(
            "SELECT COALESCE(NULLIF(bind_mode, ''), 'relay') FROM codex_profile WHERE id = 'default'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "relay".to_string());
    Ok(mode)
}

pub fn set_codex_bind_mode(db: &Path, mode: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE codex_profile SET bind_mode=?1 WHERE id='default'",
        params![mode],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Model Aliases ----

pub fn list_model_aliases(db: &Path, alias_type: &str) -> Result<Vec<ModelAlias>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, alias, alias_type, created_at FROM model_aliases WHERE alias_type = ?1 ORDER BY created_at ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![alias_type], |r| {
            Ok(ModelAlias {
                id: r.get(0)?,
                alias: r.get(1)?,
                alias_type: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn create_model_alias(db: &Path, a: &CreateModelAlias) -> Result<(), String> {
    let conn = open(db)?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, ?3)",
        params![id, a.alias, a.alias_type],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_model_alias(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM model_aliases WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_policy_round_trip_preserves_nullable_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("gateway.db");
        initialize(&db).unwrap();

        upsert_provider_policy(
            &db,
            &ProviderCompatibilityPolicy {
                provider_id: "openrouter".into(),
                system_to_user: Some(true),
                tool_to_user: None,
                disable_tools: Some(false),
                strip_unsupported_params: None,
                direct_provider_safe: Some(false),
                gateway_route_recommended: Some(true),
                codex_disable_responses: Some(true),
                codex_strict_tool_calls: None,
                codex_strip_reasoning: Some(true),
                notes: Some("manual override".into()),
                updated_by: "test".into(),
                updated_at: None,
            },
        )
        .unwrap();

        let policy = get_provider_policy(&db, "openrouter").unwrap().unwrap();
        assert_eq!(policy.system_to_user, Some(true));
        assert_eq!(policy.tool_to_user, None);
        assert_eq!(policy.disable_tools, Some(false));
        assert_eq!(policy.codex_strip_reasoning, Some(true));

        reset_provider_policy(&db, "openrouter").unwrap();
        assert!(get_provider_policy(&db, "openrouter").unwrap().is_none());
    }

    #[test]
    fn request_snapshot_round_trip_redacts_error_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("gateway.db");
        initialize(&db).unwrap();

        insert_request_snapshot(
            &db,
            &RequestDiagnosticSnapshot {
                request_id: "req-1".into(),
                surface: "claude_messages".into(),
                claude_alias: Some("claude-sonnet".into()),
                provider_id: Some("volcengine".into()),
                upstream_model: Some("deepseek-v4-pro".into()),
                status_code: Some(400),
                error_summary: Some("bad api_key sk-test-token".into()),
                original_payload_json: r#"{"api_key":"redacted by caller"}"#.into(),
                converted_payload_json: Some(r#"{"messages":[]}"#.into()),
                redaction_summary: "redacted 1 sensitive field".into(),
                created_at: None,
            },
        )
        .unwrap();

        let list = list_failed_request_snapshots(&db, 10).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].request_id, "req-1");
        assert!(!list[0]
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains("sk-test-token"));

        let snapshot = get_request_snapshot(&db, "req-1").unwrap().unwrap();
        assert_eq!(snapshot.status_code, Some(400));
        assert_eq!(
            snapshot.converted_payload_json.as_deref(),
            Some(r#"{"messages":[]}"#)
        );
    }
}
