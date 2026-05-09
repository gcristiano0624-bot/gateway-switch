use std::path::Path;
use chrono::Utc;
use rusqlite::{params, Connection};
use crate::models::*;

pub fn initialize(db: &Path) -> Result<(), String> {
    if let Some(p) = db.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(db).map_err(|e| e.to_string())?;
    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
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
        INSERT INTO gateway_profile (id, listen_host, listen_port, auth_token)
        VALUES ('default', '127.0.0.1', 3456, 'gateway-switch-token')
        ON CONFLICT(id) DO NOTHING;
        CREATE TABLE IF NOT EXISTS codex_routes (
            id TEXT PRIMARY KEY,
            codex_model TEXT NOT NULL,
            display_name TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            upstream_model TEXT NOT NULL,
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
    "#).map_err(|e| e.to_string())?;
    // Seed default claude aliases if table is empty
    {
        let conn = open(db)?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM model_aliases WHERE alias_type='claude'", [], |r| r.get(0)).unwrap_or(0);
        if count == 0 {
            let defaults = [
                "claude-opus-4-7", "claude-opus-4-20250514", "claude-opus-4-0",
                "claude-sonnet-4-6", "claude-sonnet-4-20250514", "claude-sonnet-4-5",
                "claude-sonnet-4-0", "claude-haiku-4-5", "claude-haiku-4-20250414",
                "claude-sonnet-3-7", "claude-sonnet-3-5-v2", "claude-haiku-3-5",
            ];
            for alias in defaults {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, 'claude')",
                    params![id, alias],
                );
            }
        }
        let codex_count: i64 = conn.query_row("SELECT COUNT(*) FROM model_aliases WHERE alias_type='codex'", [], |r| r.get(0)).unwrap_or(0);
        if codex_count == 0 {
            let defaults = ["gpt-4o", "gpt-4o-mini", "o3", "o4-mini", "o3-pro", "gpt-4.1", "gpt-4.1-mini", "gpt-4.1-nano"];
            for alias in defaults {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, 'codex')",
                    params![id, alias],
                );
            }
        }
    }
    Ok(())
}

fn open(db: &Path) -> Result<Connection, String> {
    Connection::open(db).map_err(|e| e.to_string())
}

// ---- Providers ----

