use tauri::State;

use crate::commands::clipboard::AppState;
use crate::models::ai::AiModelInfo;
use crate::services::ai_service::{AiStatus, transform_system};

/// Fetch the OpenRouter model catalog, free models first.
#[tauri::command]
pub async fn ai_list_models(state: State<'_, AppState>) -> Result<Vec<AiModelInfo>, String> {
    state.ai.list_models().await
}

/// Verify key + connectivity without sending any content.
#[tauri::command]
pub async fn ai_test_connection(state: State<'_, AppState>) -> Result<(), String> {
    state.ai.health_check().await
}

/// Current queue status for the settings UI.
#[tauri::command]
pub fn ai_get_status(state: State<'_, AppState>) -> Result<AiStatus, String> {
    Ok(state.ai.status())
}

/// Queue analysis for a single item (skipped automatically if sensitive).
#[tauri::command]
pub fn ai_analyze_item(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.ai.enqueue_analyze(id);
    Ok(())
}

/// Queue analysis for everything eligible that hasn't been analyzed yet.
#[tauri::command]
pub fn ai_analyze_pending(state: State<'_, AppState>) -> Result<(), String> {
    state.ai.enqueue_pending();
    Ok(())
}

/// One-shot AI action on an item (rewrite/translate/explain/summarize/fix).
#[tauri::command]
pub async fn ai_transform(
    state: State<'_, AppState>,
    id: i64,
    action: String,
    target_lang: Option<String>,
) -> Result<String, String> {
    let s = state.settings.get();
    if !s.ai_enabled {
        return Err("AI is disabled — enable it in Settings → AI.".into());
    }
    if s.ai_model.is_empty() {
        return Err("No AI model selected — pick one in Settings → AI.".into());
    }

    let item = state
        .db
        .get_item(id)?
        .ok_or_else(|| format!("item {id} not found"))?;
    if item.item_type == crate::models::clipboard::ClipboardItemType::Image {
        return Err("AI actions work on text, not images.".into());
    }
    if item.is_sensitive {
        return Err("This clip is marked sensitive — AI is blocked for it.".into());
    }
    // Second gate: run the on-device detector at call time (same rule as
    // the background queue) so clips captured while detection was off — or
    // before newer patterns existed — can never reach the provider.
    if crate::security::sensitive_detector::detect(&item.content).is_some() {
        return Err("This clip looks like a secret — AI is blocked for it.".into());
    }

    let system = transform_system(&action, target_lang.as_deref())?;
    state
        .ai
        .transform(&s.ai_model, &system, &item.content)
        .await
}
