use serde::Serialize;

/// One frequently-used item for the stats view (content never included).
#[derive(Debug, Clone, Serialize)]
pub struct TopItem {
    pub id: i64,
    pub preview: String,
    pub use_count: i64,
    pub item_type: String,
}

/// Aggregated, content-free usage statistics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Stats {
    pub total: i64,
    pub today: i64,
    pub pinned: i64,
    pub favorites: i64,
    pub sensitive: i64,
    pub images: i64,
    pub ai_processed: i64,
    pub top_used: Vec<TopItem>,
    /// Oldest item timestamp (ms), `None` when history is empty.
    pub oldest_at: Option<i64>,
}
