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

/// Result of merging a local export file into the history.
#[derive(Debug, Clone, Default)]
pub struct ImportOutcome {
    pub imported: usize,
    pub skipped_duplicates: usize,
    pub skipped_sensitive: usize,
    pub skipped_images: usize,
    pub invalid: usize,
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
        // Recover from a poisoned mutex instead of panicking every later DB
        // call: the connection itself stays valid after a panic mid-query.
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
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
        self.with_conn(|conn| {
            let removed = queries::delete_item(conn, id)?;
            if removed {
                queries::purge_orphan_sources(conn)?;
            }
            Ok(removed)
        })
    }

    /// Delete every non-pinned row; returns (rows removed, image uuids of
    /// removed rows) so the caller can delete the PNG files as well.
    pub fn clear_history(&self) -> Result<(usize, Vec<String>), String> {
        self.with_conn(queries::clear_history)
    }

    /// All image-item uuids currently in the database (start-up file sweep).
    pub fn image_uuids(&self) -> Result<Vec<String>, String> {
        self.with_conn(queries::image_uuids)
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
            // Retention just dropped rows → their provenance must go too.
            queries::purge_orphan_sources(conn)?;
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

    /// Merge a local export file into the history (import path).
    ///
    /// Each entry is re-hashed from its own content (the file is never
    /// trusted for hash/uuid), de-duplicated against the current history,
    /// re-run through the on-device sensitive detector, and stored with a
    /// fresh uuid — all inside one transaction so a bad file can't leave a
    /// half-imported history. Image entries are skipped: an export carries
    /// no PNG bytes, so importing the row would create a dead thumbnail.
    pub fn import_items(
        &self,
        entries: &[ClipboardItem],
        detect_sensitive: bool,
        skip_sensitive: bool,
    ) -> Result<ImportOutcome, String> {
        use crate::models::clipboard::{ClipboardItemType, make_preview};
        use crate::security::sensitive_detector;
        use rusqlite::params;

        self.with_conn(|conn| {
            let mut out = ImportOutcome::default();
            conn.execute("BEGIN IMMEDIATE", [])
                .map_err(|e| format!("import begin: {e}"))?;
            let result = (|| {
                let now = queries::now_ms();
                for entry in entries {
                    if entry.item_type == ClipboardItemType::Image {
                        out.skipped_images += 1;
                        continue;
                    }
                    if entry.content.trim().is_empty() {
                        out.invalid += 1;
                        continue;
                    }
                    let hash =
                        crate::services::clipboard_service::hash_bytes(entry.content.as_bytes());
                    if queries::find_by_hash(conn, &hash)?.is_some() {
                        out.skipped_duplicates += 1;
                        continue;
                    }
                    let sensitive = entry.is_sensitive
                        || (detect_sensitive
                            && sensitive_detector::detect(&entry.content).is_some());
                    if sensitive && skip_sensitive {
                        out.skipped_sensitive += 1;
                        continue;
                    }
                    let preview = if entry.preview.trim().is_empty() {
                        make_preview(&entry.content)
                    } else {
                        entry.preview.clone()
                    };
                    let created_at = if entry.created_at > 0 {
                        entry.created_at
                    } else {
                        now
                    };
                    let uuid = uuid::Uuid::new_v4().to_string();
                    let item = queries::insert_item_full(
                        conn,
                        &uuid,
                        entry.item_type.as_str(),
                        &entry.content,
                        &hash,
                        &preview,
                        created_at,
                        sensitive,
                        None,
                    )?;
                    // Restore user + usage flags from the export.
                    conn.execute(
                        "UPDATE clipboard_items SET is_pinned = ?1, is_favorite = ?2, \
                         is_archived = ?3, use_count = ?4, last_used_at = ?5, updated_at = ?6 \
                         WHERE id = ?7",
                        params![
                            entry.is_pinned as i64,
                            entry.is_favorite as i64,
                            entry.is_archived as i64,
                            entry.use_count.max(1),
                            entry.last_used_at,
                            now,
                            item.id
                        ],
                    )
                    .map_err(|e| format!("import flags failed: {e}"))?;
                    if !entry.tags.is_empty() {
                        queries::link_tags(conn, item.id, &entry.tags, "manual")?;
                    }
                    if entry.ai_summary.is_some() {
                        settings_queries::save_ai_metadata(
                            conn,
                            item.id,
                            "import",
                            "",
                            &AiAnalysis {
                                summary: entry.ai_summary.clone(),
                                category: entry.ai_category.clone(),
                                keywords: entry.ai_keywords.clone(),
                                tags: vec![],
                                confidence: None,
                            },
                        )?;
                    }
                    out.imported += 1;
                }
                Ok::<(), String>(())
            })();
            match result {
                Ok(()) => conn
                    .execute("COMMIT", [])
                    .map(|_| ())
                    .map_err(|e| format!("import commit: {e}")),
                Err(e) => {
                    let _ = conn.execute("ROLLBACK", []);
                    Err(e)
                }
            }?;
            Ok(out)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::clipboard::ClipboardItemType;

    fn service() -> DatabaseService {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        DatabaseService {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        }
    }

    /// A fake export entry (id/hash deliberately bogus: the import path
    /// must not trust them).
    fn entry(
        uuid: &str,
        item_type: ClipboardItemType,
        content: &str,
        created_at: i64,
    ) -> ClipboardItem {
        ClipboardItem {
            id: 7,
            uuid: uuid.into(),
            item_type,
            content: content.into(),
            content_hash: "bogus-hash-from-file".into(),
            preview: content.into(),
            created_at,
            updated_at: created_at,
            last_used_at: None,
            use_count: 3,
            is_pinned: false,
            is_favorite: false,
            is_archived: false,
            is_sensitive: false,
            ai_summary: None,
            ai_category: None,
            ai_keywords: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn import_roundtrip_dedupes_and_restores_flags() {
        let svc = service();
        let now = queries::now_ms();
        let mut entries = vec![
            entry("u1", ClipboardItemType::Text, "hello import", now),
            entry(
                "u2",
                ClipboardItemType::Url,
                "https://example.com",
                now - 5_000,
            ),
        ];
        entries[0].is_pinned = true;
        entries[0].tags = vec!["keep".into()];
        entries[0].ai_summary = Some("a summary".into());
        entries[1].use_count = 9;

        let out = svc.import_items(&entries, true, false).expect("import");
        assert_eq!(out.imported, 2);
        assert_eq!(out.skipped_duplicates, 0);
        assert_eq!(svc.count_items().expect("count"), 2);

        // Flags + usage + AI summary + tags restored.
        let all = svc.export_items().expect("export");
        let text = all
            .iter()
            .find(|i| i.content == "hello import")
            .expect("text entry");
        assert!(text.is_pinned);
        assert_eq!(text.use_count, 3);
        assert_eq!(text.tags, vec!["keep".to_string()]);
        assert_eq!(text.ai_summary.as_deref(), Some("a summary"));
        let url = all
            .iter()
            .find(|i| i.content == "https://example.com")
            .expect("url entry");
        assert_eq!(url.use_count, 9);

        // Re-import: content hash is recomputed from the content (the
        // bogus file hash is ignored), so everything dedupes.
        let again = svc.import_items(&entries, true, false).expect("re-import");
        assert_eq!(again.imported, 0);
        assert_eq!(again.skipped_duplicates, 2);
        assert_eq!(svc.count_items().expect("count"), 2);
    }

    #[test]
    fn import_skips_images_invalid_and_honors_skip_sensitive() {
        let svc = service();
        let now = queries::now_ms();
        let entries = vec![
            entry("i1", ClipboardItemType::Image, "image:deadbeef", now),
            entry("e1", ClipboardItemType::Text, "   ", now),
            entry(
                "s1",
                ClipboardItemType::Text,
                "password: hunter2-secret-value",
                now,
            ),
        ];

        // With skip_sensitive the secret never lands in the database.
        let out = svc.import_items(&entries, true, true).expect("import skip");
        assert_eq!(out.skipped_images, 1, "images carry no bytes in an export");
        assert_eq!(out.invalid, 1);
        assert_eq!(out.skipped_sensitive, 1);
        assert_eq!(out.imported, 0);
        assert_eq!(svc.count_items().expect("count"), 0);

        // Without skip_sensitive it imports, but re-flagged by the local
        // detector (the file claimed is_sensitive=false).
        let out = svc
            .import_items(&entries, true, false)
            .expect("import keep");
        assert_eq!(out.imported, 1);
        let items = svc.export_items().expect("export");
        assert!(items[0].is_sensitive, "detector must re-flag the secret");
    }

    #[test]
    fn clear_history_reports_image_uuids_for_file_cleanup() {
        let svc = service();
        // Images can't be imported (no PNG bytes in an export) — seed the
        // row the way the live capture path would.
        svc.with_conn(|conn| {
            queries::insert_item(
                conn,
                "img-uuid",
                "image",
                "image:abc",
                "h-img",
                "[Image 1×1]",
                queries::now_ms(),
            )
        })
        .expect("insert image row");

        let (deleted, uuids) = svc.clear_history().expect("clear");
        assert_eq!(deleted, 1);
        assert_eq!(uuids, vec!["img-uuid".to_string()]);
        assert_eq!(svc.image_uuids().expect("uuids").len(), 0);
    }
}
