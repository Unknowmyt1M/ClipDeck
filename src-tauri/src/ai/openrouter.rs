//! OpenRouter adapter — https://openrouter.ai/api/v1
//!
//! Chat-completions compatible API. Models are fetched live so the UI can
//! show free vs paid options; the API key lives in the OS credential vault.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::ai::provider::{AiProvider, ProviderKind};
use crate::models::ai::{AiAnalysis, AiModelInfo};
use crate::security::secrets;

const BASE: &str = "https://openrouter.ai/api/v1";
const APP_NAME: &str = "ClipDeck";
const APP_URL: &str = "https://github.com/Unknowmyt1M/ClipDeck";
/// Content cap so one analysis call stays cheap.
const MAX_CONTENT_CHARS: usize = 6000;

pub struct OpenRouterProvider {
    client: reqwest::Client,
}

impl OpenRouterProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    fn api_key(&self) -> Result<String, String> {
        secrets::get_openrouter_api_key().ok_or_else(|| {
            "No API key configured. Add your OpenRouter key in Settings → AI.".into()
        })
    }

    async fn chat(
        &self,
        model: &str,
        messages: &[serde_json::Value],
        temperature: f32,
    ) -> Result<String, String> {
        let key = self.api_key()?;
        let resp = self
            .client
            .post(format!("{BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {key}"))
            .header("HTTP-Referer", APP_URL)
            .header("X-Title", APP_NAME)
            .json(&json!({
                "model": model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": 1024,
            }))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("response read failed: {e}"))?;
        if !status.is_success() {
            // Surface OpenRouter's message; never includes clipboard content.
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error")?.get("message")?.as_str().map(String::from))
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(format!("OpenRouter error: {msg}"));
        }

        let parsed: ChatResponse =
            serde_json::from_str(&body).map_err(|e| format!("bad response shape: {e}"))?;
        parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "empty completion from model".to_string())
    }
}

impl Default for OpenRouterProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    pricing: Option<ModelPricing>,
}

#[derive(Deserialize)]
struct ModelPricing {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    completion: Option<String>,
}

#[async_trait]
impl AiProvider for OpenRouterProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Openrouter
    }

    async fn list_models(&self) -> Result<Vec<AiModelInfo>, String> {
        // Model listing requires auth on OpenRouter.
        let key = self.api_key()?;
        let resp = self
            .client
            .get(format!("{BASE}/models"))
            .header("Authorization", format!("Bearer {key}"))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("OpenRouter error: HTTP {}", status.as_u16()));
        }

        let parsed: ModelsResponse =
            serde_json::from_str(&body).map_err(|e| format!("bad response shape: {e}"))?;

        let mut models: Vec<AiModelInfo> = parsed
            .data
            .into_iter()
            .map(|m| {
                let prompt_price = m
                    .pricing
                    .as_ref()
                    .and_then(|p| p.prompt.as_deref())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|per_token| per_token * 1_000_000.0)
                    .unwrap_or(0.0);
                let completion_price = m
                    .pricing
                    .as_ref()
                    .and_then(|p| p.completion.as_deref())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|per_token| per_token * 1_000_000.0)
                    .unwrap_or(0.0);
                AiModelInfo {
                    id: m.id.clone(),
                    name: m.name.unwrap_or_else(|| m.id.clone()),
                    free: prompt_price <= f64::EPSILON && completion_price <= f64::EPSILON,
                    prompt_price,
                    completion_price,
                    context_length: m.context_length,
                }
            })
            .collect();

        // Free first, then by name — makes the picker friendly.
        models.sort_by(|a, b| {
            b.free
                .cmp(&a.free)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(models)
    }

    async fn analyze(&self, model: &str, content: &str) -> Result<AiAnalysis, String> {
        let content = clip(content, MAX_CONTENT_CHARS);
        let system = r#"You analyze clipboard snippets. Respond ONLY with JSON:
{"summary":"<max 120 chars>","category":"<one of: Code|Link|Email|Credentials|Numbers|Text|Document|Image|Other>","keywords":["2-5 short keywords"],"tags":["0-3 lowercase tags"],"confidence":0.0-1.0}
No markdown, no commentary."#;
        let raw = self
            .chat(
                model,
                &[
                    json!({"role": "system", "content": system}),
                    json!({"role": "user", "content": content}),
                ],
                0.2,
            )
            .await?;

        parse_analysis(&raw)
    }

    async fn complete(&self, model: &str, system: &str, user: &str) -> Result<String, String> {
        self.chat(
            model,
            &[
                json!({"role": "system", "content": system}),
                json!({"role": "user", "content": user}),
            ],
            0.7,
        )
        .await
    }

    async fn health_check(&self) -> Result<(), String> {
        if !secrets::has_openrouter_api_key() {
            return Err("No API key configured".into());
        }
        // Cheap request: list 1 model page proves auth works.
        let resp = self
            .client
            .get(format!("{BASE}/models"))
            .header("Authorization", format!("Bearer {}", self.api_key()?))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("OpenRouter error: HTTP {}", resp.status().as_u16()))
        }
    }
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Parse the model's JSON answer tolerantly (strip code fences etc.).
fn parse_analysis(raw: &str) -> Result<AiAnalysis, String> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    // Grab the outermost {...} in case the model added chatter.
    let json_str = match (cleaned.find('{'), cleaned.rfind('}')) {
        (Some(s), Some(e)) if e > s => &cleaned[s..=e],
        _ => cleaned,
    };
    serde_json::from_str::<AiAnalysis>(json_str)
        .or_else(|_| serde_json::from_str::<AiAnalysis>(cleaned))
        .map_err(|e| format!("could not parse analysis JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let a = parse_analysis(
            r#"{"summary":"s","category":"Code","keywords":["rust"],"tags":[],"confidence":0.9}"#,
        )
        .expect("parse");
        assert_eq!(a.summary.as_deref(), Some("s"));
    }

    #[test]
    fn parse_fenced_json() {
        let a = parse_analysis(
            "```json\n{\"summary\":\"x\",\"category\":\"Text\",\"keywords\":[],\"tags\":[]}\n```",
        )
        .expect("parse");
        assert_eq!(a.summary.as_deref(), Some("x"));
    }

    #[test]
    fn parse_json_with_chatter() {
        let a = parse_analysis("Here is the analysis: {\"summary\":\"y\",\"category\":\"Other\",\"keywords\":[],\"tags\":[]} hope that helps")
            .expect("parse");
        assert_eq!(a.summary.as_deref(), Some("y"));
    }

    #[test]
    fn clip_respects_boundary() {
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("héllo", 3).chars().count(), 3);
    }
}
