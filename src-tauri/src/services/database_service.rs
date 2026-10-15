use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::db::{self, queries};
use crate::models::clipboard::ClipboardItem;

/// Owns the SQLite connection and exposes typed operations.
///
/// The connection is wrapped in a Mutex: Tauri commands run on different
/// threads and SQLite connections are not Sync. WAL mode keeps this cheap.
pub struct DatabaseService {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl DatabaseService {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        let conn = db::init_database(&db_path)?;
        Ok(Self {
            conn: Mutex::new(conn),
            db_path,
        })
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let guard = self.conn.lock().expect("database mutex poisoned");
        f(&guard)
    }

    /// Save clipboard content; if the hash already exists, bump usage stats.
    /// Returns the stored item plus whether it was a duplicate.
    pub fn save_item(
        &self,
        item_type: &str,
        content: &str,
        content_hash: &str,
        preview: &str,
    ) -> Result<(ClipboardItem, bool), String> {
        let now = queries::now_ms();
        self.with_conn(|conn| {
            if let Some(existing) = queries::find_by_hash(conn, content_hash)? {
                queries::touch_item(conn, existing.id, now)?;
                let updated = queries::get_item(conn, existing.id)?
                    .ok_or_else(|| format!("touched item {} missing", existing.id))?;
                return Ok((updated, true));
            }
            let item = queries::insert_item(
                conn,
                &uuid::Uuid::new_v4().to_string(),
                item_type,
                content,
                content_hash,
                preview,
                now,
            )?;
            Ok((item, false))
        })
    }

    pub fn list_items(
        &self,
        type_filter: Option<&str>,
        query: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ClipboardItem>, String> {
        self.with_conn(|conn| queries::list_items(conn, type_filter, query, limit))
    }

    pub fn get_item(&self, id: i64) -> Result<Option<ClipboardItem>, String> {
        self.with_conn(|conn| queries::get_item(conn, id))
    }

    /// Bump usage stats for an existing item (e.g. after restore).
    pub fn touch_item(&self, id: i64) -> Result<(), String> {
        let now = queries::now_ms();
        self.with_conn(|conn| queries::touch_item(conn, id, now))
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<ClipboardItem, String> {
        self.with_conn(|conn| queries::set_pinned(conn, id, pinned))
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<ClipboardItem, String> {
        self.with_conn(|conn| queries::set_favorite(conn, id, favorite))
    }

    pub fn delete_item(&self, id: i64) -> Result<bool, String> {
        self.with_conn(|conn| queries::delete_item(conn, id))
    }

    pub fn clear_history(&self) -> Result<usize, String> {
        self.with_conn(queries::clear_history)
    }

    pub fn count_items(&self) -> Result<i64, String> {
        self.with_conn(queries::count_items)
    }
}
