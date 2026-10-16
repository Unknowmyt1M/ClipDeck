use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::models::clipboard::ClipboardItem;

const ITEM_COLUMNS: &str = "id, uuid, type, content, content_hash, preview, created_at, \
    updated_at, last_used_at, use_count, is_pinned, is_favorite, is_archived, is_sensitive";

fn row_to_item(row: &Row) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        item_type: crate::models::clipboard::ClipboardItemType::from_str(
            &row.get::<_, String>("type")?,
        ),
        content: row.get("content")?,
        content_hash: row.get("content_hash")?,
        preview: row.get("preview")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        last_used_at: row.get("last_used_at")?,
        use_count: row.get("use_count")?,
        is_pinned: row.get::<_, i64>("is_pinned")? != 0,
        is_favorite: row.get::<_, i64>("is_favorite")? != 0,
        is_archived: row.get::<_, i64>("is_archived")? != 0,
        is_sensitive: row.get::<_, i64>("is_sensitive")? != 0,
        // AI fields are attached in a second pass (`attach_ai`) to keep
        // every base query join-free.
        ai_summary: None,
        ai_category: None,
        ai_keywords: Vec::new(),
        tags: Vec::new(),
    })
}

/// (summary, category, keywords-json) triple stored in `ai_metadata`.
type AiMetaRow = (Option<String>, Option<String>, Option<String>);

/// Attach AI summary/category/keywords and tag names for a batch of items.
pub fn attach_ai(conn: &Connection, items: &mut [ClipboardItem]) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; items.len()].join(",");
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();

    let mut meta: std::collections::HashMap<i64, AiMetaRow> = std::collections::HashMap::new();
    {
        let sql = format!(
            "SELECT clipboard_id, summary, category, keywords FROM ai_metadata \
             WHERE clipboard_id IN ({placeholders})"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("attach_ai metadata prepare: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(|e| format!("attach_ai metadata query: {e}"))?;
        for row in rows.flatten() {
            meta.insert(row.0, (row.1, row.2, row.3));
        }
    }

    let mut tags: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    {
        let sql = format!(
            "SELECT ct.clipboard_id, t.name FROM clipboard_tags ct \
             JOIN tags t ON t.id = ct.tag_id \
             WHERE ct.clipboard_id IN ({placeholders}) \
             ORDER BY t.name"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("attach_ai tags prepare: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| format!("attach_ai tags query: {e}"))?;
        for row in rows.flatten() {
            tags.entry(row.0).or_default().push(row.1);
        }
    }

    for item in items.iter_mut() {
        if let Some((summary, category, keywords)) = meta.get(&item.id) {
            item.ai_summary = summary.clone();
            item.ai_category = category.clone();
            item.ai_keywords = keywords
                .as_deref()
                .and_then(|k| serde_json::from_str::<Vec<String>>(k).ok())
                .unwrap_or_default();
        }
        if let Some(list) = tags.get(&item.id) {
            item.tags = list.clone();
        }
    }
    Ok(())
}

/// Find an item by content hash. Duplicates update the existing row instead
/// of creating a new entry.
pub fn find_by_hash(conn: &Connection, hash: &str) -> Result<Option<ClipboardItem>, String> {
    conn.query_row(
        &format!("SELECT {ITEM_COLUMNS} FROM clipboard_items WHERE content_hash = ?1"),
        params![hash],
        row_to_item,
    )
    .optional()
    .map_err(|e| format!("find_by_hash failed: {e}"))
}

/// Insert a new clipboard item and return it.
pub fn insert_item(
    conn: &Connection,
    uuid: &str,
    item_type: &str,
    content: &str,
    content_hash: &str,
    preview: &str,
    created_at: i64,
) -> Result<ClipboardItem, String> {
    insert_item_full(
        conn,
        uuid,
        item_type,
        content,
        content_hash,
        preview,
        created_at,
        false,
        None,
    )
}

/// Insert with privacy flag + optional source provenance.
#[allow(clippy::too_many_arguments)]
pub fn insert_item_full(
    conn: &Connection,
    uuid: &str,
    item_type: &str,
    content: &str,
    content_hash: &str,
    preview: &str,
    created_at: i64,
    sensitive: bool,
    source_id: Option<i64>,
) -> Result<ClipboardItem, String> {
    conn.execute(
        "INSERT INTO clipboard_items (uuid, type, content, content_hash, preview, created_at, updated_at, is_sensitive, source_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8)",
        params![uuid, item_type, content, content_hash, preview, created_at, sensitive as i64, source_id],
    )
    .map_err(|e| format!("insert_item failed: {e}"))?;

    let id = conn.last_insert_rowid();
    get_item(conn, id)?.ok_or_else(|| format!("inserted item {id} not found"))
}

/// Record which application produced a capture; returns the source id.
pub fn insert_source(
    conn: &Connection,
    src: &crate::services::source_service::SourceInfo,
    created_at: i64,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO sources (application_name, application_executable, window_title, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            src.application_name,
            src.application_executable,
            src.window_title,
            created_at
        ],
    )
    .map_err(|e| format!("insert_source failed: {e}"))?;
    Ok(conn.last_insert_rowid())
}

/// Mark (or unmark) an item as sensitive.
pub fn set_sensitive(conn: &Connection, id: i64, sensitive: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE clipboard_items SET is_sensitive = ?1, updated_at = ?2 WHERE id = ?3",
        params![sensitive as i64, now_ms(), id],
    )
    .map_err(|e| format!("set_sensitive failed: {e}"))?;
    Ok(())
}

/// Bump usage stats for a duplicate capture (use_count + last_used_at).
pub fn touch_item(conn: &Connection, id: i64, now: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE clipboard_items SET use_count = use_count + 1, last_used_at = ?1, updated_at = ?1 \
         WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| format!("touch_item failed: {e}"))?;
    Ok(())
}

