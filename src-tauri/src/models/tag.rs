use serde::{Deserialize, Serialize};

/// A clipboard tag. Tagging lands with the AI/smart-metadata phases;
/// the shape is defined now so the schema and types stay stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    /// Where the tag came from: "manual", "system", or "ai".
    pub source: String,
    pub created_at: i64,
}
