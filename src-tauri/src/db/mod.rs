pub mod queries;
pub mod settings_queries;

use rusqlite::Connection;
use std::path::Path;

/// Current schema version. Bump when adding a migration.
const SCHEMA_VERSION: i64 = 2;

/// Open (or create) the SQLite database at `path` and apply migrations.
pub fn init_database(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create database directory: {e}"))?;
    }

    let conn = Connection::open(path).map_err(|e| format!("failed to open database: {e}"))?;

    // WAL keeps readers non-blocking while writes happen — important for a
    // clipboard app that writes frequently while the UI reads the history.
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("failed to enable WAL mode: {e}"))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("failed to set synchronous mode: {e}"))?;

    migrate(&conn)?;
    Ok(conn)
}

/// Apply pending migrations inside a single transaction, recorded via
/// `PRAGMA user_version` (also written inside the transaction, so a failed
/// migration rolls back completely instead of leaving a half-created schema
/// that would brick the next start-up).
///
/// Also used directly by unit tests that open an in-memory database.
pub(crate) fn migrate(conn: &Connection) -> Result<(), String> {
    // Per-connection switch that makes the `ON DELETE CASCADE` clauses in
    // the schema actually work (clear/delete currently leave ai_metadata
    // and clipboard_tags rows behind without it). Must be set outside any
    // transaction.
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("failed to enable foreign keys: {e}"))?;

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("failed to read schema version: {e}"))?;

    if version >= SCHEMA_VERSION {
        // Legacy DBs could have been stamped at this version with a
        // partially created FTS (the old migration swallowed errors after
        // writing user_version). Heal it before returning.
        if table_exists(conn, "clipboard_fts") && !fts_fully_wired(conn) {
            drop_partial_fts(conn, "healed partial FTS index from a legacy database")?;
        }
        return Ok(());
    }

    // A legacy start-up could have left a partially created v1 schema behind
    // (previous migrations were not transactional). user_version still being
    // 0 means no completed migration exists — drop the leftovers so apply_v1
    // can run clean.
    if version < 1 && table_exists(conn, "clipboard_items") {
        drop_partial_v1(conn)?;
    }

    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| format!("failed to start migration transaction: {e}"))?;
    let result = (|| {
        if version < 1 {
            apply_v1(conn)?;
        }
        if version < 2 {
            apply_v2(conn)?;
        }
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|e| format!("failed to update schema version: {e}"))?;
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute("COMMIT", [])
            .map(|_| ())
            .map_err(|e| format!("failed to commit migration: {e}")),
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

/// Remove a half-created v1 schema left by an old non-transactional
/// migration attempt (objects in reverse dependency order, all conditional).
fn drop_partial_v1(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        DROP TRIGGER IF EXISTS clipboard_items_ai;
        DROP TRIGGER IF EXISTS clipboard_items_ad;
        DROP TRIGGER IF EXISTS clipboard_items_au;
        DROP TABLE IF EXISTS clipboard_fts;
        DROP TABLE IF EXISTS ai_metadata;
        DROP TABLE IF EXISTS clipboard_tags;
        DROP TABLE IF EXISTS tags;
        DROP TABLE IF EXISTS settings;
        DROP TABLE IF EXISTS sources;
        DROP TABLE IF EXISTS clipboard_items;
        "#,
    )
    .map_err(|e| format!("failed to drop partial v1 schema: {e}"))
}

/// Migration v1: initial clipboard history schema.
fn apply_v1(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE clipboard_items (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            uuid          TEXT NOT NULL UNIQUE,
            type          TEXT NOT NULL DEFAULT 'text',
            content       TEXT NOT NULL,
            content_hash  TEXT NOT NULL,
            preview       TEXT NOT NULL DEFAULT '',
            created_at    INTEGER NOT NULL,
            updated_at    INTEGER NOT NULL,
            last_used_at  INTEGER,
            use_count     INTEGER NOT NULL DEFAULT 1,
            is_pinned     INTEGER NOT NULL DEFAULT 0,
            is_favorite   INTEGER NOT NULL DEFAULT 0,
            is_archived   INTEGER NOT NULL DEFAULT 0,
            is_sensitive  INTEGER NOT NULL DEFAULT 0,
            source_id     INTEGER
        );

        CREATE INDEX idx_clipboard_items_hash ON clipboard_items(content_hash);
        CREATE INDEX idx_clipboard_items_created_at ON clipboard_items(created_at DESC);
        CREATE INDEX idx_clipboard_items_pinned ON clipboard_items(is_pinned DESC, created_at DESC);

        CREATE TABLE sources (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            application_name      TEXT,
            application_executable TEXT,
            window_title          TEXT,
            url                   TEXT,
            created_at            INTEGER NOT NULL
        );

        CREATE TABLE tags (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL UNIQUE,
            color      TEXT,
            source     TEXT NOT NULL DEFAULT 'manual',
            created_at INTEGER NOT NULL
        );

        CREATE TABLE clipboard_tags (
            clipboard_id INTEGER NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
            tag_id       INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (clipboard_id, tag_id)
        );

        CREATE TABLE settings (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| format!("migration v1 failed: {e}"))
}