/// Get a single item by id (AI fields attached).
pub fn get_item(conn: &Connection, id: i64) -> Result<Option<ClipboardItem>, String> {
    let mut item = conn
        .query_row(
            &format!("SELECT {ITEM_COLUMNS} FROM clipboard_items WHERE id = ?1"),
            params![id],
            row_to_item,
        )
        .optional()
        .map_err(|e| format!("get_item failed: {e}"))?;
    if let Some(item) = item.as_mut() {
        attach_ai(conn, std::slice::from_mut(item))?;
    }
    Ok(item)
}

/// Load many items in one SELECT (+ a single `attach_ai` pass) preserving
/// the order of `ids` — the batched alternative to calling `get_item` in a
/// loop, which ran 3 queries per row. Archived items are skipped, matching
/// `list_items`.
pub fn get_items_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<ClipboardItem>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT {ITEM_COLUMNS} FROM clipboard_items \
         WHERE id IN ({placeholders}) AND is_archived = 0"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("get_items_by_ids prepare: {e}"))?;
    let mut items = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), row_to_item)
        .map_err(|e| format!("get_items_by_ids query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("get_items_by_ids rows: {e}"))?;
    attach_ai(conn, &mut items)?;

    // `IN (...)` gives no ordering guarantee; restore the caller's order
    // (FTS rank) explicitly.
    let order: std::collections::HashMap<i64, usize> =
        ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    items.sort_by_key(|item| order.get(&item.id).copied().unwrap_or(usize::MAX));
    Ok(items)
}

/// List items with optional filter and search query, newest first.
///
/// `type_filter` accepts a content type (`text`/`url`/`image`) plus the
/// pseudo-filters `pinned`, `favorites`, `today` and `week`.
pub fn list_items(
    conn: &Connection,
    type_filter: Option<&str>,
    query: Option<&str>,
    limit: i64,
) -> Result<Vec<ClipboardItem>, String> {
    let mut sql = format!("SELECT {ITEM_COLUMNS} FROM clipboard_items WHERE is_archived = 0");
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(t) = type_filter {
        match t {
            "pinned" => sql.push_str(" AND is_pinned = 1"),
            "favorites" => sql.push_str(" AND is_favorite = 1"),
            "today" => {
                sql.push_str(" AND COALESCE(last_used_at, created_at) >= ?");
                binds.push(Box::new(start_of_today_ms()));
            }
            "week" => {
                sql.push_str(" AND COALESCE(last_used_at, created_at) >= ?");
                binds.push(Box::new(now_ms() - 7 * 86_400_000));
            }
            other => {
                sql.push_str(" AND type = ?");
                binds.push(Box::new(other.to_string()));
            }
        }
    }
    if let Some(q) = query.filter(|q| !q.trim().is_empty()) {
        sql.push_str(" AND (content LIKE ? ESCAPE '\\')");
        let pattern = format!("%{}%", escape_like(q.trim()));
        binds.push(Box::new(pattern));
    }
    sql.push_str(" ORDER BY COALESCE(last_used_at, created_at) DESC LIMIT ?");
    binds.push(Box::new(limit));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("list_items prepare failed: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let mut items = stmt
        .query_map(params_ref.as_slice(), row_to_item)
        .map_err(|e| format!("list_items query failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_items row failed: {e}"))?;
    attach_ai(conn, &mut items)?;
    Ok(items)
}

/// UTC timestamp of local midnight (start of "today"), computed from the
/// Windows time zone so the Today filter matches the user's calendar day.
fn start_of_today_ms() -> i64 {
    use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    /// Win32 defines this locally (not exported by windows-sys).
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;

    let offset_ms: i64 = unsafe {
        let mut tzi = std::mem::MaybeUninit::<TIME_ZONE_INFORMATION>::uninit();
        let which = GetTimeZoneInformation(tzi.as_mut_ptr());
        let tzi = tzi.assume_init();
        let extra = if which == TIME_ZONE_ID_DAYLIGHT {
            tzi.DaylightBias
        } else {
            tzi.StandardBias
        };
        // Windows bias: UTC = local + bias minutes.
        (i64::from(tzi.Bias) + i64::from(extra)) * 60_000
    };

    let now = now_ms();
    let local_wall = now - offset_ms;
    let local_day_start = local_wall - local_wall.rem_euclid(86_400_000);
    local_day_start + offset_ms
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Toggle the pinned flag; returns the updated item.
pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<ClipboardItem, String> {
    conn.execute(
        "UPDATE clipboard_items SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
        params![pinned as i64, now_ms(), id],
    )
    .map_err(|e| format!("set_pinned failed: {e}"))?;
    get_item(conn, id)?.ok_or_else(|| format!("item {id} not found after pin"))
}

/// Toggle the favorite flag; returns the updated item.
pub fn set_favorite(conn: &Connection, id: i64, favorite: bool) -> Result<ClipboardItem, String> {
    conn.execute(
        "UPDATE clipboard_items SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3",
        params![favorite as i64, now_ms(), id],
    )
    .map_err(|e| format!("set_favorite failed: {e}"))?;
    get_item(conn, id)?.ok_or_else(|| format!("item {id} not found after favorite"))
}

/// Delete an item by id. Returns true if a row was removed.
pub fn delete_item(conn: &Connection, id: i64) -> Result<bool, String> {
    let n = conn
        .execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
        .map_err(|e| format!("delete_item failed: {e}"))?;
    Ok(n > 0)
}

/// Delete all non-pinned items. Returns number of rows removed.
pub fn clear_history(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM clipboard_items WHERE is_pinned = 0", [])
        .map_err(|e| format!("clear_history failed: {e}"))
}

/// Count total items.
pub fn count_items(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM clipboard_items", [], |row| row.get(0))
        .map_err(|e| format!("count_items failed: {e}"))
}

/// Link tag names to an item, creating missing tags (idempotent).
pub fn link_tags(
    conn: &Connection,
    item_id: i64,
    tag_names: &[String],
    source: &str,
) -> Result<(), String> {
    if tag_names.is_empty() {
        return Ok(());
    }
    let now = now_ms();
    for raw in tag_names {
        let name = raw.trim().to_lowercase();
        if name.is_empty() || name.chars().count() > 40 {
            continue;
        }
        conn.execute(
            "INSERT INTO tags (name, source, created_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(name) DO NOTHING",
            params![name, source, now],
        )
        .map_err(|e| format!("link_tags insert tag: {e}"))?;
        let tag_id: i64 = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
                r.get(0)
            })
            .map_err(|e| format!("link_tags lookup: {e}"))?;
        conn.execute(
            "INSERT INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2) \
             ON CONFLICT DO NOTHING",
            params![item_id, tag_id],
        )
        .map_err(|e| format!("link_tags link: {e}"))?;
    }
    Ok(())
}

/// Aggregated usage statistics (never includes clipboard content beyond
/// already-public previews).
pub fn stats(conn: &Connection) -> Result<crate::models::stats::Stats, String> {
    use crate::models::stats::{Stats, TopItem};
    let mut s = Stats::default();

    conn.query_row(
        "SELECT COUNT(*), \
            SUM(is_pinned), \
            SUM(is_favorite), \
            SUM(is_sensitive), \
            SUM(CASE WHEN type = 'image' THEN 1 ELSE 0 END), \
            MIN(created_at) \
         FROM clipboard_items",
        [],
        |row| {
            s.total = row.get(0)?;
            s.pinned = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
            s.favorites = row.get::<_, Option<i64>>(2)?.unwrap_or(0);
            s.sensitive = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
            s.images = row.get::<_, Option<i64>>(4)?.unwrap_or(0);
            s.oldest_at = row.get(5)?;
            Ok(())
        },
    )
    .map_err(|e| format!("stats aggregate failed: {e}"))?;

    s.today = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE COALESCE(last_used_at, created_at) >= ?1",
            params![start_of_today_ms()],
            |row| row.get(0),
        )
        .map_err(|e| format!("stats today failed: {e}"))?;

    s.ai_processed = conn
        .query_row("SELECT COUNT(*) FROM ai_metadata", [], |row| row.get(0))
        .map_err(|e| format!("stats ai failed: {e}"))?;

    {
        let mut stmt = conn
            .prepare(
                "SELECT id, preview, use_count, type FROM clipboard_items \
                 WHERE is_archived = 0 ORDER BY use_count DESC, last_used_at DESC LIMIT 5",
            )
            .map_err(|e| format!("stats top prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TopItem {
                    id: row.get(0)?,
                    preview: row.get(1)?,
                    use_count: row.get(2)?,
                    item_type: row.get(3)?,
                })
            })
            .map_err(|e| format!("stats top query: {e}"))?;
        s.top_used = rows.flatten().collect();
    }

    Ok(s)
}

