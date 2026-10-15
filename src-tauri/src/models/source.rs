use serde::{Deserialize, Serialize};

/// Application that owned the clipboard when an item was captured.
/// Populated in a later phase (smart metadata); stored shape defined now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: i64,
    pub application_name: Option<String>,
    pub application_executable: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub created_at: i64,
}
