use serde::{Deserialize, Serialize};

/// AI analysis output stored separately from core clipboard data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMetadata {
    pub id: i64,
    pub clipboard_id: i64,
    pub provider: String,
    pub model: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    /// JSON-encoded array of keywords.
    pub keywords: Option<String>,
    pub confidence: Option<f64>,
    pub processed_at: i64,
}

/// Structured analysis request/result used across the AI service boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnalysis {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// Free vs paid classification for an OpenRouter model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelInfo {
    pub id: String,
    pub name: String,
    pub free: bool,
    /// USD per 1M prompt tokens (0.0 for free models).
    pub prompt_price: f64,
    /// USD per 1M completion tokens (0.0 for free models).
    pub completion_price: f64,
    pub context_length: Option<u64>,
}