/// All items (any archive state) for local export.
pub fn all_items(conn: &Connection) -> Result<Vec<ClipboardItem>, String> {
    let mut items = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} FROM clipboard_items ORDER BY created_at DESC"
        ))
        .map_err(|e| format!("all_items prepare: {e}"))?
        .query_map([], row_to_item)
        .map_err(|e| format!("all_items query: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("all_items rows: {e}"))?;
    attach_ai(conn, &mut items)?;
    Ok(items)
}

/// Irreversibly delete every clipboard row (images handled by caller).
/// Returns (rows_deleted, image_uuids_that_need_file_cleanup).
pub fn wipe_all(conn: &Connection) -> Result<(usize, Vec<String>), String> {
    let uuids = {
        let mut stmt = conn
            .prepare("SELECT uuid FROM clipboard_items WHERE type = 'image'")
            .map_err(|e| format!("wipe_all prepare: {e}"))?;
        stmt.query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("wipe_all query: {e}"))?
            .flatten()
            .collect::<Vec<_>>()
    };
    let n = conn
        .execute("DELETE FROM clipboard_items", [])
        .map_err(|e| format!("wipe_all failed: {e}"))?;
    Ok((n, uuids))
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::clipboard::{detect_type, make_preview};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        crate::db::migrate(&conn).expect("migrate");
        conn
    }

    fn insert_text(conn: &Connection, content: &str) -> ClipboardItem {
        let hash = format!("sha-test-{}", content.len());
        let item_type = detect_type(content);
        insert_item(
            conn,
            &uuid::Uuid::new_v4().to_string(),
            item_type.as_str(),
            content,
            &hash,
            &make_preview(content),
            now_ms(),
        )
        .expect("insert")
    }

    #[test]
    fn insert_and_get() {
        let conn = test_conn();
        let item = insert_text(&conn, "hello world");
        let fetched = get_item(&conn, item.id).expect("get").expect("exists");
        assert_eq!(fetched.content, "hello world");
        assert_eq!(
            fetched.item_type,
            crate::models::clipboard::ClipboardItemType::Text
        );
    }

    #[test]
    fn touch_increments_use_count() {
        let conn = test_conn();
        let item = insert_text(&conn, "once");
        touch_item(&conn, item.id, now_ms()).expect("touch");
        let updated = get_item(&conn, item.id).expect("get").expect("exists");
        assert_eq!(updated.use_count, 2);
    }

    #[test]
    fn list_applies_type_filter_and_query() {
        let conn = test_conn();
        insert_text(&conn, "https://example.com");
        insert_text(&conn, "some plain text");
        let urls = list_items(&conn, Some("url"), None, 100).expect("list");
        assert_eq!(urls.len(), 1);
        let hits = list_items(&conn, None, Some("plain"), 100).expect("list");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].content, "some plain text");
    }

    #[test]
    fn clear_keeps_pinned() {
        let conn = test_conn();
        let a = insert_text(&conn, "keep me");
        let _b = insert_text(&conn, "delete me");
        set_pinned(&conn, a.id, true).expect("pin");
        let removed = clear_history(&conn).expect("clear");
        assert_eq!(removed, 1);
        assert_eq!(count_items(&conn).expect("count"), 1);
    }

    #[test]
    fn like_wildcards_escaped() {
        let conn = test_conn();
        insert_text(&conn, "100% legit");
        insert_text(&conn, "nothing here");
        let hits = list_items(&conn, None, Some("100%"), 100).expect("list");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn get_items_by_ids_preserves_order_and_skips_archived() {
        let conn = test_conn();
        let a = insert_text(&conn, "first");
        let b = insert_text(&conn, "second");

        let batch = get_items_by_ids(&conn, &[b.id, a.id]).expect("batch");
        assert_eq!(
            batch.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![b.id, a.id],
            "input order must be preserved"
        );

        conn.execute(
            "UPDATE clipboard_items SET is_archived = 1 WHERE id = ?1",
            params![a.id],
        )
        .expect("archive");
        let batch = get_items_by_ids(&conn, &[a.id, b.id]).expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].id, b.id);
    }

    #[test]
    fn get_items_by_ids_attaches_ai_metadata() {
        let conn = test_conn();
        let item = insert_text(&conn, "analyzed text");
        let analysis = crate::models::ai::AiAnalysis {
            summary: Some("a summary".into()),
            category: Some("Text".into()),
            keywords: vec!["kw".into()],
            tags: vec![],
            confidence: Some(0.8),
        };
        crate::db::settings_queries::save_ai_metadata(
            &conn,
            item.id,
            "openrouter",
            "m1",
            &analysis,
        )
        .expect("save ai");

        let batch = get_items_by_ids(&conn, &[item.id]).expect("batch");
        assert_eq!(batch[0].ai_summary.as_deref(), Some("a summary"));
        assert_eq!(batch[0].ai_category.as_deref(), Some("Text"));
    }
}
