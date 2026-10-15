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
    })
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
    conn.execute(
        "INSERT INTO clipboard_items (uuid, type, content, content_hash, preview, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![uuid, item_type, content, content_hash, preview, created_at],
    )
    .map_err(|e| format!("insert_item failed: {e}"))?;

    let id = conn.last_insert_rowid();
    get_item(conn, id)?.ok_or_else(|| format!("inserted item {id} not found"))
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

/// Get a single item by id.
pub fn get_item(conn: &Connection, id: i64) -> Result<Option<ClipboardItem>, String> {
    conn.query_row(
        &format!("SELECT {ITEM_COLUMNS} FROM clipboard_items WHERE id = ?1"),
        params![id],
        row_to_item,
    )
    .optional()
    .map_err(|e| format!("get_item failed: {e}"))
}

/// List items with optional type filter and search query, newest first.
pub fn list_items(
    conn: &Connection,
    type_filter: Option<&str>,
    query: Option<&str>,
    limit: i64,
) -> Result<Vec<ClipboardItem>, String> {
    let mut sql = format!("SELECT {ITEM_COLUMNS} FROM clipboard_items WHERE is_archived = 0");
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(t) = type_filter {
        sql.push_str(" AND type = ?");
        binds.push(Box::new(t.to_string()));
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
    let items = stmt
        .query_map(params_ref.as_slice(), row_to_item)
        .map_err(|e| format!("list_items query failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("list_items row failed: {e}"))?;
    Ok(items)
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
}
