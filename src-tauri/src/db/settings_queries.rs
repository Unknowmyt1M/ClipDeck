use rusqlite::{Connection, OptionalExtension, params};

use crate::models::ai::AiMetadata;
use crate::models::settings::Settings;

const SETTING_KEYS: [&str; 21] = [
    "theme",
    "startup",
    "global_hotkey",
    "history_limit",
    "close_to_tray",
    "pause_monitoring",
    "sensitive_detection",
    "skip_sensitive",
    "excluded_apps",
    "excluded_titles",
    "retention_days",
    "ai_enabled",
    "ai_provider",
    "ai_model",
    "ai_model_name",
    "ai_free",
    "ai_auto_analyze",
    "ai_allow_general",
    // Legacy/extra keys tolerated on read:
    "auto_tagging",
    "ai_send_sensitive",
    "onboarding_done",
];

/// Read all settings into a typed struct (missing keys keep defaults).
pub fn get_settings(conn: &Connection) -> Result<Settings, String> {
    let mut settings = Settings::default();
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| format!("get_settings prepare: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("get_settings query: {e}"))?;

    for row in rows.flatten() {
        let (key, value) = row;
        match key.as_str() {
            "theme" => settings.theme = value,
            "startup" => settings.startup = value == "true",
            "global_hotkey" => settings.global_hotkey = value,
            "history_limit" => settings.history_limit = value.parse().unwrap_or(500),
            "close_to_tray" => settings.close_to_tray = value == "true",
            "pause_monitoring" => settings.pause_monitoring = value == "true",
            "sensitive_detection" => settings.sensitive_detection = value == "true",
            "skip_sensitive" => settings.skip_sensitive = value == "true",
            "excluded_apps" => settings.excluded_apps = value,
            "excluded_titles" => settings.excluded_titles = value,
            "retention_days" => settings.retention_days = value.parse().unwrap_or(0),
            "ai_enabled" => settings.ai_enabled = value == "true",
            "ai_provider" => settings.ai_provider = value,
            "ai_model" => settings.ai_model = value,
            "ai_model_name" => settings.ai_model_name = value,
            "ai_free" => settings.ai_free = value == "true",
            "ai_auto_analyze" => settings.ai_auto_analyze = value == "true",
            "ai_allow_general" => settings.ai_allow_general = value == "true",
            _ => {}
        }
    }
    Ok(settings)
}

/// Persist one setting (upsert).
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    if !SETTING_KEYS.contains(&key) {
        return Err(format!("unknown setting key: {key}"));
    }
    let now = crate::db::queries::now_ms();
    conn.execute(
        "INSERT INTO settings (key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?3) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )
    .map_err(|e| format!("set_setting({key}) failed: {e}"))?;
    Ok(())
}

/// Batch upsert for saving a full Settings struct.
pub fn save_settings(conn: &Connection, s: &Settings) -> Result<(), String> {
    let pairs: [(&str, String); 18] = [
        ("theme", s.theme.clone()),
        ("startup", s.startup.to_string()),
        ("global_hotkey", s.global_hotkey.clone()),
        ("history_limit", s.history_limit.to_string()),
        ("close_to_tray", s.close_to_tray.to_string()),
        ("pause_monitoring", s.pause_monitoring.to_string()),
        ("sensitive_detection", s.sensitive_detection.to_string()),
        ("skip_sensitive", s.skip_sensitive.to_string()),
        ("excluded_apps", s.excluded_apps.clone()),
        ("excluded_titles", s.excluded_titles.clone()),
        ("retention_days", s.retention_days.to_string()),
        ("ai_enabled", s.ai_enabled.to_string()),
        ("ai_provider", s.ai_provider.clone()),
        ("ai_model", s.ai_model.clone()),
        ("ai_model_name", s.ai_model_name.clone()),
        ("ai_free", s.ai_free.to_string()),
        ("ai_auto_analyze", s.ai_auto_analyze.to_string()),
        ("ai_allow_general", s.ai_allow_general.to_string()),
    ];
    for (k, v) in &pairs {
        set_setting(conn, k, v)?;
    }
    Ok(())
}

// ---------- FTS search ----------

/// Whether the FTS5 index exists (created in migration v2 when supported).
pub fn fts_available(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='clipboard_fts'",
        [],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

/// Escape a user query into a safe FTS5 MATCH expression (quoted terms, AND).
pub fn build_fts_query(raw: &str) -> String {
    let terms: Vec<String> = raw
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    terms.join(" ")
}

/// FTS5 search returning matching rowids newest-first.
/// Caller must ensure `fts_available`.
pub fn search_fts(
    conn: &Connection,
    raw_query: &str,
    type_filter: Option<&str>,
    limit: i64,
) -> Result<Vec<i64>, String> {
    let match_expr = build_fts_query(raw_query);
    if match_expr.is_empty() {
        return Ok(vec![]);
    }
    let mut sql = String::from(
        "SELECT f.rowid FROM clipboard_fts f JOIN clipboard_items c ON c.id = f.rowid \
         WHERE clipboard_fts MATCH ?1 AND c.is_archived = 0",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(match_expr)];
    if let Some(t) = type_filter {
        sql.push_str(" AND c.type = ?");
        binds.push(Box::new(t.to_string()));
    }
    sql.push_str(" ORDER BY c.created_at DESC LIMIT ?");
    binds.push(Box::new(limit));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("search_fts prepare: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let ids = stmt
        .query_map(params_ref.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| format!("search_fts query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("search_fts rows: {e}"))?;
    Ok(ids)
}

// ---------- AI metadata ----------

/// Upsert AI analysis result for an item.
pub fn save_ai_metadata(
    conn: &Connection,
    clipboard_id: i64,
    provider: &str,
    model: &str,
    analysis: &crate::models::ai::AiAnalysis,
) -> Result<(), String> {
    let now = crate::db::queries::now_ms();
    let keywords = serde_json::to_string(&analysis.keywords).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO ai_metadata (clipboard_id, provider, model, summary, category, keywords, confidence, processed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(clipboard_id) DO UPDATE SET \
            provider = excluded.provider, model = excluded.model, summary = excluded.summary, \
            category = excluded.category, keywords = excluded.keywords, \
            confidence = excluded.confidence, processed_at = excluded.processed_at",
        params![
            clipboard_id,
            provider,
            model,
            analysis.summary,
            analysis.category,
            keywords,
            analysis.confidence,
            now
        ],
    )
    .map_err(|e| format!("save_ai_metadata failed: {e}"))?;
    Ok(())
}

/// Fetch AI metadata for one item.
pub fn get_ai_metadata(conn: &Connection, clipboard_id: i64) -> Result<Option<AiMetadata>, String> {
    conn.query_row(
        "SELECT id, clipboard_id, provider, model, summary, category, keywords, confidence, processed_at \
         FROM ai_metadata WHERE clipboard_id = ?1",
        params![clipboard_id],
        |row| {
            Ok(AiMetadata {
                id: row.get(0)?,
                clipboard_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                summary: row.get(4)?,
                category: row.get(5)?,
                keywords: row.get(6)?,
                confidence: row.get(7)?,
                processed_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("get_ai_metadata failed: {e}"))
}

/// Items waiting for AI analysis (enabled, general content, not yet analyzed).
pub fn pending_ai_items(conn: &Connection, limit: i64) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id FROM clipboard_items c \
             LEFT JOIN ai_metadata m ON m.clipboard_id = c.id \
             WHERE m.id IS NULL AND c.is_sensitive = 0 AND c.type != 'image' \
               AND c.is_archived = 0 AND LENGTH(c.content) BETWEEN 1 AND 6000 \
             ORDER BY c.created_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("pending_ai_items prepare: {e}"))?;
    let ids = stmt
        .query_map(params![limit], |row| row.get(0))
        .map_err(|e| format!("pending_ai_items query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("pending_ai_items rows: {e}"))?;
    Ok(ids)
}

/// Enforce retention: delete non-pinned items older than `days` (0 = keep all).
pub fn prune_retention(conn: &Connection, days: i64) -> Result<usize, String> {
    if days <= 0 {
        return Ok(0);
    }
    let cutoff = crate::db::queries::now_ms() - days * 86_400_000;
    conn.execute(
        "DELETE FROM clipboard_items WHERE is_pinned = 0 AND created_at < ?1",
        params![cutoff],
    )
    .map_err(|e| format!("prune_retention failed: {e}"))
}

/// Enforce history limit: keep newest `limit` non-pinned items.
pub fn prune_history_limit(conn: &Connection, limit: i64) -> Result<usize, String> {
    if limit <= 0 {
        return Ok(0);
    }
    conn.execute(
        "DELETE FROM clipboard_items WHERE is_pinned = 0 AND id NOT IN ( \
            SELECT id FROM clipboard_items WHERE is_pinned = 0 \
            ORDER BY created_at DESC LIMIT ?1 )",
        params![limit],
    )
    .map_err(|e| format!("prune_history_limit failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");

        let s = Settings {
            theme: "dark".into(),
            ai_enabled: true,
            ai_model: "test/model".into(),
            history_limit: 42,
            ..Settings::default()
        };
        save_settings(&conn, &s).expect("save");

        let loaded = get_settings(&conn).expect("load");
        assert_eq!(loaded.theme, "dark");
        assert!(loaded.ai_enabled);
        assert_eq!(loaded.ai_model, "test/model");
        assert_eq!(loaded.history_limit, 42);
    }

    #[test]
    fn rejects_unknown_setting_key() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        assert!(set_setting(&conn, "not_a_key", "x").is_err());
    }

    #[test]
    fn fts_query_quotes_terms() {
        assert_eq!(build_fts_query("hello world"), "\"hello\" \"world\"");
        assert_eq!(build_fts_query("say \"hi\""), "\"say\" \"\"\"hi\"\"\"");
    }

    #[test]
    fn ai_metadata_roundtrip() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");

        let now = crate::db::queries::now_ms();
        let item = crate::db::queries::insert_item(
            &conn,
            "u1",
            "text",
            "hello world",
            "h1",
            "hello world",
            now,
        )
        .expect("insert");

        let analysis = crate::models::ai::AiAnalysis {
            summary: Some("greeting".into()),
            category: Some("Text".into()),
            keywords: vec!["greeting".into()],
            tags: vec![],
            confidence: Some(0.9),
        };
        save_ai_metadata(&conn, item.id, "openrouter", "m1", &analysis).expect("save");
        let meta = get_ai_metadata(&conn, item.id)
            .expect("get")
            .expect("exists");
        assert_eq!(meta.provider, "openrouter");
        assert!(meta.summary.as_deref() == Some("greeting"));
    }

    #[test]
    fn retention_prunes_old_unpinned() {
        let conn = Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        let old = crate::db::queries::now_ms() - 10 * 86_400_000;
        crate::db::queries::insert_item(&conn, "old", "text", "x", "hx", "x", old).expect("ins");
        crate::db::queries::insert_item(
            &conn,
            "new",
            "text",
            "y",
            "hy",
            "y",
            crate::db::queries::now_ms(),
        )
        .expect("ins");
        let removed = prune_retention(&conn, 7).expect("prune");
        assert_eq!(removed, 1);
    }
}