pub fn list_providers(db: &Path) -> Result<Vec<Provider>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, base_url, auth_header, auth_scheme, api_key, enabled FROM providers ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Provider {
        id: r.get(0)?,
        name: r.get(1)?,
        base_url: r.get(2)?,
        auth_header: r.get(3)?,
        auth_scheme: r.get(4)?,
        api_key: r.get(5)?,
        enabled: r.get::<_, i64>(6)? == 1,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create_provider(db: &Path, p: &CreateProvider) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "INSERT INTO providers (id, name, base_url, auth_header, auth_scheme, api_key) VALUES (?1,?2,?3,?4,?5,?6)",
        params![p.id, p.name, p.base_url, p.auth_header, p.auth_scheme, p.api_key],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_provider(db: &Path, p: &UpdateProvider) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE providers SET name=?2, base_url=?3, auth_header=?4, auth_scheme=?5, api_key=?6, enabled=?7 WHERE id=?1",
        params![p.id, p.name, p.base_url, p.auth_header, p.auth_scheme, p.api_key, if p.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_provider(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    let refs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM model_routes WHERE provider_id = ?1",
        params![id],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if refs > 0 {
        return Err("Provider is still referenced by model routes".into());
    }
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Model Routes ----

pub fn list_routes(db: &Path) -> Result<Vec<ModelRoute>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, claude_alias, display_name, provider_id, upstream_model, enabled FROM model_routes ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(ModelRoute {
        id: r.get(0)?,
        claude_alias: r.get(1)?,
        display_name: r.get(2)?,
        provider_id: r.get(3)?,
        upstream_model: r.get(4)?,
        enabled: r.get::<_, i64>(5)? == 1,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create_route(db: &Path, r: &CreateModelRoute) -> Result<(), String> {
    let conn = open(db)?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM providers WHERE id = ?1",
        params![r.provider_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("Provider '{}' not found", r.provider_id));
    }
    conn.execute(
        "INSERT INTO model_routes (id, claude_alias, display_name, provider_id, upstream_model) VALUES (?1,?2,?3,?4,?5)",
        params![r.id, r.claude_alias, r.display_name, r.provider_id, r.upstream_model],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_route(db: &Path, r: &UpdateModelRoute) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE model_routes SET claude_alias=?2, display_name=?3, provider_id=?4, upstream_model=?5, enabled=?6 WHERE id=?1",
        params![r.id, r.claude_alias, r.display_name, r.provider_id, r.upstream_model, if r.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_route(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM model_routes WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Gateway Profile ----

pub fn get_profile(db: &Path) -> Result<GatewayProfile, String> {
    let conn = open(db)?;
    conn.query_row(
        "SELECT listen_host, listen_port, auth_token FROM gateway_profile WHERE id = 'default'",
        [],
        |r| Ok(GatewayProfile {
            listen_host: r.get(0)?,
            listen_port: r.get(1)?,
            auth_token: r.get(2)?,
        }),
    ).map_err(|e| e.to_string())
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
    conn.execute(
        "INSERT INTO request_logs (id, request_id, claude_alias, provider_id, upstream_model, status_code, duration_ms, is_stream, error_summary, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            log.request_id, log.request_id, log.claude_alias, log.provider_id,
            log.upstream_model, log.status_code.map(|v| v as i64),
            log.duration_ms.map(|v| v as i64), if log.is_stream {1} else {0},
            log.error_summary, Utc::now().to_rfc3339()
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_logs(db: &Path, limit: usize) -> Result<Vec<RequestLog>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT request_id, claude_alias, provider_id, upstream_model, status_code, duration_ms, is_stream, error_summary, created_at FROM request_logs ORDER BY created_at DESC LIMIT ?1"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![limit as i64], |r| Ok(RequestLog {
        request_id: r.get(0)?,
        claude_alias: r.get(1)?,
        provider_id: r.get(2)?,
        upstream_model: r.get(3)?,
        status_code: r.get::<_, Option<i64>>(4)?.map(|v| v as u16),
        duration_ms: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
        is_stream: r.get::<_, i64>(6)? == 1,
        error_summary: r.get(7)?,
        created_at: r.get(8)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn count_rows(db: &Path, table: &str) -> Result<i64, String> {
    let conn = open(db)?;
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

// ---- Codex Routes ----

pub fn list_codex_routes(db: &Path) -> Result<Vec<CodexRoute>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, codex_model, display_name, provider_id, upstream_model, enabled FROM codex_routes ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(CodexRoute {
        id: r.get(0)?,
        codex_model: r.get(1)?,
        display_name: r.get(2)?,
        provider_id: r.get(3)?,
        upstream_model: r.get(4)?,
        enabled: r.get::<_, i64>(5)? == 1,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create_codex_route(db: &Path, r: &CreateCodexRoute) -> Result<(), String> {
    let conn = open(db)?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM providers WHERE id = ?1",
        params![r.provider_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("Provider '{}' not found", r.provider_id));
    }
    conn.execute(
        "INSERT INTO codex_routes (id, codex_model, display_name, provider_id, upstream_model) VALUES (?1,?2,?3,?4,?5)",
        params![r.id, r.codex_model, r.display_name, r.provider_id, r.upstream_model],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_codex_route(db: &Path, r: &UpdateCodexRoute) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE codex_routes SET codex_model=?2, display_name=?3, provider_id=?4, upstream_model=?5, enabled=?6 WHERE id=?1",
        params![r.id, r.codex_model, r.display_name, r.provider_id, r.upstream_model, if r.enabled {1} else {0}],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_codex_route(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM codex_routes WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Codex Profile ----

pub fn get_codex_profile(db: &Path) -> Result<GatewayProfile, String> {
    let conn = open(db)?;
    conn.query_row(
        "SELECT listen_host, listen_port, auth_token FROM codex_profile WHERE id = 'default'",
        [],
        |r| Ok(GatewayProfile {
            listen_host: r.get(0)?,
            listen_port: r.get(1)?,
            auth_token: r.get(2)?,
        }),
    ).map_err(|e| e.to_string())
}

pub fn save_codex_profile(db: &Path, p: &GatewayProfile) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute(
        "UPDATE codex_profile SET listen_host=?1, listen_port=?2, auth_token=?3 WHERE id='default'",
        params![p.listen_host, p.listen_port, p.auth_token],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Model Aliases ----

pub fn list_model_aliases(db: &Path, alias_type: &str) -> Result<Vec<ModelAlias>, String> {
    let conn = open(db)?;
    let mut stmt = conn.prepare(
        "SELECT id, alias, alias_type, created_at FROM model_aliases WHERE alias_type = ?1 ORDER BY created_at ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![alias_type], |r| Ok(ModelAlias {
        id: r.get(0)?,
        alias: r.get(1)?,
        alias_type: r.get(2)?,
        created_at: r.get(3)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create_model_alias(db: &Path, a: &CreateModelAlias) -> Result<(), String> {
    let conn = open(db)?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO model_aliases (id, alias, alias_type) VALUES (?1, ?2, ?3)",
        params![id, a.alias, a.alias_type],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_model_alias(db: &Path, id: &str) -> Result<(), String> {
    let conn = open(db)?;
    conn.execute("DELETE FROM model_aliases WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}
