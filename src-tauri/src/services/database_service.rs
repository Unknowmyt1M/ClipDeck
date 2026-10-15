use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::db::{self, queries, settings_queries};
use crate::models::ai::{AiAnalysis, AiMetadata};
use crate::models::clipboard::ClipboardItem;
use crate::models::stats::Stats;
use crate::services::search_service::SearchService;
use crate::services::source_service::SourceInfo;

/// Extra per-capture data (privacy + provenance).
#[derive(Debug, Clone, Default)]
pub struct CaptureMeta {
    /// Pre-flagged sensitive by the detector (never sent to AI).
    pub sensitive: bool,
    /// Foreground application at capture time.
    pub source: Option<SourceInfo>,
}

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
        meta: &CaptureMeta,
    ) -> Result<(ClipboardItem, bool), String> {
        let now = queries::now_ms();
        self.with_conn(|conn| {
            if let Some(existing) = queries::find_by_hash(conn, content_hash)? {
                queries::touch_item(conn, existing.id, now)?;
                if meta.sensitive && !existing.is_sensitive {
                    queries::set_sensitive(conn, existing.id, true)?;
                }
                let updated = queries::get_item(conn, existing.id)?
                    .ok_or_else(|| format!("touched item {} missing", existing.id))?;
                return Ok((updated, true));
            }

            let source_id = match &meta.source {
                Some(src) => Some(queries::insert_source(conn, src, now)?),
                None => None,
            };
            let item = queries::insert_item_full(
                conn,
                &uuid::Uuid::new_v4().to_string(),
                item_type,
                content,
                content_hash,
                preview,
                now,
                meta.sensitive,
                source_id,
            )?;
            Ok((item, false))
        })
    }

    /// History listing with FTS5-accelerated search when available.
    pub fn list_items(
        &self,
        type_filter: Option<&str>,
        query: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ClipboardItem>, String> {
        self.with_conn(|conn| SearchService::search(conn, query, type_filter, Some(limit)))
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

    /// Enforce retention + history limit after a capture.
    /// Returns the uuids still present for image items so the caller can
    /// sweep orphaned image files on disk.
    pub fn prune(&self, history_limit: i64, retention_days: i64) -> Result<Vec<String>, String> {
        self.with_conn(|conn| {
            settings_queries::prune_retention(conn, retention_days)?;
            settings_queries::prune_history_limit(conn, history_limit)?;
            conn.prepare("SELECT uuid FROM clipboard_items WHERE type = 'image'")
                .map_err(|e| format!("prune prepare: {e}"))?
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| format!("prune query: {e}"))?
                .flatten()
                .collect::<Vec<_>>()
                .pipe(Ok)
        })
    }

    // ---------- Settings ----------

    pub fn load_settings(&self) -> Result<crate::models::settings::Settings, String> {
        self.with_conn(settings_queries::get_settings)
    }

    pub fn persist_settings(&self, s: &crate::models::settings::Settings) -> Result<(), String> {
        self.with_conn(|conn| settings_queries::save_settings(conn, s))
    }

    // ---------- AI ----------

    pub fn pending_ai_items(&self, limit: i64) -> Result<Vec<i64>, String> {
        self.with_conn(|conn| settings_queries::pending_ai_items(conn, limit))
    }

    pub fn save_ai_metadata(
        &self,
        clipboard_id: i64,
        provider: &str,
        model: &str,
        analysis: &AiAnalysis,
    ) -> Result<(), String> {
        self.with_conn(|conn| {
            settings_queries::save_ai_metadata(conn, clipboard_id, provider, model, analysis)
        })
    }

    pub fn get_ai_metadata(&self, clipboard_id: i64) -> Result<Option<AiMetadata>, String> {
        self.with_conn(|conn| settings_queries::get_ai_metadata(conn, clipboard_id))
    }

    pub fn add_ai_tags(&self, clipboard_id: i64, tags: &[String]) -> Result<(), String> {
        self.with_conn(|conn| queries::link_tags(conn, clipboard_id, tags, "ai"))
    }

    // ---------- Stats / export / wipe ----------

    pub fn stats(&self) -> Result<Stats, String> {
        self.with_conn(queries::stats)
    }

    pub fn export_items(&self) -> Result<Vec<ClipboardItem>, String> {
        self.with_conn(queries::all_items)
    }

    /// Delete everything: history rows + image files handled by caller.
    /// Returns (rows_deleted, image_uuids).
    pub fn wipe(&self) -> Result<(usize, Vec<String>), String> {
        self.with_conn(queries::wipe_all)
    }
}

/// Tiny pipe helper to keep the image_uuids body flat.
trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}
