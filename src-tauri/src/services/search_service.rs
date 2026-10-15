//! Search: FTS5 when available, LIKE fallback otherwise.
//!
//! Phase 6 scope: fast local keyword search with type filters. Semantic
//! (embedding) search is a later enhancement and not required for MVP.

use crate::db::{queries, settings_queries};
use crate::models::clipboard::ClipboardItem;

/// Max results a single search returns.
const DEFAULT_LIMIT: i64 = 200;

pub struct SearchService;

impl SearchService {
    /// Search with optional query + type filter.
    ///
    /// Query path: FTS5 (if the index exists) → hydrate by rowid order.
    /// No query: plain list with optional type filter.
    pub fn search(
        conn: &rusqlite::Connection,
        query: Option<&str>,
        type_filter: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<ClipboardItem>, String> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 1000);

        let q = query.map(str::trim).filter(|s| !s.is_empty());

        let Some(q) = q else {
            return queries::list_items(conn, type_filter, None, limit);
        };

        if settings_queries::fts_available(conn) {
            let ids = settings_queries::search_fts(conn, q, type_filter, limit)?;
            if !ids.is_empty() {
                return hydrate_in_order(conn, &ids);
            }
            // FTS returned nothing; fall through to LIKE (e.g. substring
            // shorter than a token) so search still feels complete.
        }

        queries::list_items(conn, type_filter, Some(q), limit)
    }
}

/// Load items for the given rowids preserving FTS rank order.
fn hydrate_in_order(
    conn: &rusqlite::Connection,
    ids: &[i64],
) -> Result<Vec<ClipboardItem>, String> {
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(item) = queries::get_item(conn, *id)?
            && !item.is_archived
        {
            items.push(item);
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(conn: &rusqlite::Connection) {
        let now = crate::db::queries::now_ms();
        for (i, content) in [
            "rust ownership rules",
            "grocery list milk eggs",
            "https://example.com",
        ]
        .iter()
        .enumerate()
        {
            let t = crate::models::clipboard::detect_type(content);
            crate::db::queries::insert_item(
                conn,
                &format!("u{i}"),
                t.as_str(),
                content,
                &format!("h{i}"),
                &crate::models::clipboard::make_preview(content),
                now + i as i64,
            )
            .expect("insert");
        }
    }

    #[test]
    fn finds_by_keyword() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        seed(&conn);
        let hits = SearchService::search(&conn, Some("ownership"), None, None).expect("search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("ownership"));
    }

    #[test]
    fn type_filter_with_query() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        seed(&conn);
        let hits = SearchService::search(&conn, Some("example"), Some("url"), None).expect("s");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].item_type,
            crate::models::clipboard::ClipboardItemType::Url
        );
    }

    #[test]
    fn empty_query_lists_all() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        seed(&conn);
        let all = SearchService::search(&conn, None, None, None).expect("list");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn no_results_returns_empty_not_error() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        seed(&conn);
        let hits = SearchService::search(&conn, Some("zzzznotfound"), None, None).expect("s");
        assert!(hits.is_empty());
    }
}
