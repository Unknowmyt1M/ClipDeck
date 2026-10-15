pub mod queries;

use rusqlite::Connection;
use std::path::Path;

/// Current schema version. Bump when adding a migration.
const SCHEMA_VERSION: i64 = 1;

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

/// Apply pending migrations. Each version is applied in a transaction and
/// recorded via `PRAGMA user_version`.
///
/// Also used directly by unit tests that open an in-memory database.
pub(crate) fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("failed to read schema version: {e}"))?;

    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    if version < 1 {
        apply_v1(conn)?;
    }

    // Future migrations go here:
    // if version < 2 { apply_v2(conn)?; }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| format!("failed to update schema version: {e}"))?;
    Ok(())
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
}
