//! Provider-agnostic AI abstraction.
//!
//! The app must never scatter provider-specific logic through services —
//! everything goes through [`AiProvider`]. Currently only OpenRouter is
//! implemented; future providers (local models, etc.) plug in here.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::ai::{AiAnalysis, AiModelInfo};

/// Which backend is configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Openrouter,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Openrouter => "openrouter",
        }
    }
}

/// What the UI needs to render provider state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub kind: ProviderKind,
    /// Whether an API key exists in the OS credential vault.
    pub key_configured: bool,
    /// Currently selected model id.
    pub model: String,
    /// Human label for the selected model (free/paid shown by the UI).
    pub model_name: String,
    pub model_free: bool,
}

/// Common operations every provider must support.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider identifier for storage/display.
    fn kind(&self) -> ProviderKind;

    /// Fetch available models (free and paid) for the model picker.
    async fn list_models(&self) -> Result<Vec<AiModelInfo>, String>;

    /// Analyze clipboard content → summary/category/keywords/tags.
    /// Caller is responsible for refusing to send sensitive content.
    async fn analyze(&self, model: &str, content: &str) -> Result<AiAnalysis, String>;

    /// One-shot text transformation (rewrite/translate/explain).
    async fn complete(&self, model: &str, system: &str, user: &str) -> Result<String, String>;

    /// Lightweight connectivity check.
    async fn health_check(&self) -> Result<(), String>;
}