/// Migration v2: AI metadata (stored separately from clipboard data) and
/// FTS5 search index. FTS5 may be unavailable in a stock SQLite build —
/// fall back gracefully and remember to skip FTS queries.
fn apply_v2(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS ai_metadata (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            clipboard_id  INTEGER NOT NULL UNIQUE REFERENCES clipboard_items(id) ON DELETE CASCADE,
            provider      TEXT NOT NULL,
            model         TEXT NOT NULL,
            summary       TEXT,
            category      TEXT,
            keywords      TEXT,
            confidence    REAL,
            embedding     BLOB,
            embedding_model TEXT,
            processed_at  INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| format!("migration v2 (ai_metadata) failed: {e}"))?;

    // FTS5 virtual table over clipboard item text. Content column mirrors
    // clipboard_items so we can rebuild with 'rebuild' when needed.
    // FTS5 may fail partway (table created, a trigger failed — or vice
    // versa). A half-wired index would break writes/reads subtly, so verify
    // the full set landed; otherwise drop the remnants and let
    // `search_service` degrade to LIKE.
    let fts_result = conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
            content,
            preview,
            content='clipboard_items',
            content_rowid='id',
            tokenize='unicode61'
        );

        -- Keep FTS in sync via content-table triggers.
        CREATE TRIGGER IF NOT EXISTS clipboard_items_ai AFTER INSERT ON clipboard_items BEGIN
            INSERT INTO clipboard_fts(rowid, content, preview) VALUES (new.id, new.content, new.preview);
        END;
        CREATE TRIGGER IF NOT EXISTS clipboard_items_ad AFTER DELETE ON clipboard_items BEGIN
            INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
                VALUES ('delete', old.id, old.content, old.preview);
        END;
        CREATE TRIGGER IF NOT EXISTS clipboard_items_au AFTER UPDATE ON clipboard_items BEGIN
            INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
                VALUES ('delete', old.id, old.content, old.preview);
            INSERT INTO clipboard_fts(rowid, content, preview) VALUES (new.id, new.content, new.preview);
        END;
        "#,
    );
    match fts_result {
        Ok(()) => {
            if fts_fully_wired(conn) {
                Ok(())
            } else {
                drop_partial_fts(conn, "FTS5 created without its sync triggers")
            }
        }
        Err(e) => drop_partial_fts(conn, &format!("FTS5 unavailable, LIKE fallback: {e}")),
    }
}

/// The FTS5 table exists **and** all three sync triggers are in place.
fn fts_fully_wired(conn: &Connection) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE \
             (type='table' AND name='clipboard_fts') OR \
             (type='trigger' AND name IN ('clipboard_items_ai','clipboard_items_ad','clipboard_items_au'))",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count == 4
}

/// Remove any partially created FTS objects so the app falls back to LIKE
/// search instead of failing on inconsistent FTS writes.
fn drop_partial_fts(conn: &Connection, reason: &str) -> Result<(), String> {
    let _ = conn.execute_batch(
        r#"
        DROP TRIGGER IF EXISTS clipboard_items_ai;
        DROP TRIGGER IF EXISTS clipboard_items_ad;
        DROP TRIGGER IF EXISTS clipboard_items_au;
        DROP TABLE IF EXISTS clipboard_fts;
        "#,
    );
    crate::services::logger::error("db", reason);
    Ok(()) // Non-fatal: search_service degrades to LIKE.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_schema() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        // migrate works on any connection; init_database needs a path,
        // so test migrate directly.
        migrate(&conn).expect("migrate");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='clipboard_items'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        migrate(&conn).expect("first migrate");
        migrate(&conn).expect("second migrate");
    }

    /// A half-created v1 schema (crash mid-migration) must be detected and
    /// rebuilt cleanly instead of permanently failing every start-up.
    #[test]
    fn migrate_recovers_from_partial_v1() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        // Simulate an old non-transactional migration that died after the
        // first table.
        conn.execute_batch("CREATE TABLE clipboard_items (id INTEGER PRIMARY KEY);")
            .expect("partial schema");
        migrate(&conn).expect("migrate recovers");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='clipboard_items'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, SCHEMA_VERSION);
        // The real schema is complete (column check).
        let cols: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('clipboard_items') WHERE name='content_hash'",
                [],
                |row| row.get(0),
            )
            .expect("cols");
        assert_eq!(cols, 1);
    }

    /// `foreign_keys=ON` must be active on migrated connections so
    /// `ON DELETE CASCADE` actually clears dependent AI/tag rows.
    #[test]
    fn migrate_enables_foreign_keys_cascade() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        migrate(&conn).expect("migrate");

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("pragma");
        assert_eq!(fk, 1);

        let item = crate::db::queries::insert_item(&conn, "u1", "text", "hi", "h1", "hi", 1)
            .expect("insert");
        let analysis = crate::models::ai::AiAnalysis {
            summary: Some("s".into()),
            category: None,
            keywords: vec![],
            tags: vec![],
            confidence: None,
        };
        crate::db::settings_queries::save_ai_metadata(&conn, item.id, "p", "m", &analysis)
            .expect("meta");
        crate::db::queries::delete_item(&conn, item.id).expect("delete");

        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_metadata", [], |row| row.get(0))
            .expect("count");
        assert_eq!(left, 0, "ai_metadata must cascade with the item");
    }
}
